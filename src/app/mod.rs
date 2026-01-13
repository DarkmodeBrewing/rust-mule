use crate::config::Config;
use tokio::signal;

pub async fn run(config: Config) {
    tracing::info!(log = %config.log_level, data_dir = %config.data_dir, "starting app");
    tracing::info!("press Ctrl+C to stop");

    // Main loop placeholder: wait until Ctrl+C.
    // Later this will become a select! between Ctrl+C, sockets, timers, channels, etc.
    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::warn!("received Ctrl+C");
        }
    }

    tracing::info!("shutting down gracefully");
}
