use axum::{
    Json,
    body::Body,
    body::Bytes,
    extract::Request,
    http::StatusCode,
    http::header::CONTENT_TYPE,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ApiErrorEnvelope {
    pub(crate) code: u16,
    pub(crate) message: String,
}

pub(crate) fn message_for_status(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "bad request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::PAYLOAD_TOO_LARGE => "request body too large",
        StatusCode::TOO_MANY_REQUESTS => "too many requests",
        StatusCode::SERVICE_UNAVAILABLE => "service unavailable",
        StatusCode::GATEWAY_TIMEOUT => "gateway timeout",
        StatusCode::INTERNAL_SERVER_ERROR => "internal server error",
        _ => "request failed",
    }
}

pub(crate) fn status_with_message(status: StatusCode) -> (StatusCode, Json<ApiErrorEnvelope>) {
    (
        status,
        Json(ApiErrorEnvelope {
            code: status.as_u16(),
            message: message_for_status(status).to_string(),
        }),
    )
}

pub(crate) fn parse_json_with_limit<T: serde::de::DeserializeOwned>(
    bytes: Bytes,
    max_bytes: usize,
) -> Result<T, StatusCode> {
    if bytes.len() > max_bytes {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    serde_json::from_slice::<T>(&bytes).map_err(|_| StatusCode::BAD_REQUEST)
}

pub(crate) async fn error_envelope_mw(req: Request<Body>, next: Next) -> Response {
    let is_api_v1 = req.uri().path().starts_with("/api/v1/");
    let resp = next.run(req).await;
    if !is_api_v1 {
        return resp;
    }
    if resp.status().is_success() {
        return resp;
    }
    let has_json_body = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|ct| ct.to_ascii_lowercase().starts_with("application/json"))
        .unwrap_or(false);
    if has_json_body {
        return resp;
    }

    let status = resp.status();
    let envelope_resp = status_with_message(status).into_response();
    let (mut orig_parts, _orig_body) = resp.into_parts();
    let (envelope_parts, envelope_body) = envelope_resp.into_parts();
    orig_parts.headers.extend(envelope_parts.headers);
    Response::from_parts(orig_parts, envelope_body)
}
