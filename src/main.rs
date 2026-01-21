use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg: rust_mule::config::Config = rust_mule::config_io::load_or_create_config("config.toml")
        .await
        .context("Unable to load Config")?;

    validate_cfg(&cfg)?;

    rust_mule::config::init_tracing(&cfg);
    tracing::info!("rust-mule booted");

    rust_mule::app::run(cfg).await?;
    Ok(())
}

fn validate_cfg(cfg: &rust_mule::config::Config) -> anyhow::Result<()> {
    cfg.sam
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|e| anyhow::anyhow!("Invalid sam.host '{}': {}", cfg.sam.host, e))?;

    if !(1..=65535).contains(&cfg.sam.port) {
        anyhow::bail!("Invalid sam.port '{}'", cfg.sam.port);
    }

    if cfg.sam.session_name.trim().is_empty() {
        anyhow::bail!("Invalid sam.session_name '{}'", cfg.sam.session_name);
    }

    Ok(())
}
