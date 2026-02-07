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
        KadId,
        service::{KadServiceCommand, KadServiceStatus, KadSourceEntry},
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
        .route("/kad/sources/:file_id_hex", get(kad_sources))
        .route("/kad/search_sources", post(kad_search_sources))
        .route("/kad/publish_source", post(kad_publish_source))
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

    let initial_stream = futures_util::stream::iter(
        initial
            .into_iter()
            .map(|st| Ok::<KadServiceStatus, Infallible>(st)),
    );
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
