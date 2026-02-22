use super::{ApiState, auth, cors, handlers, router, ui};
use crate::{
    config::{ApiAuthMode, Config},
    download::DownloadServiceHandle,
    kad::{
        KadId,
        service::{
            KadKeywordHit, KadKeywordHitOrigin, KadKeywordSearchInfo, KadPeerInfo,
            KadServiceCommand, KadServiceStatus,
        },
    },
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc, watch};
use tower::util::ServiceExt as _;

fn test_state(kad_cmd_tx: mpsc::Sender<KadServiceCommand>) -> ApiState {
    let (_status_tx, status_rx) = watch::channel(None);
    let (status_events_tx, _status_events_rx) = broadcast::channel(16);
    ApiState {
        token: Arc::new(tokio::sync::RwLock::new("test-token".to_string())),
        token_path: Arc::new(PathBuf::from("data/api.token")),
        config_path: Arc::new(unique_test_config_path()),
        status_rx,
        status_events_tx,
        kad_cmd_tx,
        download_handle: DownloadServiceHandle::test_handle(),
        config: Arc::new(tokio::sync::Mutex::new(Config::default())),
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        enable_debug_endpoints: true,
        auth_mode: ApiAuthMode::LocalUi,
        rate_limit_enabled: true,
        rate_limit_window: Duration::from_secs(60),
        rate_limit_auth_bootstrap_max: 30,
        rate_limit_session_max: 30,
        rate_limit_token_rotate_max: 10,
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    }
}

fn unique_test_config_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "rust_mule_api_config_test_{}_{}.toml",
        std::process::id(),
        id
    ))
}

fn test_app(state: ApiState) -> Router<()> {
    router::build_app(state.clone())
        .layer(middleware::from_fn(cors::cors_mw))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_mw))
        .layer(middleware::from_fn_with_state(
            state,
            super::rate_limit::rate_limit_mw,
        ))
}

fn sample_status() -> KadServiceStatus {
    KadServiceStatus {
        uptime_secs: 120,
        routing: 10,
        live: 3,
        live_10m: 2,
        pending: 1,
        pending_overdue: 0,
        pending_max_overdue_ms: 0,
        tracked_out_requests: 0,
        recv_req: 1,
        recv_res: 1,
        sent_reqs: 1,
        recv_ress: 1,
        res_contacts: 0,
        dropped_undecipherable: 0,
        dropped_unparsable: 0,
        dropped_legacy_kad1: 0,
        dropped_unhandled_opcode: 0,
        recv_hello_reqs: 0,
        sent_bootstrap_reqs: 0,
        recv_bootstrap_ress: 0,
        bootstrap_contacts: 0,
        sent_hellos: 0,
        recv_hello_ress: 0,
        sent_hello_acks: 0,
        recv_hello_acks: 0,
        hello_ack_skipped_no_sender_key: 0,
        timeouts: 0,
        new_nodes: 0,
        evicted: 0,
        sent_search_source_reqs: 1,
        recv_search_source_reqs: 1,
        recv_search_source_decode_failures: 0,
        source_search_hits: 1,
        source_search_misses: 0,
        source_search_results_served: 1,
        recv_search_ress: 1,
        search_results: 1,
        new_sources: 1,
        sent_search_key_reqs: 0,
        recv_search_key_reqs: 0,
        keyword_results: 1,
        new_keyword_results: 1,
        evicted_keyword_hits: 0,
        evicted_keyword_keywords: 0,
        keyword_keywords_tracked: 1,
        keyword_hits_total: 1,
        store_keyword_keywords: 1,
        store_keyword_hits_total: 1,
        source_store_files: 1,
        source_store_entries_total: 1,
        peer_unknown: 1,
        peer_verified: 1,
        peer_stable: 1,
        peer_unreliable: 0,
        recv_publish_key_reqs: 0,
        recv_publish_key_decode_failures: 0,
        sent_publish_key_ress: 0,
        sent_publish_key_reqs: 0,
        recv_publish_key_ress: 0,
        new_store_keyword_hits: 0,
        evicted_store_keyword_hits: 0,
        evicted_store_keyword_keywords: 0,
        sent_publish_source_reqs: 1,
        recv_publish_source_reqs: 1,
        recv_publish_source_decode_failures: 0,
        sent_publish_source_ress: 1,
        new_store_source_entries: 1,
        recv_publish_ress: 1,
        source_search_batch_candidates: 8,
        source_search_batch_skipped_version: 1,
        source_search_batch_sent: 6,
        source_search_batch_send_fail: 1,
        source_publish_batch_candidates: 6,
        source_publish_batch_skipped_version: 1,
        source_publish_batch_sent: 4,
        source_publish_batch_send_fail: 1,
        source_probe_first_publish_responses: 1,
        source_probe_first_search_responses: 1,
        source_probe_search_results_total: 1,
        source_probe_publish_latency_ms_total: 10,
        source_probe_search_latency_ms_total: 20,
        tracked_out_matched: 0,
        tracked_out_unmatched: 0,
        tracked_out_expired: 0,
        outbound_shaper_delayed: 0,
        outbound_shaper_drop_global_cap: 0,
        outbound_shaper_drop_peer_cap: 0,
        recv_req_total: 11,
        recv_res_total: 12,
        sent_reqs_total: 11,
        recv_ress_total: 12,
        timeouts_total: 13,
        tracked_out_matched_total: 14,
        tracked_out_unmatched_total: 15,
        tracked_out_expired_total: 16,
        outbound_shaper_delayed_total: 17,
        dropped_legacy_kad1_total: 0,
        dropped_unhandled_opcode_total: 0,
        sam_framing_desync_total: 18,
    }
}

