mod app;
mod config;
mod i2p;
mod kad;
mod net;
mod protocol;

#[tokio::main]
async fn main() {
    let config = config::Config::from_env();
    config::init_tracing(&config);
    tracing::info!("rust-mule booted");

    app::run(config).await;
}
