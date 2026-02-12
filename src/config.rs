use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use tracing_subscriber::EnvFilter;

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug)]
pub enum ConfigError {
    SerializeToml(toml::ser::Error),
    WriteTemp(std::io::Error),
    Rename(std::io::Error),
    InvalidApiHost {
        host: String,
        source: std::net::AddrParseError,
    },
    NonLoopbackApiHost {
        host: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializeToml(_) => write!(f, "failed to serialize config to TOML"),
            Self::WriteTemp(_) => write!(f, "failed writing temporary config file"),
            Self::Rename(_) => write!(f, "failed replacing config file"),
            Self::InvalidApiHost { host, .. } => write!(f, "Invalid api.host '{}'", host),
            Self::NonLoopbackApiHost { host } => write!(
                f,
                "Invalid api.host '{}': only loopback hosts are allowed (localhost/127.0.0.1/::1)",
                host
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerializeToml(source) => Some(source),
            Self::WriteTemp(source) => Some(source),
            Self::Rename(source) => Some(source),
            Self::InvalidApiHost { source, .. } => Some(source),
            Self::NonLoopbackApiHost { .. } => None,
        }
    }
}

fn default_sam_host() -> String {
    "127.0.0.1".to_string()
}
fn default_sam_port() -> u16 {
    7656
}
fn default_sam_udp_port() -> u16 {
    7655
}
fn default_session_name() -> String {
    "rust-mule".to_string()
}
fn default_forward_host() -> String {
    "127.0.0.1".to_string()
}
fn default_forward_port() -> u16 {
    // 0 = let OS choose an ephemeral port. If SAM is remote and a firewall is in the way,
    // you'll likely want to set this to a fixed port and allow inbound UDP from the SAM host.
    0
}
fn default_sam_control_timeout_secs() -> u64 {
    // I2P session creation can take a while on some routers (lease set publish, tunnel build).
    120
}
fn default_sam_datagram_transport() -> SamDatagramTransport {
    SamDatagramTransport::Tcp
}
fn default_log_level() -> String {
    // Keep stdout reasonably quiet by default; file logging can stay verbose.
    "info".to_string()
}
fn default_data_dir() -> String {
    "data".to_string()
}
fn default_log_to_file() -> bool {
    true
}
fn default_log_file_name() -> String {
    "rust-mule.log".to_string()
}
fn default_log_file_level() -> String {
    "debug".to_string()
}
fn default_auto_open_ui() -> bool {
    true
}
fn default_api_host() -> String {
    "127.0.0.1".to_string()
}
fn default_api_port() -> u16 {
    17835
}
fn default_preferences_kad_path() -> String {
    // Keep aMule/iMule naming for backwards compatibility.
    "preferencesKad.dat".to_string()
}
fn default_kad_service_enabled() -> bool {
    true
}
fn default_kad_service_runtime_secs() -> u64 {
    0
}
fn default_kad_service_crawl_every_secs() -> u64 {
    3
}
fn default_kad_service_persist_every_secs() -> u64 {
    300
}
fn default_kad_service_alpha() -> usize {
    3
}
fn default_kad_service_req_contacts() -> u8 {
    // Kad2 `KADEMLIA2_REQ` encodes the requested number of contacts in the low 5 bits (1..=31).
    // If we set 32 here, it would wrap to 0 and then be rejected/normalized to 1 by our encoder.
    31
}
fn default_kad_service_max_persist_nodes() -> usize {
    5000
}
fn default_kad_service_req_timeout_secs() -> u64 {
    45
}
fn default_kad_service_req_min_interval_secs() -> u64 {
    15
}
fn default_kad_service_bootstrap_every_secs() -> u64 {
    // Conservative refresh cadence: the iMule I2P-KAD network is small.
    30 * 60
}
fn default_kad_service_bootstrap_batch() -> usize {
    1
}
fn default_kad_service_bootstrap_min_interval_secs() -> u64 {
    // Avoid repeatedly poking the same peer.
    6 * 60 * 60
}
fn default_kad_service_hello_every_secs() -> u64 {
    10
}
fn default_kad_service_hello_batch() -> usize {
    2
}
fn default_kad_service_hello_min_interval_secs() -> u64 {
    900
}
fn default_kad_service_hello_dual_obfuscated() -> bool {
    false
}
fn default_kad_service_maintenance_every_secs() -> u64 {
    5
}
fn default_kad_service_status_every_secs() -> u64 {
    60
}
fn default_kad_service_max_failures() -> u32 {
    5
}
fn default_kad_service_evict_age_secs() -> u64 {
    // I2P peers can be very intermittent; avoid aggressive eviction by default.
    24 * 60 * 60
}
fn default_kad_service_refresh_interval_secs() -> u64 {
    45 * 60
}
fn default_kad_service_refresh_buckets_per_tick() -> usize {
    1
}
fn default_kad_service_refresh_underpopulated_min_contacts() -> usize {
    60
}
fn default_kad_service_refresh_underpopulated_every_secs() -> u64 {
    60
}
fn default_kad_service_refresh_underpopulated_buckets_per_tick() -> usize {
    2
}
fn default_kad_service_refresh_underpopulated_alpha() -> usize {
    5
}

