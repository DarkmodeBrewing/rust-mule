use axum::{
    extract::{OriginalUri, Path},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect},
};
use include_dir::{Dir, include_dir};

pub(crate) static UI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/ui");

pub(crate) async fn ui_auth() -> impl IntoResponse {
    const AUTH_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>rust-mule::auth</title>
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <main>
      <h1>rust-mule UI authentication</h1>
      <p id="status">Establishing local session...</p>
    </main>
    <script>
      (async () => {
        const status = document.getElementById('status');
        try {
          const authResp = await fetch('/api/v1/auth/bootstrap');
          if (!authResp.ok) throw new Error('token bootstrap failed');
          const authData = await authResp.json();
          const sessResp = await fetch('/api/v1/session', {
            method: 'POST',
            headers: { 'Authorization': `Bearer ${authData.token}` }
          });
          if (!sessResp.ok) throw new Error('session create failed');
          window.location.replace('/index.html');
        } catch (err) {
          status.textContent = `Auth failed: ${String(err)}`;
        }
      })();
    </script>
  </body>
</html>"#;

    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        AUTH_HTML,
    )
}

pub(crate) async fn ui_index() -> Result<axum::response::Response, StatusCode> {
    serve_ui_file("index.html").await
}

pub(crate) async fn root_index_redirect() -> Redirect {
    Redirect::to("/index.html")
}

pub(crate) async fn ui_page(
    Path(page): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    if page.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut file = page;
    if !file.ends_with(".html") {
        file.push_str(".html");
    }
    if !is_safe_ui_segment(&file) {
        return Err(StatusCode::BAD_REQUEST);
    }

    serve_ui_file(&file).await
}

pub(crate) async fn ui_asset(
    Path(path): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    if !is_safe_ui_path(&path) {
        return Err(StatusCode::BAD_REQUEST);
    }
    serve_embedded_ui_file(&format!("assets/{path}"))
}

pub(crate) async fn ui_fallback(OriginalUri(uri): OriginalUri) -> Result<Redirect, StatusCode> {
    if let Some(location) = spa_fallback_location(&uri) {
        return Ok(Redirect::to(location));
    }
    Err(StatusCode::NOT_FOUND)
}

pub(crate) fn is_safe_ui_segment(name: &str) -> bool {
    !name.contains('/') && !name.contains('\\') && !name.contains("..")
}

pub(crate) fn is_safe_ui_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains("..") {
        return false;
    }
    path.split('/').all(|c| !c.is_empty() && c != ".")
}

pub(crate) fn spa_fallback_location(uri: &axum::http::Uri) -> Option<&'static str> {
    let path = uri.path();
    if path.starts_with("/api/") || path.starts_with("/ui/assets/") {
        return None;
    }
    Some("/index.html")
}

async fn serve_ui_file(name: &str) -> Result<axum::response::Response, StatusCode> {
    serve_embedded_ui_file(name)
}

fn serve_embedded_ui_file(rel_path: &str) -> Result<axum::response::Response, StatusCode> {
    let file = UI_DIR.get_file(rel_path).ok_or(StatusCode::NOT_FOUND)?;
    let content_type = content_type_for_path(std::path::Path::new(rel_path));
    let resp = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(file.contents().to_vec()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(resp)
}

fn content_type_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
