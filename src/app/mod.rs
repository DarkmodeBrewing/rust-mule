use crate::config::Config;


pub async fn run(config: Config) {
    tracing::info!(log = %config.log_level, data_dir = %config.data_dir, "starting app");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    tracing::info!("app shutdown (placeholder)");
}
