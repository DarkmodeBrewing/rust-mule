use rust_mule::{
    api::{self, ApiServeDeps},
    config::{ApiAuthMode, Config},
    kad::service::KadServiceCommand,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), id))
}

fn reserve_loopback_port() -> u16 {
    let listener =
        std::net::TcpListener::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0)).unwrap();
    listener.local_addr().unwrap().port()
}

async fn wait_for_api_or_server_error(
    client: &reqwest::Client,
    port: u16,
    timeout: Duration,
    serve_handle: &mut tokio::task::JoinHandle<api::ApiResult<()>>,
) -> String {
    let deadline = Instant::now() + timeout;
    let v4 = format!("http://127.0.0.1:{port}");
    let v6 = format!("http://[::1]:{port}");

    loop {
        if serve_handle.is_finished() {
            let result = serve_handle
                .await
                .expect("api serve task join should not panic");
            panic!("api serve exited before readiness: {result:?}");
        }

        for base in [&v4, &v6] {
            let resp = client
                .get(format!("{base}/api/v1/health"))
                .timeout(Duration::from_millis(200))
                .send()
                .await;
            if let Ok(resp) = resp
                && resp.status().as_u16() == 200
            {
                return base.to_string();
            }
        }

        assert!(
            Instant::now() < deadline,
            "api did not become ready in time"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn startup_auth_and_session_smoke_flow() {
    let test_dir = unique_temp_dir("rust_mule_api_smoke");
    tokio::fs::create_dir_all(&test_dir)
        .await
        .expect("create temp test dir");
    let token_path = test_dir.join("api.token");
    let config_path = test_dir.join("config.toml");
    tokio::fs::write(&token_path, b"smoke-token")
        .await
        .expect("write token");
    tokio::fs::write(&config_path, b"")
        .await
        .expect("write config placeholder");

    let mut cfg = Config::default();
    let port = reserve_loopback_port();
    cfg.api.port = port;
    cfg.api.auth_mode = ApiAuthMode::LocalUi;
    let api_cfg = cfg.api.clone();

    let (_status_tx, status_rx) = tokio::sync::watch::channel(None);
    let (status_events_tx, _status_events_rx) = tokio::sync::broadcast::channel(16);
    let (kad_cmd_tx, _kad_cmd_rx) = mpsc::channel::<KadServiceCommand>(1);

    let deps = ApiServeDeps {
        app_config: cfg.clone(),
        config_path: config_path.clone(),
        token_path: token_path.clone(),
        token: "smoke-token".to_string(),
        status_rx,
        status_events_tx,
        kad_cmd_tx,
    };
    let mut serve_handle = tokio::spawn(async move { api::serve(&api_cfg, deps).await });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("reqwest client");

    let base_url =
        wait_for_api_or_server_error(&client, port, Duration::from_secs(3), &mut serve_handle)
            .await;

    let bootstrap = client
        .get(format!("{base_url}/api/v1/auth/bootstrap"))
        .send()
        .await
        .expect("bootstrap request");
    assert_eq!(bootstrap.status().as_u16(), 200);
    let bootstrap_json: serde_json::Value = bootstrap.json().await.expect("bootstrap json");
    let token = bootstrap_json["token"]
        .as_str()
        .expect("token in bootstrap response");
    assert_eq!(token, "smoke-token");

    let session = client
        .post(format!("{base_url}/api/v1/session"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("session request");
    assert_eq!(session.status().as_u16(), 200);
    let cookie_header = session
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .expect("set-cookie in session response")
        .to_string();
    let session_cookie = cookie_header
        .split(';')
        .next()
        .expect("session cookie key/value")
        .to_string();
    assert!(session_cookie.starts_with("rm_session="));

    let session_check = client
        .get(format!("{base_url}/api/v1/session/check"))
        .header("Cookie", session_cookie.clone())
        .send()
        .await
        .expect("session check request");
    assert_eq!(session_check.status().as_u16(), 200);

    let ui = client
        .get(format!("{base_url}/index.html"))
        .header("Cookie", session_cookie)
        .send()
        .await
        .expect("ui request");
    assert_eq!(ui.status().as_u16(), 200);

    serve_handle.abort();
    let _ = serve_handle.await;
    let _ = tokio::fs::remove_file(token_path).await;
    let _ = tokio::fs::remove_file(config_path).await;
    let _ = tokio::fs::remove_dir(&test_dir).await;
}
