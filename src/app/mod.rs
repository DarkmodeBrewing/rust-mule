use crate::{config::Config, net::tcp_probe::tcp_probe};
use std::time::Duration;
use tokio::{signal, time};

pub async fn run(config: Config) {
    tracing::info!(log = %config.log_level, data_dir = %config.data_dir, "starting app");
    tracing::info!(target = %config.tcp_probe_target, "tcp probe configured");
    tracing::info!("press Ctrl+C to stop");

    // Main loop placeholder: wait until Ctrl+C.
    // Later this will become a select! between Ctrl+C, sockets, timers, channels, etc.
    // Do a single probe on boot (non-fatal for now)

    let mut ticker = time::interval(Duration::from_secs(10));

    // Avoid immediate double-run if you also probe at boot
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::warn!("received Ctrl+C");
                break;
            }
            _ = ticker.tick() => {
                if let Err(err) = tcp_probe(&config.tcp_probe_target, Duration::from_secs(3)).await {
                    tracing::warn!(error = %err, "tcp probe failed (non-fatal)");
                }
            }
        }
    }

    tracing::info!("shutting down gracefully");
}
