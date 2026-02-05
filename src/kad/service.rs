use crate::{
    i2p::sam::SamKadSocket,
    kad::{
        KadId, routing::RoutingTable,
        udp_crypto,
        wire::{
            I2P_DEST_LEN, KADEMLIA2_BOOTSTRAP_RES, KADEMLIA2_HELLO_REQ, KADEMLIA2_HELLO_RES,
            KADEMLIA2_HELLO_RES_ACK, KADEMLIA2_PING, KADEMLIA2_PONG, KADEMLIA2_PUBLISH_RES,
            KADEMLIA2_PUBLISH_SOURCE_REQ, KADEMLIA2_REQ, KADEMLIA2_RES, KADEMLIA2_SEARCH_RES,
            KADEMLIA2_SEARCH_SOURCE_REQ, KadPacket, TAG_KADMISCOPTIONS,
            KADEMLIA_HELLO_REQ_DEPRECATED, KADEMLIA_HELLO_RES_DEPRECATED, KADEMLIA_REQ_DEPRECATED,
            KADEMLIA_RES_DEPRECATED, decode_kad1_req, decode_kad2_bootstrap_res, decode_kad2_hello,
            decode_kad2_publish_source_req_min, decode_kad2_req, decode_kad2_res,
            decode_kad2_search_source_req, encode_kad1_res, encode_kad2_hello,
            encode_kad2_publish_res_for_source, encode_kad2_req, encode_kad2_res,
            encode_kad2_search_res_sources,
        },
    },
    nodes::imule::ImuleNode,
};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use tokio::time::{Duration, Instant, interval};

#[derive(Debug, Clone, Copy)]
pub struct KadServiceCrypto {
    pub my_kad_id: KadId,
    pub my_dest_hash: u32,
    pub udp_key_secret: u32,
    pub my_dest: [u8; I2P_DEST_LEN],
}

#[derive(Debug, Clone)]
pub struct KadServiceConfig {
    /// If 0, run until Ctrl-C.
    pub runtime_secs: u64,
    pub crawl_every_secs: u64,
    pub persist_every_secs: u64,
    pub alpha: usize,
    /// Requested number of contacts in `KADEMLIA2_REQ` (1..=32 recommended).
    pub req_contacts: u8,
    pub max_persist_nodes: usize,

    pub req_timeout_secs: u64,
    pub req_min_interval_secs: u64,

    pub hello_every_secs: u64,
    pub hello_batch: usize,
    pub hello_min_interval_secs: u64,

    pub maintenance_every_secs: u64,
    pub status_every_secs: u64,
    pub max_failures: u32,
    pub evict_age_secs: u64,
}

