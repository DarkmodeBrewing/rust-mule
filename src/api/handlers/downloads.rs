use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::api::ApiState;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadEntry {
    pub(crate) part_number: u16,
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) state: String,
    pub(crate) downloaded_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadListResponse {
    pub(crate) queue_len: usize,
    pub(crate) recovered_on_start: usize,
    pub(crate) downloads: Vec<DownloadEntry>,
}

pub(crate) async fn downloads(
    State(state): State<ApiState>,
) -> Result<Json<DownloadListResponse>, StatusCode> {
    let items = state
        .download_handle
        .list()
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
        })
        .collect::<Vec<_>>();

    Ok(Json(DownloadListResponse {
        queue_len: downloads.len(),
        recovered_on_start: state
            .download_handle
            .recovered_count()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        downloads,
    }))
}
