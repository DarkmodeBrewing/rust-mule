use crate::{
    config::{Config, SamDatagramTransport},
    i2p::sam::{SamClient, SamDatagramSocket, SamDatagramTcp, SamError, SamKadSocket, SamKeys},
    single_instance::SingleInstanceLock,
};
use std::collections::BTreeMap;
use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{Duration, Instant};

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Config(crate::config::ConfigError),
    SingleInstance(crate::single_instance::SingleInstanceError),
    Kad(crate::kad::KadError),
    UdpKey(crate::kad::udp_key::UdpKeyError),
    Sam(crate::i2p::sam::SamError),
    SamKeys(crate::i2p::sam::keys::SamKeysError),
    B64(crate::i2p::b64::B64Error),
    Token(crate::api::token::TokenError),
    Api(crate::api::ApiError),
    Nodes(crate::nodes::imule::ImuleNodesError),
    Bootstrap(crate::kad::bootstrap::BootstrapError),
    KadService(crate::kad::service::KadServiceError),
    Http(crate::i2p::http::HttpError),
    Io(std::io::Error),
    AddrParse(std::net::AddrParseError),
    ParseInt(std::num::ParseIntError),
    BrowserOpenFailed,
    InvalidState(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(source) => write!(f, "{source}"),
            Self::SingleInstance(source) => write!(f, "{source}"),
            Self::Kad(source) => write!(f, "{source}"),
            Self::UdpKey(source) => write!(f, "{source}"),
            Self::Sam(source) => write!(f, "{source}"),
            Self::SamKeys(source) => write!(f, "{source}"),
            Self::B64(source) => write!(f, "{source}"),
            Self::Token(source) => write!(f, "{source}"),
            Self::Api(source) => write!(f, "{source}"),
            Self::Nodes(source) => write!(f, "{source}"),
            Self::Bootstrap(source) => write!(f, "{source}"),
            Self::KadService(source) => write!(f, "{source}"),
            Self::Http(source) => write!(f, "{source}"),
            Self::Io(source) => write!(f, "{source}"),
            Self::AddrParse(source) => write!(f, "{source}"),
            Self::ParseInt(source) => write!(f, "{source}"),
            Self::BrowserOpenFailed => write!(f, "browser open command failed"),
            Self::InvalidState(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(source) => Some(source),
            Self::SingleInstance(source) => Some(source),
            Self::Kad(source) => Some(source),
            Self::UdpKey(source) => Some(source),
            Self::Sam(source) => Some(source),
            Self::SamKeys(source) => Some(source),
            Self::B64(source) => Some(source),
            Self::Token(source) => Some(source),
            Self::Api(source) => Some(source),
            Self::Nodes(source) => Some(source),
            Self::Bootstrap(source) => Some(source),
            Self::KadService(source) => Some(source),
            Self::Http(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::AddrParse(source) => Some(source),
            Self::ParseInt(source) => Some(source),
            Self::BrowserOpenFailed | Self::InvalidState(_) => None,
        }
    }
}

impl From<crate::config::ConfigError> for AppError {
    fn from(value: crate::config::ConfigError) -> Self {
        Self::Config(value)
    }
}
impl From<crate::single_instance::SingleInstanceError> for AppError {
    fn from(value: crate::single_instance::SingleInstanceError) -> Self {
        Self::SingleInstance(value)
    }
}
impl From<crate::kad::KadError> for AppError {
    fn from(value: crate::kad::KadError) -> Self {
        Self::Kad(value)
    }
}
impl From<crate::kad::udp_key::UdpKeyError> for AppError {
    fn from(value: crate::kad::udp_key::UdpKeyError) -> Self {
        Self::UdpKey(value)
    }
}
impl From<crate::i2p::sam::SamError> for AppError {
    fn from(value: crate::i2p::sam::SamError) -> Self {
        Self::Sam(value)
    }
}
impl From<crate::i2p::sam::keys::SamKeysError> for AppError {
    fn from(value: crate::i2p::sam::keys::SamKeysError) -> Self {
        Self::SamKeys(value)
    }
}
impl From<crate::i2p::b64::B64Error> for AppError {
    fn from(value: crate::i2p::b64::B64Error) -> Self {
        Self::B64(value)
    }
}
impl From<crate::api::token::TokenError> for AppError {
    fn from(value: crate::api::token::TokenError) -> Self {
        Self::Token(value)
    }
}
impl From<crate::api::ApiError> for AppError {
    fn from(value: crate::api::ApiError) -> Self {
        Self::Api(value)
    }
}
impl From<crate::nodes::imule::ImuleNodesError> for AppError {
    fn from(value: crate::nodes::imule::ImuleNodesError) -> Self {
        Self::Nodes(value)
    }
}
impl From<crate::kad::bootstrap::BootstrapError> for AppError {
    fn from(value: crate::kad::bootstrap::BootstrapError) -> Self {
        Self::Bootstrap(value)
    }
}
impl From<crate::kad::service::KadServiceError> for AppError {
    fn from(value: crate::kad::service::KadServiceError) -> Self {
        Self::KadService(value)
    }
}
impl From<crate::i2p::http::HttpError> for AppError {
    fn from(value: crate::i2p::http::HttpError) -> Self {
        Self::Http(value)
    }
}
impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<std::net::AddrParseError> for AppError {
    fn from(value: std::net::AddrParseError) -> Self {
        Self::AddrParse(value)
    }
}
impl From<std::num::ParseIntError> for AppError {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::ParseInt(value)
    }
}

