use crate::share::{self, SharedLibrary, SharedLibraryFile};
use crate::transfer_rate::{RollingTransferRate, TransferRateSnapshot};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const RECENT_SESSION_RETENTION: Duration = Duration::from_secs(120);
const MAX_RECENT_SESSIONS_PER_FILE: usize = 128;

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
    pub peer_id_hex: String,
    pub requested_unix_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UploadActivitySnapshot {
    pub file_hash_md4_hex: String,
    pub total_requests: u64,
    pub requested_bytes_total: u64,
    pub rate_bps_5s: u64,
    pub rate_bps_30s: u64,
    pub zero_fill_requests_total: u64,
    pub zero_fill_requested_bytes_total: u64,
    pub zero_fill_rate_bps_5s: u64,
    pub zero_fill_rate_bps_30s: u64,
    pub zero_fill_active: bool,
    pub last_requested_unix_secs: Option<u64>,
    pub last_peer_id_hex: Option<String>,
    pub active_peer_ids: Vec<String>,
    pub active_since_unix_secs: Option<u64>,
    pub last_payload_source: Option<UploadPayloadSource>,
    pub active_ranges: Vec<UploadRangeSnapshot>,
    pub sessions: Vec<UploadSessionSnapshot>,
    pub recent_session_count: usize,
    pub recent_sessions: Vec<UploadSessionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSessionSnapshot {
    pub session_id: u64,
    pub start: u64,
    pub end: u64,
    pub bytes_total: u64,
    pub phase: UploadRangePhase,
    pub peer_id_hex: String,
    pub payload_source: Option<UploadPayloadSource>,
    pub started_unix_secs: Option<u64>,
    pub last_updated_unix_secs: Option<u64>,
    pub terminal_reason: Option<UploadTerminalReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadTerminalReason {
    Expired,
    Dropped,
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

    pub fn note_held(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        ttl: Duration,
    ) {
        self.activity
            .note_held(hash_hex, peer_id_hex, start, end, ttl);
    }

    pub fn note_sending(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        ttl: Duration,
        payload_source: UploadPayloadSource,
    ) {
        self.activity
            .note_sending(hash_hex, peer_id_hex, start, end, ttl, payload_source);
    }

    pub fn note_terminal(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        reason: UploadTerminalReason,
    ) {
        self.activity
            .note_terminal(hash_hex, peer_id_hex, start, end, reason);
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
    inner: Mutex<UploadTrackerState>,
}

#[derive(Debug, Default)]
struct UploadTrackerState {
    next_session_id: u64,
    files: HashMap<String, FileUploadActivity>,
}

#[derive(Debug, Default)]
struct FileUploadActivity {
    total_requests: u64,
    requested_bytes_total: u64,
    transfer_rate: RollingTransferRate,
    zero_fill_requests_total: u64,
    zero_fill_requested_bytes_total: u64,
    zero_fill_transfer_rate: RollingTransferRate,
    last_requested_unix_secs: Option<u64>,
    last_peer_id_hex: Option<String>,
    last_payload_source: Option<UploadPayloadSource>,
    active_ranges: Vec<TrackedUploadRange>,
    recent_sessions: Vec<TrackedUploadRange>,
}

#[derive(Debug, Clone)]
struct UploadNoteMeta {
    peer_id_hex: String,
    requested_unix_secs: Option<u64>,
    payload_source: Option<UploadPayloadSource>,
}

#[derive(Debug, Clone)]
struct TrackedUploadRange {
    session_id: u64,
    start: u64,
    end: u64,
    phase: UploadRangePhase,
    peer_id_hex: String,
    payload_source: Option<UploadPayloadSource>,
    started_unix_secs: Option<u64>,
    requested_unix_secs: Option<u64>,
    last_updated_unix_secs: Option<u64>,
    expires_at: Instant,
    terminal_reason: Option<UploadTerminalReason>,
}

impl UploadActivityTracker {
    pub fn note_held(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        ttl: Duration,
    ) {
        self.note(
            hash_hex,
            start,
            end,
            ttl,
            UploadRangePhase::Held,
            UploadNoteMeta {
                peer_id_hex: peer_id_hex.to_string(),
                requested_unix_secs: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|v| v.as_secs()),
                payload_source: None,
            },
        );
    }

    pub fn note_sending(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        ttl: Duration,
        payload_source: UploadPayloadSource,
    ) {
        self.note(
            hash_hex,
            start,
            end,
            ttl,
            UploadRangePhase::Sending,
            UploadNoteMeta {
                peer_id_hex: peer_id_hex.to_string(),
                requested_unix_secs: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|v| v.as_secs()),
                payload_source: Some(payload_source),
            },
        );
    }

    pub fn snapshot_for_hash(&self, hash_hex: &str) -> UploadActivitySnapshot {
        let now = Instant::now();
        let mut inner = recover_lock(&self.inner, "upload activity");
        let Some(file) = inner.files.get_mut(&hash_hex.to_ascii_lowercase()) else {
            return UploadActivitySnapshot::default();
        };
        prune_expired(file, now, RECENT_SESSION_RETENTION);
        snapshot_from_file(hash_hex.to_ascii_lowercase(), file)
    }

    pub fn snapshot_all(&self) -> Vec<UploadActivitySnapshot> {
        let now = Instant::now();
        let mut inner = recover_lock(&self.inner, "upload activity");
        let mut out = Vec::new();
        for (hash_hex, file) in inner.files.iter_mut() {
            prune_expired(file, now, RECENT_SESSION_RETENTION);
            if file.total_requests == 0
                && file.active_ranges.is_empty()
                && file.recent_sessions.is_empty()
            {
                continue;
            }
            out.push(snapshot_from_file(hash_hex.clone(), file));
        }
        out.sort_by(|a, b| a.file_hash_md4_hex.cmp(&b.file_hash_md4_hex));
        out
    }

    pub fn note_terminal(
        &self,
        hash_hex: &str,
        peer_id_hex: &str,
        start: u64,
        end: u64,
        reason: UploadTerminalReason,
    ) {
        let now = Instant::now();
        let mut inner = recover_lock(&self.inner, "upload activity");
        let Some(file) = inner.files.get_mut(&hash_hex.to_ascii_lowercase()) else {
            return;
        };
        prune_expired(file, now, RECENT_SESSION_RETENTION);
        if let Some(index) = file.active_ranges.iter().position(|range| {
            range.start == start && range.end == end && range.peer_id_hex == peer_id_hex
        }) {
            let mut range = file.active_ranges.remove(index);
            range.expires_at = now + RECENT_SESSION_RETENTION;
            range.terminal_reason = Some(reason);
            push_recent_session(file, range);
        }
    }

    fn note(
        &self,
        hash_hex: &str,
        start: u64,
        end: u64,
        ttl: Duration,
        phase: UploadRangePhase,
        meta: UploadNoteMeta,
    ) {
        let now = Instant::now();
        let expires_at = now + ttl;
        let mut inner = recover_lock(&self.inner, "upload activity");
        let hash_hex = hash_hex.to_ascii_lowercase();
        let existing_session_id = {
            let file = inner.files.entry(hash_hex.clone()).or_default();
            prune_expired(file, now, RECENT_SESSION_RETENTION);
            file.active_ranges
                .iter()
                .find(|range| {
                    range.start == start
                        && range.end == end
                        && range.peer_id_hex == meta.peer_id_hex
                })
                .map(|range| range.session_id)
        };
        let session_id = if let Some(session_id) = existing_session_id {
            session_id
        } else {
            inner.next_session_id = inner.next_session_id.saturating_add(1);
            inner.next_session_id
        };
        let file = inner.files.entry(hash_hex).or_default();

        if let Some(existing) = file.active_ranges.iter_mut().find(|range| {
            range.start == start && range.end == end && range.peer_id_hex == meta.peer_id_hex
        }) {
            existing.phase = phase;
            existing.expires_at = expires_at;
            existing.requested_unix_secs = meta.requested_unix_secs;
            existing.payload_source = meta.payload_source;
            existing.last_updated_unix_secs = meta.requested_unix_secs;
        } else {
            file.active_ranges.push(TrackedUploadRange {
                session_id,
                start,
                end,
                phase,
                peer_id_hex: meta.peer_id_hex.clone(),
                payload_source: meta.payload_source,
                started_unix_secs: meta.requested_unix_secs,
                requested_unix_secs: meta.requested_unix_secs,
                last_updated_unix_secs: meta.requested_unix_secs,
                expires_at,
                terminal_reason: None,
            });
        }

        file.total_requests = file.total_requests.saturating_add(1);
        file.requested_bytes_total = file
            .requested_bytes_total
            .saturating_add(end.saturating_sub(start).saturating_add(1));
        if phase == UploadRangePhase::Sending {
            let bytes = end.saturating_sub(start).saturating_add(1);
            file.transfer_rate.note_bytes(bytes);
            if meta.payload_source == Some(UploadPayloadSource::ZeroFillFallback) {
                file.zero_fill_requests_total = file.zero_fill_requests_total.saturating_add(1);
                file.zero_fill_requested_bytes_total =
                    file.zero_fill_requested_bytes_total.saturating_add(bytes);
                file.zero_fill_transfer_rate.note_bytes(bytes);
            }
        }
        file.last_requested_unix_secs = meta.requested_unix_secs;
        file.last_peer_id_hex = Some(meta.peer_id_hex);
        if let Some(payload_source) = meta.payload_source {
            file.last_payload_source = Some(payload_source);
        }
    }
}

fn snapshot_from_file(hash_hex: String, file: &mut FileUploadActivity) -> UploadActivitySnapshot {
    let mut active_peer_ids = file
        .active_ranges
        .iter()
        .map(|range| range.peer_id_hex.clone())
        .collect::<Vec<_>>();
    active_peer_ids.sort();
    active_peer_ids.dedup();
    let active_since_unix_secs = file
        .active_ranges
        .iter()
        .filter_map(|range| range.started_unix_secs)
        .min();
    let TransferRateSnapshot {
        rate_bps_5s,
        rate_bps_30s,
    } = file.transfer_rate.snapshot();
    let TransferRateSnapshot {
        rate_bps_5s: zero_fill_rate_bps_5s,
        rate_bps_30s: zero_fill_rate_bps_30s,
    } = file.zero_fill_transfer_rate.snapshot();
    let zero_fill_active = file.active_ranges.iter().any(|range| {
        range.phase == UploadRangePhase::Sending
            && range.payload_source == Some(UploadPayloadSource::ZeroFillFallback)
    });
    UploadActivitySnapshot {
        file_hash_md4_hex: hash_hex,
        total_requests: file.total_requests,
        requested_bytes_total: file.requested_bytes_total,
        rate_bps_5s,
        rate_bps_30s,
        zero_fill_requests_total: file.zero_fill_requests_total,
        zero_fill_requested_bytes_total: file.zero_fill_requested_bytes_total,
        zero_fill_rate_bps_5s,
        zero_fill_rate_bps_30s,
        zero_fill_active,
        last_requested_unix_secs: file.last_requested_unix_secs,
        last_peer_id_hex: file.last_peer_id_hex.clone(),
        active_peer_ids,
        active_since_unix_secs,
        last_payload_source: file.last_payload_source,
        active_ranges: file
            .active_ranges
            .iter()
            .map(|range| UploadRangeSnapshot {
                start: range.start,
                end: range.end,
                phase: range.phase,
                peer_id_hex: range.peer_id_hex.clone(),
                requested_unix_secs: range.requested_unix_secs,
            })
            .collect(),
        sessions: file
            .active_ranges
            .iter()
            .map(tracked_range_to_session_snapshot)
            .collect(),
        recent_session_count: file.recent_sessions.len(),
        recent_sessions: file
            .recent_sessions
            .iter()
            .map(tracked_range_to_session_snapshot)
            .collect(),
    }
}

fn prune_expired(file: &mut FileUploadActivity, now: Instant, recent_retention: Duration) {
    let mut still_active = Vec::with_capacity(file.active_ranges.len());
    let mut expired_ranges = Vec::new();
    for mut range in file.active_ranges.drain(..) {
        if range.expires_at > now {
            still_active.push(range);
            continue;
        }
        range.expires_at = now + recent_retention;
        range.terminal_reason = Some(UploadTerminalReason::Expired);
        expired_ranges.push(range);
    }
    file.active_ranges = still_active;
    for range in expired_ranges {
        file.recent_sessions.push(range);
    }
    file.recent_sessions.retain(|range| range.expires_at > now);
    if file.recent_sessions.len() > MAX_RECENT_SESSIONS_PER_FILE {
        let keep_from = file
            .recent_sessions
            .len()
            .saturating_sub(MAX_RECENT_SESSIONS_PER_FILE);
        file.recent_sessions.drain(0..keep_from);
    }
}

fn push_recent_session(file: &mut FileUploadActivity, range: TrackedUploadRange) {
    file.recent_sessions.push(range);
    if file.recent_sessions.len() > MAX_RECENT_SESSIONS_PER_FILE {
        let keep_from = file
            .recent_sessions
            .len()
            .saturating_sub(MAX_RECENT_SESSIONS_PER_FILE);
        file.recent_sessions.drain(0..keep_from);
    }
}

fn tracked_range_to_session_snapshot(range: &TrackedUploadRange) -> UploadSessionSnapshot {
    UploadSessionSnapshot {
        session_id: range.session_id,
        start: range.start,
        end: range.end,
        bytes_total: range.end.saturating_sub(range.start).saturating_add(1),
        phase: range.phase,
        peer_id_hex: range.peer_id_hex.clone(),
        payload_source: range.payload_source,
        started_unix_secs: range.started_unix_secs,
        last_updated_unix_secs: range.last_updated_unix_secs,
        terminal_reason: range.terminal_reason,
    }
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

        tracker.note_held("abcd", "peer-a", 0, 1023, ttl);
        let held = tracker.snapshot_for_hash("abcd");
        assert_eq!(held.total_requests, 1);
        assert_eq!(held.requested_bytes_total, 1024);
        assert_eq!(held.rate_bps_5s, 0);
        assert_eq!(held.rate_bps_30s, 0);
        assert_eq!(held.zero_fill_rate_bps_5s, 0);
        assert_eq!(held.zero_fill_rate_bps_30s, 0);
        assert_eq!(held.zero_fill_requests_total, 0);
        assert!(!held.zero_fill_active);
        assert_eq!(held.active_ranges.len(), 1);
        assert_eq!(held.sessions.len(), 1);
        let session_id = held.sessions[0].session_id;
        assert_eq!(held.active_ranges[0].phase, UploadRangePhase::Held);
        assert_eq!(held.active_ranges[0].peer_id_hex, "peer-a");
        assert_eq!(held.active_peer_ids, vec!["peer-a".to_string()]);
        let active_since = held.active_since_unix_secs;
        assert!(active_since.is_some());

        tracker.note_sending(
            "abcd",
            "peer-a",
            0,
            1023,
            ttl,
            UploadPayloadSource::SharedFile,
        );
        let sending = tracker.snapshot_for_hash("ABCD");
        assert_eq!(sending.total_requests, 2);
        assert_eq!(sending.requested_bytes_total, 2048);
        assert!(sending.rate_bps_5s > 0);
        assert!(sending.rate_bps_30s > 0);
        assert_eq!(sending.zero_fill_requests_total, 0);
        assert_eq!(sending.zero_fill_requested_bytes_total, 0);
        assert_eq!(sending.zero_fill_rate_bps_5s, 0);
        assert_eq!(sending.zero_fill_rate_bps_30s, 0);
        assert!(!sending.zero_fill_active);
        assert_eq!(sending.active_ranges.len(), 1);
        assert_eq!(sending.sessions.len(), 1);
        assert_eq!(sending.sessions[0].session_id, session_id);
        assert_eq!(sending.sessions[0].phase, UploadRangePhase::Sending);
        assert_eq!(sending.active_ranges[0].phase, UploadRangePhase::Sending);
        assert_eq!(sending.last_peer_id_hex.as_deref(), Some("peer-a"));
        assert_eq!(
            sending.last_payload_source,
            Some(UploadPayloadSource::SharedFile)
        );
        assert_eq!(sending.active_since_unix_secs, active_since);
    }

    #[test]
    fn tracker_snapshot_all_includes_hash() {
        let tracker = UploadActivityTracker::default();
        tracker.note_held("beef", "peer-b", 0, 63, Duration::from_secs(5));
        let snapshots = tracker.snapshot_all();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].file_hash_md4_hex, "beef");
        assert_eq!(snapshots[0].active_peer_ids, vec!["peer-b".to_string()]);
        assert_eq!(snapshots[0].sessions.len(), 1);
    }

    #[test]
    fn tracker_allocates_new_session_id_after_expiry() {
        let tracker = UploadActivityTracker::default();
        tracker.note_held("fade", "peer-expire", 0, 63, Duration::from_millis(1));
        let first = tracker.snapshot_for_hash("fade");
        assert_eq!(first.sessions.len(), 1);
        let first_id = first.sessions[0].session_id;
        std::thread::sleep(Duration::from_millis(5));
        tracker.note_held("fade", "peer-expire", 0, 63, Duration::from_secs(1));
        let second = tracker.snapshot_for_hash("fade");
        assert_eq!(second.sessions.len(), 1);
        assert!(second.sessions[0].session_id > first_id);
    }

    #[test]
    fn tracker_keeps_recent_session_history_after_expiry() {
        let tracker = UploadActivityTracker::default();
        tracker.note_sending(
            "hist",
            "peer-hist",
            0,
            127,
            Duration::from_millis(1),
            UploadPayloadSource::SharedFile,
        );
        std::thread::sleep(Duration::from_millis(5));
        let snapshot = tracker.snapshot_for_hash("hist");
        assert_eq!(snapshot.sessions.len(), 0);
        assert_eq!(snapshot.recent_sessions.len(), 1);
        assert_eq!(snapshot.recent_session_count, 1);
        assert_eq!(snapshot.recent_sessions[0].peer_id_hex, "peer-hist");
        assert_eq!(
            snapshot.recent_sessions[0].terminal_reason,
            Some(UploadTerminalReason::Expired)
        );
    }

    #[test]
    fn tracker_moves_terminalled_session_to_recent_history() {
        let tracker = UploadActivityTracker::default();
        tracker.note_held("drop", "peer-drop", 0, 127, Duration::from_secs(30));
        tracker.note_terminal("drop", "peer-drop", 0, 127, UploadTerminalReason::Dropped);

        let snapshot = tracker.snapshot_for_hash("drop");
        assert_eq!(snapshot.sessions.len(), 0);
        assert_eq!(snapshot.recent_sessions.len(), 1);
        assert_eq!(
            snapshot.recent_sessions[0].terminal_reason,
            Some(UploadTerminalReason::Dropped)
        );
    }

    #[test]
    fn tracker_keeps_first_terminal_reason_after_expiry() {
        let tracker = UploadActivityTracker::default();
        tracker.note_held("first", "peer-first", 0, 127, Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));
        let expired = tracker.snapshot_for_hash("first");
        assert_eq!(expired.sessions.len(), 0);
        assert_eq!(expired.recent_sessions.len(), 1);
        assert_eq!(
            expired.recent_sessions[0].terminal_reason,
            Some(UploadTerminalReason::Expired)
        );

        tracker.note_terminal("first", "peer-first", 0, 127, UploadTerminalReason::Dropped);
        let after_terminal = tracker.snapshot_for_hash("first");
        assert_eq!(after_terminal.recent_sessions.len(), 1);
        assert_eq!(
            after_terminal.recent_sessions[0].terminal_reason,
            Some(UploadTerminalReason::Expired)
        );
    }

    #[test]
    fn tracker_caps_recent_session_history_per_file() {
        let tracker = UploadActivityTracker::default();
        for idx in 0..140u64 {
            tracker.note_held(
                "cap",
                &format!("peer-{idx}"),
                idx * 10,
                idx * 10 + 9,
                Duration::from_millis(1),
            );
        }
        std::thread::sleep(Duration::from_millis(5));
        let snapshot = tracker.snapshot_for_hash("cap");
        assert_eq!(snapshot.sessions.len(), 0);
        assert_eq!(snapshot.recent_session_count, 128);
        assert_eq!(snapshot.recent_sessions.len(), 128);
    }

    #[test]
    fn tracker_records_zero_fill_fallback_activity() {
        let tracker = UploadActivityTracker::default();
        tracker.note_sending(
            "cafe",
            "peer-z",
            0,
            511,
            Duration::from_secs(5),
            UploadPayloadSource::ZeroFillFallback,
        );
        let snapshot = tracker.snapshot_for_hash("cafe");
        assert_eq!(snapshot.zero_fill_requests_total, 1);
        assert_eq!(snapshot.zero_fill_requested_bytes_total, 512);
        assert!(snapshot.zero_fill_rate_bps_5s > 0);
        assert!(snapshot.zero_fill_rate_bps_30s > 0);
        assert!(snapshot.zero_fill_active);
        assert_eq!(
            snapshot.last_payload_source,
            Some(UploadPayloadSource::ZeroFillFallback)
        );
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
