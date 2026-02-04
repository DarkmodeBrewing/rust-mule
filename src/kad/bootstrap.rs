use crate::{
    i2p::sam::SamKadSocket,
    kad::wire::{
        KADEMLIA2_BOOTSTRAP_REQ, KADEMLIA2_BOOTSTRAP_RES, KADEMLIA2_PONG, KadPacket,
        decode_kad2_bootstrap_res,
    },
    kad::{KadId, udp_crypto},
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

#[derive(Debug, Clone, Copy)]
pub struct BootstrapCrypto {
    pub my_kad_id: KadId,
    pub my_dest_hash: u32,
    pub udp_key_secret: u32,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            max_initial: 64,
            // I2P tunnel build + lease set publication can take a bit; give bootstrap time.
            runtime: Duration::from_secs(60),
        }
    }
}

pub async fn bootstrap(
    sock: &mut SamKadSocket,
    nodes: &[ImuleNode],
    crypto: BootstrapCrypto,
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

    let boot_plain = KadPacket::encode(KADEMLIA2_BOOTSTRAP_REQ, &[]);

    tracing::info!(peers = initial.len(), "sending initial KAD2 BOOTSTRAP_REQ");

    for n in &initial {
        let dest = n.udp_dest_b64();
        let target_kad_id = KadId(n.client_id);
        let sender_verify_key =
            udp_crypto::udp_verify_key(crypto.udp_key_secret, n.udp_dest_hash_code());
        let receiver_verify_key = if n.udp_key_ip == crypto.my_dest_hash {
            n.udp_key
        } else {
            0
        };
        let boot = udp_crypto::encrypt_kad_packet(
            &boot_plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        )?;
        sock.send_to(&dest, &boot)
            .await
            .with_context(|| "failed to send KAD2 BOOTSTRAP_REQ")?;
    }

    let deadline = Instant::now() + cfg.runtime;
    let mut pong_from = BTreeSet::<String>::new();
    let mut bootstrap_from = BTreeSet::<String>::new();
    let mut new_contacts = 0usize;
    let mut received_total = 0usize;
    let mut dropped_unparsable = 0usize;
    let mut decrypted_ok = 0usize;

    while Instant::now() < deadline {
        let remain = deadline.saturating_duration_since(Instant::now());
        let recv = match timeout(remain, sock.recv()).await {
            Ok(r) => r?,
            Err(_) => break,
        };
        received_total += 1;

        let from_hash = match crate::i2p::b64::decode(&recv.from_destination) {
            Ok(b) if b.len() >= 4 => u32::from_le_bytes(b[0..4].try_into().unwrap()),
            _ => 0,
        };

        let decrypted = match udp_crypto::decrypt_kad_packet(
            &recv.payload,
            crypto.my_kad_id,
            crypto.udp_key_secret,
            from_hash,
        ) {
            Ok(d) => {
                if d.was_obfuscated {
                    decrypted_ok += 1;
                    tracing::debug!(
                        from = %recv.from_destination,
                        rvk = d.receiver_verify_key,
                        svk = d.sender_verify_key,
                        "decrypted obfuscated KAD packet"
                    );
                }
                d.payload
            }
            Err(err) => {
                dropped_unparsable += 1;
                tracing::debug!(
                    error = %err,
                    from = %recv.from_destination,
                    "dropping undecipherable/unknown KAD packet"
                );
                continue;
            }
        };

        let pkt = match KadPacket::decode(&decrypted) {
            Ok(p) => p,
            Err(err) => {
                dropped_unparsable += 1;
                tracing::debug!(
                    error = %err,
                    from = %recv.from_destination,
                    "dropping unparsable decrypted KAD packet"
                );
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
        received_total,
        dropped_unparsable,
        decrypted_ok,
        pongs = pong_from.len(),
        boot_responses = bootstrap_from.len(),
        boot_contacts = new_contacts,
        "bootstrap summary"
    );

    Ok(())
}
