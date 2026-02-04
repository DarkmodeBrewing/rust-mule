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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Where the SAM bridge should forward inbound DATAGRAM/RAW UDP packets.
    /// Must be reachable from the SAM host (often `127.0.0.1` if SAM is local).
    pub forward_host: String,
    /// UDP port to bind locally for SAM UDP forwarding. `0` means "pick any free port".
    pub forward_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KadConfig {
    pub bootstrap_nodes_path: String,
    pub udp_port: u16,
    pub preferences_kad_path: String,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            sam: SamConfig::default(),
            kad: KadConfig::default(),
            i2p: I2PConfig::default(),
            general: GeneralConfig::default(),
        }
    }
}

impl Default for SamConfig {
    fn default() -> Self {
        Self {
            host: default_sam_host(),
            port: default_sam_port(),
            udp_port: default_sam_udp_port(),
            session_name: default_session_name(),
            forward_host: default_forward_host(),
            forward_port: default_forward_port(),
        }
    }
}

impl Default for KadConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes_path: "nodes.dat".to_string(),
            udp_port: 4665,
            preferences_kad_path: default_preferences_kad_path(),
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