fn authorized_api_get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::GET)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap()
}

fn authorized_api_post(path: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::POST)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

fn authorized_api_delete(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::DELETE)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap()
}

async fn response_json(resp: axum::response::Response) -> Value {
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn detects_loopback_addresses() {
    let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234);
    let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1234);
    let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234);

    assert!(auth::is_loopback_addr(&v4));
    assert!(auth::is_loopback_addr(&v6));
    assert!(!auth::is_loopback_addr(&other));
}

#[test]
fn api_bearer_exempt_paths_include_only_health_and_dev_auth() {
    assert!(auth::is_api_bearer_exempt_path(
        "/api/v1/health",
        ApiAuthMode::LocalUi
    ));
    assert!(auth::is_api_bearer_exempt_path(
        "/api/v1/auth/bootstrap",
        ApiAuthMode::LocalUi
    ));
    assert!(!auth::is_api_bearer_exempt_path(
        "/api/v1/status",
        ApiAuthMode::LocalUi
    ));
    assert!(!auth::is_api_bearer_exempt_path(
        "/api/v1/session",
        ApiAuthMode::LocalUi
    ));
    assert!(!auth::is_api_bearer_exempt_path(
        "/api/v1/token/rotate",
        ApiAuthMode::LocalUi
    ));
    assert!(!auth::is_api_bearer_exempt_path(
        "/api/v1/auth/bootstrap",
        ApiAuthMode::HeadlessRemote
    ));
}

#[test]
fn frontend_exempt_paths_include_only_auth_page() {
    assert!(auth::is_frontend_exempt_path("/auth"));
    assert!(!auth::is_frontend_exempt_path("/"));
    assert!(!auth::is_frontend_exempt_path("/ui"));
}

#[test]
fn parses_session_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("a=1; rm_session=session123; x=y"),
    );
    assert_eq!(
        auth::session_cookie(&headers).as_deref(),
        Some("session123")
    );
}

#[test]
fn session_cookie_ttl_and_clear_cookie_headers_are_well_formed() {
    let cookie = auth::build_session_cookie("abc", Duration::from_secs(60));
    assert!(cookie.contains("rm_session=abc"));
    assert!(cookie.contains("Max-Age=60"));
    assert!(!cookie.contains("Secure"));
    assert!(auth::clear_session_cookie().contains("Max-Age=0"));
}

#[test]
fn cleanup_expired_sessions_removes_expired_entries() {
    let now = Instant::now();
    let mut sessions = HashMap::new();
    sessions.insert("alive".to_string(), now + Duration::from_secs(10));
    sessions.insert("dead".to_string(), now - Duration::from_secs(10));
    auth::cleanup_expired_sessions(&mut sessions, now);
    assert!(sessions.contains_key("alive"));
    assert!(!sessions.contains_key("dead"));
}

