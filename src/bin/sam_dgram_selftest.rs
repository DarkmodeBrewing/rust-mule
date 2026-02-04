use anyhow::{Context, Result};
use rust_mule::config::Config;
use rust_mule::config_io::load_or_create_config;
use rust_mule::i2p::sam::{SamClient, SamDatagramTcp};
use tokio::time::{Duration, timeout};

#[tokio::main]
async fn main() -> Result<()> {
    let cfg: Config = load_or_create_config("config.toml")
        .await
        .context("Unable to load Config")?;
    rust_mule::config::init_tracing(&cfg);

    let mut sam = SamClient::connect(&cfg.sam.host, cfg.sam.port)
        .await?
        .with_timeout(Duration::from_secs(cfg.sam.control_timeout_secs));
    sam.hello("3.0", "3.3").await?;

    let (a_priv, a_pub) = sam.dest_generate().await?;
    let (b_priv, b_pub) = sam.dest_generate().await?;

    let mut a = SamDatagramTcp::connect(&cfg.sam.host, cfg.sam.port)
        .await?
        .with_timeout(Duration::from_secs(cfg.sam.control_timeout_secs));
    a.hello("3.0", "3.3").await?;
    a.session_create_datagram(
        "rust-mule-selftest-a",
        &a_priv,
        ["i2cp.messageReliability=BestEffort"],
    )
    .await?;

    let mut b = SamDatagramTcp::connect(&cfg.sam.host, cfg.sam.port)
        .await?
        .with_timeout(Duration::from_secs(cfg.sam.control_timeout_secs));
    b.hello("3.0", "3.3").await?;
    b.session_create_datagram(
        "rust-mule-selftest-b",
        &b_priv,
        ["i2cp.messageReliability=BestEffort"],
    )
    .await?;

    // Give the router a moment to publish leasesets.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let msg = b"hello-from-a";
    a.send_to(&b_pub, msg).await?;
    tracing::info!("sent A->B {} bytes", msg.len());

    let recv = timeout(Duration::from_secs(30), b.recv())
        .await
        .context("timed out waiting for B.recv()")??;
    tracing::info!(
        from = %recv.from_destination,
        len = recv.payload.len(),
        "B received"
    );

    let msg2 = b"hello-from-b";
    b.send_to(&a_pub, msg2).await?;
    tracing::info!("sent B->A {} bytes", msg2.len());

    let recv2 = timeout(Duration::from_secs(30), a.recv())
        .await
        .context("timed out waiting for A.recv()")??;
    tracing::info!(
        from = %recv2.from_destination,
        len = recv2.payload.len(),
        "A received"
    );

    // Cleanup. Be tolerant of races.
    let _ = sam.session_destroy("rust-mule-selftest-a").await;
    let _ = sam.session_destroy("rust-mule-selftest-b").await;

    Ok(())
}
