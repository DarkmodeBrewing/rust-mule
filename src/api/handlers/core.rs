use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
};
use futures_util::Stream;
use serde::Serialize;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    api::{
        ApiState, SESSION_TTL,
        auth::{
            build_session_cookie, cleanup_expired_sessions, clear_session_cookie,
            generate_session_id, is_loopback_addr, session_cookie,
        },
    },
    kad::service::KadServiceStatus,
};

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) ok: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthBootstrapResponse {
    pub(crate) token: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenRotateResponse {
    pub(crate) token: String,
    pub(crate) sessions_cleared: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SessionResponse {
    pub(crate) ok: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct StatusResponse {
    #[serde(flatten)]
    pub(crate) status: KadServiceStatus,
    /// Aggregate download transfer rate over the last 5 seconds, in bytes per second.
    pub(crate) download_rate_bps_5s: u64,
    /// Aggregate download transfer rate over the last 30 seconds, in bytes per second.
    pub(crate) download_rate_bps_30s: u64,
    /// Aggregate upload transfer rate over the last 5 seconds, in bytes per second.
    pub(crate) upload_rate_bps_5s: u64,
    /// Aggregate upload transfer rate over the last 30 seconds, in bytes per second.
    pub(crate) upload_rate_bps_30s: u64,
    /// Aggregate zero-fill fallback upload rate over the last 5 seconds, in bytes per second.
    pub(crate) zero_fill_upload_rate_bps_5s: u64,
    /// Aggregate zero-fill fallback upload rate over the last 30 seconds, in bytes per second.
    pub(crate) zero_fill_upload_rate_bps_30s: u64,
    pub(crate) zero_fill_active_uploads: usize,
    pub(crate) zero_fill_warning: bool,
}

fn saturating_rate_sum<I>(rates: I) -> u64
where
    I: IntoIterator<Item = u64>,
{
    rates
        .into_iter()
        .fold(0u64, |acc, rate| acc.saturating_add(rate))
}

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

pub(crate) async fn auth_bootstrap(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<AuthBootstrapResponse>, StatusCode> {
    if !is_loopback_addr(&addr) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(AuthBootstrapResponse {
        token: state.token.read().await.clone(),
    }))
}

pub(crate) async fn token_rotate(
    State(state): State<ApiState>,
) -> Result<Json<TokenRotateResponse>, StatusCode> {
    let new_token = crate::api::token::rotate_token(state.token_path.as_path())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    {
        let mut token = state.token.write().await;
        *token = new_token.clone();
    }
    {
        let mut sessions = state.sessions.lock().await;
        sessions.clear();
    }
    Ok(Json(TokenRotateResponse {
        token: new_token,
        sessions_cleared: true,
    }))
}

pub(crate) async fn create_session(
    State(state): State<ApiState>,
) -> Result<impl IntoResponse, StatusCode> {
    let session_id = generate_session_id()?;
    let cookie = build_session_cookie(&session_id, SESSION_TTL);
    {
        let mut sessions = state.sessions.lock().await;
        cleanup_expired_sessions(&mut sessions, Instant::now());
        sessions.insert(session_id, Instant::now() + SESSION_TTL);
    }

    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(SessionResponse { ok: true }),
    ))
}

pub(crate) async fn session_check(State(_state): State<ApiState>) -> Json<SessionResponse> {
    Json(SessionResponse { ok: true })
}

pub(crate) async fn session_logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if let Some(session_id) = session_cookie(&headers) {
        let mut sessions = state.sessions.lock().await;
        sessions.remove(&session_id);
    }
    Ok((
        [(header::SET_COOKIE, clear_session_cookie())],
        Json(SessionResponse { ok: true }),
    ))
}

pub(crate) async fn status(
    State(state): State<ApiState>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let Some(s) = (*state.status_rx.borrow()).clone() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    let downloads = match state.download_handle.snapshot().await {
        Ok((_download_status, downloads)) => downloads,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "failed to snapshot downloads for status; reporting zero aggregate download rates"
            );
            Vec::new()
        }
    };
    let uploads = state.upload_service.snapshot_all();
    Ok(Json(StatusResponse {
        status: s,
        download_rate_bps_5s: saturating_rate_sum(downloads.iter().map(|d| d.rate_bps_5s)),
        download_rate_bps_30s: saturating_rate_sum(downloads.iter().map(|d| d.rate_bps_30s)),
        upload_rate_bps_5s: saturating_rate_sum(uploads.iter().map(|u| u.rate_bps_5s)),
        upload_rate_bps_30s: saturating_rate_sum(uploads.iter().map(|u| u.rate_bps_30s)),
        zero_fill_upload_rate_bps_5s: saturating_rate_sum(
            uploads.iter().map(|u| u.zero_fill_rate_bps_5s),
        ),
        zero_fill_upload_rate_bps_30s: saturating_rate_sum(
            uploads.iter().map(|u| u.zero_fill_rate_bps_30s),
        ),
        zero_fill_active_uploads: uploads.iter().filter(|u| u.zero_fill_active).count(),
        zero_fill_warning: uploads
            .iter()
            .any(|u| u.zero_fill_active || u.zero_fill_rate_bps_5s > 0),
    }))
}

pub(crate) async fn events(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.status_events_tx.subscribe();
    let initial = (*state.status_rx.borrow()).clone();
    let stream = status_sse_stream(
        initial,
        BroadcastStream::new(rx),
        state.sse_serialize_fallback_total,
    );
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

fn status_sse_stream(
    initial: Option<KadServiceStatus>,
    stream: BroadcastStream<KadServiceStatus>,
    fallback_counter: Arc<AtomicU64>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    use futures_util::StreamExt as _;

    let initial_stream =
        futures_util::stream::iter(initial.into_iter().map(Ok::<KadServiceStatus, Infallible>));
    let merged = initial_stream.chain(stream.filter_map(|msg| async move { msg.ok().map(Ok) }));

    merged.map(move |Ok(status)| {
        let json = match serde_json::to_string(&status) {
            Ok(json) => json,
            Err(err) => {
                let total = fallback_counter.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(
                    error = %err,
                    fallback_total = total,
                    "failed to serialize SSE status payload; emitting fallback"
                );
                "{}".to_string()
            }
        };
        Ok(Event::default().event("status").data(json))
    })
}
