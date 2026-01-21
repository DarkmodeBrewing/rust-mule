use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

fn default_sam_host() -> String {
    "127.0.0.1".to_string()
}
fn default_sam_port() -> u16 {
    7656
}
fn default_session_name() -> String {
    "rust-mule".to_string()
}
fn default_log_level() -> String {
    "debug".to_string()
}
fn default_data_dir() -> String {
    "data".to_string()
}
fn default_tcp_probe_target() -> String {
    "1.1.1.1:443".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)] // <--- this makes missing fields fall back to Default::default()
pub struct Config {
    pub sam: SamConfig,
    pub kad: KadConfig,
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SamConfig {
    pub host: String,
    pub port: u16,
    pub session_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KadConfig {
    pub bootstrap_nodes_path: String,
    pub udp_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub data_dir: String,
    pub tcp_probe_target: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sam: SamConfig::default(),
            kad: KadConfig::default(),
            general: GeneralConfig::default(),
        }
    }
}

impl Default for SamConfig {
    fn default() -> Self {
        Self {
            host: default_sam_host(),
            port: default_sam_port(),
            session_name: default_session_name(),
        }
    }
}

impl Default for KadConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes_path: "nodes.dat".to_string(),
            udp_port: 4665,
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            data_dir: default_data_dir(),
            tcp_probe_target: default_tcp_probe_target(),
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

/*
 use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sam: SamSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamSection {
    pub host: String,
    pub port: u16,
    pub session_id: String,

    /// If set, we use this as DESTINATION when creating the session.
    /// If missing, we generate one and persist it.
    pub destination_priv: Option<String>,

    /// Optional I2CP options (leave empty initially)
    pub options: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sam: SamSection {
                host: "10.99.0.2".to_string(),
                port: 7656,
                session_id: "rust-mule".to_string(),
                destination_priv: None,
                options: None,
            },
        }
    }
}

pub fn config_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("se", "darkmode", "rust-mule")
        .context("Could not resolve config directory")?;

    let dir = proj.config_dir();
    fs::create_dir_all(dir).context("Failed to create config directory")?;
    Ok(dir.join("config.toml"))
}

pub fn load_or_create_config() -> Result<(AppConfig, PathBuf)> {
    let path = config_path()?;

    if !path.exists() {
        let cfg = AppConfig::default();
        let toml = toml::to_string_pretty(&cfg)?;
        fs::write(&path, toml).context("Failed to write default config")?;
        return Ok((cfg, path));
    }

    let content = fs::read_to_string(&path).context("Failed to read config")?;
    let cfg: AppConfig = toml::from_str(&content).context("Failed to parse config")?;
    Ok((cfg, path))
}

pub fn save_config(cfg: &AppConfig, path: &PathBuf) -> Result<()> {
    let toml = toml::to_string_pretty(cfg)?;
    fs::write(path, toml).context("Failed to write config")?;
    Ok(())
}

 */