pub async fn run(config: Config) -> AppResult<()> {
    tracing::info!(
        event = "app_start",
        log = %config.general.log_level,
        data_dir = %config.general.data_dir,
        "starting app"
    );

    // Prevent starting two instances with the same `data/` directory (and thus the same
    // `data/sam.keys`), which causes the router to reject sessions with "duplicate destination".
    let lock_path = Path::new(&config.general.data_dir).join("rust-mule.lock");
    let _instance_lock = SingleInstanceLock::acquire(&lock_path)?;
    tracing::info!(
        event = "instance_lock_acquired",
        path = %lock_path.display(),
        "instance lock acquired"
    );

    // Load or create aMule/iMule-compatible KadID.
    let prefs_path =
        std::path::Path::new(&config.general.data_dir).join(&config.kad.preferences_kad_path);
    let kad_prefs = crate::kad::load_or_create_preferences_kad(&prefs_path).await?;
    tracing::info!(
        event = "kad_identity_ready",
        kad_id = %crate::logging::redact_hex(&kad_prefs.kad_id.to_hex_lower()),
        prefs = %prefs_path.display(),
        "Loaded Kademlia identity"
    );

    // Load or create a persistent Kad UDP obfuscation secret, iMule-style.
    //
    // This is intentionally not user-configurable (to prevent low-entropy values).
    if let Some(v) = config.kad.deprecated_udp_key_secret
        && v != 0
    {
        tracing::warn!(
            configured = v,
            "config option kad.udp_key_secret is deprecated and ignored; delete it from config.toml"
        );
    }
    let udp_key_path =
        std::path::Path::new(&config.general.data_dir).join("kad_udp_key_secret.dat");
    let udp_key_secret = crate::kad::udp_key::load_or_create_udp_key_secret(&udp_key_path).await?;
    tracing::info!(
        path = %udp_key_path.display(),
        "Loaded/generated Kad UDP key secret"
    );

    tracing::info!("Loading SAM keys");

    tracing::info!("testing i2p + SAM connectivity");
    let mut sam: SamClient = SamClient::connect(&config.sam.host, config.sam.port)
        .await?
        .with_timeout(Duration::from_secs(config.sam.control_timeout_secs));
    let reply = sam.hello("3.0", "3.3").await?;
    tracing::debug!(reply = %reply.raw, "SAM HELLO reply");

    // Load SAM destination keys.
    //
    // Canonical location is `data/sam.keys` (under `general.data_dir`) so we don't commit secrets
    // in `config.toml`.
    let sam_keys_path = Path::new(&config.general.data_dir).join("sam.keys");
    tracing::info!(path = %sam_keys_path.display(), "SAM keys file");
    let sam_keys: Option<SamKeys> = SamKeys::load(&sam_keys_path).await?;

    // Ensure we have a long-lived destination.
    let base_session_name = &config.sam.session_name;
    let keys: SamKeys = if let Some(k) = sam_keys {
        k
    } else {
        tracing::info!("No SAM destination in config; generating new identity");

        let (generated_priv, generated_pub) = sam.dest_generate().await?;
        tracing::info!(
            priv_len = generated_priv.len(),
            pub_len = generated_pub.len(),
            "DEST generated"
        );

        let keys = SamKeys {
            pub_key: generated_pub,
            priv_key: generated_priv,
        };
        SamKeys::store(&sam_keys_path, &keys).await?;
        tracing::info!(path = %sam_keys_path.display(), "saved SAM keys");
        keys
    };

    tracing::info!(
        priv_len = keys.priv_key.trim().len(),
        pub_len = keys.pub_key.trim().len(),
        path = %sam_keys_path.display(),
        "SAM keys ready"
    );

    let my_dest_bytes = crate::i2p::b64::decode(&keys.pub_key)?;
    if my_dest_bytes.len() != crate::kad::wire::I2P_DEST_LEN {
        return Err(AppError::InvalidState(format!(
            "decoded SAM PUB key has wrong length: {} bytes (expected {})",
            my_dest_bytes.len(),
            crate::kad::wire::I2P_DEST_LEN
        )));
    }
    let mut hash_bytes = [0u8; 4];
    hash_bytes.copy_from_slice(&my_dest_bytes[..4]);
    let my_dest_hash: u32 = u32::from_le_bytes(hash_bytes);
    let mut my_dest = [0u8; crate::kad::wire::I2P_DEST_LEN];
    my_dest.copy_from_slice(&my_dest_bytes);

    let kad_session_id = format!("{base_session_name}-kad");

    // KAD-over-I2P uses SAM `STYLE=DATAGRAM` sessions. We support both UDP-forwarding and
    // iMule-style TCP datagrams.
    let mut kad_sock: SamKadSocket =
        create_kad_socket(&config, &mut sam, &kad_session_id, &keys).await?;

    let data_dir = Path::new(&config.general.data_dir);

    // Command channel used by the (future) GUI/API to instruct the Kad service (search/publish/etc).
    let (kad_cmd_tx, kad_cmd_rx) = mpsc::channel(128);

    // Local HTTP API (REST + SSE) for control plane and future GUI.
    //
    // This is always on and requires a bearer token stored under `data/`.
    let token_path = data_dir.join("api.token");
    let token = crate::api::token::load_or_create_token(&token_path).await?;
    tracing::info!(path = %token_path.display(), "api token ready");

    let (stx, etx) = crate::api::new_channels();
    let srx = stx.subscribe();
    let api_cfg = config.api.clone();
    let api_port = api_cfg.port;
    tracing::info!(
        event = "ui_available",
        "rust-mule UI available at: http://localhost:{}",
        api_port
    );
    let etx_for_server = etx.clone();
    let cmd_tx_for_server = kad_cmd_tx.clone();
    let api_runtime_config = config.clone();
    let token_path_for_server = token_path.clone();
    tokio::spawn(async move {
        if let Err(err) = crate::api::serve(
            &api_cfg,
            api_runtime_config,
            token_path_for_server,
            token,
            srx,
            etx_for_server,
            cmd_tx_for_server,
        )
        .await
        {
            tracing::error!(error = %err, "api server stopped");
        }
    });
    maybe_auto_open_ui(config.general.auto_open_ui, api_port, token_path.clone());

    let status_tx: Option<watch::Sender<Option<crate::kad::service::KadServiceStatus>>> = Some(stx);
    let status_events_tx: Option<broadcast::Sender<crate::kad::service::KadServiceStatus>> =
        Some(etx);

    let nodes_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let preferred_nodes_path = nodes_path.clone();

    // Seed files under `data/`.
    //
    // For distributable builds we embed an initseed snapshot in `assets/` (via `include_bytes!`)
    // so we don't depend on repo-local reference folders like `source_ref/` at runtime.
    let initseed_path = data_dir.join("nodes.initseed.dat");
    let fallback_path = data_dir.join("nodes.fallback.dat");
    ensure_nodes_seed_files(&initseed_path, &fallback_path).await;

    // Pick which file we load from (and log why).
    let (nodes_path, nodes_source) =
        pick_nodes_dat(&preferred_nodes_path, &initseed_path, &fallback_path);
    let mut nodes = crate::nodes::imule::nodes_dat_contacts(&nodes_path).await?;
    tracing::info!(
        path = %nodes_path.display(),
        source = nodes_source,
        count = nodes.len(),
        "loaded nodes.dat"
    );

    // If our persisted `data/nodes.dat` ever shrinks to a tiny set (e.g. after a long eviction
    // cycle), re-seed it from repo-bundled reference nodes to avoid getting stuck with too few
    // bootstrap candidates on the next run.
    if nodes_path == preferred_nodes_path && nodes.len() < 50 {
        let original_count = nodes.len();
        let mut merged = BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();
        for n in nodes.drain(..) {
            merged.insert(n.udp_dest_b64(), n);
        }

        for p in [&initseed_path, &fallback_path] {
            if !p.exists() {
                continue;
            }
            if let Ok(extra) = crate::nodes::imule::nodes_dat_contacts(p).await {
                for n in extra {
                    merged.entry(n.udp_dest_b64()).or_insert(n);
                }
            }
        }

        let mut out_nodes: Vec<_> = merged.into_values().collect();
        out_nodes.sort_by_key(|n| {
            (
                std::cmp::Reverse(n.verified),
                std::cmp::Reverse(n.kad_version),
            )
        });
        out_nodes.truncate(config.kad.service_max_persist_nodes.max(2000));

        if out_nodes.len() >= 50 && out_nodes.len() > original_count {
            tracing::warn!(
                from = original_count,
                to = out_nodes.len(),
                path = %preferred_nodes_path.display(),
                "nodes.dat seed pool was small; re-seeded from initseed/fallback"
            );
            let _ =
                crate::nodes::imule::persist_nodes_dat_v2(&preferred_nodes_path, &out_nodes).await;
            nodes = out_nodes;
        } else {
            nodes = out_nodes;
        }
    }

    // If we are not using the persisted `data/nodes.dat`, try to refresh over I2P.
    // iMule defaults to `http://www.imule.i2p/nodes2.dat`.
    if nodes_source != "primary" && nodes_source != "custom" {
        match try_download_nodes2_dat(
            &mut sam,
            &config.sam.host,
            config.sam.port,
            base_session_name,
            &keys.priv_key,
            &preferred_nodes_path,
        )
        .await
        {
            Ok(downloaded) => nodes = downloaded,
            Err(err) => {
                tracing::warn!(error = %err, "failed to download nodes2.dat over I2P; continuing with bundled nodes.dat")
            }
        }
    }

    let outcome = crate::kad::bootstrap::bootstrap(
        &mut kad_sock,
        &nodes,
        crate::kad::bootstrap::BootstrapCrypto {
            my_kad_id: kad_prefs.kad_id,
            my_dest_hash,
            udp_key_secret,
            my_dest,
        },
        Default::default(),
    )
    .await?;

    // Merge known nodes + discovered peers, persist once, and optionally continue in a long-lived
    // service loop (crawler + routing table).
    let out_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let mut merged = BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();

    for n in nodes.into_iter() {
        merged.insert(n.udp_dest_b64(), n);
    }

    for n in outcome.discovered {
        let k = n.udp_dest_b64();
        merged
            .entry(k)
            .and_modify(|old| {
                // Prefer newer/verified entries, and keep UDP keys if we learn them.
                if n.kad_version > old.kad_version {
                    old.kad_version = n.kad_version;
                }
                if n.verified {
                    old.verified = true;
                }
                if old.udp_key == 0 && n.udp_key != 0 {
                    old.udp_key = n.udp_key;
                    old.udp_key_ip = n.udp_key_ip;
                }
                if old.client_id == [0u8; 16] && n.client_id != [0u8; 16] {
                    old.client_id = n.client_id;
                }
            })
            .or_insert(n);
    }

    let mut out_nodes: Vec<crate::nodes::imule::ImuleNode> = merged.into_values().collect();
    out_nodes.sort_by_key(|n| {
        (
            std::cmp::Reverse(n.verified),
            std::cmp::Reverse(n.kad_version),
        )
    });
    out_nodes.truncate(config.kad.service_max_persist_nodes.max(2000));
    if let Err(err) = crate::nodes::imule::persist_nodes_dat_v2(&out_path, &out_nodes).await {
        tracing::warn!(
            error = %err,
            path = %out_path.display(),
            "failed to persist refreshed nodes.dat"
        );
    } else {
        tracing::info!(
            path = %out_path.display(),
            count = out_nodes.len(),
            "persisted refreshed nodes.dat"
        );
    }

    if config.kad.service_enabled {
        let mut svc = crate::kad::service::KadService::new(kad_prefs.kad_id, kad_cmd_rx);
        let crypto = crate::kad::service::KadServiceCrypto {
            my_kad_id: kad_prefs.kad_id,
            my_dest_hash,
            udp_key_secret,
            my_dest,
        };
        let svc_cfg = crate::kad::service::KadServiceConfig {
            runtime_secs: config.kad.service_runtime_secs,
            crawl_every_secs: config.kad.service_crawl_every_secs,
            persist_every_secs: config.kad.service_persist_every_secs,
            alpha: config.kad.service_alpha,
            req_contacts: config.kad.service_req_contacts,
            max_persist_nodes: config.kad.service_max_persist_nodes,
            req_timeout_secs: config.kad.service_req_timeout_secs,
            req_min_interval_secs: config.kad.service_req_min_interval_secs,
            bootstrap_every_secs: config.kad.service_bootstrap_every_secs,
            bootstrap_batch: config.kad.service_bootstrap_batch,
            bootstrap_min_interval_secs: config.kad.service_bootstrap_min_interval_secs,
            hello_every_secs: config.kad.service_hello_every_secs,
            hello_batch: config.kad.service_hello_batch,
            hello_min_interval_secs: config.kad.service_hello_min_interval_secs,
            hello_dual_obfuscated: config.kad.service_hello_dual_obfuscated,
            maintenance_every_secs: config.kad.service_maintenance_every_secs,
            status_every_secs: config.kad.service_status_every_secs,
            max_failures: config.kad.service_max_failures,
            evict_age_secs: config.kad.service_evict_age_secs,

            refresh_interval_secs: config.kad.service_refresh_interval_secs,
            refresh_buckets_per_tick: config.kad.service_refresh_buckets_per_tick,
            refresh_underpopulated_min_contacts: config
                .kad
                .service_refresh_underpopulated_min_contacts,
            refresh_underpopulated_every_secs: config.kad.service_refresh_underpopulated_every_secs,
            refresh_underpopulated_buckets_per_tick: config
                .kad
                .service_refresh_underpopulated_buckets_per_tick,
            refresh_underpopulated_alpha: config.kad.service_refresh_underpopulated_alpha,

            keyword_require_interest: config.kad.service_keyword_require_interest,
            keyword_interest_ttl_secs: config.kad.service_keyword_interest_ttl_secs,
            keyword_results_ttl_secs: config.kad.service_keyword_results_ttl_secs,
            keyword_max_keywords: config.kad.service_keyword_max_keywords,
            keyword_max_total_hits: config.kad.service_keyword_max_total_hits,
            keyword_max_hits_per_keyword: config.kad.service_keyword_max_hits_per_keyword,

            store_keyword_max_keywords: config.kad.service_store_keyword_max_keywords,
            store_keyword_max_total_hits: config.kad.service_store_keyword_max_total_hits,
            store_keyword_evict_age_secs: config.kad.service_store_keyword_evict_age_secs,
        };

        let mut seed_once = Some(out_nodes);
        let mut backoff = Duration::from_secs(1);
        loop {
            let seed = seed_once.take().unwrap_or_default();
            let res = crate::kad::service::run_service(
                &mut svc,
                &mut kad_sock,
                seed,
                crypto,
                svc_cfg.clone(),
                &out_path,
                status_tx.clone(),
                status_events_tx.clone(),
            )
            .await;

            match res {
                Ok(()) => break,
                Err(err) if is_recoverable_sam_kad_error(&err) => {
                    tracing::warn!(
                        error = %err,
                        backoff_secs = backoff.as_secs(),
                        "kad service lost SAM socket; recreating session and continuing"
                    );

                    // Best-effort: persist current routing snapshot before reconnecting.
                    let snap = svc
                        .routing()
                        .snapshot_nodes(config.kad.service_max_persist_nodes);
                    let _ = crate::nodes::imule::persist_nodes_dat_v2(&out_path, &snap).await;

                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));

                    kad_sock = create_kad_socket(&config, &mut sam, &kad_session_id, &keys).await?;
                    continue;
                }
                Err(err) => return Err(AppError::KadService(err)),
            }
        }
    }

    tracing::info!("shutting down gracefully");
    Ok(())
}

