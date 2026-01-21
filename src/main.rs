#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg: rust_mule::config::Config = rust_mule::config_io::load_or_create_config("config.toml")
        .await
        .expect("Unable to read or create the config.toml file");

    rust_mule::config::init_tracing(&cfg);
    tracing::info!("rust-mule booted");

    rust_mule::app::run(cfg).await?;
    Ok(())
}
