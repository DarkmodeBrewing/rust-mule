use axum::{
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware,
};

pub(crate) async fn cors_mw(
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let origin = req.headers().get(axum::http::header::ORIGIN).cloned();

    if let Some(origin) = origin {
        if !is_allowed_origin(&origin) {
            return Err(StatusCode::FORBIDDEN);
        }

        if req.method() == Method::OPTIONS {
            let mut resp = axum::response::Response::new(axum::body::Body::empty());
            *resp.status_mut() = StatusCode::NO_CONTENT;
            apply_cors_headers(resp.headers_mut(), &origin);
            return Ok(resp);
        }

        let mut resp = next.run(req).await;
        apply_cors_headers(resp.headers_mut(), &origin);
        return Ok(resp);
    }

    Ok(next.run(req).await)
}

pub(crate) fn apply_cors_headers(headers: &mut HeaderMap, origin: &HeaderValue) {
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        origin.clone(),
    );
    headers.insert(axum::http::header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, PATCH, OPTIONS"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type"),
    );
}

pub(crate) fn is_allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };

    let Some(host) = uri.host() else {
        return false;
    };

    let host = host.trim_start_matches('[').trim_end_matches(']');

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}