fn maybe_auto_open_ui(enabled: bool, port: u16, token_path: PathBuf) {
    if !enabled {
        tracing::info!("UI auto-open disabled by config (general.auto_open_ui=false)");
        return;
    }

    tokio::spawn(async move {
        let url = format!("http://localhost:{port}/index.html");
        let ready = wait_for_ui_bootstrap(port, &token_path, Duration::from_secs(30)).await;
        if !ready {
            tracing::warn!(
                port,
                path = %token_path.display(),
                "UI auto-open skipped: API/UI/token did not become ready before timeout"
            );
            return;
        }
        if let Err(err) = open_url_in_default_browser(&url).await {
            tracing::warn!(url = %url, error = %err, "failed to auto-open UI in browser");
            return;
        }
        tracing::info!(url = %url, "opened UI in default browser");
    });
}

async fn wait_for_ui_bootstrap(port: u16, token_path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !token_path.exists() {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }

        let api_ok = http_get_status(port, "/api/v1/health")
            .await
            .is_some_and(|s| s == 200);
        let ui_ok = http_get_status(port, "/index.html")
            .await
            .is_some_and(|s| s == 200);
        if api_ok && ui_ok {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    false
}

async fn http_get_status(port: u16, path: &str) -> Option<u16> {
    let mut stream = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.ok()?;

    let mut buf = [0u8; 256];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .ok()?
        .ok()?;
    let line_end = buf[..n].iter().position(|b| *b == b'\n')?;
    let line = std::str::from_utf8(&buf[..line_end]).ok()?.trim();
    let mut parts = line.split_whitespace();
    let _http = parts.next()?;
    parts.next()?.parse::<u16>().ok()
}

async fn open_url_in_default_browser(url: &str) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    let status = tokio::process::Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg(url)
        .status()
        .await?;

    #[cfg(target_os = "macos")]
    let status = tokio::process::Command::new("open")
        .arg(url)
        .status()
        .await?;

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = tokio::process::Command::new("xdg-open")
        .arg(url)
        .status()
        .await?;

    if !status.success() {
        return Err(AppError::BrowserOpenFailed);
    }
    Ok(())
}

