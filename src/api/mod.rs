use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware,
    response::sse::{Event, Sse},
    routing::get,
};
use futures_util::Stream;
use serde::Serialize;
use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;

use crate::{config::ApiConfig, kad::service::KadServiceStatus};

pub mod token;

#[derive(Clone)]
pub struct ApiState {
    token: String,
    status_rx: watch::Receiver<Option<KadServiceStatus>>,
    status_events_tx: broadcast::Sender<KadServiceStatus>,
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
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/events", get(events))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_mw));

    tracing::info!(addr = %addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
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
