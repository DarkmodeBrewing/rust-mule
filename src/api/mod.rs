use axum::{
    Json, Router,
    extract::{ConnectInfo, OriginalUri, Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware,
    response::{
        IntoResponse, Redirect,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures_util::Stream;
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::BroadcastStream;
use tracing_subscriber::EnvFilter;

use crate::{
    config::{ApiConfig, Config, parse_api_bind_host},
    kad::{
        KadId, keyword,
        service::{
            KadKeywordHit, KadKeywordSearchInfo, KadPeerInfo, KadServiceCommand, KadServiceStatus,
            KadSourceEntry, RoutingBucketSummary, RoutingNodeSummary, RoutingSummary,
        },
    },
};

pub mod token;

static UI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/ui");
const SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const SESSION_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct ApiState {
    token: Arc<tokio::sync::RwLock<String>>,
    token_path: Arc<PathBuf>,
    status_rx: watch::Receiver<Option<KadServiceStatus>>,
    status_events_tx: broadcast::Sender<KadServiceStatus>,
    kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    config: Arc<tokio::sync::Mutex<Config>>,
    sessions: Arc<tokio::sync::Mutex<HashMap<String, Instant>>>,
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
) -> anyhow::Result<()> {
    let bind_ip = parse_api_bind_host(&cfg.host)?;
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
    let app = build_app(state.clone())
        .layer(middleware::from_fn(cors_mw))
        .layer(middleware::from_fn_with_state(state.clone(), auth_mw));

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SESSION_SWEEP_INTERVAL).await;
            let mut sessions = sessions_for_sweeper.lock().await;
            cleanup_expired_sessions(&mut sessions, Instant::now());
        }
    });

    tracing::info!(addr = %addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn build_app(state: ApiState) -> Router<()> {
    // Canonical API surface.
    let v1 = Router::new()
        .route("/health", get(health))
        .route("/dev/auth", get(dev_auth))
        .route("/token/rotate", post(token_rotate))
        .route("/session", post(create_session))
        .route("/session/check", get(session_check))
        .route("/session/logout", post(session_logout))
        .route("/status", get(status))
        .route("/events", get(events))
        .route("/settings", get(settings_get).patch(settings_patch))
        .route("/searches", get(searches))
        .route(
            "/searches/:search_id",
            get(search_details).delete(search_delete),
        )
        .route("/searches/:search_id/stop", post(search_stop))
        .route("/kad/peers", get(kad_peers))
        .route("/debug/routing/summary", get(debug_routing_summary))
        .route("/debug/routing/buckets", get(debug_routing_buckets))
        .route("/debug/routing/nodes", get(debug_routing_nodes))
        .route("/debug/lookup_once", post(debug_lookup_once))
        .route("/debug/probe_peer", post(debug_probe_peer))
        .route("/kad/sources/:file_id_hex", get(kad_sources))
        .route(
            "/kad/keyword_results/:keyword_id_hex",
            get(kad_keyword_results),
        )
        .route("/kad/search_sources", post(kad_search_sources))
        .route("/kad/search_keyword", post(kad_search_keyword))
        .route("/kad/publish_source", post(kad_publish_source))
        .route("/kad/publish_keyword", post(kad_publish_keyword));

    Router::new()
        .route("/", get(root_index_redirect))
        .route("/auth", get(ui_auth))
        .route("/index.html", get(ui_index))
        .route("/ui", get(ui_index))
        .route("/ui/", get(ui_index))
        .route("/ui/:page", get(ui_page))
        .route("/ui/assets/*path", get(ui_asset))
        .fallback(get(ui_fallback))
        .nest("/api/v1", v1)
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct KadSourcesReq {
    file_id_hex: String,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KadSearchKeywordReq {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    keyword_id_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KadPublishKeywordReq {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    keyword_id_hex: Option<String>,
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(default)]
    file_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct QueuedResponse {
    queued: bool,
}

#[derive(Debug, Serialize)]
struct KadSourcesResponse {
    file_id_hex: String,
    sources: Vec<KadSourceJson>,
}

#[derive(Debug, Serialize)]
struct KadSourceJson {
    source_id_hex: String,
    udp_dest_b64: String,
}

#[derive(Debug, Serialize)]
struct KadKeywordResultsResponse {
    keyword_id_hex: String,
    hits: Vec<KadKeywordHitJson>,
}

#[derive(Debug, Serialize)]
struct SearchListResponse {
    searches: Vec<KadKeywordSearchInfo>,
}

#[derive(Debug, Serialize)]
struct SearchDetailsResponse {
    search: KadKeywordSearchInfo,
    hits: Vec<KadKeywordHitJson>,
}

#[derive(Debug, Serialize)]
struct SearchStopResponse {
    stopped: bool,
}

#[derive(Debug, Deserialize)]
struct SearchDeleteQuery {
    #[serde(default = "default_true")]
    purge_results: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct SearchDeleteResponse {
    deleted: bool,
    purged_results: bool,
}

#[derive(Debug, Serialize)]
struct KadPeersResponse {
    peers: Vec<KadPeerInfo>,
}

#[derive(Debug, Serialize)]
struct RoutingSummaryResponse {
    summary: RoutingSummary,
}

#[derive(Debug, Serialize)]
struct RoutingBucketsResponse {
    buckets: Vec<RoutingBucketSummary>,
}

#[derive(Debug, Serialize)]
struct RoutingNodesResponse {
    bucket: usize,
    nodes: Vec<RoutingNodeSummary>,
}

#[derive(Debug, Deserialize)]
struct RoutingNodesQuery {
    bucket: usize,
}

#[derive(Debug, Deserialize)]
struct DebugLookupReq {
    #[serde(default)]
    target_id_hex: Option<String>,
}

#[derive(Debug, Serialize)]
struct DebugLookupResponse {
    target_id_hex: String,
}

#[derive(Debug, Deserialize)]
struct DebugProbeReq {
    udp_dest_b64: String,
    keyword_id_hex: String,
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(default)]
    file_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct DebugProbeResponse {
    queued: bool,
    peer_found: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SettingsGeneral {
    log_level: String,
    log_to_file: bool,
    log_file_level: String,
    auto_open_ui: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SettingsSam {
    host: String,
    port: u16,
    session_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct SettingsApi {
    host: String,
    port: u16,
}

#[derive(Debug, Clone, Serialize)]
struct SettingsPayload {
    general: SettingsGeneral,
    sam: SettingsSam,
    api: SettingsApi,
}

#[derive(Debug, Serialize)]
struct SettingsResponse {
    settings: SettingsPayload,
    restart_required: bool,
}

#[derive(Debug, Deserialize)]
struct SettingsPatchGeneral {
    #[serde(default)]
    log_level: Option<String>,
    #[serde(default)]
    log_to_file: Option<bool>,
    #[serde(default)]
    log_file_level: Option<String>,
    #[serde(default)]
    auto_open_ui: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SettingsPatchSam {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    session_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SettingsPatchApi {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct SettingsPatchRequest {
    #[serde(default)]
    general: Option<SettingsPatchGeneral>,
    #[serde(default)]
    sam: Option<SettingsPatchSam>,
    #[serde(default)]
    api: Option<SettingsPatchApi>,
}

impl SettingsPayload {
    fn from_config(cfg: &Config) -> Self {
        Self {
            general: SettingsGeneral {
                log_level: cfg.general.log_level.clone(),
                log_to_file: cfg.general.log_to_file,
                log_file_level: cfg.general.log_file_level.clone(),
                auto_open_ui: cfg.general.auto_open_ui,
            },
            sam: SettingsSam {
                host: cfg.sam.host.clone(),
                port: cfg.sam.port,
                session_name: cfg.sam.session_name.clone(),
            },
            api: SettingsApi {
                host: cfg.api.host.clone(),
                port: cfg.api.port,
            },
        }
    }
}

fn validate_settings(cfg: &Config) -> Result<(), StatusCode> {
    cfg.sam
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if !(1..=65535).contains(&cfg.sam.port) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if cfg.sam.session_name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    parse_api_bind_host(&cfg.api.host).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !(1..=65535).contains(&cfg.api.port) {
        return Err(StatusCode::BAD_REQUEST);
    }

    EnvFilter::try_new(cfg.general.log_level.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;
    EnvFilter::try_new(cfg.general.log_file_level.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(())
}

fn apply_settings_patch(cfg: &mut Config, patch: SettingsPatchRequest) {
    if let Some(general) = patch.general {
        if let Some(log_level) = general.log_level {
            cfg.general.log_level = log_level.trim().to_string();
        }
        if let Some(log_to_file) = general.log_to_file {
            cfg.general.log_to_file = log_to_file;
        }
        if let Some(log_file_level) = general.log_file_level {
            cfg.general.log_file_level = log_file_level.trim().to_string();
        }
        if let Some(auto_open_ui) = general.auto_open_ui {
            cfg.general.auto_open_ui = auto_open_ui;
        }
    }

    if let Some(sam) = patch.sam {
        if let Some(host) = sam.host {
            cfg.sam.host = host.trim().to_string();
        }
        if let Some(port) = sam.port {
            cfg.sam.port = port;
        }
        if let Some(session_name) = sam.session_name {
            cfg.sam.session_name = session_name.trim().to_string();
        }
    }

    if let Some(api) = patch.api {
        if let Some(host) = api.host {
            cfg.api.host = host.trim().to_string();
        }
        if let Some(port) = api.port {
            cfg.api.port = port;
        }
    }
}

#[derive(Debug, Serialize)]
struct KadKeywordHitJson {
    file_id_hex: String,
    filename: String,
    file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publish_info: Option<u32>,
    origin: String,
}

async fn kad_sources(
    State(state): State<ApiState>,
    axum::extract::Path(file_id_hex): axum::extract::Path<String>,
) -> Result<Json<KadSourcesResponse>, StatusCode> {
    let file = KadId::from_hex(&file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadSourceEntry>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetSources {
            file,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let sources = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let sources = sources
        .into_iter()
        .map(|s| KadSourceJson {
            source_id_hex: s.source_id.to_hex_lower(),
            udp_dest_b64: crate::i2p::b64::encode(&s.udp_dest),
        })
        .collect::<Vec<_>>();

    Ok(Json(KadSourcesResponse {
        file_id_hex: file.to_hex_lower(),
        sources,
    }))
}

async fn kad_keyword_results(
    State(state): State<ApiState>,
    axum::extract::Path(keyword_id_hex): axum::extract::Path<String>,
) -> Result<Json<KadKeywordResultsResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&keyword_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordHit>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordResults {
            keyword: keyword_id,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = hits
        .into_iter()
        .map(|h| KadKeywordHitJson {
            file_id_hex: h.file_id.to_hex_lower(),
            filename: h.filename,
            file_size: h.file_size,
            file_type: h.file_type,
            publish_info: h.publish_info,
            origin: h.origin.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Json(KadKeywordResultsResponse {
        keyword_id_hex: keyword_id.to_hex_lower(),
        hits,
    }))
}

async fn searches(State(state): State<ApiState>) -> Result<Json<SearchListResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordSearchInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordSearches { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let searches = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchListResponse { searches }))
}

async fn search_details(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
) -> Result<Json<SearchDetailsResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (searches_tx, searches_rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordSearchInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordSearches {
            respond_to: searches_tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let searches = tokio::time::timeout(std::time::Duration::from_secs(2), searches_rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let search = searches
        .into_iter()
        .find(|s| s.search_id_hex == keyword_id.to_hex_lower())
        .ok_or(StatusCode::NOT_FOUND)?;

    let (hits_tx, hits_rx) = tokio::sync::oneshot::channel::<Vec<KadKeywordHit>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetKeywordResults {
            keyword: keyword_id,
            respond_to: hits_tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = tokio::time::timeout(std::time::Duration::from_secs(2), hits_rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let hits = hits
        .into_iter()
        .map(|h| KadKeywordHitJson {
            file_id_hex: h.file_id.to_hex_lower(),
            filename: h.filename,
            file_size: h.file_size,
            file_type: h.file_type,
            publish_info: h.publish_info,
            origin: h.origin.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Json(SearchDetailsResponse { search, hits }))
}

async fn search_stop(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
) -> Result<Json<SearchStopResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::StopKeywordSearch {
            keyword: keyword_id,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let stopped = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchStopResponse { stopped }))
}

async fn search_delete(
    State(state): State<ApiState>,
    axum::extract::Path(search_id): axum::extract::Path<String>,
    Query(q): Query<SearchDeleteQuery>,
) -> Result<Json<SearchDeleteResponse>, StatusCode> {
    let keyword_id = KadId::from_hex(&search_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::DeleteKeywordSearch {
            keyword: keyword_id,
            purge_results: q.purge_results,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let deleted = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(SearchDeleteResponse {
        deleted,
        purged_results: q.purge_results,
    }))
}

async fn kad_peers(State(state): State<ApiState>) -> Result<Json<KadPeersResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<KadPeerInfo>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetPeers { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let peers = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadPeersResponse { peers }))
}

async fn debug_routing_summary(
    State(state): State<ApiState>,
) -> Result<Json<RoutingSummaryResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<RoutingSummary>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingSummary { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let summary = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingSummaryResponse { summary }))
}

async fn debug_routing_buckets(
    State(state): State<ApiState>,
) -> Result<Json<RoutingBucketsResponse>, StatusCode> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoutingBucketSummary>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingBuckets { respond_to: tx })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let buckets = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingBucketsResponse { buckets }))
}

async fn debug_routing_nodes(
    State(state): State<ApiState>,
    Query(query): Query<RoutingNodesQuery>,
) -> Result<Json<RoutingNodesResponse>, StatusCode> {
    if query.bucket >= 128 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoutingNodeSummary>>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::GetRoutingNodes {
            bucket: query.bucket,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let nodes = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(RoutingNodesResponse {
        bucket: query.bucket,
        nodes,
    }))
}

async fn debug_lookup_once(
    State(state): State<ApiState>,
    Json(req): Json<DebugLookupReq>,
) -> Result<Json<DebugLookupResponse>, StatusCode> {
    let target = match req.target_id_hex.as_deref() {
        Some(hex) => Some(KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?),
        None => None,
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<KadId>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::StartDebugLookup {
            target,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let target_id = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(DebugLookupResponse {
        target_id_hex: target_id.to_hex_lower(),
    }))
}

async fn debug_probe_peer(
    State(state): State<ApiState>,
    Json(req): Json<DebugProbeReq>,
) -> Result<Json<DebugProbeResponse>, StatusCode> {
    if req.filename.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let keyword = KadId::from_hex(&req.keyword_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .kad_cmd_tx
        .send(KadServiceCommand::DebugProbePeer {
            dest_b64: req.udp_dest_b64,
            keyword,
            file,
            filename: req.filename,
            file_size: req.file_size,
            file_type: req.file_type,
            respond_to: tx,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let peer_found = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(DebugProbeResponse {
        queued: peer_found,
        peer_found,
    }))
}

async fn kad_search_sources(
    State(state): State<ApiState>,
    Json(req): Json<KadSourcesReq>,
) -> Result<Json<QueuedResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_size = req.file_size.unwrap_or(0);
    state
        .kad_cmd_tx
        .send(KadServiceCommand::SearchSources { file, file_size })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(QueuedResponse { queued: true }))
}

#[derive(Debug, Serialize)]
struct KadSearchKeywordResponse {
    queued: bool,
    keyword: String,
    keyword_id_hex: String,
}

async fn kad_search_keyword(
    State(state): State<ApiState>,
    Json(req): Json<KadSearchKeywordReq>,
) -> Result<Json<KadSearchKeywordResponse>, StatusCode> {
    let (word, keyword_id) = if let Some(hex) = req.keyword_id_hex.as_deref() {
        let id = KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?;
        ("".to_string(), id)
    } else if let Some(q) = req.query.as_deref() {
        keyword::query_to_keyword_id(q).map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    state
        .kad_cmd_tx
        .send(KadServiceCommand::SearchKeyword {
            keyword: keyword_id,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadSearchKeywordResponse {
        queued: true,
        keyword: word,
        keyword_id_hex: keyword_id.to_hex_lower(),
    }))
}

async fn kad_publish_source(
    State(state): State<ApiState>,
    Json(req): Json<KadSourcesReq>,
) -> Result<Json<QueuedResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_size = req.file_size.unwrap_or(0);
    state
        .kad_cmd_tx
        .send(KadServiceCommand::PublishSource { file, file_size })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(QueuedResponse { queued: true }))
}

async fn kad_publish_keyword(
    State(state): State<ApiState>,
    Json(req): Json<KadPublishKeywordReq>,
) -> Result<Json<KadSearchKeywordResponse>, StatusCode> {
    let file = KadId::from_hex(&req.file_id_hex).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (word, keyword_id) = if let Some(hex) = req.keyword_id_hex.as_deref() {
        let id = KadId::from_hex(hex).map_err(|_| StatusCode::BAD_REQUEST)?;
        ("".to_string(), id)
    } else if let Some(q) = req.query.as_deref() {
        keyword::query_to_keyword_id(q).map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    state
        .kad_cmd_tx
        .send(KadServiceCommand::PublishKeyword {
            keyword: keyword_id,
            file,
            filename: req.filename,
            file_size: req.file_size,
            file_type: req.file_type,
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(KadSearchKeywordResponse {
        queued: true,
        keyword: word,
        keyword_id_hex: keyword_id.to_hex_lower(),
    }))
}

async fn settings_get(State(state): State<ApiState>) -> Result<Json<SettingsResponse>, StatusCode> {
    let cfg = state.config.lock().await;
    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&cfg),
        restart_required: true,
    }))
}

async fn settings_patch(
    State(state): State<ApiState>,
    Json(patch): Json<SettingsPatchRequest>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let mut cfg = state.config.lock().await;
    let mut next = cfg.clone();
    apply_settings_patch(&mut next, patch);
    validate_settings(&next)?;
    next.persist()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    *cfg = next.clone();

    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&next),
        restart_required: true,
    }))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct DevAuthResponse {
    token: String,
}

#[derive(Debug, Serialize)]
struct TokenRotateResponse {
    token: String,
    sessions_cleared: bool,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    ok: bool,
}

async fn dev_auth(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<DevAuthResponse>, StatusCode> {
    if !is_loopback_addr(&addr) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(DevAuthResponse {
        token: state.token.read().await.clone(),
    }))
}

async fn token_rotate(
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

async fn create_session(State(state): State<ApiState>) -> Result<impl IntoResponse, StatusCode> {
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

async fn session_check(State(_state): State<ApiState>) -> Json<SessionResponse> {
    Json(SessionResponse { ok: true })
}

async fn session_logout(
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

async fn status(State(state): State<ApiState>) -> Result<Json<KadServiceStatus>, StatusCode> {
    let Some(s) = (*state.status_rx.borrow()).clone() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    Ok(Json(s))
}

async fn events(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.status_events_tx.subscribe();
    let initial = (*state.status_rx.borrow()).clone();
    let stream = status_sse_stream(initial, BroadcastStream::new(rx));
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn ui_auth() -> impl IntoResponse {
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
          const authResp = await fetch('/api/v1/dev/auth');
          if (!authResp.ok) throw new Error('dev auth failed');
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

fn status_sse_stream(
    initial: Option<KadServiceStatus>,
    stream: BroadcastStream<KadServiceStatus>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    use futures_util::StreamExt as _;

    let initial_stream =
        futures_util::stream::iter(initial.into_iter().map(Ok::<KadServiceStatus, Infallible>));
    let merged = initial_stream.chain(stream.filter_map(|msg| async move { msg.ok().map(Ok) }));

    merged.map(|Ok(status)| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
        Ok(Event::default().event("status").data(json))
    })
}

async fn auth_mw(
    State(state): State<ApiState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // CORS preflight should not require bearer auth.
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

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let auth = auth.strip_prefix("Bearer ")?;
    Some(auth.to_string())
}

fn is_loopback_addr(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

fn is_api_bearer_exempt_path(path: &str) -> bool {
    matches!(path, "/api/v1/health" | "/api/v1/dev/auth")
}

fn is_frontend_exempt_path(path: &str) -> bool {
    path == "/auth"
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
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

async fn has_valid_session(headers: &HeaderMap, state: &ApiState) -> bool {
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

fn cleanup_expired_sessions(sessions: &mut HashMap<String, Instant>, now: Instant) {
    sessions.retain(|_, expiry| *expiry > now);
}

fn build_session_cookie(session_id: &str, ttl: Duration) -> String {
    format!(
        "rm_session={session_id}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        ttl.as_secs()
    )
}

fn clear_session_cookie() -> String {
    "rm_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0".to_string()
}

fn generate_session_id() -> Result<String, StatusCode> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

async fn cors_mw(
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

fn apply_cors_headers(headers: &mut HeaderMap, origin: &HeaderValue) {
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

fn is_allowed_origin(origin: &HeaderValue) -> bool {
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

async fn ui_index() -> Result<axum::response::Response, StatusCode> {
    serve_ui_file("index.html").await
}

async fn root_index_redirect() -> Redirect {
    Redirect::to("/index.html")
}

async fn ui_page(Path(page): Path<String>) -> Result<axum::response::Response, StatusCode> {
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

async fn ui_asset(Path(path): Path<String>) -> Result<axum::response::Response, StatusCode> {
    if !is_safe_ui_path(&path) {
        return Err(StatusCode::BAD_REQUEST);
    }
    serve_embedded_ui_file(&format!("assets/{path}"))
}

async fn ui_fallback(OriginalUri(uri): OriginalUri) -> Result<Redirect, StatusCode> {
    if let Some(location) = spa_fallback_location(&uri) {
        return Ok(Redirect::to(location));
    }
    Err(StatusCode::NOT_FOUND)
}

fn is_safe_ui_segment(name: &str) -> bool {
    !name.contains('/') && !name.contains('\\') && !name.contains("..")
}

fn is_safe_ui_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains("..") {
        return false;
    }
    path.split('/').all(|c| !c.is_empty() && c != ".")
}

fn spa_fallback_location(uri: &axum::http::Uri) -> Option<&'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use tokio::sync::{broadcast, mpsc, watch};
    use tower::util::ServiceExt as _;

    fn test_state(kad_cmd_tx: mpsc::Sender<KadServiceCommand>) -> ApiState {
        let (_status_tx, status_rx) = watch::channel(None);
        let (status_events_tx, _status_events_rx) = broadcast::channel(16);
        ApiState {
            token: Arc::new(tokio::sync::RwLock::new("test-token".to_string())),
            token_path: Arc::new(PathBuf::from("data/api.token")),
            status_rx,
            status_events_tx,
            kad_cmd_tx,
            config: Arc::new(tokio::sync::Mutex::new(Config::default())),
            sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    fn test_app(state: ApiState) -> Router<()> {
        build_app(state.clone())
            .layer(middleware::from_fn(cors_mw))
            .layer(middleware::from_fn_with_state(state, auth_mw))
    }

    #[test]
    fn detects_loopback_addresses() {
        let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234);
        let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1234);
        let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234);

        assert!(is_loopback_addr(&v4));
        assert!(is_loopback_addr(&v6));
        assert!(!is_loopback_addr(&other));
    }

    #[test]
    fn parse_api_bind_host_accepts_only_loopback() {
        assert!(parse_api_bind_host("localhost").is_ok());
        assert!(parse_api_bind_host("127.0.0.1").is_ok());
        assert!(parse_api_bind_host("::1").is_ok());
        assert!(parse_api_bind_host("0.0.0.0").is_err());
        assert!(parse_api_bind_host("10.0.0.1").is_err());
    }

    #[test]
    fn api_bearer_exempt_paths_include_only_health_and_dev_auth() {
        assert!(is_api_bearer_exempt_path("/api/v1/health"));
        assert!(is_api_bearer_exempt_path("/api/v1/dev/auth"));
        assert!(!is_api_bearer_exempt_path("/api/v1/status"));
        assert!(!is_api_bearer_exempt_path("/api/v1/session"));
        assert!(!is_api_bearer_exempt_path("/api/v1/token/rotate"));
    }

    #[test]
    fn frontend_exempt_paths_include_only_auth_page() {
        assert!(is_frontend_exempt_path("/auth"));
        assert!(!is_frontend_exempt_path("/"));
        assert!(!is_frontend_exempt_path("/ui"));
    }

    #[test]
    fn parses_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; rm_session=session123; x=y"),
        );
        assert_eq!(session_cookie(&headers).as_deref(), Some("session123"));
    }

    #[test]
    fn session_cookie_ttl_and_clear_cookie_headers_are_well_formed() {
        let cookie = build_session_cookie("abc", Duration::from_secs(60));
        assert!(cookie.contains("rm_session=abc"));
        assert!(cookie.contains("Max-Age=60"));
        assert!(clear_session_cookie().contains("Max-Age=0"));
    }

    #[test]
    fn cleanup_expired_sessions_removes_expired_entries() {
        let now = Instant::now();
        let mut sessions = HashMap::new();
        sessions.insert("alive".to_string(), now + Duration::from_secs(10));
        sessions.insert("dead".to_string(), now - Duration::from_secs(10));
        cleanup_expired_sessions(&mut sessions, now);
        assert!(sessions.contains_key("alive"));
        assert!(!sessions.contains_key("dead"));
    }

    #[test]
    fn validates_ui_paths() {
        assert!(is_safe_ui_path("css/base.css"));
        assert!(!is_safe_ui_path("../etc/passwd"));
        assert!(!is_safe_ui_path("css\\base.css"));
        assert!(is_safe_ui_segment("index.html"));
        assert!(!is_safe_ui_segment("../index.html"));
    }

    #[test]
    fn spa_fallback_redirects_unknown_non_api_paths_to_root() {
        let uri: axum::http::Uri = "/nonexisting.php?queryParams=whatever".parse().unwrap();
        assert_eq!(spa_fallback_location(&uri), Some("/index.html"));
    }

    #[test]
    fn spa_fallback_does_not_capture_api_or_asset_paths() {
        let api_uri: axum::http::Uri = "/api/v1/does-not-exist".parse().unwrap();
        let asset_uri: axum::http::Uri = "/ui/assets/js/missing.js".parse().unwrap();
        assert_eq!(spa_fallback_location(&api_uri), None);
        assert_eq!(spa_fallback_location(&asset_uri), None);
    }

    #[test]
    fn allows_loopback_origins_for_cors() {
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "http://localhost:3000"
        )));
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "http://127.0.0.1:17835"
        )));
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "http://[::1]:5173"
        )));
    }

    #[test]
    fn rejects_non_loopback_origins_for_cors() {
        assert!(!is_allowed_origin(&HeaderValue::from_static(
            "http://192.168.1.10:3000"
        )));
        assert!(!is_allowed_origin(&HeaderValue::from_static(
            "https://example.com"
        )));
        assert!(!is_allowed_origin(&HeaderValue::from_static("null")));
    }

    #[test]
    fn cors_allow_methods_includes_put_and_patch() {
        let mut headers = HeaderMap::new();
        let origin = HeaderValue::from_static("http://localhost:3000");
        apply_cors_headers(&mut headers, &origin);

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
                UI_DIR.get_file(path).is_some(),
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

        let resp = search_stop(State(state), axum::extract::Path(search_id))
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

        let resp = search_delete(
            State(state),
            axum::extract::Path(search_id),
            Query(SearchDeleteQuery {
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
            cfg.general.log_level = "info,rust_mule=debug".to_string();
            cfg.general.auto_open_ui = false;
        }

        let resp = settings_get(State(state)).await.expect("settings_get ok");
        assert_eq!(resp.0.settings.sam.session_name, "session-a");
        assert_eq!(resp.0.settings.api.port, 18080);
        assert!(!resp.0.settings.general.auto_open_ui);
        assert!(resp.0.restart_required);
    }

    #[tokio::test]
    async fn settings_patch_updates_and_persists_config() {
        let (tx, _rx) = mpsc::channel(1);
        let state = test_state(tx);

        let original = tokio::fs::read_to_string("config.toml")
            .await
            .expect("read config.toml");

        let result = settings_patch(
            State(state.clone()),
            Json(SettingsPatchRequest {
                general: Some(SettingsPatchGeneral {
                    log_level: Some("info".to_string()),
                    log_to_file: Some(false),
                    log_file_level: None,
                    auto_open_ui: Some(false),
                }),
                sam: Some(SettingsPatchSam {
                    host: None,
                    port: None,
                    session_name: Some("test-session".to_string()),
                }),
                api: Some(SettingsPatchApi {
                    host: None,
                    port: Some(17836),
                }),
            }),
        )
        .await;

        tokio::fs::write("config.toml", original)
            .await
            .expect("restore config.toml");

        let resp = result.expect("settings_patch should succeed");
        assert_eq!(resp.0.settings.sam.session_name, "test-session");
        assert_eq!(resp.0.settings.api.port, 17836);
        assert!(!resp.0.settings.general.log_to_file);
        assert!(!resp.0.settings.general.auto_open_ui);
        assert!(resp.0.restart_required);
    }

    #[tokio::test]
    async fn settings_patch_rejects_invalid_values() {
        let (tx, _rx) = mpsc::channel(1);
        let state = test_state(tx);
        let resp = settings_patch(
            State(state),
            Json(SettingsPatchRequest {
                general: Some(SettingsPatchGeneral {
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
        let resp_non_loopback_api_host = settings_patch(
            State(state),
            Json(SettingsPatchRequest {
                general: None,
                sam: None,
                api: Some(SettingsPatchApi {
                    host: Some("0.0.0.0".to_string()),
                    port: None,
                }),
            }),
        )
        .await;
        assert!(matches!(
            resp_non_loopback_api_host,
            Err(StatusCode::BAD_REQUEST)
        ));
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
            status_rx,
            status_events_tx,
            kad_cmd_tx: tx,
            config: Arc::new(tokio::sync::Mutex::new(Config::default())),
            sessions: Arc::new(tokio::sync::Mutex::new(HashMap::from([(
                "s1".to_string(),
                Instant::now() + Duration::from_secs(30),
            )]))),
        };

        let resp = token_rotate(State(state.clone()))
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
}