async fn try_download_nodes2_dat(
    sam: &mut SamClient,
    sam_host: &str,
    sam_port: u16,
    base_session_name: &str,
    _priv_key: &str,
    out: &Path,
) -> AppResult<Vec<crate::nodes::imule::ImuleNode>> {
    let url = "http://www.imule.i2p/nodes2.dat";
    let (host, port, path) = parse_http_url(url)?;
    if port != 80 {
        return Err(AppError::InvalidState(format!(
            "only port 80 is supported for now (got {port})"
        )));
    }

    // Resolve eepsite hostname to destination.
    let dest = match sam.naming_lookup(host).await {
        Ok(d) => d,
        Err(err) if host.starts_with("www.") => {
            let alt = &host["www.".len()..];
            tracing::warn!(
                host,
                alt,
                error = %err,
                "NAMING LOOKUP failed; trying without 'www.'"
            );
            sam.naming_lookup(alt).await?
        }
        Err(err) => return Err(AppError::Sam(err)),
    };

    // Ensure a STREAM session exists for outgoing HTTP.
    //
    // NOTE: SAM typically forbids attaching the same Destination to multiple sessions. Since we
    // already use our configured Destination for the KAD datagram session, create a temporary
    // Destination just for this download (iMule uses separate tcp/udp privkeys as well).
    let stream_id = format!("{base_session_name}-stream");
    let (stream_priv, _stream_pub) = sam.dest_generate().await?;

    let _ = sam.session_destroy(&stream_id).await;
    sam.session_create(
        "STREAM",
        &stream_id,
        &stream_priv,
        ["i2cp.messageReliability=BestEffort"],
    )
    .await?;

    let stream = crate::i2p::sam::SamStream::connect(sam_host, sam_port, &stream_id, &dest).await?;
    let resp =
        crate::i2p::http::http_get_bytes(stream, host, path, Duration::from_secs(60)).await?;

    if resp.status != 200 {
        return Err(AppError::InvalidState(format!(
            "nodes2.dat download returned HTTP {}",
            resp.status
        )));
    }

    // Persist to configured bootstrap path for reproducible future runs.
    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = out.with_extension("tmp");
    tokio::fs::write(&tmp, &resp.body).await?;
    tokio::fs::rename(&tmp, &out).await?;

    let nodes = crate::nodes::imule::parse_nodes_dat(&resp.body)?;

    tracing::info!(url, path = %out.display(), count = nodes.len(), "downloaded fresh nodes.dat over I2P");
    let _ = sam.session_destroy(&stream_id).await;
    Ok(nodes)
}