impl Default for KadServiceConfig {
    fn default() -> Self {
        Self {
            runtime_secs: 0,
            crawl_every_secs: 3,
            persist_every_secs: 300,
            alpha: 3,
            req_contacts: 32,
            max_persist_nodes: 5000,

            req_timeout_secs: 45,
            req_min_interval_secs: 15,

            hello_every_secs: 10,
            hello_batch: 2,
            hello_min_interval_secs: 900,

            maintenance_every_secs: 5,
            status_every_secs: 60,
            max_failures: 5,
            evict_age_secs: 3600,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct KadServiceStats {
    sent_reqs: u64,
    recv_ress: u64,
    sent_hellos: u64,
    recv_hello_ress: u64,
    timeouts: u64,
    new_nodes: u64,
    evicted: u64,
}

pub struct KadService {
    routing: RoutingTable,
    // Minimal (in-memory) source index: file ID -> (source ID -> UDP dest).
    sources_by_file: BTreeMap<KadId, BTreeMap<KadId, [u8; I2P_DEST_LEN]>>,

    pending_reqs: HashMap<String, Instant>,
    crawl_round: u64,
    stats_window: KadServiceStats,
}

impl KadService {
    pub fn new(my_id: KadId) -> Self {
        Self {
            routing: RoutingTable::new(my_id),
            sources_by_file: BTreeMap::new(),
            pending_reqs: HashMap::new(),
            crawl_round: 0,
            stats_window: KadServiceStats::default(),
        }
    }

    pub fn routing(&self) -> &RoutingTable {
        &self.routing
    }

    pub fn routing_mut(&mut self) -> &mut RoutingTable {
        &mut self.routing
    }
}

pub async fn run_service(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    initial_nodes: impl IntoIterator<Item = ImuleNode>,
    crypto: KadServiceCrypto,
    cfg: KadServiceConfig,
    persist_path: &std::path::Path,
) -> Result<()> {
    let now = Instant::now();
    for n in initial_nodes {
        svc.routing.upsert(n, now);
    }
    tracing::info!(nodes = svc.routing.len(), "kad service started");

    let mut crawl_tick = interval(Duration::from_secs(cfg.crawl_every_secs.max(1)));
    let mut persist_tick = interval(Duration::from_secs(cfg.persist_every_secs.max(5)));
    let mut hello_tick = interval(Duration::from_secs(cfg.hello_every_secs.max(1)));
    let mut maintenance_tick = interval(Duration::from_secs(cfg.maintenance_every_secs.max(1)));
    let mut status_tick = interval(Duration::from_secs(cfg.status_every_secs.max(5)));

    // Optional runtime deadline (0 => forever).
    let deadline = if cfg.runtime_secs == 0 {
        None
    } else {
        Some(Instant::now() + Duration::from_secs(cfg.runtime_secs))
    };

    loop {
        if let Some(d) = deadline
            && Instant::now() >= d
        {
            tracing::info!("kad service runtime deadline reached");
            break;
        }

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received Ctrl-C");
                break;
            }

            _ = crawl_tick.tick() => {
                crawl_once(svc, sock, crypto, &cfg).await?;
            }

            _ = persist_tick.tick() => {
                persist_snapshot(svc, persist_path, cfg.max_persist_nodes).await;
            }

            _ = hello_tick.tick() => {
                send_hello_batch(svc, sock, crypto, &cfg).await?;
            }

            _ = maintenance_tick.tick() => {
                maintenance(svc, &cfg).await;
            }

            _ = status_tick.tick() => {
                status_report(svc).await;
            }

            recv = sock.recv() => {
                let recv = recv?;
                handle_inbound(svc, sock, recv.from_destination, recv.payload, crypto).await?;
            }
        }
    }

    persist_snapshot(svc, persist_path, cfg.max_persist_nodes).await;
    tracing::info!("kad service stopped");
    Ok(())
}

async fn persist_snapshot(svc: &KadService, path: &std::path::Path, max_nodes: usize) {
    let nodes = svc.routing.snapshot_nodes(max_nodes);
    if let Err(err) = crate::nodes::imule::persist_nodes_dat_v2(path, &nodes).await {
        tracing::warn!(error = %err, path = %path.display(), "failed to persist nodes.dat");
    } else {
        tracing::info!(path = %path.display(), count = nodes.len(), "persisted nodes.dat");
    }
}

async fn crawl_once(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    svc.crawl_round = svc.crawl_round.wrapping_add(1);
    let now = Instant::now();

    // Mix targets to avoid repeatedly sampling the same region:
    // - mostly random IDs for exploration
    // - occasionally our own ID to keep buckets around us fresh
    let target = if svc.crawl_round.is_multiple_of(4) {
        crypto.my_kad_id
    } else {
        KadId::random()?
    };

    let req_min = Duration::from_secs(cfg.req_min_interval_secs.max(1));
    let mut peers = svc.routing.select_query_candidates(
        cfg.alpha.max(1),
        now,
        req_min,
        cfg.max_failures,
    );
    peers.retain(|p| !svc.pending_reqs.contains_key(&p.udp_dest_b64()));
    if peers.is_empty() {
        return Ok(());
    }

    let req_kind = cfg.req_contacts.clamp(1, 32);

    for p in peers {
        let dest = p.udp_dest_b64();
        let target_kad_id = KadId(p.client_id);

        // NOTE: iMule's `KADEMLIA2_REQ` includes a `check` field which must match the *receiver's*
        // KadID (used to discard packets not intended for this node). If we put our own KadID here,
        // peers will silently ignore the request and we'll never get `KADEMLIA2_RES`.
        let req_payload = encode_kad2_req(req_kind, target, target_kad_id);
        let req_plain = KadPacket::encode(KADEMLIA2_REQ, &req_payload);

        let sender_verify_key =
            udp_crypto::udp_verify_key(crypto.udp_key_secret, p.udp_dest_hash_code());
        let receiver_verify_key = if p.udp_key_ip == crypto.my_dest_hash {
            p.udp_key
        } else {
            0
        };

        let out = if p.kad_version >= 6 {
            udp_crypto::encrypt_kad_packet(
                &req_plain,
                target_kad_id,
                receiver_verify_key,
                sender_verify_key,
            )?
        } else {
            req_plain.clone()
        };

        if let Err(err) = sock.send_to(&dest, &out).await {
            tracing::debug!(error = %err, to = %dest, "failed sending KAD2 REQ (crawl)");
        } else {
            tracing::trace!(to = %dest, "sent KAD2 REQ (crawl)");
            svc.stats_window.sent_reqs += 1;
            svc.pending_reqs.insert(
                dest.clone(),
                Instant::now() + Duration::from_secs(cfg.req_timeout_secs.max(5)),
            );
            svc.routing.mark_queried_by_dest(&dest, Instant::now());
        }
    }

    Ok(())
}

async fn send_hello_batch(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let min_interval = Duration::from_secs(cfg.hello_min_interval_secs.max(60));
    let mut peers = svc.routing.select_hello_candidates(
        cfg.hello_batch.max(1),
        now,
        min_interval,
        cfg.max_failures,
    );
    if peers.is_empty() {
        return Ok(());
    }

    let hello_plain_payload = encode_kad2_hello(8, crypto.my_kad_id, &crypto.my_dest);
    let hello_plain = KadPacket::encode(KADEMLIA2_HELLO_REQ, &hello_plain_payload);

    for p in peers.drain(..) {
        let dest = p.udp_dest_b64();
        let target_kad_id = KadId(p.client_id);
        let sender_verify_key =
            udp_crypto::udp_verify_key(crypto.udp_key_secret, p.udp_dest_hash_code());
        let receiver_verify_key = if p.udp_key_ip == crypto.my_dest_hash {
            p.udp_key
        } else {
            0
        };

        let out = udp_crypto::encrypt_kad_packet(
            &hello_plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        )?;

        if let Err(err) = sock.send_to(&dest, &out).await {
            tracing::debug!(error = %err, to = %dest, "failed sending KAD2 HELLO_REQ (service)");
        } else {
            tracing::trace!(to = %dest, "sent KAD2 HELLO_REQ (service)");
            svc.stats_window.sent_hellos += 1;
            svc.routing.mark_hello_sent_by_dest(&dest, now);
        }
    }

    Ok(())
}

async fn maintenance(svc: &mut KadService, cfg: &KadServiceConfig) {
    let now = Instant::now();

    // Expire pending queries and mark failures.
    let mut expired = Vec::new();
    for (dest, deadline) in &svc.pending_reqs {
        if *deadline <= now {
            expired.push(dest.clone());
        }
    }
    for dest in expired {
        svc.pending_reqs.remove(&dest);
        svc.routing.mark_failure_by_dest(&dest);
        svc.routing.mark_queried_by_dest(&dest, now);
        svc.stats_window.timeouts += 1;
        tracing::trace!(to = %dest, "KAD2 REQ timed out; marking failure");
    }

    // Evict dead entries to keep the table healthy.
    let evicted = svc.routing.evict(
        now,
        cfg.max_failures,
        Duration::from_secs(cfg.evict_age_secs.max(60)),
    );
    if evicted > 0 {
        svc.stats_window.evicted += evicted as u64;
        tracing::info!(evicted, remaining = svc.routing.len(), "evicted stale peers");
    }
}

async fn status_report(svc: &mut KadService) {
    let routing = svc.routing.len();
    let live = svc.routing.live_count();
    let pending = svc.pending_reqs.len();
    let w = svc.stats_window;
    svc.stats_window = KadServiceStats::default();

    tracing::info!(
        routing,
        live,
        pending,
        sent_reqs = w.sent_reqs,
        recv_ress = w.recv_ress,
        sent_hellos = w.sent_hellos,
        recv_hello_ress = w.recv_hello_ress,
        timeouts = w.timeouts,
        new_nodes = w.new_nodes,
        evicted = w.evicted,
        "kad service status"
    );
}

async fn handle_inbound(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    from_dest_b64: String,
    payload: Vec<u8>,
    crypto: KadServiceCrypto,
) -> Result<()> {
    let now = Instant::now();
    let from_dest_raw = crate::i2p::b64::decode(&from_dest_b64).ok();
    let from_hash = match &from_dest_raw {
        Some(b) if b.len() >= 4 => u32::from_le_bytes(b[0..4].try_into().unwrap()),
        _ => 0,
    };

    let decrypted =
        match udp_crypto::decrypt_kad_packet(&payload, crypto.my_kad_id, crypto.udp_key_secret, from_hash) {
            Ok(d) => d,
            Err(err) => {
            tracing::trace!(error = %err, from = %from_dest_b64, "dropping undecipherable/unknown KAD packet");
            return Ok(());
        }
    };

    let valid_receiver_key = if decrypted.was_obfuscated {
        let expected = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
        expected == decrypted.receiver_verify_key
    } else {
        false
    };

    // Update liveness + (if known) sender UDP key.
    svc.routing
        .update_sender_keys_by_dest(&from_dest_b64, now, decrypted.sender_verify_key, crypto.my_dest_hash, valid_receiver_key);

    let pkt = match KadPacket::decode(&decrypted.payload) {
        Ok(p) => p,
        Err(err) => {
            tracing::trace!(error = %err, from = %from_dest_b64, "dropping unparsable decrypted KAD packet");
            return Ok(());
        }
    };

    match pkt.opcode {
        KADEMLIA_HELLO_REQ_DEPRECATED => {
            // Reply with our Kad1 contact details, iMule-style:
            //   <ClientID 16><UDPDest 387><TCPDest 387><Type 1>
            let mut payload = Vec::with_capacity(16 + 2 * I2P_DEST_LEN + 1);
            payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
            payload.extend_from_slice(&crypto.my_dest);
            payload.extend_from_slice(&crypto.my_dest);
            payload.push(0); // self contact type in iMule

            let res = KadPacket::encode(KADEMLIA_HELLO_RES_DEPRECATED, &payload);
            let _ = sock.send_to(&from_dest_b64, &res).await;
        }

        KADEMLIA2_BOOTSTRAP_RES => {
            if let Ok(res) = decode_kad2_bootstrap_res(&pkt.payload) {
                // Sender itself.
                if let Some(raw) = &from_dest_raw
                    && raw.len() == I2P_DEST_LEN
                {
                    let mut udp_dest = [0u8; I2P_DEST_LEN];
                    udp_dest.copy_from_slice(raw);
                    svc.routing.upsert(ImuleNode {
                        kad_version: res.sender_kad_version,
                        client_id: res.sender_id.0,
                        udp_dest,
                        udp_key: if decrypted.was_obfuscated { decrypted.sender_verify_key } else { 0 },
                        udp_key_ip: if decrypted.was_obfuscated { crypto.my_dest_hash } else { 0 },
                        verified: valid_receiver_key,
                    }, now);
                }

                // Harvest contacts list.
                for c in res.contacts {
                    svc.routing.upsert(ImuleNode {
                        kad_version: c.kad_version,
                        client_id: c.node_id.0,
                        udp_dest: c.udp_dest,
                        udp_key: 0,
                        udp_key_ip: 0,
                        verified: false,
                    }, now);
                }
            }
        }

        KADEMLIA2_HELLO_REQ => {
            let hello = match decode_kad2_hello(&pkt.payload) {
                Ok(h) => h,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 HELLO_REQ payload");
                    return Ok(());
                }
            };

            if let Some(raw) = &from_dest_raw
                && raw.len() == I2P_DEST_LEN
            {
                let mut udp_dest = [0u8; I2P_DEST_LEN];
                udp_dest.copy_from_slice(raw);
                svc.routing.upsert(ImuleNode {
                    kad_version: hello.kad_version,
                    client_id: hello.node_id.0,
                    udp_dest,
                    udp_key: if decrypted.was_obfuscated { decrypted.sender_verify_key } else { 0 },
                    udp_key_ip: if decrypted.was_obfuscated { crypto.my_dest_hash } else { 0 },
                    verified: valid_receiver_key,
                }, now);
            }

            let receiver_verify_key = decrypted.sender_verify_key;
            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);

            let mut res_payload = encode_kad2_hello(8, crypto.my_kad_id, &crypto.my_dest);
            // Ask for HELLO_RES_ACK (TAG_KADMISCOPTIONS bit 0x04).
            let tag_count_idx = res_payload.len() - 1;
            res_payload[tag_count_idx] = 1; // tag count
            res_payload.push(0x89); // TAGTYPE_UINT8 | 0x80 (numeric)
            res_payload.push(TAG_KADMISCOPTIONS);
            res_payload.push(0x04);

            let res_plain = KadPacket::encode(KADEMLIA2_HELLO_RES, &res_payload);
            let out = if hello.kad_version >= 6 && decrypted.was_obfuscated {
                udp_crypto::encrypt_kad_packet(&res_plain, hello.node_id, receiver_verify_key, sender_verify_key)?
            } else {
                res_plain
            };

            let _ = sock.send_to(&from_dest_b64, &out).await;
        }

