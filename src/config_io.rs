use crate::config::Config;
use anyhow::{Context, Result};
use std::{fs, path::Path};

pub async fn load_or_create_config(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();

    if !path.exists() {
        let default_cfg: Config = Config::default();
        save_config(path, &default_cfg)
            .await
            .with_context(|| format!("Failed to create default config at {}", path.display()))?;
        return Ok(default_cfg);
    }

    let content: String = fs::read_to_string(path)
        .with_context(|| format!("Failed reading config file {}", path.display()))?;

    let cfg: Config =
        toml::from_str(&content).with_context(|| format!("Invalid TOML in {}", path.display()))?;

    Ok(cfg)
}

pub async fn save_config(path: impl AsRef<Path>, cfg: &Config) -> Result<()> {
    let path = path.as_ref();

    // Pretty output TOML
    let toml_string = toml::to_string_pretty(cfg).context("Failed serializing config to TOML")?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed creating directory {}", parent.display()))?;
    }

    fs::write(path, toml_string)
        .with_context(|| format!("Failed writing config file {}", path.display()))?;

    Ok(())
}
