use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadRangePhase {
    Held,
    Sending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRangeSnapshot {
    pub start: u64,
    pub end: u64,
    pub phase: UploadRangePhase,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UploadActivitySnapshot {
    pub total_requests: u64,
    pub requested_bytes_total: u64,
    pub last_requested_unix_secs: Option<u64>,
    pub active_ranges: Vec<UploadRangeSnapshot>,
}

#[derive(Debug, Default)]
pub struct UploadActivityTracker {
    inner: Mutex<HashMap<String, FileUploadActivity>>,
}

#[derive(Debug, Default)]
struct FileUploadActivity {
    total_requests: u64,
    requested_bytes_total: u64,
    last_requested_unix_secs: Option<u64>,
    active_ranges: Vec<TrackedUploadRange>,
}

#[derive(Debug, Clone)]
struct TrackedUploadRange {
    start: u64,
    end: u64,
    phase: UploadRangePhase,
    expires_at: Instant,
}

impl UploadActivityTracker {
    pub fn note_held(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration) {
        self.note(hash_hex, start, end, ttl, UploadRangePhase::Held);
    }

    pub fn note_sending(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration) {
        self.note(hash_hex, start, end, ttl, UploadRangePhase::Sending);
    }

    pub fn snapshot_for_hash(&self, hash_hex: &str) -> UploadActivitySnapshot {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("upload activity lock poisoned");
        let Some(file) = inner.get_mut(&hash_hex.to_ascii_lowercase()) else {
            return UploadActivitySnapshot::default();
        };
        prune_expired(file, now);
        UploadActivitySnapshot {
            total_requests: file.total_requests,
            requested_bytes_total: file.requested_bytes_total,
            last_requested_unix_secs: file.last_requested_unix_secs,
            active_ranges: file
                .active_ranges
                .iter()
                .map(|range| UploadRangeSnapshot {
                    start: range.start,
                    end: range.end,
                    phase: range.phase,
                })
                .collect(),
        }
    }

    fn note(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration, phase: UploadRangePhase) {
        let now = Instant::now();
        let expires_at = now + ttl;
        let mut inner = self.inner.lock().expect("upload activity lock poisoned");
        let file = inner.entry(hash_hex.to_ascii_lowercase()).or_default();
        prune_expired(file, now);

        if let Some(existing) = file
            .active_ranges
            .iter_mut()
            .find(|range| range.start == start && range.end == end)
        {
            existing.phase = phase;
            existing.expires_at = expires_at;
        } else {
            file.active_ranges.push(TrackedUploadRange {
                start,
                end,
                phase,
                expires_at,
            });
        }

        file.total_requests = file.total_requests.saturating_add(1);
        file.requested_bytes_total = file
            .requested_bytes_total
            .saturating_add(end.saturating_sub(start).saturating_add(1));
        file.last_requested_unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|v| v.as_secs());
    }
}

fn prune_expired(file: &mut FileUploadActivity, now: Instant) {
    file.active_ranges.retain(|range| range.expires_at > now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_updates_range_phase_and_totals() {
        let tracker = UploadActivityTracker::default();
        let ttl = Duration::from_secs(5);

        tracker.note_held("abcd", 0, 1023, ttl);
        let held = tracker.snapshot_for_hash("abcd");
        assert_eq!(held.total_requests, 1);
        assert_eq!(held.requested_bytes_total, 1024);
        assert_eq!(held.active_ranges.len(), 1);
        assert_eq!(held.active_ranges[0].phase, UploadRangePhase::Held);

        tracker.note_sending("abcd", 0, 1023, ttl);
        let sending = tracker.snapshot_for_hash("ABCD");
        assert_eq!(sending.total_requests, 2);
        assert_eq!(sending.requested_bytes_total, 2048);
        assert_eq!(sending.active_ranges.len(), 1);
        assert_eq!(sending.active_ranges[0].phase, UploadRangePhase::Sending);
    }
}
