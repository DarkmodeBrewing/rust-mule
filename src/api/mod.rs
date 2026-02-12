use axum::{
    Json, Router,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::sse::{Event, Sse},
    routing::{get, post},
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    config::ApiConfig,
    kad::{
        KadId, keyword,
        service::{
            KadKeywordHit, KadPeerInfo, KadServiceCommand, KadServiceStatus, KadSourceEntry,
            RoutingBucketSummary, RoutingNodeSummary, RoutingSummary,
        },
    },
};

pub mod token;

#[derive(Clone)]
pub struct ApiState {
    token: String,
    status_rx: watch::Receiver<Option<KadServiceStatus>>,
    status_events_tx: broadcast::Sender<KadServiceStatus>,
    kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
}

pub fn new_channels() -> (
    watch::Sender<Option<KadServiceStatus>>,
    broadcast::Sender<KadServiceStatus>,
) {
    let (status_tx, _status_rx) = watch::channel(None);
    let (status_events_tx, _status_events_rx) = broadcast::channel(1024);
    (status_tx, status_events_tx)
}

pub async fn serve(
    cfg: &ApiConfig,
    token: String,
    status_rx: watch::Receiver<Option<KadServiceStatus>>,
    status_events_tx: broadcast::Sender<KadServiceStatus>,
    kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
) -> anyhow::Result<()> {
    let bind_ip: std::net::IpAddr = cfg
        .host
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid api.host '{}': {e}", cfg.host))?;
    let addr = SocketAddr::new(bind_ip, cfg.port);

    let state = ApiState {
        token,
        status_rx,
        status_events_tx,
        kad_cmd_tx,
    };

    // Canonical API surface.
    let v1 = Router::new()
        .route("/health", get(health))
        .route("/dev/auth", get(dev_auth))
        .route("/status", get(status))
        .route("/events", get(events))
        .route("/kad/peers", get(kad_peers))
        .route("/debug/routing/summary", get(debug_routing_summary))
        .route("/debug/routing/buckets", get(debug_routing_buckets))
        .route("/debug/routing/nodes", get(debug_routing_nodes))
        .route("/debug/lookup_once", post(debug_lookup_once))
        .route("/debug/probe_peer", post(debug_probe_peer))
        .route("/kad/sources/:file_id_hex", get(kad_sources))
        .route(
            "/kad/keyword_results/:keyword_id_hex",
            get(kad_keyword_results),
        )
        .route("/kad/search_sources", post(kad_search_sources))
        .route("/kad/search_keyword", post(kad_search_keyword))
        .route("/kad/publish_source", post(kad_publish_source))
        .route("/kad/publish_keyword", post(kad_publish_keyword));

    let app = Router::new()
        .nest("/api/v1", v1)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_mw));

    tracing::info!(addr = %addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct KadSourcesReq {
    file_id_hex: String,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KadSearchKeywordReq {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    keyword_id_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KadPublishKeywordReq {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    keyword_id_hex: Option<String>,
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(default)]
    file_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct QueuedResponse {
    queued: bool,
}

#[derive(Debug, Serialize)]
struct KadSourcesResponse {
    file_id_hex: String,
    sources: Vec<KadSourceJson>,
}

#[derive(Debug, Serialize)]
struct KadSourceJson {
    source_id_hex: String,
    udp_dest_b64: String,
}

#[derive(Debug, Serialize)]
struct KadKeywordResultsResponse {
    keyword_id_hex: String,
    hits: Vec<KadKeywordHitJson>,
}

#[derive(Debug, Serialize)]
struct KadPeersResponse {
    peers: Vec<KadPeerInfo>,
}

#[derive(Debug, Serialize)]
struct RoutingSummaryResponse {
    summary: RoutingSummary,
}

#[derive(Debug, Serialize)]
struct RoutingBucketsResponse {
    buckets: Vec<RoutingBucketSummary>,
}

#[derive(Debug, Serialize)]
struct RoutingNodesResponse {
    bucket: usize,
    nodes: Vec<RoutingNodeSummary>,
}

#[derive(Debug, Deserialize)]
struct RoutingNodesQuery {
    bucket: usize,
}

#[derive(Debug, Deserialize)]
struct DebugLookupReq {
    #[serde(default)]
    target_id_hex: Option<String>,
}

#[derive(Debug, Serialize)]
struct DebugLookupResponse {
    target_id_hex: String,
}

#[derive(Debug, Deserialize)]
struct DebugProbeReq {
    udp_dest_b64: String,
    keyword_id_hex: String,
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(default)]
    file_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct DebugProbeResponse {
    queued: bool,
    peer_found: bool,
}

#[derive(Debug, Serialize)]
struct KadKeywordHitJson {
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publish_info: Option<u32>,
    origin: String,
}

async fn kad_sources(
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

async fn kad_keyword_results(
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

async fn kad_peers(State(state): State<ApiState>) -> Result<Json<KadPeersResponse>, StatusCode> {
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

async fn debug_routing_summary(
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

async fn debug_routing_buckets(
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

async fn debug_routing_nodes(
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

async fn debug_lookup_once(
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

async fn debug_probe_peer(
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

async fn kad_search_sources(
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

#[derive(Debug, Serialize)]
struct KadSearchKeywordResponse {
    queued: bool,
    keyword: String,
    keyword_id_hex: String,
}

async fn kad_search_keyword(
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

async fn kad_publish_source(
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

async fn kad_publish_keyword(
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct DevAuthResponse {
    token: String,
}

async fn dev_auth(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<DevAuthResponse>, StatusCode> {
    if !is_loopback_addr(&addr) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(DevAuthResponse {
        token: state.token.clone(),
    }))
}

async fn status(State(state): State<ApiState>) -> Result<Json<KadServiceStatus>, StatusCode> {
    let Some(s) = (*state.status_rx.borrow()).clone() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    Ok(Json(s))
}

async fn events(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.status_events_tx.subscribe();
    let initial = (*state.status_rx.borrow()).clone();
    let stream = status_sse_stream(initial, BroadcastStream::new(rx));
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

fn status_sse_stream(
    initial: Option<KadServiceStatus>,
    stream: BroadcastStream<KadServiceStatus>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    use futures_util::StreamExt as _;

    let initial_stream =
        futures_util::stream::iter(initial.into_iter().map(Ok::<KadServiceStatus, Infallible>));
    let merged = initial_stream.chain(stream.filter_map(|msg| async move { msg.ok().map(Ok) }));

    merged.map(|Ok(status)| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
        Ok(Event::default().event("status").data(json))
    })
}

async fn auth_mw(
    State(state): State<ApiState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Allow health checks without auth (useful for local dev).
    if is_auth_exempt_path(req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let provided = bearer_token(req.headers()).ok_or(StatusCode::UNAUTHORIZED)?;
    if provided != state.token {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let auth = auth.strip_prefix("Bearer ")?;
    Some(auth.to_string())
}

fn is_loopback_addr(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

fn is_auth_exempt_path(path: &str) -> bool {
    matches!(path, "/api/v1/health" | "/api/v1/dev/auth")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn detects_loopback_addresses() {
        let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234);
        let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1234);
        let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234);

        assert!(is_loopback_addr(&v4));
        assert!(is_loopback_addr(&v6));
        assert!(!is_loopback_addr(&other));
    }

    #[test]
    fn auth_exempt_paths_include_only_v1_health_and_dev_auth() {
        assert!(is_auth_exempt_path("/api/v1/health"));
        assert!(is_auth_exempt_path("/api/v1/dev/auth"));
        assert!(!is_auth_exempt_path("/health"));
        assert!(!is_auth_exempt_path("/dev/auth"));
        assert!(!is_auth_exempt_path("/api/v1/status"));
    }
}
