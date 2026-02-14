use axum::{
    middleware,
    response::{IntoResponse, Redirect},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use axum::http::{HeaderMap, Method, StatusCode, header};

use crate::api::ApiState;

pub(crate) async fn auth_mw(
    axum::extract::State(state): axum::extract::State<ApiState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.method() == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let path = req.uri().path();
    if is_frontend_exempt_path(path) {
        return Ok(next.run(req).await);
    }

    if path == "/api/v1/events" {
        if has_valid_session(req.headers(), &state).await {
            return Ok(next.run(req).await);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    if path == "/api/v1/session/check" || path == "/api/v1/session/logout" {
        if has_valid_session(req.headers(), &state).await {
            return Ok(next.run(req).await);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    if path.starts_with("/api/") {
        if is_api_bearer_exempt_path(path) {
            return Ok(next.run(req).await);
        }
        let provided = bearer_token(req.headers()).ok_or(StatusCode::UNAUTHORIZED)?;
        let current_token = state.token.read().await.clone();
        if provided != current_token {
            return Err(StatusCode::FORBIDDEN);
        }
        return Ok(next.run(req).await);
    }

    if has_valid_session(req.headers(), &state).await {
        return Ok(next.run(req).await);
    }

    Ok(Redirect::to("/auth").into_response())
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let auth = auth.strip_prefix("Bearer ")?;
    Some(auth.to_string())
}

pub(crate) fn is_loopback_addr(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

pub(crate) fn is_api_bearer_exempt_path(path: &str) -> bool {
    matches!(path, "/api/v1/health" | "/api/v1/dev/auth")
}

pub(crate) fn is_frontend_exempt_path(path: &str) -> bool {
    path == "/auth"
}

pub(crate) fn session_cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let token = part.trim();
        if let Some((k, v)) = token.split_once('=')
            && k == "rm_session"
            && !v.is_empty()
        {
            return Some(v.to_string());
        }
    }
    None
}

pub(crate) async fn has_valid_session(headers: &HeaderMap, state: &ApiState) -> bool {
    let Some(session_id) = session_cookie(headers) else {
        return false;
    };
    let mut sessions = state.sessions.lock().await;
    let now = Instant::now();
    cleanup_expired_sessions(&mut sessions, now);
    match sessions.get(&session_id) {
        Some(expiry) if *expiry > now => true,
        Some(_) => {
            sessions.remove(&session_id);
            false
        }
        None => false,
    }
}

pub(crate) fn cleanup_expired_sessions(sessions: &mut HashMap<String, Instant>, now: Instant) {
    sessions.retain(|_, expiry| *expiry > now);
}

pub(crate) fn build_session_cookie(session_id: &str, ttl: Duration) -> String {
    format!(
        "rm_session={session_id}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        ttl.as_secs()
    )
}

pub(crate) fn clear_session_cookie() -> String {
    "rm_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0".to_string()
}

pub(crate) fn generate_session_id() -> Result<String, StatusCode> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
