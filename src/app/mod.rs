use crate::{
    config::Config,
    net::tcp_probe::tcp_probe,
    nodes::{detect::node_dat_version, parse::KadNode, parse::nodes_dat_contacts},
};
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

    // // PArse version
    // if let Ok(version) = node_dat_version("./datfiles/nodes.dat").await {
    //     println!("Nodes.dat version: {:?}", version.0);
    // } else {
    //     eprint!("Failed to get node.dat version")
    // }

    // //Parse node.dat
    // if let Ok(nodes) = nodes_dat_contacts("./datfiles/nodes.dat").await {
    //     for n in nodes.iter() {
    //         println!("{:?}", n);
    //     }

    //     log_kad_nodes(nodes);
    // } else {
    //     eprintln!("Failed to parse nodes.dat");
    // }

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

fn log_kad_nodes(nodes: Vec<KadNode>) {
    println!("loaded {} nodes", nodes.len());

    let multicast = nodes.iter().filter(|n| n.ip.octets()[0] >= 224).count();
    let private = nodes
        .iter()
        .filter(|n| {
            let [a, b, _, _] = n.ip.octets();
            a == 10
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 127)
                || a == 0
        })
        .count();

    println!(
        "multicast-ish: {multicast}, private-ish: {private}, total: {}",
        nodes.len()
    );

    for n in &nodes {
        if n.ip.octets()[0] >= 224 {
            println!("still weird ip: {:?}", n.ip);
        }
    }
}
