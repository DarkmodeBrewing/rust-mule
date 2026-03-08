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
use crate::upload::{UploadActivitySnapshot, UploadRangePhase};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadEntry {
    pub(crate) part_number: u16,
    pub(crate) file_name: String,
    pub(crate) file_hash_md4_hex: String,
    pub(crate) file_size: u64,
    pub(crate) state: String,
    pub(crate) downloaded_bytes: u64,
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

    let mut files = Vec::with_capacity(state.shared_library.files().len());
    for file in state.shared_library.files() {
        let (queued_downloads, inflight_downloads) =
            download_activity_for_file(&downloads, &file.file_hash_md4_hex);
        let kad_publish_status =
            shared_publish_status_for_file(&state, &file.file_hash_md4_hex).await;
        let publish_status = state
            .publish_tracker
            .snapshot_for_hash(&file.file_hash_md4_hex);
        let upload_activity = state
            .upload_activity
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
