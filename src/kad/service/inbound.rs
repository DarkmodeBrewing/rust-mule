use super::*;

pub(super) async fn handle_inbound_impl(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    from_dest_b64: String,
    payload: Vec<u8>,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    let now = Instant::now();
    let from_dest_raw = crate::i2p::b64::decode(&from_dest_b64).ok();
    let from_hash = match &from_dest_raw {
        Some(b) if b.len() >= 4 => {
            let mut h = [0u8; 4];
            h.copy_from_slice(&b[..4]);
            u32::from_le_bytes(h)
        }
        _ => 0,
    };

    let decrypted = match udp_crypto::decrypt_kad_packet(
        &payload,
        crypto.my_kad_id,
        crypto.udp_key_secret,
        from_hash,
    ) {
        Ok(d) => d,
        Err(err) => {
            svc.stats_window.dropped_undecipherable += 1;
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
    svc.routing.update_sender_keys_by_dest(
        &from_dest_b64,
        now,
        decrypted.sender_verify_key,
        crypto.my_dest_hash,
        valid_receiver_key,
    );

    let pkt = match KadPacket::decode(&decrypted.payload) {
        Ok(p) => p,
        Err(err) => {
            svc.stats_window.dropped_unparsable += 1;
            tracing::trace!(error = %err, from = %from_dest_b64, "dropping unparsable decrypted KAD packet");
            return Ok(());
        }
    };

    tracing::debug!(
        event = "kad_inbound_packet",
        from = %crate::i2p::b64::short(&from_dest_b64),
        opcode = format_args!("0x{:02x}", pkt.opcode),
        opcode_name = kad_opcode_name(pkt.opcode),
        dispatch = kad_dispatch_target(pkt.opcode),
        payload_len = pkt.payload.len(),
        was_obfuscated = decrypted.was_obfuscated,
        sender_verify_key = decrypted.sender_verify_key,
        receiver_verify_key = decrypted.receiver_verify_key,
        valid_receiver_key,
        "received inbound KAD packet"
    );

    if !inbound_request_allowed(svc, from_hash, pkt.opcode, now) {
        tracing::debug!(
            event = "kad_inbound_drop",
            from = %crate::i2p::b64::short(&from_dest_b64),
            opcode = format_args!("0x{:02x}", pkt.opcode),
            opcode_name = kad_opcode_name(pkt.opcode),
            reason = "request_rate_limited",
            "dropped inbound KAD packet"
        );
        return Ok(());
    }
    if !consume_tracked_out_request(svc, &from_dest_b64, pkt.opcode, pkt.payload.len(), now) {
        let (expected_request_opcodes, peer_tracked_requests, total_tracked_requests) = svc
            .last_unmatched_response
            .as_ref()
            .filter(|d| d.from_dest_b64 == from_dest_b64 && d.response_opcode == pkt.opcode)
            .map(|d| {
                (
                    d.expected_request_opcodes
                        .iter()
                        .map(|op| kad_opcode_name(*op))
                        .collect::<Vec<_>>(),
                    d.peer_tracked_requests,
                    d.total_tracked_requests,
                )
            })
            .unwrap_or_else(|| (Vec::new(), 0, 0));
        tracing::debug!(
            event = "kad_inbound_drop",
            from = %crate::i2p::b64::short(&from_dest_b64),
            opcode = format_args!("0x{:02x}", pkt.opcode),
            opcode_name = kad_opcode_name(pkt.opcode),
            reason = "unrequested_response",
            expected_request_opcodes = ?expected_request_opcodes,
            peer_tracked_requests,
            total_tracked_requests,
            "dropped inbound KAD packet"
        );
        return Ok(());
    }

    match pkt.opcode {
        KADEMLIA_HELLO_REQ_DEPRECATED => {
            // Reply with our Kad1 contact details, iMule-style:
            //   <ClientID 16><UDPDest 387><TCPDest 387><Type 1>
            let mut payload = Vec::with_capacity(16 + 2 * I2P_DEST_LEN + 1);
            payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
            payload.extend_from_slice(&crypto.my_dest);
            payload.extend_from_slice(&crypto.my_dest);
            // iMule uses `CContact::Self().WriteToKad1Contact`, which writes `GetType()`.
            // For KAD contacts, the default "good contact" type is 3.
            payload.push(3);

            let res = KadPacket::encode(KADEMLIA_HELLO_RES_DEPRECATED, &payload);
            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &res,
                KADEMLIA_HELLO_RES_DEPRECATED,
            )
            .await;
        }

        KADEMLIA2_BOOTSTRAP_REQ => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            let contacts = svc.routing.closest_to(crypto.my_kad_id, 20, from_hash);
            let kad2_contacts = contacts
                .into_iter()
                .map(|n| crate::kad::wire::Kad2Contact {
                    kad_version: n.kad_version,
                    node_id: KadId(n.client_id),
                    udp_dest: n.udp_dest,
                })
                .collect::<Vec<_>>();
            let payload =
                encode_kad2_bootstrap_res(crypto.my_kad_id, 8, &crypto.my_dest, &kad2_contacts);
            let res_plain = KadPacket::encode(KADEMLIA2_BOOTSTRAP_RES, &payload);
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
            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_BOOTSTRAP_RES,
            )
            .await;
        }

        KADEMLIA2_BOOTSTRAP_RES => {
            if let Ok(res) = decode_kad2_bootstrap_res(&pkt.payload) {
                svc.stats_window.recv_bootstrap_ress += 1;
                svc.stats_window.bootstrap_contacts += res.contacts.len() as u64;

                let before = svc.routing.len();
                // Sender itself.
                if let Some(raw) = &from_dest_raw
                    && raw.len() == I2P_DEST_LEN
                {
                    let mut udp_dest = [0u8; I2P_DEST_LEN];
                    udp_dest.copy_from_slice(raw);
                    let _ = svc.routing.upsert(
                        ImuleNode {
                            kad_version: res.sender_kad_version,
                            client_id: res.sender_id.0,
                            udp_dest,
                            udp_key: if decrypted.was_obfuscated {
                                decrypted.sender_verify_key
                            } else {
                                0
                            },
                            udp_key_ip: if decrypted.was_obfuscated {
                                crypto.my_dest_hash
                            } else {
                                0
                            },
                            verified: valid_receiver_key,
                        },
                        now,
                    );
                }
                svc.routing.mark_seen_by_dest(&from_dest_b64, now);
                if let Err(err) =
                    maybe_hello_on_inbound(svc, sock, crypto, cfg, &from_dest_b64, now).await
                {
                    tracing::debug!(
                        error = %err,
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        "failed HELLO preflight on BOOTSTRAP_RES"
                    );
                }

                // Harvest contacts list.
                for c in res.contacts {
                    let _ = svc.routing.upsert(
                        ImuleNode {
                            kad_version: c.kad_version,
                            client_id: c.node_id.0,
                            udp_dest: c.udp_dest,
                            udp_key: 0,
                            udp_key_ip: 0,
                            verified: false,
                        },
                        now,
                    );
                }

                let inserted = svc.routing.len().saturating_sub(before);
                if inserted > 0 {
                    svc.stats_window.new_nodes += inserted as u64;
                    tracing::debug!(
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        inserted,
                        routing = svc.routing.len(),
                        "learned new nodes from KAD2 BOOTSTRAP_RES"
                    );
                }
            }
        }

        KADEMLIA2_HELLO_REQ => {
            svc.stats_window.recv_hello_reqs += 1;
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
                let _ = svc.routing.upsert(
                    ImuleNode {
                        kad_version: hello.kad_version,
                        client_id: hello.node_id.0,
                        udp_dest,
                        udp_key: if decrypted.was_obfuscated {
                            decrypted.sender_verify_key
                        } else {
                            0
                        },
                        udp_key_ip: if decrypted.was_obfuscated {
                            crypto.my_dest_hash
                        } else {
                            0
                        },
                        verified: valid_receiver_key,
                    },
                    now,
                );
            }
            svc.routing.mark_received_hello_by_dest(&from_dest_b64, now);

            if let Some(agent) = &hello.agent
                && let Some(st) = svc.routing.get_mut_by_dest(&from_dest_b64)
                && st.peer_agent.is_none()
            {
                st.peer_agent = Some(agent.clone());
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    agent = %agent,
                    "learned peer agent from Kad2 HELLO_REQ"
                );
            }

            let receiver_verify_key = decrypted.sender_verify_key;
            let sender_verify_key = udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);

            let mut res_payload = encode_kad2_hello(8, crypto.my_kad_id, &crypto.my_dest);
            // Ask for HELLO_RES_ACK (TAG_KADMISCOPTIONS bit 0x04).
            // NOTE: `encode_kad2_hello` already includes one tag (our agent). Bump count and append misc options.
            let tag_count_idx = 1 + 16 + I2P_DEST_LEN;
            res_payload[tag_count_idx] = 2; // tag count
            res_payload.push(0x89); // TAGTYPE_UINT8 | 0x80 (numeric)
            res_payload.push(TAG_KADMISCOPTIONS);
            res_payload.push(0x04);

            let res_plain = KadPacket::encode(KADEMLIA2_HELLO_RES, &res_payload);
            let out = if hello.kad_version >= 6 && decrypted.was_obfuscated {
                udp_crypto::encrypt_kad_packet(
                    &res_plain,
                    hello.node_id,
                    receiver_verify_key,
                    sender_verify_key,
                )?
            } else {
                res_plain
            };

            if shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_HELLO_RES,
            )
            .await
            .is_ok_and(|sent| sent)
            {
                track_outgoing_request(svc, &from_dest_b64, KADEMLIA2_HELLO_RES, now, None);
            }
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
                let _ = svc.routing.upsert(
                    ImuleNode {
                        kad_version: hello.kad_version,
                        client_id: hello.node_id.0,
                        udp_dest,
                        udp_key: if decrypted.was_obfuscated {
                            decrypted.sender_verify_key
                        } else {
                            0
                        },
                        udp_key_ip: if decrypted.was_obfuscated {
                            crypto.my_dest_hash
                        } else {
                            0
                        },
                        verified: valid_receiver_key,
                    },
                    now,
                );
            }
            svc.routing.mark_received_hello_by_dest(&from_dest_b64, now);

            if let Some(agent) = &hello.agent
                && let Some(st) = svc.routing.get_mut_by_dest(&from_dest_b64)
                && st.peer_agent.is_none()
            {
                st.peer_agent = Some(agent.clone());
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    agent = %agent,
                    "learned peer agent from Kad2 HELLO_RES"
                );
            }

            let misc = hello.tags.get(&TAG_KADMISCOPTIONS).copied().unwrap_or(0) as u8;
            let wants_ack = (misc & 0x04) != 0;
            if wants_ack && decrypted.sender_verify_key != 0 {
                let mut ack_payload = Vec::with_capacity(16 + 1);
                ack_payload.extend_from_slice(&crypto.my_kad_id.to_crypt_bytes());
                ack_payload.push(0);
                let ack_plain = KadPacket::encode(KADEMLIA2_HELLO_RES_ACK, &ack_payload);
                let sender_verify_key =
                    udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
                let ack = udp_crypto::encrypt_kad_packet_with_receiver_key(
                    &ack_plain,
                    decrypted.sender_verify_key,
                    sender_verify_key,
                )?;
                if shaper_send(
                    svc,
                    sock,
                    cfg,
                    OutboundClass::Response,
                    &from_dest_b64,
                    &ack,
                    KADEMLIA2_HELLO_RES_ACK,
                )
                .await
                .is_ok_and(|sent| sent)
                {
                    svc.stats_window.sent_hello_acks += 1;
                    tracing::debug!(
                        to = %crate::i2p::b64::short(&from_dest_b64),
                        "sent HELLO_RES_ACK"
                    );
                }
            } else if wants_ack {
                svc.stats_window.hello_ack_skipped_no_sender_key += 1;
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    "skipped HELLO_RES_ACK (missing sender key)"
                );
            }
            svc.stats_window.recv_hello_ress += 1;
        }

        KADEMLIA2_HELLO_RES_ACK => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            svc.stats_window.recv_hello_acks += 1;
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                "got HELLO_RES_ACK"
            );
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

            // If this peer included its sender ID (iMule-style), learn it even if we haven't
            // seen a HELLO yet. This helps the routing table grow when peers initiate contact.
            if let (Some(sender_id), Some(raw)) = (req.sender_id, &from_dest_raw)
                && raw.len() == I2P_DEST_LEN
            {
                let before = svc.routing.len();
                let mut udp_dest = [0u8; I2P_DEST_LEN];
                udp_dest.copy_from_slice(raw);
                let _ = svc.routing.upsert(
                    ImuleNode {
                        // We only know it's Kad2 because it's using Kad2 opcodes. Use a conservative
                        // minimum so we can query it; HELLO/BOOTSTRAP will refine this later.
                        kad_version: 6,
                        client_id: sender_id.0,
                        udp_dest,
                        udp_key: if decrypted.was_obfuscated {
                            decrypted.sender_verify_key
                        } else {
                            0
                        },
                        udp_key_ip: if decrypted.was_obfuscated {
                            crypto.my_dest_hash
                        } else {
                            0
                        },
                        verified: valid_receiver_key,
                    },
                    now,
                );
                let inserted = svc.routing.len().saturating_sub(before);
                if inserted > 0 {
                    svc.stats_window.new_nodes += inserted as u64;
                    tracing::debug!(
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        inserted,
                        routing = svc.routing.len(),
                        "learned new node from inbound KAD2 REQ sender_id"
                    );
                }
            }
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);

            let max = (req.requested_contacts as usize).min(32);
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
            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_RES,
            )
            .await;
        }

        KADEMLIA_REQ_DEPRECATED => {
            svc.stats_window.dropped_legacy_kad1 += 1;
            tracing::debug!(
                event = "kad_inbound_drop",
                opcode = format_args!("0x{:02x}", pkt.opcode),
                opcode_name = kad_opcode_name(pkt.opcode),
                from = %crate::i2p::b64::short(&from_dest_b64),
                reason = "legacy_kad1_disabled",
                "dropped inbound KAD1 request"
            );
            return Ok(());
        }

        KADEMLIA2_RES => {
            let res = match decode_kad2_res(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 RES payload");
                    return Ok(());
                }
            };
            let target = res.target;
            let contacts = res.contacts;
            let contact_ids = contacts.iter().map(|c| c.node_id).collect::<Vec<_>>();
            tracing::trace!(from = %from_dest_b64, contacts = contacts.len(), "got KAD2 RES");
            svc.pending_reqs.remove(&from_dest_b64);
            svc.stats_window.recv_ress += 1;
            svc.stats_window.res_contacts += contacts.len() as u64;
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            if let Err(err) =
                maybe_hello_on_inbound(svc, sock, crypto, cfg, &from_dest_b64, now).await
            {
                tracing::debug!(
                    error = %err,
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    "failed HELLO preflight on KAD2 RES"
                );
            }
            let mut total = 0usize;
            let mut inserted = 0usize;
            let mut updated = 0usize;
            let mut ignored_zero = 0usize;
            let mut ignored_self = 0usize;
            let mut already_id = 0usize;
            let mut already_dest = 0usize;
            let mut dest_mismatch = 0usize;
            for c in contacts {
                total += 1;
                let id = c.node_id;
                let node = ImuleNode {
                    kad_version: c.kad_version,
                    client_id: c.node_id.0,
                    udp_dest: c.udp_dest,
                    udp_key: 0,
                    udp_key_ip: 0,
                    verified: false,
                };
                let dest_b64 = node.udp_dest_b64();
                if svc.routing.contains_id(id) {
                    already_id += 1;
                }
                if let Some(existing_id) = svc.routing.id_for_dest(&dest_b64) {
                    if existing_id == id {
                        already_dest += 1;
                    } else {
                        dest_mismatch += 1;
                    }
                }
                let outcome = svc.routing.upsert(
                    ImuleNode {
                        kad_version: node.kad_version,
                        client_id: node.client_id,
                        udp_dest: node.udp_dest,
                        udp_key: node.udp_key,
                        udp_key_ip: node.udp_key_ip,
                        verified: node.verified,
                    },
                    now,
                );
                match outcome {
                    crate::kad::routing::UpsertOutcome::Inserted => inserted += 1,
                    crate::kad::routing::UpsertOutcome::Updated => updated += 1,
                    crate::kad::routing::UpsertOutcome::IgnoredZeroId => ignored_zero += 1,
                    crate::kad::routing::UpsertOutcome::IgnoredSelf => ignored_self += 1,
                }
            }
            if inserted > 0 {
                svc.stats_window.new_nodes += inserted as u64;
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    inserted,
                    routing = svc.routing.len(),
                    "learned new nodes from KAD2 RES"
                );
            }
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                total,
                inserted,
                updated,
                ignored_zero,
                ignored_self,
                already_id,
                already_dest,
                dest_mismatch,
                routing = svc.routing.len(),
                "KAD2 RES contact acceptance stats"
            );

            handle_lookup_response(
                svc,
                now,
                target,
                &from_dest_b64,
                &contact_ids,
                inserted as u64,
            );

            // If this response was part of a user-initiated keyword lookup/publish, nudge the
            // job forward immediately instead of waiting for the next maintenance tick.
            if svc.keyword_jobs.contains_key(&target) {
                progress_keyword_job(svc, sock, crypto, cfg, target, now).await?;
            }
        }

        KADEMLIA2_PING => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
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
            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_PONG,
            )
            .await;
        }

        KADEMLIA2_PONG => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
        }

        KADEMLIA2_PUBLISH_KEY_REQ => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            // Be lenient: if we can't parse the full payload, we still want to ACK (if we can
            // read the keyword prefix) to stop retransmits, and store any entries we did parse.
            let (keyword, entries, complete, declared_count) =
                match decode_kad2_publish_key_req_lenient(&pkt.payload) {
                    Ok(r) => (r.keyword, r.entries, r.complete, r.declared_count),
                    Err(err) => {
                        svc.stats_window.recv_publish_key_decode_failures += 1;

                        let short = crate::i2p::b64::short(&from_dest_b64).to_string();
                        if svc
                            .publish_key_decode_fail_logged
                            .insert(from_dest_b64.clone())
                        {
                            if svc.publish_key_decode_fail_logged.len() > 2048 {
                                svc.publish_key_decode_fail_logged.clear();
                            }
                            tracing::warn!(
                                from = %short,
                                error = %err,
                                len = pkt.payload.len(),
                                head_hex = %hex_head(&pkt.payload, 64),
                                "failed to decode KAD2 PUBLISH_KEY_REQ payload (lenient); will try to ACK by prefix"
                            );
                        }

                        // Try to extract the keyword prefix for an ACK.
                        let keyword = match decode_kad2_publish_key_keyword_prefix(&pkt.payload) {
                            Ok(k) => k,
                            Err(_) => return Ok(()),
                        };

                        // Reply with Kad2 publish result (key shape) so peers stop retransmitting.
                        let res_payload = encode_kad2_publish_res_for_key(keyword, 0);
                        let res_plain = KadPacket::encode(KADEMLIA2_PUBLISH_RES, &res_payload);
                        let sender_verify_key =
                            udp_crypto::udp_verify_key(crypto.udp_key_secret, from_hash);
                        let out = if decrypted.was_obfuscated && decrypted.sender_verify_key != 0 {
                            udp_crypto::encrypt_kad_packet_with_receiver_key(
                                &res_plain,
                                decrypted.sender_verify_key,
                                sender_verify_key,
                            )?
                        } else {
                            res_plain
                        };
                        if shaper_send(
                            svc,
                            sock,
                            cfg,
                            OutboundClass::Response,
                            &from_dest_b64,
                            &out,
                            KADEMLIA2_PUBLISH_RES,
                        )
                        .await
                        .is_ok_and(|sent| sent)
                        {
                            svc.stats_window.sent_publish_key_ress += 1;
                        }
                        return Ok(());
                    }
                };

            svc.stats_window.recv_publish_key_reqs += 1;
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                keyword = %crate::logging::redact_hex(&keyword.to_hex_lower()),
                declared = declared_count,
                parsed = entries.len(),
                complete,
                len = pkt.payload.len(),
                "recv PUBLISH_KEY_REQ"
            );
            if !complete {
                svc.stats_window.recv_publish_key_decode_failures += 1;

                let short = crate::i2p::b64::short(&from_dest_b64).to_string();
                if svc
                    .publish_key_decode_fail_logged
                    .insert(from_dest_b64.clone())
                {
                    if svc.publish_key_decode_fail_logged.len() > 2048 {
                        svc.publish_key_decode_fail_logged.clear();
                    }
                    tracing::warn!(
                        event = "publish_key_req_partial_decode",
                        from = %short,
                        keyword = %crate::logging::redact_hex(&keyword.to_hex_lower()),
                        declared = declared_count,
                        parsed = entries.len(),
                        len = pkt.payload.len(),
                        head_hex = %hex_head(&pkt.payload, 64),
                        "truncated/unparseable KAD2 PUBLISH_KEY_REQ payload; storing partial entries and ACKing"
                    );
                }
            }

            let mut inserted = 0u64;
            for e in entries {
                let (Some(filename), Some(file_size)) = (e.filename, e.file_size) else {
                    continue;
                };
                if filename.is_empty() {
                    continue;
                }
                let m = svc.keyword_store_by_keyword.entry(keyword).or_default();
                match m.get_mut(&e.file) {
                    Some(st) => {
                        st.hit.filename = filename;
                        st.hit.file_size = file_size;
                        st.hit.file_type = e.file_type;
                        st.hit.publish_info = None;
                        st.last_seen = now;
                    }
                    None => {
                        m.insert(
                            e.file,
                            KeywordHitState {
                                hit: KadKeywordHit {
                                    file_id: e.file,
                                    filename,
                                    file_size,
                                    file_type: e.file_type,
                                    publish_info: None,
                                    origin: KadKeywordHitOrigin::Network,
                                },
                                last_seen: now,
                            },
                        );
                        svc.keyword_store_total = svc.keyword_store_total.saturating_add(1);
                        inserted += 1;
                    }
                }
            }

            if inserted > 0 {
                svc.stats_window.new_store_keyword_hits += inserted;
                enforce_keyword_store_limits(svc, cfg, now);
            }

            // Reply with Kad2 publish result (key shape) so peers stop retransmitting.
            let res_payload = encode_kad2_publish_res_for_key(keyword, 0);
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
            if shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_PUBLISH_RES,
            )
            .await
            .is_ok_and(|sent| sent)
            {
                svc.stats_window.sent_publish_key_ress += 1;
            }
        }

        KADEMLIA2_PUBLISH_SOURCE_REQ => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            svc.stats_window.recv_publish_source_reqs += 1;
            let req = match decode_kad2_publish_source_req_min(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    svc.stats_window.recv_publish_source_decode_failures += 1;
                    tracing::debug!(
                        error = %err,
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        "failed to decode KAD2 PUBLISH_SOURCE_REQ payload"
                    );
                    return Ok(());
                }
            };
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                file = %crate::logging::redact_hex(&req.file.to_hex_lower()),
                source = %crate::logging::redact_hex(&req.source.to_hex_lower()),
                "recv PUBLISH_SOURCE_REQ"
            );

            let mut inserted = false;
            if let Some(raw) = &from_dest_raw
                && raw.len() == I2P_DEST_LEN
            {
                let mut udp_dest = [0u8; I2P_DEST_LEN];
                udp_dest.copy_from_slice(raw);
                inserted = svc
                    .sources_by_file
                    .entry(req.file)
                    .or_default()
                    .insert(req.source, udp_dest)
                    .is_none();
                if inserted {
                    svc.stats_window.new_store_source_entries += 1;
                }
            } else {
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    file = %crate::logging::redact_hex(&req.file.to_hex_lower()),
                    source = %crate::logging::redact_hex(&req.source.to_hex_lower()),
                    "skipping source store insert (missing/invalid sender destination bytes)"
                );
            }

            let count = svc
                .sources_by_file
                .get(&req.file)
                .map(|m| m.len() as u32)
                .unwrap_or(0);
            let (source_store_files, source_store_entries_total) = source_store_totals(svc);
            tracing::info!(
                event = "source_store_update",
                from = %crate::i2p::b64::short(&from_dest_b64),
                file = %crate::logging::redact_hex(&req.file.to_hex_lower()),
                source = %crate::logging::redact_hex(&req.source.to_hex_lower()),
                inserted,
                file_sources = count,
                source_store_files,
                source_store_entries_total,
                "source store update from PUBLISH_SOURCE_REQ"
            );

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

            if shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_PUBLISH_RES,
            )
            .await
            .is_ok_and(|sent| sent)
            {
                svc.stats_window.sent_publish_source_ress += 1;
            }
        }

        KADEMLIA2_SEARCH_KEY_REQ => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            svc.stats_window.recv_search_key_reqs += 1;
            let req = match decode_kad2_search_key_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        "failed to decode KAD2 SEARCH_KEY_REQ payload"
                    );
                    return Ok(());
                }
            };
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                target = %crate::logging::redact_hex(&req.target.to_hex_lower()),
                start = req.start_position,
                restrictive = req.restrictive,
                "recv SEARCH_KEY_REQ"
            );

            let results = svc
                .keyword_store_by_keyword
                .get(&req.target)
                .map(|m| {
                    m.values()
                        .skip(req.start_position as usize)
                        .take(64)
                        .map(|st| {
                            (
                                st.hit.file_id,
                                st.hit.filename.clone(),
                                st.hit.file_size,
                                st.hit.file_type.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let payload = encode_kad2_search_res_keyword(crypto.my_kad_id, req.target, &results);
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

            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_SEARCH_RES,
            )
            .await;
        }

        KADEMLIA2_SEARCH_SOURCE_REQ => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            svc.stats_window.recv_search_source_reqs += 1;
            let req = match decode_kad2_search_source_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    svc.stats_window.recv_search_source_decode_failures += 1;
                    tracing::debug!(
                        error = %err,
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        "failed to decode KAD2 SEARCH_SOURCE_REQ payload"
                    );
                    return Ok(());
                }
            };
            tracing::debug!(
                from = %crate::i2p::b64::short(&from_dest_b64),
                target = %crate::logging::redact_hex(&req.target.to_hex_lower()),
                start = req.start_position,
                file_size = req.file_size,
                "recv SEARCH_SOURCE_REQ"
            );

            let total_sources_for_file = svc
                .sources_by_file
                .get(&req.target)
                .map(BTreeMap::len)
                .unwrap_or(0);
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
            let returned = results.len();
            if returned > 0 {
                svc.stats_window.source_search_hits += 1;
            } else {
                svc.stats_window.source_search_misses += 1;
            }
            svc.stats_window.source_search_results_served += returned as u64;
            tracing::info!(
                event = "source_store_query",
                from = %crate::i2p::b64::short(&from_dest_b64),
                file = %crate::logging::redact_hex(&req.target.to_hex_lower()),
                start = req.start_position,
                file_size = req.file_size,
                available = total_sources_for_file,
                returned,
                hit = returned > 0,
                "served SEARCH_SOURCE_REQ from source store"
            );

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

            let _ = shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Response,
                &from_dest_b64,
                &out,
                KADEMLIA2_SEARCH_RES,
            )
            .await;
        }

        KADEMLIA2_SEARCH_RES => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            if let Err(err) =
                maybe_hello_on_inbound(svc, sock, crypto, cfg, &from_dest_b64, now).await
            {
                tracing::debug!(
                    error = %err,
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    "failed HELLO preflight on SEARCH_RES"
                );
            }
            let res: Kad2SearchRes = match decode_kad2_search_res(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(
                        event = "source_probe_search_res_decode_failed",
                        error = %err,
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        payload_len = pkt.payload.len(),
                        "failed to decode KAD2 SEARCH_RES payload"
                    );
                    return Ok(());
                }
            };

            svc.stats_window.recv_search_ress += 1;
            let results_len = res.results.len();
            svc.stats_window.search_results += results_len as u64;

            let mut keyword_entries = 0u64;
            let mut inserted_sources = 0u64;
            let mut inserted_keywords = 0u64;
            let mut source_results_in_packet = 0u64;

            for r in res.results {
                if let Some(dest) = r.tags.best_udp_dest() {
                    // Source-style result: key = file ID, answer = source ID.
                    let m = svc.sources_by_file.entry(res.key).or_default();
                    source_results_in_packet = source_results_in_packet.saturating_add(1);
                    if m.insert(r.answer, dest).is_none() {
                        inserted_sources += 1;
                        svc.stats_window.new_store_source_entries += 1;
                    }
                    continue;
                }

                if let (Some(filename), Some(file_size)) =
                    (r.tags.filename.clone(), r.tags.file_size)
                {
                    // Keyword-style result: key = keyword hash, answer = file ID.
                    if cfg.keyword_require_interest && !svc.keyword_interest.contains_key(&res.key)
                    {
                        continue;
                    }
                    keyword_entries += 1;
                    let hit = KadKeywordHit {
                        file_id: r.answer,
                        filename,
                        file_size,
                        file_type: r.tags.file_type.clone(),
                        publish_info: r.tags.publish_info,
                        origin: KadKeywordHitOrigin::Network,
                    };
                    let m = svc.keyword_hits_by_keyword.entry(res.key).or_default();
                    match m.get_mut(&hit.file_id) {
                        Some(state) => {
                            state.hit = hit;
                            state.last_seen = now;
                        }
                        None => {
                            m.insert(
                                hit.file_id,
                                KeywordHitState {
                                    hit,
                                    last_seen: now,
                                },
                            );
                            svc.keyword_hits_total = svc.keyword_hits_total.saturating_add(1);
                            inserted_keywords += 1;
                        }
                    }
                }
            }

            if keyword_entries > 0 {
                enforce_keyword_per_keyword_cap(svc, cfg, res.key);
                enforce_keyword_total_cap(svc, cfg, now);
            }

            if inserted_sources > 0 {
                svc.stats_window.new_sources += inserted_sources;
            }
            if keyword_entries > 0 {
                svc.stats_window.keyword_results += keyword_entries;
            }
            if inserted_keywords > 0 {
                svc.stats_window.new_keyword_results += inserted_keywords;
            }
            if source_results_in_packet > 0 || svc.source_probe_by_file.contains_key(&res.key) {
                on_source_search_response(
                    svc,
                    res.key,
                    &from_dest_b64,
                    source_results_in_packet,
                    now,
                );
            }

            if keyword_entries > 0 || inserted_sources > 0 {
                tracing::info!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    key = %res.key.to_hex_lower(),
                    results = results_len,
                    inserted_sources,
                    keyword_entries,
                    inserted_keywords,
                    "got SEARCH_RES (non-empty)"
                );
            } else {
                tracing::debug!(
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    key = %res.key.to_hex_lower(),
                    results = results_len,
                    inserted_sources,
                    keyword_entries,
                    inserted_keywords,
                    "got SEARCH_RES"
                );
            }
        }

        KADEMLIA2_PUBLISH_RES => {
            svc.routing.mark_seen_by_dest(&from_dest_b64, now);
            if let Err(err) =
                maybe_hello_on_inbound(svc, sock, crypto, cfg, &from_dest_b64, now).await
            {
                tracing::debug!(
                    error = %err,
                    from = %crate::i2p::b64::short(&from_dest_b64),
                    "failed HELLO preflight on PUBLISH_RES"
                );
            }
            // Publish results can be different shapes (key/source/notes).
            // - Key:   <u128 key><u8 load>
            // - Source: <u128 file><u32 sources><u32 complete><u8 load>
            match pkt.payload.len() {
                17 => {
                    let res: Kad2PublishResKey = match decode_kad2_publish_res_key(&pkt.payload) {
                        Ok(r) => r,
                        Err(err) => {
                            tracing::debug!(
                                event = "source_probe_publish_res_key_decode_failed",
                                error = %err,
                                from = %crate::i2p::b64::short(&from_dest_b64),
                                len = pkt.payload.len(),
                                "unparsed KAD2 PUBLISH_RES (key)"
                            );
                            return Ok(());
                        }
                    };
                    svc.stats_window.recv_publish_key_ress += 1;
                    if let Some(job) = svc.keyword_jobs.get_mut(&res.key)
                        && !job.got_publish_ack
                    {
                        job.got_publish_ack = true;
                        tracing::info!(
                            from = %crate::i2p::b64::short(&from_dest_b64),
                            key = %res.key.to_hex_lower(),
                            load = res.load,
                            "got PUBLISH_RES (key) ack; publish job complete"
                        );
                    } else {
                        tracing::debug!(
                            from = %crate::i2p::b64::short(&from_dest_b64),
                            key = %res.key.to_hex_lower(),
                            load = res.load,
                            "got PUBLISH_RES (key)"
                        );
                    }
                }
                25.. => {
                    let res: Kad2PublishRes = match decode_kad2_publish_res(&pkt.payload) {
                        Ok(r) => r,
                        Err(err) => {
                            tracing::debug!(
                                event = "source_probe_publish_res_source_decode_failed",
                                error = %err,
                                from = %crate::i2p::b64::short(&from_dest_b64),
                                len = pkt.payload.len(),
                                "unparsed KAD2 PUBLISH_RES (source)"
                            );
                            return Ok(());
                        }
                    };
                    svc.stats_window.recv_publish_ress += 1;
                    on_source_publish_response(svc, res.file, &from_dest_b64, now);
                    tracing::debug!(
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        file = %crate::logging::redact_hex(&res.file.to_hex_lower()),
                        sources = res.source_count,
                        complete = res.complete_count,
                        load = res.load,
                        "got PUBLISH_RES (source)"
                    );
                }
                _ => {
                    tracing::trace!(
                        from = %from_dest_b64,
                        len = pkt.payload.len(),
                        "unhandled KAD2 PUBLISH_RES shape"
                    );
                }
            }
        }

        other => {
            let (reason, legacy) = if matches!(
                other,
                KADEMLIA_HELLO_REQ_DEPRECATED
                    | KADEMLIA_HELLO_RES_DEPRECATED
                    | KADEMLIA_RES_DEPRECATED
                    | KADEMLIA_SEARCH_REQ_DEPRECATED
                    | KADEMLIA_SEARCH_RES_DEPRECATED
                    | KADEMLIA_SEARCH_NOTES_REQ_DEPRECATED
                    | KADEMLIA_PUBLISH_REQ_DEPRECATED
                    | KADEMLIA_PUBLISH_RES_DEPRECATED
                    | KADEMLIA_PUBLISH_NOTES_REQ_DEPRECATED
            ) {
                ("legacy_kad1_disabled", true)
            } else {
                ("unhandled_opcode", false)
            };
            if legacy {
                svc.stats_window.dropped_legacy_kad1 += 1;
            } else {
                svc.stats_window.dropped_unhandled_opcode += 1;
            }
            tracing::debug!(
                event = "kad_inbound_drop",
                opcode = format_args!("0x{other:02x}"),
                from = %from_dest_b64,
                len = pkt.payload.len(),
                opcode_name = kad_opcode_name(other),
                reason,
                "dropped inbound KAD packet"
            );
        }
    }

    Ok(())
}
