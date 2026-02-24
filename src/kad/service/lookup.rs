use super::*;

pub(super) fn start_lookup_impl(
    svc: &mut KadService,
    target: KadId,
    kind: LookupKind,
    alpha_override: Option<usize>,
    now: Instant,
) {
    let task = LookupTask {
        kind,
        target,
        started_at: now,
        last_progress: now,
        iteration: 0,
        alpha_override,
        queried: HashSet::new(),
        inflight: HashSet::new(),
        known: BTreeMap::new(),
        new_nodes: 0,
    };
    svc.lookup_queue.push_back(task);
}

pub(super) fn tick_refresh_impl(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    let total = svc.routing.len();
    let underpopulated = total < cfg.refresh_underpopulated_min_contacts;

    let mut bucket_counts = vec![0usize; svc.routing.bucket_count()];
    for st in svc.routing.snapshot_states() {
        if let Some(idx) = svc.routing.bucket_index_for(KadId(st.node.client_id))
            && idx < bucket_counts.len()
        {
            bucket_counts[idx] += 1;
        }
    }

    let mut stale_buckets = Vec::new();
    for (i, count) in bucket_counts
        .iter()
        .copied()
        .enumerate()
        .take(svc.routing.bucket_count())
    {
        let last_activity = svc.routing.bucket_last_activity(i);
        let idle_secs = last_activity
            .map(|t| now.saturating_duration_since(t).as_secs())
            .unwrap_or(u64::MAX / 2);
        let last_refresh = svc.routing.bucket_last_refresh(i);
        let refresh_age = last_refresh
            .map(|t| now.saturating_duration_since(t).as_secs())
            .unwrap_or(u64::MAX / 2);
        if idle_secs >= cfg.refresh_interval_secs && refresh_age >= cfg.refresh_interval_secs / 2 {
            stale_buckets.push((i, idle_secs, count));
        }
    }

    stale_buckets.sort_by_key(|v| std::cmp::Reverse(v.1));

    if underpopulated
        && now
            .saturating_duration_since(svc.last_underpopulated_refresh)
            .as_secs()
            >= cfg.refresh_underpopulated_every_secs
    {
        let mut picked = 0usize;
        for (bucket, idle, count) in stale_buckets
            .iter()
            .take(cfg.refresh_underpopulated_buckets_per_tick)
        {
            if count == &0 {
                tracing::info!(
                    bucket,
                    idle_secs = *idle,
                    bucket_counts = ?bucket_counts,
                    "refreshing empty bucket (underpopulated)"
                );
            }
            let target = random_id_in_bucket_impl(svc.routing.my_id(), *bucket);
            svc.routing.mark_bucket_refreshed(*bucket, now);
            start_lookup_impl(
                svc,
                target,
                LookupKind::Refresh { bucket: *bucket },
                Some(cfg.refresh_underpopulated_alpha.max(1)),
                now,
            );
            picked += 1;
            if picked >= cfg.refresh_underpopulated_buckets_per_tick {
                break;
            }
        }
        if picked > 0 {
            svc.last_underpopulated_refresh = now;
        }
    }

    if now
        .saturating_duration_since(svc.last_refresh_tick)
        .as_secs()
        >= 1
    {
        let mut picked = 0usize;
        for (bucket, idle, count) in stale_buckets.iter().take(cfg.refresh_buckets_per_tick) {
            if count == &0 {
                tracing::info!(
                    bucket,
                    idle_secs = *idle,
                    bucket_counts = ?bucket_counts,
                    "refreshing empty bucket"
                );
            }
            let target = random_id_in_bucket_impl(svc.routing.my_id(), *bucket);
            svc.routing.mark_bucket_refreshed(*bucket, now);
            start_lookup_impl(
                svc,
                target,
                LookupKind::Refresh { bucket: *bucket },
                None,
                now,
            );
            picked += 1;
            if picked >= cfg.refresh_buckets_per_tick {
                break;
            }
        }
        if picked > 0 {
            svc.last_refresh_tick = now;
        }
    }
}

