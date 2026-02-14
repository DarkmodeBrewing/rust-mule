use axum::middleware;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    config::{ApiConfig, Config, parse_api_bind_host},
    kad::service::{KadServiceCommand, KadServiceStatus},
};

mod auth;
mod cors;
mod handlers;
mod router;
mod ui;

pub mod token;

pub(crate) const SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const SESSION_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug)]
pub enum ApiError {
    Config(crate::config::ConfigError),
    Bind(std::io::Error),
    Serve(std::io::Error),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(source) => write!(f, "{source}"),
            Self::Bind(source) => write!(f, "failed to bind API listener: {source}"),
            Self::Serve(source) => write!(f, "API server failed: {source}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(source) => Some(source),
            Self::Bind(source) => Some(source),
            Self::Serve(source) => Some(source),
        }
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub(crate) token: Arc<tokio::sync::RwLock<String>>,
    pub(crate) token_path: Arc<PathBuf>,
    pub(crate) status_rx: watch::Receiver<Option<KadServiceStatus>>,
    pub(crate) status_events_tx: broadcast::Sender<KadServiceStatus>,
    pub(crate) kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    pub(crate) config: Arc<tokio::sync::Mutex<Config>>,
    pub(crate) sessions: Arc<tokio::sync::Mutex<HashMap<String, Instant>>>,
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
    app_config: Config,
    token_path: PathBuf,
    token: String,
    status_rx: watch::Receiver<Option<KadServiceStatus>>,
    status_events_tx: broadcast::Sender<KadServiceStatus>,
    kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
) -> ApiResult<()> {
    let bind_ip = parse_api_bind_host(&cfg.host).map_err(ApiError::Config)?;
    let addr = SocketAddr::new(bind_ip, cfg.port);

    let state = ApiState {
        token: Arc::new(tokio::sync::RwLock::new(token)),
        token_path: Arc::new(token_path),
        status_rx,
        status_events_tx,
        kad_cmd_tx,
        config: Arc::new(tokio::sync::Mutex::new(app_config)),
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };

    let sessions_for_sweeper = state.sessions.clone();
    let app = router::build_app(state.clone())
        .layer(middleware::from_fn(cors::cors_mw))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_mw));

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SESSION_SWEEP_INTERVAL).await;
            let mut sessions = sessions_for_sweeper.lock().await;
            auth::cleanup_expired_sessions(&mut sessions, Instant::now());
        }
    });

    tracing::info!(addr = %addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(ApiError::Bind)?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(ApiError::Serve)?;
    Ok(())
}

#[cfg(test)]
mod tests;
