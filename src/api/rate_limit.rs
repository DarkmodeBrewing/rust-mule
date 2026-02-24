use axum::{
    extract::ConnectInfo,
    http::{Method, StatusCode},
    middleware,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};

use crate::api::ApiState;

#[derive(Debug, Clone)]
pub(crate) struct RateLimitBucket {
    pub(crate) window_start: Instant,
    pub(crate) count: u32,
}

pub(crate) async fn rate_limit_mw(
    axum::extract::State(state): axum::extract::State<ApiState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.method() == Method::OPTIONS || !state.rate_limit_enabled {
        return Ok(next.run(req).await);
    }

    let Some(limit) = rate_limit_for_path(req.uri().path(), req.method(), &state) else {
        return Ok(next.run(req).await);
    };

    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let now = Instant::now();
    let key = format!("{}|{}|{}", ip, req.method(), req.uri().path());

    let mut buckets = state.rate_limits.lock().await;
    let stale_after = state.rate_limit_window.saturating_mul(2);
    buckets.retain(|_, bucket| now.duration_since(bucket.window_start) <= stale_after);
    let bucket = buckets.entry(key).or_insert(RateLimitBucket {
        window_start: now,
        count: 0,
    });

    if now.duration_since(bucket.window_start) >= state.rate_limit_window {
        bucket.window_start = now;
        bucket.count = 0;
    }

    if bucket.count >= limit {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    bucket.count = bucket.count.saturating_add(1);

    Ok(next.run(req).await)
}

fn rate_limit_for_path(path: &str, method: &Method, state: &ApiState) -> Option<u32> {
    let read_limit = state.rate_limit_session_max.saturating_mul(6).max(1);
    let query_limit = state.rate_limit_session_max.saturating_mul(4).max(1);
    let mutate_limit = state.rate_limit_session_max.max(1);

    match (method, path) {
        (&Method::GET, "/api/v1/auth/bootstrap") => Some(state.rate_limit_auth_bootstrap_max),
        (&Method::POST, "/api/v1/session") => Some(state.rate_limit_session_max),
        (&Method::POST, "/api/v1/token/rotate") => Some(state.rate_limit_token_rotate_max),
        (&Method::GET, "/api/v1/status") => Some(read_limit),
        (&Method::GET, "/api/v1/events") => Some(read_limit),
        (&Method::GET, "/api/v1/settings") => Some(query_limit),
        (&Method::PATCH, "/api/v1/settings") => Some(mutate_limit),
        (&Method::GET, "/api/v1/downloads") => Some(query_limit),
        (&Method::POST, "/api/v1/downloads") => Some(mutate_limit),
        _ if path.starts_with("/api/v1/downloads/") => Some(mutate_limit),
        (&Method::GET, "/api/v1/searches") => Some(query_limit),
        _ if path.starts_with("/api/v1/searches/") => Some(mutate_limit),
        (&Method::GET, "/api/v1/kad/peers") => Some(query_limit),
        _ if path.starts_with("/api/v1/kad/") && *method == Method::GET => Some(query_limit),
        _ if path.starts_with("/api/v1/kad/") && *method == Method::POST => Some(mutate_limit),
        _ => None,
    }
}
