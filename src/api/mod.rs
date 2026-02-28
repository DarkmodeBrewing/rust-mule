use axum::middleware;
use std::{
    collections::HashMap,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    sync::atomic::AtomicU64,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    config::{ApiAuthMode, ApiConfig, Config},
    download::DownloadServiceHandle,
    kad::service::{KadServiceCommand, KadServiceStatus},
};

mod auth;
mod cors;
mod error;
mod handlers;
mod rate_limit;
mod router;
mod ui;

pub mod token;

pub(crate) const SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const SESSION_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Hard cap on the number of concurrent live sessions to prevent memory exhaustion.
pub(crate) const MAX_SESSIONS: usize = 1024;
pub(crate) const API_CMD_TIMEOUT: Duration = Duration::from_secs(5);

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug)]
pub enum ApiError {
    Bind(std::io::Error),
    Serve(std::io::Error),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(source) => write!(f, "failed to bind API listener: {source}"),
            Self::Serve(source) => write!(f, "API server failed: {source}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bind(source) => Some(source),
            Self::Serve(source) => Some(source),
        }
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub(crate) token: Arc<tokio::sync::RwLock<String>>,
    pub(crate) token_path: Arc<PathBuf>,
    pub(crate) config_path: Arc<PathBuf>,
    pub(crate) status_rx: watch::Receiver<Option<KadServiceStatus>>,
    pub(crate) status_events_tx: broadcast::Sender<KadServiceStatus>,
    pub(crate) kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    pub(crate) download_handle: DownloadServiceHandle,
    pub(crate) config: Arc<tokio::sync::Mutex<Config>>,
    pub(crate) sessions: Arc<tokio::sync::Mutex<HashMap<String, Instant>>>,
    pub(crate) enable_debug_endpoints: bool,
    pub(crate) auth_mode: ApiAuthMode,
    pub(crate) rate_limit_enabled: bool,
    pub(crate) rate_limit_window: Duration,
    pub(crate) rate_limit_auth_bootstrap_max: u32,
    pub(crate) rate_limit_session_max: u32,
    pub(crate) rate_limit_token_rotate_max: u32,
    pub(crate) rate_limits: Arc<tokio::sync::Mutex<HashMap<String, rate_limit::RateLimitBucket>>>,
    pub(crate) sse_serialize_fallback_total: Arc<AtomicU64>,
}

pub struct ApiServeDeps {
    pub app_config: Config,
    pub config_path: PathBuf,
    pub token_path: PathBuf,
    pub token: String,
    pub status_rx: watch::Receiver<Option<KadServiceStatus>>,
    pub status_events_tx: broadcast::Sender<KadServiceStatus>,
    pub kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    pub download_handle: DownloadServiceHandle,
}

pub fn new_channels() -> (
    watch::Sender<Option<KadServiceStatus>>,
    broadcast::Sender<KadServiceStatus>,
) {
    let (status_tx, _status_rx) = watch::channel(None);
    let (status_events_tx, _status_events_rx) = broadcast::channel(1024);
    (status_tx, status_events_tx)
}

async fn bind_loopback_listeners(
    port: u16,
) -> Result<Vec<(SocketAddr, tokio::net::TcpListener)>, ApiError> {
    let mut listeners = Vec::new();
    let mut bind_errors = Vec::new();
    let candidates = [
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
    ];

    for ip in candidates {
        let addr = SocketAddr::new(ip, port);
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listeners.push((addr, listener)),
            Err(err) => bind_errors.push((addr, err)),
        }
    }

    if listeners.is_empty() {
        let details = bind_errors
            .iter()
            .map(|(addr, err)| format!("{addr}: {err}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ApiError::Bind(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("failed to bind loopback listeners ({details})"),
        )));
    }

    for (addr, err) in bind_errors {
        tracing::warn!(addr = %addr, error = %err, "api loopback listener unavailable");
    }

    Ok(listeners)
}

pub async fn serve(cfg: &ApiConfig, deps: ApiServeDeps) -> ApiResult<()> {
    let state = ApiState {
        token: Arc::new(tokio::sync::RwLock::new(deps.token)),
        token_path: Arc::new(deps.token_path),
        config_path: Arc::new(deps.config_path),
        status_rx: deps.status_rx,
        status_events_tx: deps.status_events_tx,
        kad_cmd_tx: deps.kad_cmd_tx,
        download_handle: deps.download_handle,
        config: Arc::new(tokio::sync::Mutex::new(deps.app_config)),
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        enable_debug_endpoints: cfg.enable_debug_endpoints,
        auth_mode: cfg.auth_mode,
        rate_limit_enabled: cfg.rate_limit_enabled,
        rate_limit_window: Duration::from_secs(cfg.rate_limit_window_secs.max(1)),
        rate_limit_auth_bootstrap_max: cfg.rate_limit_auth_bootstrap_max_per_window.max(1),
        rate_limit_session_max: cfg.rate_limit_session_max_per_window.max(1),
        rate_limit_token_rotate_max: cfg.rate_limit_token_rotate_max_per_window.max(1),
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        sse_serialize_fallback_total: Arc::new(AtomicU64::new(0)),
    };

    let sessions_for_sweeper = state.sessions.clone();
    let app = router::build_app(state.clone())
        .layer(middleware::from_fn(cors::cors_mw))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_mw))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_mw,
        ))
        .layer(middleware::from_fn(error::error_envelope_mw));

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SESSION_SWEEP_INTERVAL).await;
            let mut sessions = sessions_for_sweeper.lock().await;
            auth::cleanup_expired_sessions(&mut sessions, Instant::now());
        }
    });

    let listeners = bind_loopback_listeners(cfg.port).await?;
    let mut servers = tokio::task::JoinSet::new();
    for (addr, listener) in listeners {
        tracing::info!(addr = %addr, "api server listening");
        let app = app.clone();
        servers.spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
        });
    }

    while let Some(joined) = servers.join_next().await {
        match joined {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                servers.abort_all();
                return Err(ApiError::Serve(err));
            }
            Err(err) => {
                servers.abort_all();
                return Err(ApiError::Serve(io::Error::other(format!(
                    "api server task failed: {err}"
                ))));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