#[test]
fn validates_ui_paths() {
    assert!(ui::is_safe_ui_path("css/base.css"));
    assert!(!ui::is_safe_ui_path("../etc/passwd"));
    assert!(!ui::is_safe_ui_path("css\\base.css"));
    assert!(ui::is_safe_ui_segment("index.html"));
    assert!(!ui::is_safe_ui_segment("../index.html"));
}

#[test]
fn spa_fallback_redirects_unknown_non_api_paths_to_root() {
    let uri: axum::http::Uri = "/nonexisting.php?queryParams=whatever".parse().unwrap();
    assert_eq!(ui::spa_fallback_location(&uri), Some("/index.html"));
}

#[test]
fn spa_fallback_does_not_capture_api_or_asset_paths() {
    let api_uri: axum::http::Uri = "/api/v1/does-not-exist".parse().unwrap();
    let asset_uri: axum::http::Uri = "/ui/assets/js/missing.js".parse().unwrap();
    assert_eq!(ui::spa_fallback_location(&api_uri), None);
    assert_eq!(ui::spa_fallback_location(&asset_uri), None);
}

#[test]
fn allows_loopback_origins_for_cors() {
    assert!(cors::is_allowed_origin(&HeaderValue::from_static(
        "http://localhost:3000"
    )));
    assert!(cors::is_allowed_origin(&HeaderValue::from_static(
        "http://127.0.0.1:17835"
    )));
    assert!(cors::is_allowed_origin(&HeaderValue::from_static(
        "http://[::1]:5173"
    )));
}

#[test]
fn rejects_non_loopback_origins_for_cors() {
    assert!(!cors::is_allowed_origin(&HeaderValue::from_static(
        "http://192.168.1.10:3000"
    )));
    assert!(!cors::is_allowed_origin(&HeaderValue::from_static(
        "https://example.com"
    )));
    assert!(!cors::is_allowed_origin(&HeaderValue::from_static("null")));
}