fn default_kad_service_keyword_require_interest() -> bool {
    // Only keep keyword results for searches we initiated (prevents unsolicited cache growth).
    true
}
fn default_kad_service_keyword_interest_ttl_secs() -> u64 {
    // Keep keywords "active" for a day after a search or results read.
    24 * 60 * 60
}
fn default_kad_service_keyword_results_ttl_secs() -> u64 {
    // Keyword results are ephemeral; drop entries we haven't re-seen after a day.
    24 * 60 * 60
}
fn default_kad_service_keyword_max_keywords() -> usize {
    64
}
fn default_kad_service_keyword_max_total_hits() -> usize {
    50_000
}
fn default_kad_service_keyword_max_hits_per_keyword() -> usize {
    2_000
}

fn default_kad_service_store_keyword_max_keywords() -> usize {
    // How many distinct keyword IDs we are willing to store entries for (DHT role).
    1024
}
fn default_kad_service_store_keyword_max_total_hits() -> usize {
    // Global hard cap for stored keyword->file entries.
    200_000
}
fn default_kad_service_store_keyword_evict_age_secs() -> u64 {
    // Stored keyword entries are intermittent on I2P; keep them around for a while.
    14 * 24 * 60 * 60
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sam: SamConfig,
    pub kad: KadConfig,
    pub general: GeneralConfig,
    pub api: ApiConfig,
}