fn parse_http_url(url: &str) -> AppResult<(&str, u16, &str)> {
    let url = url.strip_prefix("http://").ok_or_else(|| {
        AppError::InvalidState(format!("only http:// URLs are supported (got '{url}')"))
    })?;

    let (host_port, path) = match url.find('/') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, "/"),
    };

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>()?)
    } else {
        (host_port, 80)
    };
    Ok((host, port, path))
}

fn resolve_path(data_dir: &str, p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    Path::new(data_dir).join(path)
}

fn pick_nodes_dat(preferred: &Path, initseed: &Path, fallback: &Path) -> (PathBuf, &'static str) {
    if preferred.exists() {
        return (preferred.to_path_buf(), "primary");
    }

    // Only fall back automatically for the default `data/nodes.dat` case.
    let preferred_name = preferred.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if preferred_name != "nodes.dat" {
        return (preferred.to_path_buf(), "custom");
    }

    if initseed.exists() {
        return (initseed.to_path_buf(), "initseed");
    }
    if fallback.exists() {
        return (fallback.to_path_buf(), "fallback");
    }

    (preferred.to_path_buf(), "missing")
}

async fn create_kad_socket(
    config: &Config,
    sam: &mut SamClient,
    kad_session_id: &str,
    keys: &SamKeys,
) -> AppResult<SamKadSocket> {
    Ok(match config.sam.datagram_transport {
        SamDatagramTransport::Tcp => {
            let mut dg = SamDatagramTcp::connect(&config.sam.host, config.sam.port)
                .await?
                .with_timeout(Duration::from_secs(config.sam.control_timeout_secs));
            dg.hello("3.0", "3.3").await?;

            let mut attempt = 0u32;
            let mut backoff = Duration::from_secs(1);
            loop {
                attempt += 1;
                let res = dg
                    .session_create_datagram(
                        kad_session_id,
                        &keys.priv_key,
                        ["i2cp.messageReliability=BestEffort"],
                    )
                    .await;

                match res {
                    Ok(_) => break,
                    Err(err) => {
                        // Recoverable-ish: the session already exists (common after a reconnect).
                        let is_dup_id = matches!(
                            &err,
                            crate::i2p::sam::SamError::ReplyError {
                                result: Some(r),
                                ..
                            } if r == "DUPLICATED_ID"
                        );
                        let is_dup_msg = err.to_string().contains("Session already exists")
                            || err.to_string().contains("Duplicate");

                        if is_dup_id || is_dup_msg {
                            tracing::warn!(
                                session = %kad_session_id,
                                "SAM session exists; destroying and retrying"
                            );
                            let _ = dg.session_destroy(kad_session_id).await;
                        } else if err.to_string().contains("duplicate destination")
                            || err.to_string().contains("Failed to build tunnels")
                            || err.to_string().contains("Disconnected from router")
                        {
                            tracing::warn!(
                                session = %kad_session_id,
                                attempt,
                                backoff_secs = backoff.as_secs(),
                                error = %err,
                                "SAM session creation failed; retrying after backoff"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                        } else {
                            return Err(AppError::Sam(err));
                        }

                        if attempt >= 8 {
                            return Err(AppError::Sam(err));
                        }
                        continue;
                    }
                }
            }

            tracing::info!(
                session = %kad_session_id,
                transport = "tcp",
                "SAM DATAGRAM session ready"
            );
            SamKadSocket::Tcp(dg)
        }

        SamDatagramTransport::UdpForward => {
            let sam_host_ip: IpAddr = config.sam.host.parse()?;
            let sam_forward_ip: IpAddr = config.sam.forward_host.parse()?;

            if sam_forward_ip.is_loopback() && !sam_host_ip.is_loopback() {
                tracing::warn!(
                    sam_host = %sam_host_ip,
                    forward_host = %sam_forward_ip,
                    "sam.forward_host is loopback but the SAM bridge is remote; UDP forwarding will not reach this process"
                );
            }
            if config.sam.forward_port == 0 && sam_host_ip != sam_forward_ip {
                tracing::warn!(
                    sam_host = %sam_host_ip,
                    forward_host = %sam_forward_ip,
                    "sam.forward_port=0 (ephemeral) with remote SAM bridge; consider setting a fixed forward_port and opening/mapping it for UDP"
                );
            }

            let bind_ip = if sam_forward_ip.is_loopback() {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            } else {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            };

            let dg: SamDatagramSocket = SamDatagramSocket::bind_for_forwarding(
                kad_session_id.to_string(),
                sam_host_ip,
                config.sam.udp_port,
                bind_ip,
                config.sam.forward_port,
            )
            .await?;

            // Create SAM datagram session (idempotent-ish).
            let mut attempt = 0u32;
            let mut backoff = Duration::from_secs(1);
            loop {
                attempt += 1;
                let res = sam
                    .session_create_datagram_forward(
                        kad_session_id,
                        &keys.priv_key,
                        dg.forward_port()?,
                        sam_forward_ip,
                        ["i2cp.messageReliability=BestEffort"],
                    )
                    .await;

                match res {
                    Ok(_) => break,
                    Err(err) => {
                        let is_dup_id = matches!(
                            &err,
                            crate::i2p::sam::SamError::ReplyError {
                                result: Some(r),
                                ..
                            } if r == "DUPLICATED_ID"
                        );
                        let is_dup_msg = err.to_string().contains("Session already exists")
                            || err.to_string().contains("Duplicate");

                        if is_dup_id || is_dup_msg {
                            tracing::warn!(
                                session = %kad_session_id,
                                "SAM session exists; destroying and retrying"
                            );
                            let _ = sam.session_destroy(kad_session_id).await;
                        } else if err.to_string().contains("duplicate destination")
                            || err.to_string().contains("Failed to build tunnels")
                            || err.to_string().contains("Disconnected from router")
                        {
                            tracing::warn!(
                                session = %kad_session_id,
                                attempt,
                                backoff_secs = backoff.as_secs(),
                                error = %err,
                                "SAM session creation failed; retrying after backoff"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                        } else {
                            return Err(AppError::Sam(err));
                        }

                        if attempt >= 8 {
                            return Err(AppError::Sam(err));
                        }
                        continue;
                    }
                }
            }

            tracing::info!(
                session = %kad_session_id,
                transport = "udp_forward",
                forward = %dg.forward_addr()?,
                sam_udp = %dg.sam_udp_addr(),
                "SAM DATAGRAM session ready"
            );
            SamKadSocket::UdpForward(dg)
        }
    })
}

fn is_recoverable_sam_kad_error(err: &crate::kad::service::KadServiceError) -> bool {
    // We treat SAM socket loss / framing desync as recoverable: re-create the session and keep
    // the Kad service running.
    //
    // Prefer typed SAM errors when available, but keep a defensive string fallback since
    // not every path has been migrated yet.
    if let crate::kad::service::KadServiceError::Sam(sam) = err {
        return matches!(
            sam,
            SamError::Closed
                | SamError::Timeout { .. }
                | SamError::Io { .. }
                | SamError::BadFrame { .. }
                | SamError::FramingDesync { .. }
        );
    }

    false
}

async fn ensure_nodes_seed_files(initseed: &Path, fallback: &Path) {
    // We intentionally keep this best-effort: failures should not prevent startup.
    let _ = tokio::fs::create_dir_all(initseed.parent().unwrap_or_else(|| Path::new("data"))).await;

    // Bundled seed snapshot for first-run bootstrapping.
    // This is embedded in the binary so distributed builds don't require extra files.
    const INITSEED_BYTES: &[u8] = include_bytes!("../assets/nodes.initseed.dat");

    if !initseed.exists() {
        let tmp = initseed.with_extension("tmp");
        if tokio::fs::write(&tmp, INITSEED_BYTES).await.is_ok()
            && tokio::fs::rename(&tmp, initseed).await.is_ok()
        {
            tracing::info!(
                dst = %initseed.display(),
                bytes = INITSEED_BYTES.len(),
                "created nodes.initseed.dat from embedded initseed"
            );
        }
    }

    if !fallback.exists() {
        // For now, fallback is just a copy of initseed. We can evolve this later into a
        // "last-known-good" snapshot if desired.
        let src_bytes = if initseed.exists() {
            tokio::fs::read(initseed).await.ok()
        } else {
            Some(INITSEED_BYTES.to_vec())
        };

        if let Some(bytes) = src_bytes {
            let tmp = fallback.with_extension("tmp");
            if tokio::fs::write(&tmp, &bytes).await.is_ok()
                && tokio::fs::rename(&tmp, fallback).await.is_ok()
            {
                tracing::info!(
                    dst = %fallback.display(),
                    bytes = bytes.len(),
                    "created nodes.fallback.dat from initseed"
                );
            }
        }
    }
}
