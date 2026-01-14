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
