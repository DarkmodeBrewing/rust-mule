use crate::{
    i2p::sam::SamKadSocket,
    kad::wire::{
        KADEMLIA2_BOOTSTRAP_REQ, KADEMLIA2_BOOTSTRAP_RES, KADEMLIA2_PING, KADEMLIA2_PONG,
        KadPacket, decode_kad2_bootstrap_res,
    },
    nodes::imule::ImuleNode,
};
use anyhow::{Context, Result};
use std::{collections::BTreeSet, time::Duration};
use tokio::time::{Instant, timeout};

#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    pub max_initial: usize,
    pub runtime: Duration,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            max_initial: 16,
            runtime: Duration::from_secs(10),
        }
    }
}

pub async fn bootstrap(
    sock: &mut SamKadSocket,
    nodes: &[ImuleNode],
    cfg: BootstrapConfig,
) -> Result<()> {
    let mut initial = nodes
        .iter()
        .filter(|n| n.kad_version != 0)
        .collect::<Vec<_>>();
    // Prefer verified and newer nodes first; it tends to yield faster responses.
    initial.sort_by_key(|n| {
        (
            std::cmp::Reverse(n.verified),
            std::cmp::Reverse(n.kad_version),
        )
    });
    initial.truncate(cfg.max_initial);
    if initial.is_empty() {
        anyhow::bail!("no bootstrap nodes provided");
    }

    let ping = KadPacket::encode(KADEMLIA2_PING, &[]);
    let boot = KadPacket::encode(KADEMLIA2_BOOTSTRAP_REQ, &[]);

    tracing::info!(
        peers = initial.len(),
        "sending initial KAD2 PING + BOOTSTRAP_REQ"
    );

    for n in &initial {
        let dest = n.udp_dest_b64();
        sock.send_to(&dest, &ping)
            .await
            .with_context(|| "failed to send KAD2 PING")?;
        sock.send_to(&dest, &boot)
            .await
            .with_context(|| "failed to send KAD2 BOOTSTRAP_REQ")?;
    }

    let deadline = Instant::now() + cfg.runtime;
    let mut pong_from = BTreeSet::<String>::new();
    let mut bootstrap_from = BTreeSet::<String>::new();
    let mut new_contacts = 0usize;

    while Instant::now() < deadline {
        let remain = deadline.saturating_duration_since(Instant::now());
        let recv = match timeout(remain, sock.recv()).await {
            Ok(r) => r?,
            Err(_) => break,
        };

        let pkt = match KadPacket::decode(&recv.payload) {
            Ok(p) => p,
            Err(err) => {
                tracing::debug!(error = %err, from = %recv.from_destination, "dropping unparsable KAD packet");
                continue;
            }
        };

        match pkt.opcode {
            KADEMLIA2_PONG => {
                if pong_from.insert(recv.from_destination.clone()) {
                    tracing::info!(from = %recv.from_destination, "got KAD2 PONG");
                }
            }
            KADEMLIA2_BOOTSTRAP_RES => {
                if bootstrap_from.insert(recv.from_destination.clone()) {
                    tracing::info!(from = %recv.from_destination, "got KAD2 BOOTSTRAP_RES");
                }
                match decode_kad2_bootstrap_res(&pkt.payload) {
                    Ok(res) => {
                        new_contacts += res.contacts.len();
                        tracing::info!(
                            sender_kad_version = res.sender_kad_version,
                            contacts = res.contacts.len(),
                            "decoded BOOTSTRAP_RES"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to decode BOOTSTRAP_RES payload");
                    }
                }
            }
            other => {
                tracing::debug!(
                    opcode = format_args!("0x{other:02x}"),
                    from = %recv.from_destination,
                    len = pkt.payload.len(),
                    "received unhandled KAD2 packet"
                );
            }
        }
    }

    tracing::info!(
        pongs = pong_from.len(),
        boot_responses = bootstrap_from.len(),
        boot_contacts = new_contacts,
        "bootstrap summary"
    );

    Ok(())
}