pub(super) async fn tick_lookups_impl(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    let now = Instant::now();

    if svc.active_lookup.is_none()
        && let Some(next) = svc.lookup_queue.pop_front()
    {
        svc.active_lookup = Some(next);
    }

    let Some(mut task) = svc.active_lookup.take() else {
        return Ok(());
    };

    if task.known.is_empty() {
        let peers = closest_peers_by_distance(svc, task.target, 64, 2, 0);
        for p in peers {
            task.known.entry(KadId(p.client_id)).or_insert(p);
        }
    }

    let alpha = task.alpha_override.unwrap_or(cfg.alpha.max(1));
    let mut sent = 0usize;
    let mut dests = Vec::new();
    let candidates = closest_peers_by_distance(svc, task.target, 64, 2, 0);
    for p in candidates {
        if sent >= alpha {
            break;
        }
        let dest = p.udp_dest_b64();
        if task.queried.contains(&dest) || task.inflight.contains(&dest) {
            continue;
        }
        let requested_contacts = cfg.req_contacts.clamp(1, 31);
        if send_kad2_req(svc, sock, crypto, cfg, requested_contacts, task.target, &p)
            .await
            .unwrap_or(false)
        {
            task.queried.insert(dest.clone());
            task.inflight.insert(dest.clone());
            task.known.entry(KadId(p.client_id)).or_insert(p);
            dests.push(crate::i2p::b64::short(&dest).to_string());
            sent += 1;
        }
    }

    if sent > 0 {
        task.iteration += 1;
        let (closest, set_size) = lookup_closest_impl(&task);
        match task.kind {
            LookupKind::Debug => {
                tracing::info!(
                    event = "lookup_debug_step",
                    target = %crate::logging::redact_hex(&task.target.to_hex_lower()),
                    iter = task.iteration,
                    set_size,
                    closest = %closest,
                    inflight = task.inflight.len(),
                    "debug lookup step"
                );
            }
            LookupKind::Refresh { bucket } => {
                tracing::debug!(
                    target = %crate::logging::redact_hex(&task.target.to_hex_lower()),
                    bucket,
                    iter = task.iteration,
                    set_size,
                    closest = %closest,
                    inflight = task.inflight.len(),
                    "refresh lookup step"
                );
            }
        }
    }

    let stalled =
        now.saturating_duration_since(task.last_progress).as_secs() > cfg.req_timeout_secs;
    let elapsed_secs = now.saturating_duration_since(task.started_at).as_secs();
    let done = task.iteration >= 8 || (sent == 0 && task.inflight.is_empty()) || stalled;
    if done {
        match task.kind {
            LookupKind::Debug => {
                tracing::info!(
                    event = "lookup_debug_finished",
                    target = %crate::logging::redact_hex(&task.target.to_hex_lower()),
                    iter = task.iteration,
                    new_nodes = task.new_nodes,
                    stalled,
                    elapsed_secs,
                    "debug lookup finished"
                );
            }
            LookupKind::Refresh { bucket } => {
                if stalled {
                    tracing::debug!(
                        target = %crate::logging::redact_hex(&task.target.to_hex_lower()),
                        bucket,
                        iter = task.iteration,
                        new_nodes = task.new_nodes,
                        elapsed_secs,
                        "refresh lookup stalled"
                    );
                }
            }
        }
    } else {
        svc.active_lookup = Some(task);
    }

    Ok(())
}

pub(super) fn handle_lookup_response_impl(
    svc: &mut KadService,
    now: Instant,
    target: KadId,
    from_dest: &str,
    contacts: &[KadId],
    inserted: u64,
) {
    let Some(task) = svc.active_lookup.as_mut() else {
        return;
    };
    if task.target != target {
        return;
    }
    task.inflight.remove(from_dest);
    if inserted > 0 {
        task.new_nodes = task.new_nodes.saturating_add(inserted);
        task.last_progress = now;
    }
    for id in contacts {
        if !task.known.contains_key(id)
            && let Some(st) = svc.routing.get_by_id(*id)
        {
            task.known.insert(*id, st.node.clone());
        }
    }
}

pub(super) fn lookup_closest_impl(task: &LookupTask) -> (String, usize) {
    let mut best: Option<[u8; 16]> = None;
    for id in task.known.keys() {
        let dist = xor_distance_impl(task.target, *id);
        if best.is_none() || dist < best.expect("best checked for none") {
            best = Some(dist);
        }
    }
    let closest = best
        .map(hex_distance_impl)
        .unwrap_or_else(|| "none".to_string());
    (closest, task.known.len())
}

pub(super) fn hex_distance_impl(dist: [u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in dist {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

pub(super) fn xor_distance_impl(a: KadId, b: KadId) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = a.0[i] ^ b.0[i];
    }
    out
}

pub(super) fn random_id_in_bucket_impl(my_id: KadId, bucket: usize) -> KadId {
    let mut dist = [0u8; 16];
    let mut rand = [0u8; 16];
    let _ = getrandom::getrandom(&mut rand);

    for bit in 0..128 {
        let byte = bit / 8;
        let bit_in_byte = 7 - (bit % 8);
        let mask = 1u8 << bit_in_byte;
        let set = if bit == bucket {
            true
        } else if bit > bucket {
            (rand[byte] & mask) != 0
        } else {
            false
        };
        if set {
            dist[byte] |= mask;
        }
    }

    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = my_id.0[i] ^ dist[i];
    }
    KadId(out)
}