        KADEMLIA2_HELLO_RES => {
            let hello = match decode_kad2_hello(&pkt.payload) {
                Ok(h) => h,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 HELLO_RES payload");
                    return Ok(());
                }
            };

            if let Some(raw) = &from_dest_raw
                && raw.len() == I2P_DEST_LEN
            {
                let mut udp_dest = [0u8; I2P_DEST_LEN];
                udp_dest.copy_from_slice(raw);
                svc.routing.upsert(ImuleNode {
                    kad_version: hello.kad_version,
                    client_id: hello.node_id.0,
                    udp_dest,
                    udp_key: if decrypted.was_obfuscated { decrypted.sender_verify_key } else { 0 },
                    udp_key_ip: if decrypted.was_obfuscated { crypto.my_dest_hash } else { 0 },
                    verified: valid_receiver_key,
                }, now);
            }

            let misc = hello.tags.get(&TAG_KADMISCOPTIONS).copied().unwrap_or(0) as u8;
            let wants_ack = (misc & 0x04) != 0;
            if wants_ack && decrypted.sender_verify_key != 0 {
                let mut ack_payload = Vec::with_capacity(16 + 1);
                ack_payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
                ack_payload.push(0);
                let ack_plain = KadPacket::encode(KADEMLIA2_HELLO_RES_ACK, &ack_payload);
                let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
                let ack = udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &ack_plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?;
                let _ = sock.send_to(&from_dest_b64, &ack).await;
            }
            svc.stats_window.recv_hello_ress += 1;
        }

        KADEMLIA2_HELLO_RES_ACK => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
        }

        KADEMLIA2_REQ => {
            let req = match decode_kad2_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 REQ payload");
                    return Ok(());
                }
            };
            if req.check != crypto.my_kad_id {
                return Ok(());
            }

            let max = (req.kind as usize).min(32);
            let contacts = svc.routing.closest_to(req.target, max, from_hash);
            let kad2_contacts = contacts
                .iter()
                .map(|n| crate::kad::wire::Kad2Contact {
                    kad_version: n.kad_version,
                    node_id: KadId(n.client_id),
                    udp_dest: n.udp_dest,
                })
                .collect::<Vec<_>>();
            let res_payload = encode_kad2_res(req.target, &kad2_contacts);
            let res_plain = KadPacket::encode(KADEMLIA2_RES, &res_payload);

            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
            let out = if decrypted.was_obfuscated && decrypted.sender_verify_key != 0 {
                udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &res_plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?
            } else {
                res_plain
            };
            let _ = sock.send_to(&from_dest_b64, &out).await;
        }

        KADEMLIA_REQ_DEPRECATED => {
            let req = match decode_kad1_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD1 REQ payload");
                    return Ok(());
                }
            };
            if req.check != crypto.my_kad_id {
                return Ok(());
            }

            let max = (req.kind as usize).min(16);
            let contacts = svc.routing.closest_to(req.target, max, from_hash);
            let kad1_contacts = contacts
                .iter()
                .map(|n| (KadId(n.client_id), n.udp_dest))
                .collect::<Vec<_>>();
            let res_payload = encode_kad1_res(req.target, &kad1_contacts);
            let res_plain = KadPacket::encode(KADEMLIA_RES_DEPRECATED, &res_payload);
            let _ = sock.send_to(&from_dest_b64, &res_plain).await;
        }

        KADEMLIA2_RES => {
            let res = match decode_kad2_res(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 RES payload");
                    return Ok(());
                }
            };
            tracing::trace!(from = %from_dest_b64, contacts = res.contacts.len(), "got KAD2 RES");
            svc.pending_reqs.remove(&from_dest_b64);
            svc.stats_window.recv_ress += 1;
            let mut new_nodes = 0usize;
            for c in res.contacts {
                if !svc.routing.contains_id(c.node_id) {
                    new_nodes += 1;
                }
                svc.routing.upsert(ImuleNode {
                    kad_version: c.kad_version,
                    client_id: c.node_id.0,
                    udp_dest: c.udp_dest,
                    udp_key: 0,
                    udp_key_ip: 0,
                    verified: false,
                }, now);
            }
            if new_nodes > 0 {
                svc.stats_window.new_nodes += new_nodes as u64;
                tracing::debug!(from = %from_dest_b64, new_nodes, routing = svc.routing.len(), "learned new nodes from KAD2 RES");
            }
        }

        KADEMLIA2_PING => {
            // PONG has an empty payload.
            let pong_plain = KadPacket::encode(KADEMLIA2_PONG, &[]);
            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
            let out = if decrypted.was_obfuscated && decrypted.sender_verify_key != 0 {
                udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &pong_plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?
            } else {
                pong_plain
            };
            let _ = sock.send_to(&from_dest_b64, &out).await;
        }

        KADEMLIA2_PONG => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
        }

        KADEMLIA2_PUBLISH_SOURCE_REQ => {
            let req = match decode_kad2_publish_source_req_min(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 PUBLISH_SOURCE_REQ payload");
                    return Ok(());
                }
            };

            if let Some(raw) = &from_dest_raw
                && raw.len() == I2P_DEST_LEN
            {
                let mut udp_dest = [0u8; I2P_DEST_LEN];
                udp_dest.copy_from_slice(raw);
                svc.sources_by_file.entry(req.file).or_default().insert(req.source, udp_dest);
            }

            let count = svc
                .sources_by_file
                .get(&req.file)
                .map(|m| m.len() as u32)
                .unwrap_or(0);

            let res_payload = encode_kad2_publish_res_for_source(req.file, count, count, 0);
            let res_plain = KadPacket::encode(KADEMLIA2_PUBLISH_RES, &res_payload);
            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
            let out = if decrypted.was_obfuscated && decrypted.sender_verify_key != 0 {
                udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &res_plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?
            } else {
                res_plain
            };

            let _ = sock.send_to(&from_dest_b64, &out).await;
        }

        KADEMLIA2_SEARCH_SOURCE_REQ => {
            let req = match decode_kad2_search_source_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 SEARCH_SOURCE_REQ payload");
                    return Ok(());
                }
            };

            let results = svc
                .sources_by_file
                .get(&req.target)
                .map(|m| {
                    m.iter()
                        .skip(req.start_position as usize)
                        .take(64)
                        .map(|(sid, dest)| (*sid, *dest))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let payload = encode_kad2_search_res_sources(crypto.my_kad_id, req.target, &results);
            let plain = KadPacket::encode(KADEMLIA2_SEARCH_RES, &payload);
            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
            let out = if decrypted.was_obfuscated && decrypted.sender_verify_key != 0 {
                udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?
            } else {
                plain
            };

            let _ = sock.send_to(&from_dest_b64, &out).await;
        }

        other => {
            tracing::trace!(
                opcode = format_args!("0x{other:02x}"),
                from = %from_dest_b64,
                len = pkt.payload.len(),
                "received unhandled KAD2 packet"
            );
        }
    }

    Ok(())
}
