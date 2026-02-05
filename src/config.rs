use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

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
    "debug".to_string()
}
fn default_data_dir() -> String {
    "data".to_string()
}
fn default_preferences_kad_path() -> String {
    // Keep aMule/iMule naming for backwards compatibility.
    "preferencesKad.dat".to_string()
}
fn default_kad_udp_key_secret() -> u32 {
    // 0 means "generate on first run".
    0
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
    32
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
fn default_kad_service_hello_every_secs() -> u64 {
    10
}
fn default_kad_service_hello_batch() -> usize {
    2
}
fn default_kad_service_hello_min_interval_secs() -> u64 {
    900
}
fn default_kad_service_maintenance_every_secs() -> u64 {
    5
}
fn default_kad_service_max_failures() -> u32 {
    5
}
fn default_kad_service_evict_age_secs() -> u64 {
    3600
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sam: SamConfig,
    pub kad: KadConfig,
    pub i2p: I2PConfig,
    pub general: GeneralConfig,
}

impl Config {
    pub async fn persist(&self) -> anyhow::Result<()> {
        let path = "config.toml";
        let tmp_path = format!("{}.tmp", path);
        let toml = toml::to_string_pretty(self)?;

        tokio::fs::write(&tmp_path, toml).await?;
        tokio::fs::rename(&tmp_path, path).await?;

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
    /// Equivalent to iMule's `thePrefs::GetKadUDPKey()`; used for UDP obfuscation verify keys.
    pub udp_key_secret: u32,

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

    pub service_hello_every_secs: u64,
    pub service_hello_batch: usize,
    pub service_hello_min_interval_secs: u64,

    pub service_maintenance_every_secs: u64,
    pub service_max_failures: u32,
    pub service_evict_age_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct I2PConfig {
    pub sam_private_key: String,
    pub sam_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub data_dir: String,
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
            udp_key_secret: default_kad_udp_key_secret(),

            service_enabled: default_kad_service_enabled(),
            service_runtime_secs: default_kad_service_runtime_secs(),
            service_crawl_every_secs: default_kad_service_crawl_every_secs(),
            service_persist_every_secs: default_kad_service_persist_every_secs(),
            service_alpha: default_kad_service_alpha(),
            service_req_contacts: default_kad_service_req_contacts(),
            service_max_persist_nodes: default_kad_service_max_persist_nodes(),

            service_req_timeout_secs: default_kad_service_req_timeout_secs(),
            service_req_min_interval_secs: default_kad_service_req_min_interval_secs(),

            service_hello_every_secs: default_kad_service_hello_every_secs(),
            service_hello_batch: default_kad_service_hello_batch(),
            service_hello_min_interval_secs: default_kad_service_hello_min_interval_secs(),

            service_maintenance_every_secs: default_kad_service_maintenance_every_secs(),
            service_max_failures: default_kad_service_max_failures(),
            service_evict_age_secs: default_kad_service_evict_age_secs(),
        }
    }
}

impl Default for I2PConfig {
    fn default() -> Self {
        Self {
            sam_private_key: "".to_string(),
            sam_public_key: "".to_string(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            data_dir: default_data_dir(),
        }
    }
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

    let env_filter = std::env::var("RUST_LOG")
        .ok()
        .or_else(|| Some(config.general.log_level.clone()))
        .unwrap_or_else(|| "info".to_string());

    let filter = EnvFilter::try_new(env_filter).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .init();
}
