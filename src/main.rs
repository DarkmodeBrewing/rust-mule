use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg: rust_mule::config::Config = rust_mule::config_io::load_or_create_config("config.toml")
        .await
        .context("Unable to load Config")?;

    validate_cfg(&cfg)?;

    rust_mule::config::init_tracing(&cfg);
    tracing::info!("rust-mule booting");

    rust_mule::app::run(cfg).await?;
    Ok(())
}

fn validate_cfg(cfg: &rust_mule::config::Config) -> anyhow::Result<()> {
    use rust_mule::config::SamDatagramTransport;

    cfg.sam
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|e| anyhow::anyhow!("Invalid sam.host '{}': {}", cfg.sam.host, e))?;

    if !(1..=65535).contains(&cfg.sam.port) {
        anyhow::bail!("Invalid sam.port '{}'", cfg.sam.port);
    }

    if matches!(cfg.sam.datagram_transport, SamDatagramTransport::UdpForward)
        && !(1..=65535).contains(&cfg.sam.udp_port)
    {
        anyhow::bail!("Invalid sam.udp_port '{}'", cfg.sam.udp_port);
    }

    if cfg.sam.session_name.trim().is_empty() {
        anyhow::bail!("Invalid sam.session_name '{}'", cfg.sam.session_name);
    }

    if matches!(cfg.sam.datagram_transport, SamDatagramTransport::UdpForward) {
        cfg.sam
            .forward_host
            .parse::<std::net::IpAddr>()
            .map_err(|e| {
                anyhow::anyhow!("Invalid sam.forward_host '{}': {}", cfg.sam.forward_host, e)
            })?;
    }

    if cfg.sam.control_timeout_secs == 0 {
        anyhow::bail!(
            "Invalid sam.control_timeout_secs '{}'",
            cfg.sam.control_timeout_secs
        );
    }

    cfg.api
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|e| anyhow::anyhow!("Invalid api.host '{}': {}", cfg.api.host, e))?;
    if !(1..=65535).contains(&cfg.api.port) {
        anyhow::bail!("Invalid api.port '{}'", cfg.api.port);
    }

    Ok(())
}
