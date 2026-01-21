use crate::{config::Config, i2p::sam::SamClient, net::tcp_probe::tcp_probe};
use std::time::Duration;
use tokio::{io::AsyncWriteExt, signal, time};

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    tracing::info!(log = %config.general.log_level, data_dir = %config.general.data_dir, "starting app");
    tracing::info!(target = %config.general.tcp_probe_target, "tcp probe configured");
    tracing::info!("press Ctrl+C to stop");

    let mut ticker: time::Interval = time::interval(Duration::from_secs(10));
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    tracing::info!(
        priv_len = config.i2p.sam_private_key.trim().len(),
        pub_len = config.i2p.sam_public_key.trim().len(),
        "Loaded SAM keys from config"
    );

    tracing::info!("Testing i2p + SAM connectivity");

    let mut sam: SamClient = SamClient::connect(&config.sam.host, config.sam.port).await?;
    let reply = sam.hello("3.0", "3.3").await?;
    tracing::info!("(RAW) SAM Replies: {}", reply.raw);

    // Create SAM session
    let session_name = &config.sam.session_name;
    let priv_key: String = if !config.i2p.sam_private_key.trim().is_empty() {
        config.i2p.sam_private_key.clone()
    } else {
        tracing::info!("No SAM destination in config; generating new identity");

        let (generated_priv, generated_pub) = sam.dest_generate().await?;
        tracing::info!(
            priv_len = generated_priv.len(),
            pub_len = generated_pub.len(),
            "DEST generated"
        );

        config.i2p.sam_private_key = generated_priv.clone();
        config.i2p.sam_public_key = generated_pub.clone();
        config.persist().await?;

        generated_priv
    };

    sam.session_create_idempotent(session_name, &priv_key)
        .await?;
    tracing::info!(session=%session_name, "SAM session ready");

    // Create a stream
    let dest: String = sam.naming_lookup("stats.i2p").await?;
    let s: crate::i2p::sam::client::SamStream =
        SamClient::stream_connect(&config.sam.host, config.sam.port, session_name, &dest).await?;

    //SamClient::http_probe(s).await?;

    // loop {
    //     tokio::select! {
    //         _ = signal::ctrl_c() => {
    //             tracing::warn!("received Ctrl+C");
    //             break;
    //         }
    //         _ = ticker.tick() => {
    //             if let Err(err) = tcp_probe(&config.general.tcp_probe_target, Duration::from_secs(3)).await {
    //                 tracing::warn!(error = %err, "tcp probe failed (non-fatal)");
    //             }
    //         }
    //     }
    // }

    tracing::info!("shutting down gracefully");
    Ok(())
}

// fn log_kad_nodes(nodes: Vec<KadNode>) {
//     println!("loaded {} nodes", nodes.len());

//     let multicast = nodes.iter().filter(|n| n.ip.octets()[0] >= 224).count();
//     let private = nodes
//         .iter()
//         .filter(|n| {
//             let [a, b, _, _] = n.ip.octets();
//             a == 10
//                 || (a == 172 && (16..=31).contains(&b))
//                 || (a == 192 && b == 168)
//                 || (a == 127)
//                 || a == 0
//         })
//         .count();

//     println!(
//         "multicast-ish: {multicast}, private-ish: {private}, total: {}",
//         nodes.len()
//     );

//     for n in &nodes {
//         if n.ip.octets()[0] >= 224 {
//             println!("still weird ip: {:?}", n.ip);
//         }
//     }
// }

/*
#[tokio::main]
async fn main() -> Result<()> {
    let cfg = SamConfig {
        host: "10.99.0.2".to_string(),
        port: 7656,
        session_id: "rust-mule".to_string(),
        destination: "TRANSIENT".to_string(),
    };

    let mut sam: SamClient = SamClient::connect(&cfg.host, cfg.port).await?;
    sam.hello().await?;

    let (_pub_key, priv_key) = sam.dest_generate(None).await?;
    println!("Generated PRIV length: {}", priv_key.len());

    // Create session using the freshly generated key (for now)
    sam.stream_session_create(&cfg.session_id, &priv_key, &cfg.options)
        .await?;

    // Create a STREAM session. This proves we can negotiate a usable session.
    sam.stream_session_create(&cfg.session_id, &cfg.destination, &cfg.options)
        .await?;

    // Small extra proof that the control channel stays alive and responds:
    sam.ping().await?;

    println!("SAM connection OK ✅");
    Ok(())
}


*/
