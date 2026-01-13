use crate::{config::Config, net::tcp_probe::tcp_probe};
use std::time::Duration;
use tokio::signal;

pub async fn run(config: Config) {
    tracing::info!(log = %config.log_level, data_dir = %config.data_dir, "starting app");
    tracing::info!(target = %config.tcp_probe_target, "tcp probe configured");
    tracing::info!("press Ctrl+C to stop");

    // Main loop placeholder: wait until Ctrl+C.
    // Later this will become a select! between Ctrl+C, sockets, timers, channels, etc.
    // Do a single probe on boot (non-fatal for now)
    if let Err(err) = tcp_probe(&config.tcp_probe_target, Duration::from_secs(3)).await {
        tracing::warn!(error = %err, "tcp probe failed (non-fatal)");
    }

    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::warn!("received Ctrl+C");
        }
    }

    tracing::info!("shutting down gracefully");
}