impl Config {
    pub async fn persist(&self) -> Result<()> {
        let path = "config.toml";
        let tmp_path = format!("{}.tmp", path);
        let toml = toml::to_string_pretty(self).map_err(ConfigError::SerializeToml)?;

        tokio::fs::write(&tmp_path, toml)
            .await
            .map_err(ConfigError::WriteTemp)?;
        tokio::fs::rename(&tmp_path, path)
            .await
            .map_err(ConfigError::Rename)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SamConfig {
    pub host: String,
    pub port: u16,
    pub udp_port: u16,
    pub session_name: String,
    pub datagram_transport: SamDatagramTransport,
    /// Where the SAM bridge should forward inbound DATAGRAM/RAW UDP packets.
    /// Must be reachable from the SAM host (often `127.0.0.1` if SAM is local).
    pub forward_host: String,
    /// UDP port to bind locally for SAM UDP forwarding. `0` means "pick any free port".
    pub forward_port: u16,
    /// Timeout for SAM TCP control-channel read/write operations.
    pub control_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamDatagramTransport {
    /// Create `STYLE=DATAGRAM` with `HOST`+`PORT`, then send/receive over the SAM UDP port.
    UdpForward,
    /// Create `STYLE=DATAGRAM` without forwarding, then send/receive via `DATAGRAM SEND` and
    /// `DATAGRAM RECEIVED` frames on the SAM TCP socket (iMule-style).
    Tcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KadConfig {
    pub bootstrap_nodes_path: String,
    pub udp_port: u16,
    pub preferences_kad_path: String,
    /// Deprecated: `kad.udp_key_secret` used to be configurable.
    ///
    /// We now always load/store the secret in `data/kad_udp_key_secret.dat` to avoid users
    /// accidentally setting low-entropy values which degrade interoperability.
    #[serde(default, rename = "udp_key_secret", skip_serializing)]
    pub deprecated_udp_key_secret: Option<u32>,

    /// Run the long-lived Kad service loop after the initial bootstrap.
    pub service_enabled: bool,
    /// If 0, run until Ctrl-C.
    pub service_runtime_secs: u64,
    pub service_crawl_every_secs: u64,
    pub service_persist_every_secs: u64,
    pub service_alpha: usize,
    pub service_req_contacts: u8,
    pub service_max_persist_nodes: usize,

    pub service_req_timeout_secs: u64,
    pub service_req_min_interval_secs: u64,

    pub service_bootstrap_every_secs: u64,
    pub service_bootstrap_batch: usize,
    pub service_bootstrap_min_interval_secs: u64,

    pub service_hello_every_secs: u64,
    pub service_hello_batch: usize,
    pub service_hello_min_interval_secs: u64,
    /// Optional: send a second obfuscated HELLO_REQ if we already have a receiver key.
    /// This diverges from iMule's plain-HELLO behavior and is experimental.
    pub service_hello_dual_obfuscated: bool,

    pub service_maintenance_every_secs: u64,
    pub service_status_every_secs: u64,
    pub service_max_failures: u32,
    pub service_evict_age_secs: u64,

    pub service_refresh_interval_secs: u64,
    pub service_refresh_buckets_per_tick: usize,
    pub service_refresh_underpopulated_min_contacts: usize,
    pub service_refresh_underpopulated_every_secs: u64,
    pub service_refresh_underpopulated_buckets_per_tick: usize,
    pub service_refresh_underpopulated_alpha: usize,

    // Keyword search result caching (in-memory)
    pub service_keyword_require_interest: bool,
    pub service_keyword_interest_ttl_secs: u64,
    pub service_keyword_results_ttl_secs: u64,
    pub service_keyword_max_keywords: usize,
    pub service_keyword_max_total_hits: usize,
    pub service_keyword_max_hits_per_keyword: usize,

    // DHT keyword storage (when other peers publish keywords to us)
    pub service_store_keyword_max_keywords: usize,
    pub service_store_keyword_max_total_hits: usize,
    pub service_store_keyword_evict_age_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub data_dir: String,
    pub log_to_file: bool,
    pub log_file_name: String,
    pub log_file_level: String,
    pub auto_open_ui: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    /// Bind IP address (default: 127.0.0.1).
    pub host: String,
    /// Bind TCP port.
    pub port: u16,
}

impl Default for SamConfig {
    fn default() -> Self {
        Self {
            host: default_sam_host(),
            port: default_sam_port(),
            udp_port: default_sam_udp_port(),
            session_name: default_session_name(),
            datagram_transport: default_sam_datagram_transport(),
            forward_host: default_forward_host(),
            forward_port: default_forward_port(),
            control_timeout_secs: default_sam_control_timeout_secs(),
        }
    }
}

impl Default for KadConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes_path: "nodes.dat".to_string(),
            udp_port: 4665,
            preferences_kad_path: default_preferences_kad_path(),
            deprecated_udp_key_secret: None,

            service_enabled: default_kad_service_enabled(),
            service_runtime_secs: default_kad_service_runtime_secs(),
            service_crawl_every_secs: default_kad_service_crawl_every_secs(),
            service_persist_every_secs: default_kad_service_persist_every_secs(),
            service_alpha: default_kad_service_alpha(),
            service_req_contacts: default_kad_service_req_contacts(),
            service_max_persist_nodes: default_kad_service_max_persist_nodes(),

            service_req_timeout_secs: default_kad_service_req_timeout_secs(),
            service_req_min_interval_secs: default_kad_service_req_min_interval_secs(),

            service_bootstrap_every_secs: default_kad_service_bootstrap_every_secs(),
            service_bootstrap_batch: default_kad_service_bootstrap_batch(),
            service_bootstrap_min_interval_secs: default_kad_service_bootstrap_min_interval_secs(),

            service_hello_every_secs: default_kad_service_hello_every_secs(),
            service_hello_batch: default_kad_service_hello_batch(),
            service_hello_min_interval_secs: default_kad_service_hello_min_interval_secs(),
            service_hello_dual_obfuscated: default_kad_service_hello_dual_obfuscated(),

            service_maintenance_every_secs: default_kad_service_maintenance_every_secs(),
            service_status_every_secs: default_kad_service_status_every_secs(),
            service_max_failures: default_kad_service_max_failures(),
            service_evict_age_secs: default_kad_service_evict_age_secs(),

            service_refresh_interval_secs: default_kad_service_refresh_interval_secs(),
            service_refresh_buckets_per_tick: default_kad_service_refresh_buckets_per_tick(),
            service_refresh_underpopulated_min_contacts:
                default_kad_service_refresh_underpopulated_min_contacts(),
            service_refresh_underpopulated_every_secs:
                default_kad_service_refresh_underpopulated_every_secs(),
            service_refresh_underpopulated_buckets_per_tick:
                default_kad_service_refresh_underpopulated_buckets_per_tick(),
            service_refresh_underpopulated_alpha: default_kad_service_refresh_underpopulated_alpha(
            ),

            service_keyword_require_interest: default_kad_service_keyword_require_interest(),
            service_keyword_interest_ttl_secs: default_kad_service_keyword_interest_ttl_secs(),
            service_keyword_results_ttl_secs: default_kad_service_keyword_results_ttl_secs(),
            service_keyword_max_keywords: default_kad_service_keyword_max_keywords(),
            service_keyword_max_total_hits: default_kad_service_keyword_max_total_hits(),
            service_keyword_max_hits_per_keyword: default_kad_service_keyword_max_hits_per_keyword(
            ),

            service_store_keyword_max_keywords: default_kad_service_store_keyword_max_keywords(),
            service_store_keyword_max_total_hits: default_kad_service_store_keyword_max_total_hits(
            ),
            service_store_keyword_evict_age_secs: default_kad_service_store_keyword_evict_age_secs(
            ),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            data_dir: default_data_dir(),
            log_to_file: default_log_to_file(),
            log_file_name: default_log_file_name(),
            log_file_level: default_log_file_level(),
            auto_open_ui: default_auto_open_ui(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
        }
    }
}

pub fn parse_api_bind_host(host: &str) -> Result<std::net::IpAddr> {
    let host = host.trim();
    let ip = if host.eq_ignore_ascii_case("localhost") {
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    } else {
        host.parse::<std::net::IpAddr>()
            .map_err(|source| ConfigError::InvalidApiHost {
                host: host.to_string(),
                source,
            })?
    };

    if !ip.is_loopback() {
        return Err(ConfigError::NonLoopbackApiHost {
            host: host.to_string(),
        });
    }
    Ok(ip)
}

pub fn init_tracing(config: &Config) {
    // Priority order:
    // 1) RUST_LOG (standard in Rust ecosystem)
    // 2) RUST_MULE_LOG (your app-specific env)
    // 3) default (info)
    //
    // Example:
    // RUST_LOG=info,rust_mule=debug
    // RUST_MULE_LOG=debug

    use tracing_subscriber::Layer as _;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    // Stdout filter:
    // - If user sets `RUST_LOG`, respect it (standard Rust convention).
    // - Otherwise use `general.log_level`.
    let stdout_filter = std::env::var("RUST_LOG")
        .ok()
        .unwrap_or_else(|| config.general.log_level.clone());
    let stdout_filter =
        EnvFilter::try_new(stdout_filter).unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .with_filter(stdout_filter);

    if !config.general.log_to_file {
        tracing_subscriber::registry().with(stdout_layer).init();
        return;
    }

    // File filter:
    // - If user sets `RUST_MULE_LOG_FILE`, it overrides config (useful to keep stdout quiet and file verbose).
    // - Otherwise use `general.log_file_level`.
    let file_filter = std::env::var("RUST_MULE_LOG_FILE")
        .ok()
        .unwrap_or_else(|| config.general.log_file_level.clone());
    let file_filter = EnvFilter::try_new(file_filter).unwrap_or_else(|_| EnvFilter::new("debug"));

    // Keep runtime artifacts in `data/` and logs in a dedicated subdir.
    let log_dir = Path::new(&config.general.data_dir).join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let (prefix, suffix) = split_log_file_name(&config.general.log_file_name);
    cleanup_old_logs(
        &log_dir,
        &prefix,
        &suffix,
        Duration::from_secs(30 * 24 * 60 * 60),
    );
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(&prefix)
        .filename_suffix(&suffix)
        .build(&log_dir)
        .unwrap_or_else(|_| {
            tracing_appender::rolling::daily(&log_dir, &config.general.log_file_name)
        });
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    static GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
        std::sync::OnceLock::new();
    let _ = GUARD.set(guard);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .with_writer(file_writer)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

fn split_log_file_name(file_name: &str) -> (String, String) {
    let p = Path::new(file_name);
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("rust-mule");
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("log");
    (stem.to_string(), ext.to_string())
}

fn cleanup_old_logs(log_dir: &Path, prefix: &str, suffix: &str, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if !is_matching_rotated_log_file(name, prefix, suffix) {
            continue;
        }

        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let Ok(age) = std::time::SystemTime::now().duration_since(modified) else {
            continue;
        };
        if age <= max_age {
            continue;
        }
        if let Err(err) = std::fs::remove_file(&path) {
            tracing::warn!(path = %path.display(), error = %err, "failed to delete old log file");
        }
    }
}

fn is_matching_rotated_log_file(name: &str, prefix: &str, suffix: &str) -> bool {
    let prefix_marker = format!("{prefix}.");
    let suffix_marker = format!(".{suffix}");
    if !name.starts_with(&prefix_marker) || !name.ends_with(&suffix_marker) {
        return false;
    }
    let start = prefix_marker.len();
    let end = name.len().saturating_sub(suffix_marker.len());
    end > start
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nonce = format!(
            "{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        p.push(nonce);
        p
    }

    #[test]
    fn split_log_file_name_works_for_default_and_custom_ext() {
        assert_eq!(
            split_log_file_name("rust-mule.log"),
            ("rust-mule".to_string(), "log".to_string())
        );
        assert_eq!(
            split_log_file_name("custom.txt"),
            ("custom".to_string(), "txt".to_string())
        );
    }

    #[test]
    fn cleanup_old_logs_removes_only_matching_rotated_files() {
        let dir = temp_dir("rust-mule-log-cleanup");
        std::fs::create_dir_all(&dir).expect("create temp log dir");

        let old_match = dir.join("rust-mule.2026-01-01.log");
        let keep_other = dir.join("notes.txt");
        std::fs::write(&old_match, b"x").expect("write rotated log");
        std::fs::write(&keep_other, b"x").expect("write unrelated file");

        std::thread::sleep(Duration::from_millis(10));
        cleanup_old_logs(&dir, "rust-mule", "log", Duration::ZERO);

        assert!(
            !old_match.exists(),
            "matching rotated log should be removed"
        );
        assert!(keep_other.exists(), "non-matching file should be kept");

        let _ = std::fs::remove_file(&keep_other);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_matching_rotated_log_file_checks_prefix_and_suffix() {
        assert!(is_matching_rotated_log_file(
            "rust-mule.2026-02-12.log",
            "rust-mule",
            "log"
        ));
        assert!(!is_matching_rotated_log_file(
            "rust-mule.log",
            "rust-mule",
            "log"
        ));
        assert!(!is_matching_rotated_log_file(
            "other.2026-02-12.log",
            "rust-mule",
            "log"
        ));
    }
}
