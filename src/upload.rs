use crate::share::{self, SharedLibrary, SharedLibraryFile};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

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
    pub file_hash_md4_hex: String,
    pub total_requests: u64,
    pub requested_bytes_total: u64,
    pub last_requested_unix_secs: Option<u64>,
    pub active_ranges: Vec<UploadRangeSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadPayloadSource {
    SharedFile,
    ZeroFillFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPayloadBuild {
    pub payload: Vec<u8>,
    pub source: UploadPayloadSource,
}

#[derive(Debug, Clone)]
pub struct UploadService {
    shared_library: Arc<RwLock<SharedLibrary>>,
    activity: Arc<UploadActivityTracker>,
}

impl UploadService {
    pub fn new(shared_library: Arc<RwLock<SharedLibrary>>) -> Self {
        Self {
            shared_library,
            activity: Arc::new(UploadActivityTracker::default()),
        }
    }

    pub fn tracker(&self) -> Arc<UploadActivityTracker> {
        Arc::clone(&self.activity)
    }

    pub fn note_held(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration) {
        self.activity.note_held(hash_hex, start, end, ttl);
    }

    pub fn note_sending(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration) {
        self.activity.note_sending(hash_hex, start, end, ttl);
    }

    pub fn snapshot_for_hash(&self, hash_hex: &str) -> UploadActivitySnapshot {
        self.activity.snapshot_for_hash(hash_hex)
    }

    pub fn snapshot_all(&self) -> Vec<UploadActivitySnapshot> {
        self.activity.snapshot_all()
    }

    pub async fn build_sending_part_payload(
        &self,
        file_hash: &[u8; 16],
        start: u64,
        end: u64,
    ) -> UploadPayloadBuild {
        let end_exclusive = end.saturating_add(1);
        let hash_hex = crate::kad::KadId(*file_hash).to_hex_lower();
        let shared_file = self
            .shared_library
            .read()
            .await
            .get_by_hash_hex(&hash_hex)
            .cloned();
        let (block_data, source) = match shared_file {
            Some(file) => match tokio::task::spawn_blocking({
                let file_for_read = file.clone();
                move || share::read_shared_block(&file_for_read, start, end)
            })
            .await
            {
                Ok(Ok(body)) => (body, UploadPayloadSource::SharedFile),
                Ok(Err(err)) => {
                    warn_zero_fill(
                        "shared_upload_fallback_zero_fill",
                        &hash_hex,
                        &file,
                        start,
                        end,
                        &err,
                    );
                    (
                        zero_fill_block(start, end),
                        UploadPayloadSource::ZeroFillFallback,
                    )
                }
                Err(err) => {
                    warn_zero_fill_join(
                        "shared_upload_fallback_zero_fill_join",
                        &hash_hex,
                        &file,
                        start,
                        end,
                        &err,
                    );
                    (
                        zero_fill_block(start, end),
                        UploadPayloadSource::ZeroFillFallback,
                    )
                }
            },
            None => (
                zero_fill_block(start, end),
                UploadPayloadSource::ZeroFillFallback,
            ),
        };
        let payload = crate::download::protocol::encode_sendingpart_payload(
            *file_hash,
            start,
            end_exclusive,
            &block_data,
        )
        .expect("valid sendingpart payload");
        UploadPayloadBuild { payload, source }
    }
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
        let mut inner = recover_lock(&self.inner, "upload activity");
        let Some(file) = inner.get_mut(&hash_hex.to_ascii_lowercase()) else {
            return UploadActivitySnapshot::default();
        };
        prune_expired(file, now);
        snapshot_from_file(hash_hex.to_ascii_lowercase(), file)
    }

    pub fn snapshot_all(&self) -> Vec<UploadActivitySnapshot> {
        let now = Instant::now();
        let mut inner = recover_lock(&self.inner, "upload activity");
        let mut out = Vec::new();
        for (hash_hex, file) in inner.iter_mut() {
            prune_expired(file, now);
            if file.total_requests == 0 && file.active_ranges.is_empty() {
                continue;
            }
            out.push(snapshot_from_file(hash_hex.clone(), file));
        }
        out.sort_by(|a, b| a.file_hash_md4_hex.cmp(&b.file_hash_md4_hex));
        out
    }

    fn note(&self, hash_hex: &str, start: u64, end: u64, ttl: Duration, phase: UploadRangePhase) {
        let now = Instant::now();
        let expires_at = now + ttl;
        let mut inner = recover_lock(&self.inner, "upload activity");
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

fn snapshot_from_file(hash_hex: String, file: &FileUploadActivity) -> UploadActivitySnapshot {
    UploadActivitySnapshot {
        file_hash_md4_hex: hash_hex,
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

fn prune_expired(file: &mut FileUploadActivity, now: Instant) {
    file.active_ranges.retain(|range| range.expires_at > now);
}

fn recover_lock<'a, T>(mutex: &'a Mutex<T>, label: &str) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(inner) => inner,
        Err(poisoned) => {
            tracing::warn!("{label} lock poisoned; continuing with recovered state");
            poisoned.into_inner()
        }
    }
}

fn zero_fill_block(start: u64, end: u64) -> Vec<u8> {
    vec![0u8; end.saturating_sub(start).saturating_add(1) as usize]
}

fn warn_zero_fill(
    throttle_key: &'static str,
    hash_hex: &str,
    file: &SharedLibraryFile,
    start: u64,
    end: u64,
    err: &share::ShareError,
) {
    if crate::logging::warn_throttled(throttle_key, Duration::from_secs(30)) {
        tracing::warn!(
            hash = %hash_hex,
            path = %file.canonical_path.display(),
            start,
            end,
            error = %err,
            "shared upload fallback used zero-filled payload after read failure"
        );
    }
}

fn warn_zero_fill_join(
    throttle_key: &'static str,
    hash_hex: &str,
    file: &SharedLibraryFile,
    start: u64,
    end: u64,
    err: &tokio::task::JoinError,
) {
    if crate::logging::warn_throttled(throttle_key, Duration::from_secs(30)) {
        tracing::warn!(
            hash = %hash_hex,
            path = %file.canonical_path.display(),
            start,
            end,
            error = %err,
            "shared upload fallback used zero-filled payload after blocking read join failure"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::{SharedLibraryBuild, load_or_rebuild_shared_library};

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

    #[test]
    fn tracker_snapshot_all_includes_hash() {
        let tracker = UploadActivityTracker::default();
        tracker.note_held("beef", 0, 63, Duration::from_secs(5));
        let snapshots = tracker.snapshot_all();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].file_hash_md4_hex, "beef");
    }

    #[tokio::test]
    async fn upload_service_reads_shared_file_payload() {
        let root = temp_dir("upload-service");
        let data_dir = root.join("data");
        let shared_dir = root.join("shared");
        tokio::fs::create_dir_all(&data_dir)
            .await
            .expect("data dir");
        tokio::fs::create_dir_all(&shared_dir)
            .await
            .expect("shared dir");
        tokio::fs::write(shared_dir.join("shared.bin"), b"abcdefghij")
            .await
            .expect("write");
        let roots =
            crate::share::canonicalize_share_roots(&[shared_dir.display().to_string()], &data_dir)
                .expect("roots");
        let SharedLibraryBuild { library, .. } =
            load_or_rebuild_shared_library(&roots, &data_dir.join("shared_library.json"))
                .await
                .expect("library");
        let file = library.files()[0].clone();
        let service = UploadService::new(Arc::new(RwLock::new(library)));

        let build = service
            .build_sending_part_payload(&file.file_id.0, 2, 5)
            .await;
        assert_eq!(build.source, UploadPayloadSource::SharedFile);
        let decoded =
            crate::download::protocol::decode_sendingpart_payload(&build.payload).expect("decode");
        assert_eq!(decoded.file_hash, file.file_id.0);
        assert_eq!(decoded.start, 2);
        assert_eq!(decoded.end_exclusive, 6);
        assert_eq!(decoded.data, b"cdef");

        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn upload_service_zero_fills_for_missing_hash() {
        let service = UploadService::new(Arc::new(RwLock::new(SharedLibrary::default())));
        let build = service.build_sending_part_payload(&[0xAA; 16], 0, 3).await;
        assert_eq!(build.source, UploadPayloadSource::ZeroFillFallback);
        let decoded =
            crate::download::protocol::decode_sendingpart_payload(&build.payload).expect("decode");
        assert_eq!(decoded.file_hash, [0xAA; 16]);
        assert_eq!(decoded.start, 0);
        assert_eq!(decoded.end_exclusive, 4);
        assert_eq!(decoded.data, vec![0u8; 4]);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rust_mule_upload_test_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }
}
