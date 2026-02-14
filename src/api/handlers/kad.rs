use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiState,
    kad::{
        KadId, keyword,
        service::{
            KadKeywordHit, KadKeywordSearchInfo, KadPeerInfo, KadServiceCommand, KadSourceEntry,
            RoutingBucketSummary, RoutingNodeSummary, RoutingSummary,
        },
    },
};

#[derive(Debug, Deserialize)]
pub(crate) struct KadSourcesReq {
    pub(crate) file_id_hex: String,
    #[serde(default)]
    pub(crate) file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KadSearchKeywordReq {
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) keyword_id_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KadPublishKeywordReq {
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) keyword_id_hex: Option<String>,
    pub(crate) file_id_hex: String,
    pub(crate) filename: String,
    pub(crate) file_size: u64,
    #[serde(default)]
    pub(crate) file_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QueuedResponse {
    pub(crate) queued: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadSourcesResponse {
    pub(crate) file_id_hex: String,
    pub(crate) sources: Vec<KadSourceJson>,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadSourceJson {
    pub(crate) source_id_hex: String,
    pub(crate) udp_dest_b64: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadKeywordResultsResponse {
    pub(crate) keyword_id_hex: String,
    pub(crate) hits: Vec<KadKeywordHitJson>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchListResponse {
    pub(crate) searches: Vec<KadKeywordSearchInfo>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchDetailsResponse {
    pub(crate) search: KadKeywordSearchInfo,
    pub(crate) hits: Vec<KadKeywordHitJson>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchStopResponse {
    pub(crate) stopped: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchDeleteQuery {
    #[serde(default = "default_true")]
    pub(crate) purge_results: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchDeleteResponse {
    pub(crate) deleted: bool,
    pub(crate) purged_results: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadPeersResponse {
    pub(crate) peers: Vec<KadPeerInfo>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RoutingSummaryResponse {
    pub(crate) summary: RoutingSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct RoutingBucketsResponse {
    pub(crate) buckets: Vec<RoutingBucketSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RoutingNodesResponse {
    pub(crate) bucket: usize,
    pub(crate) nodes: Vec<RoutingNodeSummary>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RoutingNodesQuery {
    pub(crate) bucket: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DebugLookupReq {
    #[serde(default)]
    pub(crate) target_id_hex: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DebugLookupResponse {
    pub(crate) target_id_hex: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DebugProbeReq {
    pub(crate) udp_dest_b64: String,
    pub(crate) keyword_id_hex: String,
    pub(crate) file_id_hex: String,
    pub(crate) filename: String,
    pub(crate) file_size: u64,
    #[serde(default)]
    pub(crate) file_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DebugProbeResponse {
    pub(crate) queued: bool,
    pub(crate) peer_found: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadKeywordHitJson {
    pub(crate) file_id_hex: String,
    pub(crate) filename: String,
    pub(crate) file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) publish_info: Option<u32>,
    pub(crate) origin: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct KadSearchKeywordResponse {
    pub(crate) queued: bool,
    pub(crate) keyword: String,
    pub(crate) keyword_id_hex: String,
}

pub(crate) async fn kad_sources(
    State(state): State<ApiState>,
    axum::extract::Path(file_id_hex): axum::extract::Path<String>,
) -> Result<Json<KadSourcesResponse>, StatusCode> {
    let file = KadId::from_hex(&file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadSourceEntry>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetSources {
            file,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let sources = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let sources = sources
        .into_iter()
        .map(|s| KadSourceJson {
            source_id_hex: s.source_id.to_hex_lower(),
            udp_dest_b64: crate::i2p::b64::encode(&s.udp_dest),
        })
        .collect::<Vec<_>>();

    Ok(Json(KadSourcesResponse {
        file_id_hex: file.to_hex_lower(),
        sources,
    }))
}

pub(crate) async fn kad_keyword_results(
    State(state): State<ApiState>,
    axum::extract::Path(keyword_id_hex): axum::extract::Path<String>,
) -> Result<Json<KadKeywordResultsResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&keyword_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordHit>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordResults {
            keyword: keyword_id,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = hits
        .into_iter()
        .map(|h| KadKeywordHitJson {
            file_id_hex: h.file_id.to_hex_lower(),
            filename: h.filename,
            file_size: h.file_size,
            file_type: h.file_type,
            publish_info: h.publish_info,
            origin: h.origin.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Json(KadKeywordResultsResponse {
        keyword_id_hex: keyword_id.to_hex_lower(),
        hits,
    }))
}

pub(crate) async fn searches(
    State(state): State<ApiState>,
) -> Result<Json<SearchListResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordSearchInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordSearches { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let searches = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchListResponse { searches }))
}

pub(crate) async fn search_details(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
) -> Result<Json<SearchDetailsResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (searches_tx, searches_rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordSearchInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordSearches {
            respond_to: searches_tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let searches = tokio::time::timeout(std::time::Duration::from_secs(2), searches_rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let search = searches
        .into_iter()
        .find(|s| s.search_id_hex == keyword_id.to_hex_lower())
        .ok_or(StatusCode::NOT_FOUND)?;

    let (hits_tx, hits_rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordHit>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordResults {
            keyword: keyword_id,
            respond_to: hits_tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = tokio::time::timeout(std::time::Duration::from_secs(2), hits_rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = hits
        .into_iter()
        .map(|h| KadKeywordHitJson {
            file_id_hex: h.file_id.to_hex_lower(),
            filename: h.filename,
            file_size: h.file_size,
            file_type: h.file_type,
            publish_info: h.publish_info,
            origin: h.origin.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Json(SearchDetailsResponse { search, hits }))
}

pub(crate) async fn search_stop(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
) -> Result<Json<SearchStopResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::StopKeywordSearch {
            keyword: keyword_id,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let stopped = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchStopResponse { stopped }))
}

pub(crate) async fn search_delete(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
    Query(q): Query<SearchDeleteQuery>,
) -> Result<Json<SearchDeleteResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::DeleteKeywordSearch {
            keyword: keyword_id,
            purge_results: q.purge_results,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let deleted = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchDeleteResponse {
        deleted,
        purged_results: q.purge_results,
    }))
}

pub(crate) async fn kad_peers(
    State(state): State<ApiState>,
) -> Result<Json<KadPeersResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadPeerInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetPeers { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let peers = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadPeersResponse { peers }))
}

pub(crate) async fn debug_routing_summary(
    State(state): State<ApiState>,
) -> Result<Json<RoutingSummaryResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<RoutingSummary>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingSummary { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let summary = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingSummaryResponse { summary }))
}

pub(crate) async fn debug_routing_buckets(
    State(state): State<ApiState>,
) -> Result<Json<RoutingBucketsResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoutingBucketSummary>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingBuckets { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let buckets = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingBucketsResponse { buckets }))
}

pub(crate) async fn debug_routing_nodes(
    State(state): State<ApiState>,
    Query(query): Query<RoutingNodesQuery>,
) -> Result<Json<RoutingNodesResponse>, StatusCode> {
    if query.bucket >= 128 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoutingNodeSummary>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingNodes {
            bucket: query.bucket,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let nodes = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingNodesResponse {
        bucket: query.bucket,
        nodes,
    }))
}

pub(crate) async fn debug_lookup_once(
    State(state): State<ApiState>,
    Json(req): Json<DebugLookupReq>,
) -> Result<Json<DebugLookupResponse>, StatusCode> {
    let target = match req.target_id_hex.as_deref() {
        Some(hex) => Some(KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?),
        None => None,
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<KadId>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::StartDebugLookup {
            target,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let target_id = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(DebugLookupResponse {
        target_id_hex: target_id.to_hex_lower(),
    }))
}

pub(crate) async fn debug_probe_peer(
    State(state): State<ApiState>,
    Json(req): Json<DebugProbeReq>,
) -> Result<Json<DebugProbeResponse>, StatusCode> {
    if req.filename.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let keyword = KadId::from_hex(&req.keyword_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::DebugProbePeer {
            dest_b64: req.udp_dest_b64,
            keyword,
            file,
            filename: req.filename,
            file_size: req.file_size,
            file_type: req.file_type,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let peer_found = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(DebugProbeResponse {
        queued: peer_found,
        peer_found,
    }))
}

pub(crate) async fn kad_search_sources(
    State(state): State<ApiState>,
    Json(req): Json<KadSourcesReq>,
) -> Result<Json<QueuedResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_size = req.file_size.unwrap_or(0);
    state
        .kad_cmd_tx
        .send(KadServiceCommand::SearchSources { file, file_size })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(QueuedResponse { queued: true }))
}

pub(crate) async fn kad_search_keyword(
    State(state): State<ApiState>,
    Json(req): Json<KadSearchKeywordReq>,
) -> Result<Json<KadSearchKeywordResponse>, StatusCode> {
    let (word, keyword_id) = if let Some(hex) = req.keyword_id_hex.as_deref() {
        let id = KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?;
        ("".to_string(), id)
    } else if let Some(q) = req.query.as_deref() {
        keyword::query_to_keyword_id(q).map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    state
        .kad_cmd_tx
        .send(KadServiceCommand::SearchKeyword {
            keyword: keyword_id,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadSearchKeywordResponse {
        queued: true,
        keyword: word,
        keyword_id_hex: keyword_id.to_hex_lower(),
    }))
}

pub(crate) async fn kad_publish_source(
    State(state): State<ApiState>,
    Json(req): Json<KadSourcesReq>,
) -> Result<Json<QueuedResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_size = req.file_size.unwrap_or(0);
    state
        .kad_cmd_tx
        .send(KadServiceCommand::PublishSource { file, file_size })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(QueuedResponse { queued: true }))
}

pub(crate) async fn kad_publish_keyword(
    State(state): State<ApiState>,
    Json(req): Json<KadPublishKeywordReq>,
) -> Result<Json<KadSearchKeywordResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (word, keyword_id) = if let Some(hex) = req.keyword_id_hex.as_deref() {
        let id = KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?;
        ("".to_string(), id)
    } else if let Some(q) = req.query.as_deref() {
        keyword::query_to_keyword_id(q).map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    state
        .kad_cmd_tx
        .send(KadServiceCommand::PublishKeyword {
            keyword: keyword_id,
            file,
            filename: req.filename,
            file_size: req.file_size,
            file_type: req.file_type,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadSearchKeywordResponse {
        queued: true,
        keyword: word,
        keyword_id_hex: keyword_id.to_hex_lower(),
    }))
}
