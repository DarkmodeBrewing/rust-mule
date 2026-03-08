use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SharedPublishSnapshot {
    pub source_last_attempt_unix_secs: Option<u64>,
    pub source_last_result: Option<String>,
    pub source_attempts: u64,
    pub keyword_last_attempt_unix_secs: Option<u64>,
    pub keyword_last_result: Option<String>,
    pub keyword_attempts: u64,
    pub keyword_queued: u64,
    pub keyword_failed: u64,
}

#[derive(Debug, Default)]
pub struct SharedPublishTracker {
    inner: Mutex<HashMap<String, SharedPublishSnapshot>>,
}

impl SharedPublishTracker {
    pub fn note_source_queued(&self, hash_hex: &str) {
        self.with_entry(hash_hex, |entry| {
            entry.source_attempts = entry.source_attempts.saturating_add(1);
            entry.source_last_attempt_unix_secs = now_unix_secs();
            entry.source_last_result = Some("queued".to_string());
        });
    }

    pub fn note_source_queue_failed(&self, hash_hex: &str) {
        self.with_entry(hash_hex, |entry| {
            entry.source_attempts = entry.source_attempts.saturating_add(1);
            entry.source_last_attempt_unix_secs = now_unix_secs();
            entry.source_last_result = Some("queue_failed".to_string());
        });
    }

    pub fn note_keyword_queued(&self, hash_hex: &str) {
        self.with_entry(hash_hex, |entry| {
            entry.keyword_attempts = entry.keyword_attempts.saturating_add(1);
            entry.keyword_queued = entry.keyword_queued.saturating_add(1);
            entry.keyword_last_attempt_unix_secs = now_unix_secs();
            entry.keyword_last_result = Some("queued".to_string());
        });
    }

    pub fn note_keyword_queue_failed(&self, hash_hex: &str) {
        self.with_entry(hash_hex, |entry| {
            entry.keyword_attempts = entry.keyword_attempts.saturating_add(1);
            entry.keyword_failed = entry.keyword_failed.saturating_add(1);
            entry.keyword_last_attempt_unix_secs = now_unix_secs();
            entry.keyword_last_result = Some("queue_failed".to_string());
        });
    }

    pub fn snapshot_for_hash(&self, hash_hex: &str) -> SharedPublishSnapshot {
        let inner = self
            .inner
            .lock()
            .expect("shared publish tracker lock poisoned");
        inner
            .get(&hash_hex.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    fn with_entry(&self, hash_hex: &str, f: impl FnOnce(&mut SharedPublishSnapshot)) {
        let mut inner = self
            .inner
            .lock()
            .expect("shared publish tracker lock poisoned");
        let entry = inner.entry(hash_hex.to_ascii_lowercase()).or_default();
        f(entry);
    }
}

fn now_unix_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|v| v.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_records_source_and_keyword_queue_outcomes() {
        let tracker = SharedPublishTracker::default();
        tracker.note_source_queued("abcd");
        tracker.note_keyword_queued("abcd");
        tracker.note_keyword_queue_failed("abcd");

        let snap = tracker.snapshot_for_hash("ABCD");
        assert_eq!(snap.source_attempts, 1);
        assert_eq!(snap.source_last_result.as_deref(), Some("queued"));
        assert_eq!(snap.keyword_attempts, 2);
        assert_eq!(snap.keyword_queued, 1);
        assert_eq!(snap.keyword_failed, 1);
        assert_eq!(snap.keyword_last_result.as_deref(), Some("queue_failed"));
    }
}
