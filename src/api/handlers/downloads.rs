use axum::{Json, body::Bytes, extract::State, http::StatusCode};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::api::{
    ApiState,
    error::{ApiErrorEnvelope, parse_json_with_limit, status_with_message},
};
use crate::download::service::DownloadDetail;
use crate::download::{CreateDownloadRequest, DownloadError, DownloadSummary};
use crate::kad::{
    KadId,
    service::{KadServiceCommand, KadSharedPublishStatus},
};
use crate::shared_ops::SharedActionRejectReason;
use crate::upload::{
    UploadActivitySnapshot, UploadPayloadSource, UploadRangePhase, UploadTerminalReason,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadEntry {
    pub(crate) part_number: u16,
    pub(crate) file_name: String,
    pub(crate) file_hash_md4_hex: String,
    pub(crate) file_size: u64,
    pub(crate) state: String,
    pub(crate) downloaded_bytes: u64,
    /// Average transfer rate over the last 5 seconds, in bytes per second.
    pub(crate) rate_bps_5s: u64,
    /// Average transfer rate over the last 30 seconds, in bytes per second.
    pub(crate) rate_bps_30s: u64,
    pub(crate) progress_pct: u8,
    pub(crate) missing_ranges: usize,
    pub(crate) inflight_ranges: usize,
    pub(crate) retry_count: u32,
    pub(crate) last_error: Option<String>,
    pub(crate) source_count: usize,
    pub(crate) missing_range_spans: Vec<ByteRangeEntry>,
    pub(crate) inflight_range_spans: Vec<ByteRangeEntry>,
    pub(crate) created_unix_secs: u64,
    pub(crate) updated_unix_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ByteRangeEntry {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadListResponse {
    pub(crate) queue_len: usize,
    pub(crate) recovered_on_start: usize,
    pub(crate) reserve_calls_total: u64,
    pub(crate) reserve_granted_blocks_total: u64,
    pub(crate) reserve_denied_cooldown_total: u64,
    pub(crate) reserve_denied_peer_cap_total: u64,
    pub(crate) reserve_denied_download_cap_total: u64,
    pub(crate) reserve_denied_state_total: u64,
    pub(crate) reserve_empty_no_missing_total: u64,
    pub(crate) downloads: Vec<DownloadEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SharedFileEntry {
    pub(crate) file_name: String,
    pub(crate) relative_path: String,
    pub(crate) file_hash_md4_hex: String,
    pub(crate) file_size: u64,
    pub(crate) source_count: usize,
    pub(crate) local_source_cached: bool,
    pub(crate) source_publish_attempts: u64,
    pub(crate) source_publish_last_result: Option<String>,
    pub(crate) source_publish_last_attempt_unix_secs: Option<u64>,
    pub(crate) source_publish_response_received: bool,
    pub(crate) source_publish_first_response_latency_ms: Option<u64>,
    pub(crate) keyword_publish_attempts: u64,
    pub(crate) keyword_publish_queued: u64,
    pub(crate) keyword_publish_failed: u64,
    pub(crate) keyword_publish_last_result: Option<String>,
    pub(crate) keyword_publish_last_attempt_unix_secs: Option<u64>,
    pub(crate) keyword_publish_total: usize,
    pub(crate) keyword_publish_acked: usize,
    pub(crate) queued_downloads: usize,
    pub(crate) inflight_downloads: usize,
    pub(crate) queued_uploads: usize,
    pub(crate) inflight_uploads: usize,
    pub(crate) total_upload_requests: u64,
    pub(crate) requested_bytes_total: u64,
    pub(crate) last_requested_unix_secs: Option<u64>,
    pub(crate) queued_upload_ranges: Vec<ByteRangeEntry>,
    pub(crate) inflight_upload_ranges: Vec<ByteRangeEntry>,
    pub(crate) active_request: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SharedFilesResponse {
    pub(crate) files: Vec<SharedFileEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UploadEntry {
    pub(crate) file_name: Option<String>,
    pub(crate) relative_path: Option<String>,
    pub(crate) file_hash_md4_hex: String,
    pub(crate) total_upload_requests: u64,
    pub(crate) requested_bytes_total: u64,
    /// Average transfer rate over the last 5 seconds, in bytes per second.
    pub(crate) rate_bps_5s: u64,
    /// Average transfer rate over the last 30 seconds, in bytes per second.
    pub(crate) rate_bps_30s: u64,
    pub(crate) zero_fill_requests_total: u64,
    pub(crate) zero_fill_requested_bytes_total: u64,
    /// Average zero-fill fallback rate over the last 5 seconds, in bytes per second.
    pub(crate) zero_fill_rate_bps_5s: u64,
    /// Average zero-fill fallback rate over the last 30 seconds, in bytes per second.
    pub(crate) zero_fill_rate_bps_30s: u64,
    pub(crate) zero_fill_active: bool,
    pub(crate) last_requested_unix_secs: Option<u64>,
    pub(crate) last_peer_id_hex: Option<String>,
    pub(crate) active_peer_ids: Vec<String>,
    pub(crate) active_since_unix_secs: Option<u64>,
    pub(crate) last_payload_source: Option<String>,
    pub(crate) session_count: usize,
    pub(crate) sessions: Vec<UploadSessionEntry>,
    pub(crate) recent_session_count: usize,
    pub(crate) recent_sessions: Vec<UploadSessionEntry>,
    pub(crate) held_ranges: Vec<ByteRangeEntry>,
    pub(crate) sending_ranges: Vec<ByteRangeEntry>,
    pub(crate) active_request: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UploadSessionEntry {
    pub(crate) session_id: u64,
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) bytes_total: u64,
    pub(crate) phase: String,
    pub(crate) peer_id_hex: String,
    pub(crate) payload_source: Option<String>,
    pub(crate) started_unix_secs: Option<u64>,
    pub(crate) last_updated_unix_secs: Option<u64>,
    pub(crate) terminal_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UploadListResponse {
    pub(crate) uploads: Vec<UploadEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SharedActionsResponse {
    pub(crate) actions: Vec<crate::shared_ops::SharedActionStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SharedActionResponse {
    pub(crate) started: bool,
    pub(crate) reason: Option<crate::shared_ops::SharedActionRejectReason>,
    pub(crate) status: crate::shared_ops::SharedActionStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SharedActionRequestBody {
    #[serde(default)]
    pub(crate) confirm: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateDownloadRequestBody {
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) file_hash_md4_hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadActionResponse {
    pub(crate) download: DownloadEntry,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadDeleteResponse {
    pub(crate) deleted: bool,
}

pub(crate) async fn downloads(
    State(state): State<ApiState>,
) -> Result<Json<DownloadListResponse>, StatusCode> {
    let (status, items) = state
        .download_handle
        .snapshot_detailed()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let source_counts = join_all(
        items
            .iter()
            .map(|item| source_count_for_file(&state, &item.summary.file_hash_md4_hex)),
    )
    .await;
    let downloads = items
        .iter()
        .zip(source_counts)
        .map(|(item, source_count)| download_entry_from_detail(item, source_count))
        .collect();

    Ok(Json(DownloadListResponse {
        queue_len: status.queue_len,
        recovered_on_start: status.recovered_on_start,
        reserve_calls_total: status.reserve_calls_total,
        reserve_granted_blocks_total: status.reserve_granted_blocks_total,
        reserve_denied_cooldown_total: status.reserve_denied_cooldown_total,
        reserve_denied_peer_cap_total: status.reserve_denied_peer_cap_total,
        reserve_denied_download_cap_total: status.reserve_denied_download_cap_total,
        reserve_denied_state_total: status.reserve_denied_state_total,
        reserve_empty_no_missing_total: status.reserve_empty_no_missing_total,
        downloads,
    }))
}

pub(crate) async fn shared_files(
    State(state): State<ApiState>,
) -> Result<Json<SharedFilesResponse>, StatusCode> {
    let (_status, downloads) = state
        .download_handle
        .snapshot_detailed()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let shared_library = state.shared_library.read().await.clone();
    let shared_files = shared_library.files();
    let shared_publish_statuses = join_all(
        shared_files
            .iter()
            .map(|file| shared_publish_status_for_file(&state, &file.file_hash_md4_hex)),
    )
    .await;

    let mut files = Vec::with_capacity(shared_files.len());
    for (file, kad_publish_status) in shared_files.iter().zip(shared_publish_statuses) {
        let (queued_downloads, inflight_downloads) =
            download_activity_for_file(&downloads, &file.file_hash_md4_hex);
        let publish_status = state
            .publish_tracker
            .snapshot_for_hash(&file.file_hash_md4_hex);
        let upload_activity = state
            .upload_service
            .snapshot_for_hash(&file.file_hash_md4_hex);
        let queued_upload_ranges = upload_ranges_by_phase(&upload_activity, UploadRangePhase::Held);
        let inflight_upload_ranges =
            upload_ranges_by_phase(&upload_activity, UploadRangePhase::Sending);

        files.push(SharedFileEntry {
            file_name: file
                .relative_path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| file.relative_path.display().to_string()),
            relative_path: file.relative_path.display().to_string(),
            file_hash_md4_hex: file.file_hash_md4_hex.clone(),
            file_size: file.file_size,
            source_count: 0,
            local_source_cached: kad_publish_status.local_source_cached,
            source_publish_attempts: publish_status.source_attempts,
            source_publish_last_result: publish_status.source_last_result,
            source_publish_last_attempt_unix_secs: publish_status.source_last_attempt_unix_secs,
            source_publish_response_received: kad_publish_status.source_publish_response_received,
            source_publish_first_response_latency_ms: kad_publish_status
                .source_publish_first_response_latency_ms,
            keyword_publish_attempts: publish_status.keyword_attempts,
            keyword_publish_queued: publish_status.keyword_queued,
            keyword_publish_failed: publish_status.keyword_failed,
            keyword_publish_last_result: publish_status.keyword_last_result,
            keyword_publish_last_attempt_unix_secs: publish_status.keyword_last_attempt_unix_secs,
            keyword_publish_total: kad_publish_status.keyword_publish_total,
            keyword_publish_acked: kad_publish_status.keyword_publish_acked,
            queued_downloads,
            inflight_downloads,
            queued_uploads: queued_upload_ranges.len(),
            inflight_uploads: inflight_upload_ranges.len(),
            total_upload_requests: upload_activity.total_requests,
            requested_bytes_total: upload_activity.requested_bytes_total,
            last_requested_unix_secs: upload_activity.last_requested_unix_secs,
            queued_upload_ranges,
            inflight_upload_ranges,
            active_request: !upload_activity.active_ranges.is_empty(),
        });
    }
    Ok(Json(SharedFilesResponse { files }))
}

pub(crate) async fn uploads(
    State(state): State<ApiState>,
) -> Result<Json<UploadListResponse>, StatusCode> {
    let shared_library = state.shared_library.read().await;
    let uploads = state
        .upload_service
        .snapshot_all()
        .into_iter()
        .map(|snapshot| {
            let held_ranges = upload_ranges_by_phase(&snapshot, UploadRangePhase::Held);
            let sending_ranges = upload_ranges_by_phase(&snapshot, UploadRangePhase::Sending);
            let shared_file = shared_library.get_by_hash_hex(&snapshot.file_hash_md4_hex);
            let file_hash_md4_hex = snapshot.file_hash_md4_hex.clone();
            UploadEntry {
                file_name: shared_file.and_then(|file| {
                    file.relative_path
                        .file_name()
                        .map(|v| v.to_string_lossy().to_string())
                }),
                relative_path: shared_file.map(|file| file.relative_path.display().to_string()),
                file_hash_md4_hex,
                total_upload_requests: snapshot.total_requests,
                requested_bytes_total: snapshot.requested_bytes_total,
                rate_bps_5s: snapshot.rate_bps_5s,
                rate_bps_30s: snapshot.rate_bps_30s,
                zero_fill_requests_total: snapshot.zero_fill_requests_total,
                zero_fill_requested_bytes_total: snapshot.zero_fill_requested_bytes_total,
                zero_fill_rate_bps_5s: snapshot.zero_fill_rate_bps_5s,
                zero_fill_rate_bps_30s: snapshot.zero_fill_rate_bps_30s,
                zero_fill_active: snapshot.zero_fill_active,
                last_requested_unix_secs: snapshot.last_requested_unix_secs,
                last_peer_id_hex: snapshot.last_peer_id_hex,
                active_peer_ids: snapshot.active_peer_ids,
                active_since_unix_secs: snapshot.active_since_unix_secs,
                last_payload_source: snapshot.last_payload_source.map(|source| match source {
                    UploadPayloadSource::SharedFile => "shared_file".to_string(),
                    UploadPayloadSource::ZeroFillFallback => "zero_fill_fallback".to_string(),
                }),
                session_count: snapshot.sessions.len(),
                sessions: snapshot
                    .sessions
                    .into_iter()
                    .map(session_snapshot_to_entry)
                    .collect(),
                recent_session_count: snapshot.recent_session_count,
                recent_sessions: snapshot
                    .recent_sessions
                    .into_iter()
                    .map(session_snapshot_to_entry)
                    .collect(),
                held_ranges,
                sending_ranges,
                active_request: !snapshot.active_ranges.is_empty(),
            }
        })
        .collect();
    Ok(Json(UploadListResponse { uploads }))
}

fn session_snapshot_to_entry(session: crate::upload::UploadSessionSnapshot) -> UploadSessionEntry {
    UploadSessionEntry {
        session_id: session.session_id,
        start: session.start,
        end: session.end,
        bytes_total: session.bytes_total,
        phase: match session.phase {
            UploadRangePhase::Held => "held".to_string(),
            UploadRangePhase::Sending => "sending".to_string(),
        },
        peer_id_hex: session.peer_id_hex,
        payload_source: session.payload_source.map(|source| match source {
            UploadPayloadSource::SharedFile => "shared_file".to_string(),
            UploadPayloadSource::ZeroFillFallback => "zero_fill_fallback".to_string(),
        }),
        started_unix_secs: session.started_unix_secs,
        last_updated_unix_secs: session.last_updated_unix_secs,
        terminal_reason: session.terminal_reason.map(|reason| match reason {
            UploadTerminalReason::Expired => "expired".to_string(),
        }),
    }
}

pub(crate) async fn shared_actions(
    State(state): State<ApiState>,
) -> Result<Json<SharedActionsResponse>, StatusCode> {
    let snapshot = state.shared_ops.action_snapshot().await;
    Ok(Json(SharedActionsResponse {
        actions: snapshot.actions,
    }))
}

pub(crate) async fn shared_reindex(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<(StatusCode, Json<SharedActionResponse>), (StatusCode, Json<ApiErrorEnvelope>)> {
    let req: SharedActionRequestBody =
        parse_json_with_limit(body, 4 * 1024).map_err(status_with_message)?;
    shared_action_confirmed(req.confirm)?;
    let response = state.shared_ops.start_reindex().await;
    Ok((
        map_shared_action_status(&response),
        Json(SharedActionResponse {
            started: response.started,
            reason: response.reason,
            status: response.status,
        }),
    ))
}

pub(crate) async fn shared_republish_sources(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<(StatusCode, Json<SharedActionResponse>), (StatusCode, Json<ApiErrorEnvelope>)> {
    let req: SharedActionRequestBody =
        parse_json_with_limit(body, 4 * 1024).map_err(status_with_message)?;
    shared_action_confirmed(req.confirm)?;
    let response = state.shared_ops.start_republish_sources().await;
    Ok((
        map_shared_action_status(&response),
        Json(SharedActionResponse {
            started: response.started,
            reason: response.reason,
            status: response.status,
        }),
    ))
}

pub(crate) async fn shared_republish_keywords(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<(StatusCode, Json<SharedActionResponse>), (StatusCode, Json<ApiErrorEnvelope>)> {
    let req: SharedActionRequestBody =
        parse_json_with_limit(body, 4 * 1024).map_err(status_with_message)?;
    shared_action_confirmed(req.confirm)?;
    let response = state.shared_ops.start_republish_keywords().await;
    Ok((
        map_shared_action_status(&response),
        Json(SharedActionResponse {
            started: response.started,
            reason: response.reason,
            status: response.status,
        }),
    ))
}

pub(crate) async fn downloads_create(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<(StatusCode, Json<DownloadActionResponse>), (StatusCode, Json<ApiErrorEnvelope>)> {
    let req: CreateDownloadRequestBody =
        parse_json_with_limit(body, 8 * 1024).map_err(status_with_message)?;
    let summary = state
        .download_handle
        .create_download(CreateDownloadRequest {
            file_name: req.file_name,
            file_size: req.file_size,
            file_hash_md4_hex: req.file_hash_md4_hex,
        })
        .await
        .map_err(map_download_error_envelope)?;

    Ok((
        StatusCode::CREATED,
        Json(DownloadActionResponse {
            download: download_entry_from_summary(&summary),
        }),
    ))
}

pub(crate) async fn downloads_pause(
    State(state): State<ApiState>,
    axum::extract::Path(part_number): axum::extract::Path<u16>,
) -> Result<Json<DownloadActionResponse>, StatusCode> {
    let summary = state
        .download_handle
        .pause(part_number)
        .await
        .map_err(map_download_error)?;
    Ok(Json(DownloadActionResponse {
        download: download_entry_from_summary(&summary),
    }))
}

pub(crate) async fn downloads_resume(
    State(state): State<ApiState>,
    axum::extract::Path(part_number): axum::extract::Path<u16>,
) -> Result<Json<DownloadActionResponse>, StatusCode> {
    let summary = state
        .download_handle
        .resume(part_number)
        .await
        .map_err(map_download_error)?;
    Ok(Json(DownloadActionResponse {
        download: download_entry_from_summary(&summary),
    }))
}

pub(crate) async fn downloads_cancel(
    State(state): State<ApiState>,
    axum::extract::Path(part_number): axum::extract::Path<u16>,
) -> Result<Json<DownloadActionResponse>, StatusCode> {
    let summary = state
        .download_handle
        .cancel(part_number)
        .await
        .map_err(map_download_error)?;
    Ok(Json(DownloadActionResponse {
        download: download_entry_from_summary(&summary),
    }))
}

pub(crate) async fn downloads_delete(
    State(state): State<ApiState>,
    axum::extract::Path(part_number): axum::extract::Path<u16>,
) -> Result<Json<DownloadDeleteResponse>, StatusCode> {
    state
        .download_handle
        .delete(part_number)
        .await
        .map_err(map_download_error)?;
    Ok(Json(DownloadDeleteResponse { deleted: true }))
}

fn map_download_error(err: DownloadError) -> StatusCode {
    match err {
        DownloadError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        DownloadError::NotFound(_) => StatusCode::NOT_FOUND,
        DownloadError::InvalidTransition { .. } => StatusCode::CONFLICT,
        DownloadError::ChannelClosed => StatusCode::SERVICE_UNAVAILABLE,
        DownloadError::Store(_) | DownloadError::ServiceJoin(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn download_entry_from_detail(detail: &DownloadDetail, source_count: usize) -> DownloadEntry {
    DownloadEntry {
        part_number: detail.summary.part_number,
        file_name: detail.summary.file_name.clone(),
        file_hash_md4_hex: detail.summary.file_hash_md4_hex.clone(),
        file_size: detail.summary.file_size,
        state: format!("{:?}", detail.summary.state).to_lowercase(),
        downloaded_bytes: detail.summary.downloaded_bytes,
        rate_bps_5s: detail.summary.rate_bps_5s,
        rate_bps_30s: detail.summary.rate_bps_30s,
        progress_pct: detail.summary.progress_pct,
        missing_ranges: detail.summary.missing_ranges,
        inflight_ranges: detail.summary.inflight_ranges,
        retry_count: detail.summary.retry_count,
        last_error: detail.summary.last_error.clone(),
        source_count,
        missing_range_spans: detail
            .missing_ranges
            .iter()
            .map(|r| ByteRangeEntry {
                start: r.start,
                end: r.end,
            })
            .collect(),
        inflight_range_spans: detail
            .inflight_ranges
            .iter()
            .map(|r| ByteRangeEntry {
                start: r.start,
                end: r.end,
            })
            .collect(),
        created_unix_secs: detail.created_unix_secs,
        updated_unix_secs: detail.updated_unix_secs,
    }
}

fn download_entry_from_summary(summary: &DownloadSummary) -> DownloadEntry {
    DownloadEntry {
        part_number: summary.part_number,
        file_name: summary.file_name.clone(),
        file_hash_md4_hex: summary.file_hash_md4_hex.clone(),
        file_size: summary.file_size,
        state: format!("{:?}", summary.state).to_lowercase(),
        downloaded_bytes: summary.downloaded_bytes,
        rate_bps_5s: summary.rate_bps_5s,
        rate_bps_30s: summary.rate_bps_30s,
        progress_pct: summary.progress_pct,
        missing_ranges: summary.missing_ranges,
        inflight_ranges: summary.inflight_ranges,
        retry_count: summary.retry_count,
        last_error: summary.last_error.clone(),
        source_count: 0,
        missing_range_spans: Vec::new(),
        inflight_range_spans: Vec::new(),
        created_unix_secs: 0,
        updated_unix_secs: 0,
    }
}

async fn source_count_for_file(state: &ApiState, hash_hex: &str) -> usize {
    const SOURCE_COUNT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
    let Ok(file) = KadId::from_hex(hash_hex) else {
        return 0;
    };
    let (tx, rx) = oneshot::channel();
    if state
        .kad_cmd_tx
        .send(KadServiceCommand::GetSources {
            file,
            respond_to: tx,
        })
        .await
        .is_err()
    {
        return 0;
    }
    match tokio::time::timeout(SOURCE_COUNT_TIMEOUT, rx).await {
        Ok(Ok(items)) => items.len(),
        _ => 0,
    }
}

async fn shared_publish_status_for_file(
    state: &ApiState,
    hash_hex: &str,
) -> KadSharedPublishStatus {
    const STATUS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
    let Ok(file) = KadId::from_hex(hash_hex) else {
        return KadSharedPublishStatus::default();
    };
    let (tx, rx) = oneshot::channel();
    if state
        .kad_cmd_tx
        .send(KadServiceCommand::GetSharedPublishStatus {
            file,
            respond_to: tx,
        })
        .await
        .is_err()
    {
        return KadSharedPublishStatus::default();
    }
    match tokio::time::timeout(STATUS_TIMEOUT, rx).await {
        Ok(Ok(status)) => status,
        _ => KadSharedPublishStatus::default(),
    }
}

fn map_download_error_envelope(err: DownloadError) -> (StatusCode, Json<ApiErrorEnvelope>) {
    match err {
        DownloadError::InvalidInput(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorEnvelope {
                code: StatusCode::BAD_REQUEST.as_u16(),
                message: msg,
            }),
        ),
        other => status_with_message(map_download_error(other)),
    }
}

fn map_shared_action_status(response: &crate::shared_ops::SharedActionStartResponse) -> StatusCode {
    if response.started {
        StatusCode::ACCEPTED
    } else {
        match response.reason {
            Some(SharedActionRejectReason::CooldownActive) => StatusCode::TOO_MANY_REQUESTS,
            Some(SharedActionRejectReason::AlreadyRunning) | None => StatusCode::CONFLICT,
        }
    }
}

fn shared_action_confirmed(confirmed: bool) -> Result<(), (StatusCode, Json<ApiErrorEnvelope>)> {
    if confirmed {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorEnvelope {
                code: StatusCode::BAD_REQUEST.as_u16(),
                message: "confirmation required".to_string(),
            }),
        ))
    }
}

fn download_activity_for_file(
    downloads: &[DownloadDetail],
    file_hash_md4_hex: &str,
) -> (usize, usize) {
    let mut queued_downloads = 0usize;
    let mut inflight_downloads = 0usize;
    for download in downloads {
        if !download
            .summary
            .file_hash_md4_hex
            .eq_ignore_ascii_case(file_hash_md4_hex)
        {
            continue;
        }
        queued_downloads += 1;
        if !download.inflight_ranges.is_empty() {
            inflight_downloads += 1;
        }
    }
    (queued_downloads, inflight_downloads)
}

fn upload_ranges_by_phase(
    activity: &UploadActivitySnapshot,
    phase: UploadRangePhase,
) -> Vec<ByteRangeEntry> {
    activity
        .active_ranges
        .iter()
        .filter(|range| range.phase == phase)
        .map(|range| ByteRangeEntry {
            start: range.start,
            end: range.end,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_error_preserves_detail_message() {
        let (status, body) = map_download_error_envelope(DownloadError::InvalidInput(
            "file hash must be 32 hex chars".to_string(),
        ));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0.code, StatusCode::BAD_REQUEST.as_u16());
        assert_eq!(body.0.message, "file hash must be 32 hex chars");
    }
}