#[test]
fn cors_allow_methods_includes_put_and_patch() {
    let mut headers = HeaderMap::new();
    let origin = HeaderValue::from_static("http://localhost:3000");
    cors::apply_cors_headers(&mut headers, &origin);

    let methods = headers
        .get(axum::http::header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    assert!(methods.contains("PUT"));
    assert!(methods.contains("PATCH"));
}

#[test]
fn embeds_required_ui_files() {
    let required = [
        "index.html",
        "search.html",
        "search_details.html",
        "node_stats.html",
        "log.html",
        "settings.html",
        "assets/css/base.css",
        "assets/css/layout.css",
        "assets/css/typography.css",
        "assets/css/color-dark.css",
        "assets/css/colors-light.css",
        "assets/css/color-hc.css",
        "assets/js/alpine.min.js",
        "assets/js/app.js",
        "assets/js/chart.min.js",
        "assets/js/helpers.js",
        "assets/js/theme-init.js",
    ];

    for path in required {
        assert!(
            ui::UI_DIR.get_file(path).is_some(),
            "missing embedded ui file: {path}",
        );
    }
}

#[tokio::test]
async fn search_stop_dispatches_service_command() {
    let (tx, mut rx) = mpsc::channel(1);
    let state = test_state(tx);
    let search_id = "00112233445566778899aabbccddeeff".to_string();

    let expected = KadId::from_hex(&search_id).unwrap();

    let waiter = tokio::spawn(async move {
        let cmd = rx.recv().await.expect("expected command");
        match cmd {
            KadServiceCommand::StopKeywordSearch {
                keyword,
                respond_to,
            } => {
                assert_eq!(keyword, expected);
                let _ = respond_to.send(true);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    });

    let resp = handlers::search_stop(State(state), axum::extract::Path(search_id))
        .await
        .expect("search_stop should succeed");
    assert!(resp.0.stopped);
    waiter.await.unwrap();
}

#[tokio::test]
async fn search_delete_dispatches_with_default_purge_true() {
    let (tx, mut rx) = mpsc::channel(1);
    let state = test_state(tx);
    let search_id = "00112233445566778899aabbccddeeff".to_string();
    let expected = KadId::from_hex(&search_id).unwrap();

    let waiter = tokio::spawn(async move {
        let cmd = rx.recv().await.expect("expected command");
        match cmd {
            KadServiceCommand::DeleteKeywordSearch {
                keyword,
                purge_results,
                respond_to,
            } => {
                assert_eq!(keyword, expected);
                assert!(purge_results);
                let _ = respond_to.send(true);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    });

    let resp = handlers::search_delete(
        State(state),
        axum::extract::Path(search_id),
        Query(handlers::SearchDeleteQuery {
            purge_results: true,
        }),
    )
    .await
    .expect("search_delete should succeed");
    assert!(resp.0.deleted);
    assert!(resp.0.purged_results);
    waiter.await.unwrap();
}

#[tokio::test]
async fn settings_get_returns_config_snapshot() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);

    {
        let mut cfg = state.config.lock().await;
        cfg.sam.session_name = "session-a".to_string();
        cfg.api.port = 18080;
        cfg.api.enable_debug_endpoints = false;
        cfg.api.auth_mode = ApiAuthMode::HeadlessRemote;
        cfg.api.rate_limit_enabled = true;
        cfg.api.rate_limit_window_secs = 90;
        cfg.api.rate_limit_auth_bootstrap_max_per_window = 12;
        cfg.api.rate_limit_session_max_per_window = 13;
        cfg.api.rate_limit_token_rotate_max_per_window = 7;
        cfg.general.log_level = "info,rust_mule=debug".to_string();
        cfg.general.auto_open_ui = false;
    }

    let resp = handlers::settings_get(State(state))
        .await
        .expect("settings_get ok");
    assert_eq!(resp.0.settings.sam.session_name, "session-a");
    assert_eq!(resp.0.settings.api.port, 18080);
    assert!(!resp.0.settings.api.enable_debug_endpoints);
    assert!(matches!(
        resp.0.settings.api.auth_mode,
        ApiAuthMode::HeadlessRemote
    ));
    assert!(resp.0.settings.api.rate_limit_enabled);
    assert_eq!(resp.0.settings.api.rate_limit_window_secs, 90);
    assert_eq!(
        resp.0.settings.api.rate_limit_auth_bootstrap_max_per_window,
        12
    );
    assert_eq!(resp.0.settings.api.rate_limit_session_max_per_window, 13);
    assert_eq!(
        resp.0.settings.api.rate_limit_token_rotate_max_per_window,
        7
    );
    assert!(!resp.0.settings.general.auto_open_ui);
    assert!(resp.0.restart_required);
}

#[tokio::test]
async fn settings_patch_updates_and_persists_config() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);

    let result = handlers::settings_patch(
        State(state.clone()),
        Json(handlers::SettingsPatchRequest {
            general: Some(handlers::SettingsPatchGeneral {
                log_level: Some("info".to_string()),
                log_to_file: Some(false),
                log_file_level: None,
                auto_open_ui: Some(false),
            }),
            sam: Some(handlers::SettingsPatchSam {
                host: None,
                port: None,
                session_name: Some("test-session".to_string()),
            }),
            api: Some(handlers::SettingsPatchApi {
                port: Some(17836),
                enable_debug_endpoints: Some(false),
                auth_mode: Some(ApiAuthMode::HeadlessRemote),
                rate_limit_enabled: Some(true),
                rate_limit_window_secs: Some(60),
                rate_limit_auth_bootstrap_max_per_window: Some(30),
                rate_limit_session_max_per_window: Some(30),
                rate_limit_token_rotate_max_per_window: Some(10),
            }),
        }),
    )
    .await;

    let resp = result.expect("settings_patch should succeed");
    assert_eq!(resp.0.settings.sam.session_name, "test-session");
    assert_eq!(resp.0.settings.api.port, 17836);
    assert!(!resp.0.settings.api.enable_debug_endpoints);
    assert!(matches!(
        resp.0.settings.api.auth_mode,
        ApiAuthMode::HeadlessRemote
    ));
    assert!(resp.0.settings.api.rate_limit_enabled);
    assert_eq!(resp.0.settings.api.rate_limit_window_secs, 60);
    assert_eq!(
        resp.0.settings.api.rate_limit_auth_bootstrap_max_per_window,
        30
    );
    assert_eq!(resp.0.settings.api.rate_limit_session_max_per_window, 30);
    assert_eq!(
        resp.0.settings.api.rate_limit_token_rotate_max_per_window,
        10
    );
    assert!(!resp.0.settings.general.log_to_file);
    assert!(!resp.0.settings.general.auto_open_ui);
    assert!(resp.0.restart_required);
}

#[tokio::test]
async fn settings_patch_rejects_invalid_values() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    let resp = handlers::settings_patch(
        State(state),
        Json(handlers::SettingsPatchRequest {
            general: Some(handlers::SettingsPatchGeneral {
                log_level: Some("not-a-filter=[".to_string()),
                log_to_file: None,
                log_file_level: None,
                auto_open_ui: None,
            }),
            sam: None,
            api: None,
        }),
    )
    .await;
    assert!(matches!(resp, Err(StatusCode::BAD_REQUEST)));

    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    let resp_invalid_api_port = handlers::settings_patch(
        State(state),
        Json(handlers::SettingsPatchRequest {
            general: None,
            sam: None,
            api: Some(handlers::SettingsPatchApi {
                port: Some(0),
                enable_debug_endpoints: None,
                auth_mode: None,
                rate_limit_enabled: None,
                rate_limit_window_secs: None,
                rate_limit_auth_bootstrap_max_per_window: None,
                rate_limit_session_max_per_window: None,
                rate_limit_token_rotate_max_per_window: None,
            }),
        }),
    )
    .await;
    assert!(matches!(
        resp_invalid_api_port,
        Err(StatusCode::BAD_REQUEST)
    ));

    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    let resp_bad_rate_limit = handlers::settings_patch(
        State(state),
        Json(handlers::SettingsPatchRequest {
            general: None,
            sam: None,
            api: Some(handlers::SettingsPatchApi {
                port: None,
                enable_debug_endpoints: None,
                auth_mode: None,
                rate_limit_enabled: Some(true),
                rate_limit_window_secs: Some(0),
                rate_limit_auth_bootstrap_max_per_window: Some(0),
                rate_limit_session_max_per_window: Some(1),
                rate_limit_token_rotate_max_per_window: Some(1),
            }),
        }),
    )
    .await;
    assert!(matches!(resp_bad_rate_limit, Err(StatusCode::BAD_REQUEST)));
}

