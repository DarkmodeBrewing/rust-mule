use axum::{Json, body::Bytes, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::api::{ApiState, error::parse_json_with_limit};
use crate::download::{CreateDownloadRequest, DownloadError};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadEntry {
    pub(crate) part_number: u16,
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) state: String,
    pub(crate) downloaded_bytes: u64,
    pub(crate) progress_pct: u8,
    pub(crate) missing_ranges: usize,
    pub(crate) inflight_ranges: usize,
    pub(crate) retry_count: u32,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadListResponse {
    pub(crate) queue_len: usize,
    pub(crate) recovered_on_start: usize,
    pub(crate) reserve_denied_cooldown_total: u64,
    pub(crate) reserve_denied_peer_cap_total: u64,
    pub(crate) reserve_denied_download_cap_total: u64,
    pub(crate) downloads: Vec<DownloadEntry>,
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
        .snapshot()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let downloads = items
        .iter()
        .map(|d| DownloadEntry {
            part_number: d.part_number,
            file_name: d.file_name.clone(),
            file_size: d.file_size,
            state: format!("{:?}", d.state).to_lowercase(),
            downloaded_bytes: d.downloaded_bytes,
            progress_pct: d.progress_pct,
            missing_ranges: d.missing_ranges,
            inflight_ranges: d.inflight_ranges,
            retry_count: d.retry_count,
            last_error: d.last_error.clone(),
        })
        .collect::<Vec<_>>();

    Ok(Json(DownloadListResponse {
        queue_len: status.queue_len,
        recovered_on_start: status.recovered_on_start,
        reserve_denied_cooldown_total: status.reserve_denied_cooldown_total,
        reserve_denied_peer_cap_total: status.reserve_denied_peer_cap_total,
        reserve_denied_download_cap_total: status.reserve_denied_download_cap_total,
        downloads,
    }))
}

pub(crate) async fn downloads_create(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<(StatusCode, Json<DownloadActionResponse>), StatusCode> {
    let req: CreateDownloadRequestBody = parse_json_with_limit(body, 8 * 1024)?;
    let summary = state
        .download_handle
        .create_download(CreateDownloadRequest {
            file_name: req.file_name,
            file_size: req.file_size,
            file_hash_md4_hex: req.file_hash_md4_hex,
        })
        .await
        .map_err(map_download_error)?;

    Ok((
        StatusCode::CREATED,
        Json(DownloadActionResponse {
            download: DownloadEntry {
                part_number: summary.part_number,
                file_name: summary.file_name,
                file_size: summary.file_size,
                state: format!("{:?}", summary.state).to_lowercase(),
                downloaded_bytes: summary.downloaded_bytes,
                progress_pct: summary.progress_pct,
                missing_ranges: summary.missing_ranges,
                inflight_ranges: summary.inflight_ranges,
                retry_count: summary.retry_count,
                last_error: summary.last_error,
            },
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
        download: DownloadEntry {
            part_number: summary.part_number,
            file_name: summary.file_name,
            file_size: summary.file_size,
            state: format!("{:?}", summary.state).to_lowercase(),
            downloaded_bytes: summary.downloaded_bytes,
            progress_pct: summary.progress_pct,
            missing_ranges: summary.missing_ranges,
            inflight_ranges: summary.inflight_ranges,
            retry_count: summary.retry_count,
            last_error: summary.last_error,
        },
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
        download: DownloadEntry {
            part_number: summary.part_number,
            file_name: summary.file_name,
            file_size: summary.file_size,
            state: format!("{:?}", summary.state).to_lowercase(),
            downloaded_bytes: summary.downloaded_bytes,
            progress_pct: summary.progress_pct,
            missing_ranges: summary.missing_ranges,
            inflight_ranges: summary.inflight_ranges,
            retry_count: summary.retry_count,
            last_error: summary.last_error,
        },
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
        download: DownloadEntry {
            part_number: summary.part_number,
            file_name: summary.file_name,
            file_size: summary.file_size,
            state: format!("{:?}", summary.state).to_lowercase(),
            downloaded_bytes: summary.downloaded_bytes,
            progress_pct: summary.progress_pct,
            missing_ranges: summary.missing_ranges,
            inflight_ranges: summary.inflight_ranges,
            retry_count: summary.retry_count,
            last_error: summary.last_error,
        },
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
