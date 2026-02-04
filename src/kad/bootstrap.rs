use crate::{
    i2p::sam::SamKadSocket,
    kad::wire::{
        I2P_DEST_LEN, KADEMLIA_HELLO_REQ_DEPRECATED, KADEMLIA_HELLO_RES_DEPRECATED,
        KADEMLIA2_BOOTSTRAP_REQ, KADEMLIA2_BOOTSTRAP_RES, KADEMLIA2_HELLO_REQ, KADEMLIA2_HELLO_RES,
        KADEMLIA2_HELLO_RES_ACK, KADEMLIA2_PONG, KadPacket, TAG_KADMISCOPTIONS,
        decode_kad2_bootstrap_res, decode_kad2_hello, encode_kad2_hello,
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
    pub my_dest: [u8; I2P_DEST_LEN],
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
    let hello_plain_payload = encode_kad2_hello(8, crypto.my_kad_id, &crypto.my_dest);
    let hello_plain = KadPacket::encode(KADEMLIA2_HELLO_REQ, &hello_plain_payload);

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

        if n.kad_version >= 6 {
            let boot = udp_crypto::encrypt_kad_packet(
                &boot_plain,
                target_kad_id,
                receiver_verify_key,
                sender_verify_key,
            )?;
            sock.send_to(&dest, &boot)
                .await
                .with_context(|| "failed to send KAD2 BOOTSTRAP_REQ")?;

            // Proactively send HELLO_REQ as well to encourage key exchange and being added to routing tables.
            let hello = udp_crypto::encrypt_kad_packet(
                &hello_plain,
                target_kad_id,
                receiver_verify_key,
                sender_verify_key,
            )?;
            sock.send_to(&dest, &hello)
                .await
                .with_context(|| "failed to send KAD2 HELLO_REQ")?;
        } else {
            sock.send_to(&dest, &boot_plain)
                .await
                .with_context(|| "failed to send KAD2 BOOTSTRAP_REQ (plain)")?;
            sock.send_to(&dest, &hello_plain)
                .await
                .with_context(|| "failed to send KAD2 HELLO_REQ (plain)")?;
        }
    }

    let deadline = Instant::now() + cfg.runtime;
    let mut pong_from = BTreeSet::<String>::new();
    let mut bootstrap_from = BTreeSet::<String>::new();
    let mut new_contacts = 0usize;
    let mut received_total = 0usize;
    let mut dropped_unparsable = 0usize;
    let mut decrypted_ok = 0usize;
    let mut hello_reqs = 0usize;
    let mut hello2_reqs = 0usize;
    let mut hello2_ress = 0usize;
    let mut hello2_ack_sent = 0usize;
    let mut hello2_ack_recv = 0usize;

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
                d
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

        let valid_receiver_key = if decrypted.was_obfuscated {
            let expected = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
            expected == decrypted.receiver_verify_key
        } else {
            false
        };

        let pkt = match KadPacket::decode(&decrypted.payload) {
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
            KADEMLIA_HELLO_REQ_DEPRECATED => {
                hello_reqs += 1;
                tracing::info!(from = %recv.from_destination, "got KAD1 HELLO_REQ (deprecated)");

                // Reply with our Kad1 contact details, iMule-style:
                //   <ClientID 16><UDPDest 387><TCPDest 387><Type 1>
                let mut payload = Vec::with_capacity(16 + 2 * I2P_DEST_LEN + 1);
                payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
                payload.extend_from_slice(&crypto.my_dest);
                payload.extend_from_slice(&crypto.my_dest);
                payload.push(0); // self contact type in iMule

                let res = KadPacket::encode(KADEMLIA_HELLO_RES_DEPRECATED, &payload);
                if let Err(err) = sock.send_to(&recv.from_destination, &res).await {
                    tracing::warn!(
                        error = %err,
                        to = %recv.from_destination,
                        "failed sending KAD1 HELLO_RES"
                    );
                } else {
                    tracing::info!(to = %recv.from_destination, "sent KAD1 HELLO_RES");
                }
            }
            KADEMLIA2_HELLO_REQ => {
                hello2_reqs += 1;
                let hello = match decode_kad2_hello(&pkt.payload) {
                    Ok(h) => h,
                    Err(err) => {
                        dropped_unparsable += 1;
                        tracing::debug!(
                            error = %err,
                            from = %recv.from_destination,
                            "failed to decode KAD2 HELLO_REQ payload"
                        );
                        continue;
                    }
                };

                tracing::info!(
                    from = %recv.from_destination,
                    kad_version = hello.kad_version,
                    valid_receiver_key,
                    "got KAD2 HELLO_REQ"
                );

                // If the peer didn't send a usable sender key, we can still reply, but can't use
                // receiver-key crypto for some followups.
                let receiver_verify_key = decrypted.sender_verify_key;
                let sender_verify_key =
                    udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);

                let mut res_payload = encode_kad2_hello(8, crypto.my_kad_id, &crypto.my_dest);
                // If we couldn't validate the receiver key, mimic iMule and request an ACK (Kad v8 only).
                if !valid_receiver_key && hello.kad_version >= 8 {
                    // TagList: 1 tag: TAG_KADMISCOPTIONS (u8) with bit2 set.
                    *res_payload
                        .last_mut()
                        .expect("encode_kad2_hello always appends a tag count") = 1;
                    res_payload.push(0x88); // TAGTYPE_UINT8 | 0x80 (numeric)
                    res_payload.push(TAG_KADMISCOPTIONS);
                    res_payload.push(0x04);
                }

                let res_plain = KadPacket::encode(KADEMLIA2_HELLO_RES, &res_payload);
                let res = if hello.kad_version >= 6 && decrypted.was_obfuscated {
                    udp_crypto::encrypt_kad_packet(
                        &res_plain,
                        hello.node_id,
                        receiver_verify_key,
                        sender_verify_key,
                    )?
                } else {
                    res_plain
                };
                if let Err(err) = sock.send_to(&recv.from_destination, &res).await {
                    tracing::warn!(
                        error = %err,
                        to = %recv.from_destination,
                        "failed sending KAD2 HELLO_RES"
                    );
                } else {
                    tracing::info!(to = %recv.from_destination, "sent KAD2 HELLO_RES");
                }
            }
            KADEMLIA2_HELLO_RES => {
                hello2_ress += 1;
                let hello = match decode_kad2_hello(&pkt.payload) {
                    Ok(h) => h,
                    Err(err) => {
                        dropped_unparsable += 1;
                        tracing::debug!(
                            error = %err,
                            from = %recv.from_destination,
                            "failed to decode KAD2 HELLO_RES payload"
                        );
                        continue;
                    }
                };

                let misc = hello.tags.get(&TAG_KADMISCOPTIONS).copied().unwrap_or(0) as u8;
                let wants_ack = (misc & 0x04) != 0;
                tracing::info!(
                    from = %recv.from_destination,
                    kad_version = hello.kad_version,
                    valid_receiver_key,
                    wants_ack,
                    "got KAD2 HELLO_RES"
                );

                if wants_ack {
                    let receiver_verify_key = decrypted.sender_verify_key;
                    if receiver_verify_key == 0 {
                        tracing::warn!(
                            from = %recv.from_destination,
                            "peer requested HELLO_RES_ACK but sender_verify_key was 0"
                        );
                    } else {
                        let mut ack_payload = Vec::with_capacity(16 + 1);
                        ack_payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
                        ack_payload.push(0); // no tags
                        let ack_plain = KadPacket::encode(KADEMLIA2_HELLO_RES_ACK, &ack_payload);
                        let sender_verify_key =
                            udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);

                        // iMule encrypts HELLO_RES_ACK using the receiver verify key.
                        let ack = udp_crypto::encrypt_kad_packet_with_receiver_key(
                            &ack_plain,
                            receiver_verify_key,
                            sender_verify_key,
                        )?;

                        if let Err(err) = sock.send_to(&recv.from_destination, &ack).await {
                            tracing::warn!(
                                error = %err,
                                to = %recv.from_destination,
                                "failed sending KAD2 HELLO_RES_ACK"
                            );
                        } else {
                            hello2_ack_sent += 1;
                            tracing::info!(to = %recv.from_destination, "sent KAD2 HELLO_RES_ACK");
                        }
                    }
                }
            }
            KADEMLIA2_HELLO_RES_ACK => {
                hello2_ack_recv += 1;
                tracing::info!(from = %recv.from_destination, valid_receiver_key, "got KAD2 HELLO_RES_ACK");
            }
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
        hello_reqs,
        hello2_reqs,
        hello2_ress,
        hello2_ack_sent,
        hello2_ack_recv,
        pongs = pong_from.len(),
        boot_responses = bootstrap_from.len(),
        boot_contacts = new_contacts,
        "bootstrap summary"
    );

    Ok(())
}