#[tokio::test]
async fn unauthenticated_ui_route_redirects_to_auth() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    let app = test_app(state);
    let req = Request::builder()
        .uri("/index.html")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/auth");
}

#[tokio::test]
async fn authenticated_ui_route_with_session_cookie_succeeds() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            "session-ok".to_string(),
            Instant::now() + Duration::from_secs(60),
        );
    }
    let app = test_app(state);
    let req = Request::builder()
        .uri("/index.html")
        .method(Method::GET)
        .header(header::COOKIE, "rm_session=session-ok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn events_rejects_bearer_only_but_accepts_session_cookie() {
    let (tx, _rx) = mpsc::channel(1);
    let state = test_state(tx);
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            "events-session".to_string(),
            Instant::now() + Duration::from_secs(60),
        );
    }
    let app = test_app(state);

    let bearer_req = Request::builder()
        .uri("/api/v1/events")
        .method(Method::GET)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let bearer_resp = app.clone().oneshot(bearer_req).await.unwrap();
    assert_eq!(bearer_resp.status(), StatusCode::UNAUTHORIZED);

    let cookie_req = Request::builder()
        .uri("/api/v1/events")
        .method(Method::GET)
        .header(header::COOKIE, "rm_session=events-session")
        .body(Body::empty())
        .unwrap();
    let cookie_resp = app.oneshot(cookie_req).await.unwrap();
    assert_eq!(cookie_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn debug_routes_are_404_when_debug_endpoints_disabled() {
    let (tx, _rx) = mpsc::channel(1);
    let mut state = test_state(tx);
    state.enable_debug_endpoints = false;
    let app = test_app(state);
    let req = Request::builder()
        .uri("/api/v1/debug/routing/summary")
        .method(Method::GET)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn auth_bootstrap_route_is_404_in_headless_mode() {
    let (tx, _rx) = mpsc::channel(1);
    let mut state = test_state(tx);
    state.auth_mode = ApiAuthMode::HeadlessRemote;
    let app = test_app(state);
    let req = Request::builder()
        .uri("/api/v1/auth/bootstrap")
        .method(Method::GET)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_route_is_rate_limited_when_threshold_exceeded() {
    let (tx, _rx) = mpsc::channel(1);
    let mut state = test_state(tx);
    state.rate_limit_enabled = true;
    state.rate_limit_session_max = 2;
    state.rate_limit_window = Duration::from_secs(60);
    let app = test_app(state);

    for _ in 0..2 {
        let req = Request::builder()
            .uri("/api/v1/session")
            .method(Method::POST)
            .header(header::AUTHORIZATION, "Bearer test-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let req = Request::builder()
        .uri("/api/v1/session")
        .method(Method::POST)
        .header(header::AUTHORIZATION, "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn token_rotate_updates_state_file_and_clears_sessions() {
    let (tx, _rx) = mpsc::channel(1);
    let test_dir = std::env::temp_dir().join(format!(
        "rust_mule_token_rotate_test_{}",
        std::process::id()
    ));
    let _ = tokio::fs::create_dir_all(&test_dir).await;
    let token_path = test_dir.join("api.token");
    tokio::fs::write(&token_path, b"old-token")
        .await
        .expect("write old token");

    let (_status_tx, status_rx) = watch::channel(None);
    let (status_events_tx, _status_events_rx) = broadcast::channel(16);
    let state = ApiState {
        token: Arc::new(tokio::sync::RwLock::new("old-token".to_string())),
        token_path: Arc::new(token_path.clone()),
        config_path: Arc::new(unique_test_config_path()),
        status_rx,
        status_events_tx,
        kad_cmd_tx: tx,
        download_handle: DownloadServiceHandle::test_handle(),
        config: Arc::new(tokio::sync::Mutex::new(Config::default())),
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::from([(
            "s1".to_string(),
            Instant::now() + Duration::from_secs(30),
        )]))),
        enable_debug_endpoints: true,
        auth_mode: ApiAuthMode::LocalUi,
        rate_limit_enabled: true,
        rate_limit_window: Duration::from_secs(60),
        rate_limit_auth_bootstrap_max: 30,
        rate_limit_session_max: 30,
        rate_limit_token_rotate_max: 10,
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };

    let resp = handlers::token_rotate(State(state.clone()))
        .await
        .expect("rotate should succeed");
    assert!(resp.0.sessions_cleared);
    assert_ne!(resp.0.token, "old-token");

    let current = state.token.read().await.clone();
    assert_eq!(current, resp.0.token);
    let file = tokio::fs::read_to_string(&token_path)
        .await
        .expect("read token");
    assert_eq!(file.trim(), resp.0.token);
    assert!(state.sessions.lock().await.is_empty());

    let _ = tokio::fs::remove_file(&token_path).await;
    let _ = tokio::fs::remove_dir(&test_dir).await;
}

#[tokio::test]
async fn ui_api_contract_endpoints_return_expected_shapes() {
    let (kad_tx, mut kad_rx) = mpsc::channel(16);
    let (status_tx, status_rx) = watch::channel(Some(sample_status()));
    let (status_events_tx, _status_events_rx) = broadcast::channel(16);
    let state = ApiState {
        token: Arc::new(tokio::sync::RwLock::new("test-token".to_string())),
        token_path: Arc::new(PathBuf::from("data/api.token")),
        config_path: Arc::new(unique_test_config_path()),
        status_rx,
        status_events_tx,
        kad_cmd_tx: kad_tx,
        download_handle: DownloadServiceHandle::test_handle(),
        config: Arc::new(tokio::sync::Mutex::new(Config::default())),
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        enable_debug_endpoints: true,
        auth_mode: ApiAuthMode::LocalUi,
        rate_limit_enabled: true,
        rate_limit_window: Duration::from_secs(60),
        rate_limit_auth_bootstrap_max: 30,
        rate_limit_session_max: 30,
        rate_limit_token_rotate_max: 10,
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };
    // Keep sender alive.
    let _status_tx_guard = status_tx;

    let search_id_hex = "00112233445566778899aabbccddeeff".to_string();
    let keyword_id = KadId::from_hex(&search_id_hex).unwrap();

    let responder = tokio::spawn(async move {
        while let Some(cmd) = kad_rx.recv().await {
            match cmd {
                KadServiceCommand::GetKeywordSearches { respond_to } => {
                    let _ = respond_to.send(vec![KadKeywordSearchInfo {
                        search_id_hex: search_id_hex.clone(),
                        keyword_id_hex: search_id_hex.clone(),
                        state: "running".to_string(),
                        created_secs_ago: 5,
                        hits: 1,
                        want_search: true,
                        publish_enabled: false,
                        got_publish_ack: false,
                    }]);
                }
                KadServiceCommand::GetKeywordResults {
                    keyword,
                    respond_to,
                } => {
                    assert_eq!(keyword, keyword_id);
                    let _ = respond_to.send(vec![KadKeywordHit {
                        file_id: KadId::from_hex("ffeeddccbbaa00998877665544332211").unwrap(),
                        filename: "demo-file.bin".to_string(),
                        file_size: 1234,
                        file_type: Some("Pro".to_string()),
                        publish_info: Some(1),
                        origin: KadKeywordHitOrigin::Network,
                    }]);
                }
                KadServiceCommand::GetPeers { respond_to } => {
                    let _ = respond_to.send(vec![KadPeerInfo {
                        kad_id_hex: search_id_hex.clone(),
                        udp_dest_b64: "dest".repeat(129),
                        udp_dest_short: "dest...".to_string(),
                        kad_version: 8,
                        verified: true,
                        udp_key: 1,
                        udp_key_ip: 2,
                        failures: 0,
                        peer_agent: Some("rust-mule".to_string()),
                        last_seen_secs_ago: 1,
                        last_inbound_secs_ago: Some(2),
                        last_queried_secs_ago: Some(3),
                        last_bootstrap_secs_ago: Some(4),
                        last_hello_secs_ago: Some(5),
                    }]);
                }
                other => panic!("unexpected command: {other:?}"),
            }
        }
    });

    let app = test_app(state);

    let status_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/status"))
        .await
        .unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_json = response_json(status_resp).await;
    assert!(status_json.get("routing").and_then(Value::as_u64).is_some());
    assert!(
        status_json
            .get("recv_req")
            .and_then(Value::as_u64)
            .is_some()
    );
    assert!(
        status_json
            .get("recv_res")
            .and_then(Value::as_u64)
            .is_some()
    );
    assert_eq!(
        status_json.get("recv_req").and_then(Value::as_u64),
        status_json.get("sent_reqs").and_then(Value::as_u64)
    );
    assert_eq!(
        status_json.get("recv_res").and_then(Value::as_u64),
        status_json.get("recv_ress").and_then(Value::as_u64)
    );
    assert_eq!(
        status_json.get("recv_req_total").and_then(Value::as_u64),
        status_json.get("sent_reqs_total").and_then(Value::as_u64)
    );
    assert_eq!(
        status_json.get("recv_res_total").and_then(Value::as_u64),
        status_json.get("recv_ress_total").and_then(Value::as_u64)
    );
    assert!(
        status_json
            .get("source_search_batch_sent")
            .and_then(Value::as_u64)
            .is_some()
    );
    assert!(
        status_json
            .get("source_probe_search_latency_ms_total")
            .and_then(Value::as_u64)
            .is_some()
    );

    let searches_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/searches"))
        .await
        .unwrap();
    assert_eq!(searches_resp.status(), StatusCode::OK);
    let searches_json = response_json(searches_resp).await;
    let searches = searches_json
        .get("searches")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(searches.len(), 1);
    assert_eq!(
        searches[0].get("search_id_hex").and_then(Value::as_str),
        Some("00112233445566778899aabbccddeeff")
    );

    let details_resp = app
        .clone()
        .oneshot(authorized_api_get(
            "/api/v1/searches/00112233445566778899aabbccddeeff",
        ))
        .await
        .unwrap();
    assert_eq!(details_resp.status(), StatusCode::OK);
    let details_json = response_json(details_resp).await;
    assert!(details_json.get("search").is_some());
    assert_eq!(
        details_json
            .get("hits")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(details_json["hits"][0]["origin"].as_str(), Some("network"));

    let keyword_results_resp = app
        .clone()
        .oneshot(authorized_api_get(
            "/api/v1/kad/keyword_results/00112233445566778899aabbccddeeff",
        ))
        .await
        .unwrap();
    assert_eq!(keyword_results_resp.status(), StatusCode::OK);
    let keyword_results_json = response_json(keyword_results_resp).await;
    assert_eq!(
        keyword_results_json["keyword_id_hex"].as_str(),
        Some("00112233445566778899aabbccddeeff")
    );
    assert_eq!(
        keyword_results_json
            .get("hits")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let peers_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/kad/peers"))
        .await
        .unwrap();
    assert_eq!(peers_resp.status(), StatusCode::OK);
    let peers_json = response_json(peers_resp).await;
    assert_eq!(
        peers_json
            .get("peers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(peers_json["peers"][0]["kad_version"].as_u64(), Some(8));

    let settings_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/settings"))
        .await
        .unwrap();
    assert_eq!(settings_resp.status(), StatusCode::OK);
    let settings_json = response_json(settings_resp).await;
    assert!(settings_json.get("settings").is_some());
    assert!(settings_json.get("restart_required").is_some());

    let downloads_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/downloads"))
        .await
        .unwrap();
    assert_eq!(downloads_resp.status(), StatusCode::OK);
    let downloads_json = response_json(downloads_resp).await;
    assert!(downloads_json.get("queue_len").is_some());
    assert!(downloads_json.get("recovered_on_start").is_some());
    assert_eq!(
        downloads_json
            .get("downloads")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    drop(app);
    responder.abort();
}

#[tokio::test]
async fn download_mutation_endpoints_update_service_state() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rust_mule_api_download_test_{}_{}",
        std::process::id(),
        stamp
    ));
    tokio::fs::create_dir_all(&root).await.expect("mkdir");
    let (download_handle, _download_status_rx, download_join) = crate::download::start_service(
        crate::download::DownloadServiceConfig::from_data_dir(&root),
    )
    .await
    .expect("start download service");

    let (kad_tx, _kad_rx) = mpsc::channel(1);
    let mut state = test_state(kad_tx);
    state.download_handle = download_handle.clone();
    let app = test_app(state);

    let create_resp = app
        .clone()
        .oneshot(authorized_api_post(
            "/api/v1/downloads",
            json!({
                "file_name":"movie.iso",
                "file_size": 1234,
                "file_hash_md4_hex": "0123456789abcdef0123456789abcdef"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_json = response_json(create_resp).await;
    assert_eq!(create_json["download"]["part_number"].as_u64(), Some(1));
    assert_eq!(create_json["download"]["state"].as_str(), Some("queued"));

    let pause_resp = app
        .clone()
        .oneshot(authorized_api_post("/api/v1/downloads/1/pause", json!({})))
        .await
        .unwrap();
    assert_eq!(pause_resp.status(), StatusCode::OK);
    let pause_json = response_json(pause_resp).await;
    assert_eq!(pause_json["download"]["state"].as_str(), Some("paused"));

    let resume_resp = app
        .clone()
        .oneshot(authorized_api_post("/api/v1/downloads/1/resume", json!({})))
        .await
        .unwrap();
    assert_eq!(resume_resp.status(), StatusCode::OK);
    let resume_json = response_json(resume_resp).await;
    assert_eq!(resume_json["download"]["state"].as_str(), Some("queued"));

    let cancel_resp = app
        .clone()
        .oneshot(authorized_api_post("/api/v1/downloads/1/cancel", json!({})))
        .await
        .unwrap();
    assert_eq!(cancel_resp.status(), StatusCode::OK);
    let cancel_json = response_json(cancel_resp).await;
    assert_eq!(cancel_json["download"]["state"].as_str(), Some("cancelled"));

    let pause_after_cancel = app
        .clone()
        .oneshot(authorized_api_post("/api/v1/downloads/1/pause", json!({})))
        .await
        .unwrap();
    assert_eq!(pause_after_cancel.status(), StatusCode::CONFLICT);

    let list_resp = app
        .clone()
        .oneshot(authorized_api_get("/api/v1/downloads"))
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_json = response_json(list_resp).await;
    assert_eq!(
        list_json
            .get("downloads")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        list_json["downloads"][0]["state"].as_str(),
        Some("cancelled")
    );

    let delete_resp = app
        .clone()
        .oneshot(authorized_api_delete("/api/v1/downloads/1"))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);
    let delete_json = response_json(delete_resp).await;
    assert_eq!(delete_json["deleted"].as_bool(), Some(true));

    let delete_missing = app
        .clone()
        .oneshot(authorized_api_delete("/api/v1/downloads/999"))
        .await
        .unwrap();
    assert_eq!(delete_missing.status(), StatusCode::NOT_FOUND);

    drop(app);
    download_handle.shutdown().await.expect("download shutdown");
    download_join.await.expect("join").expect("service");
    let _ = tokio::fs::remove_dir_all(&root).await;
}
