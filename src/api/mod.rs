use axum::{
    Json, Router,
    extract::State,
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

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/events", get(events))
        .route("/kad/peers", get(kad_peers))
        .route("/kad/sources/:file_id_hex", get(kad_sources))
        .route(
            "/kad/keyword_results/:keyword_id_hex",
            get(kad_keyword_results),
        )
        .route("/kad/search_sources", post(kad_search_sources))
        .route("/kad/search_keyword", post(kad_search_keyword))
        .route("/kad/publish_source", post(kad_publish_source))
        .route("/kad/publish_keyword", post(kad_publish_keyword))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_mw));

    tracing::info!(addr = %addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
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
    if req.uri().path() == "/health" {
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
