use super::{KadService, RoutingBucketSummary, RoutingNodeSummary, RoutingSummary};
use crate::kad::KadId;
use tokio::time::Instant;

fn age_secs(now: Instant, t: Option<Instant>) -> Option<u64> {
    t.map(|ts| now.saturating_duration_since(ts).as_secs())
}

pub(super) fn build_routing_summary(svc: &KadService, now: Instant) -> RoutingSummary {
    let states = svc.routing.snapshot_states();
    let bucket_count = svc.routing.bucket_count();
    let mut bucket_counts = vec![0usize; bucket_count];
    let mut verified = 0usize;
    let mut unverified = 0usize;
    let mut last_seen_ages = Vec::with_capacity(states.len());
    let mut last_inbound_ages = Vec::with_capacity(states.len());

    for st in &states {
        if st.node.verified {
            verified += 1;
        } else {
            unverified += 1;
        }
        if let Some(idx) = svc.routing.bucket_index_for(KadId(st.node.client_id))
            && idx < bucket_counts.len()
        {
            bucket_counts[idx] += 1;
        }
        last_seen_ages.push(now.saturating_duration_since(st.last_seen).as_secs());
        if let Some(t) = st.last_inbound {
            last_inbound_ages.push(now.saturating_duration_since(t).as_secs());
        }
    }

    let buckets_empty = bucket_counts.iter().filter(|v| **v == 0).count();
    let mut counts_sorted = bucket_counts.clone();
    counts_sorted.sort_unstable();
    let bucket_fill_min = *counts_sorted.first().unwrap_or(&0);
    let bucket_fill_max = *counts_sorted.last().unwrap_or(&0);
    let bucket_fill_median = if counts_sorted.is_empty() {
        0
    } else {
        counts_sorted[counts_sorted.len() / 2]
    };

    let last_seen_min_secs = last_seen_ages.iter().copied().min();
    let last_seen_max_secs = last_seen_ages.iter().copied().max();
    let last_inbound_min_secs = last_inbound_ages.iter().copied().min();
    let last_inbound_max_secs = last_inbound_ages.iter().copied().max();

    RoutingSummary {
        bucket_count,
        total_nodes: states.len(),
        verified_nodes: verified,
        unverified_nodes: unverified,
        buckets_empty,
        bucket_fill_min,
        bucket_fill_median,
        bucket_fill_max,
        last_seen_min_secs,
        last_seen_max_secs,
        last_inbound_min_secs,
        last_inbound_max_secs,
    }
}

pub(super) fn build_routing_buckets(svc: &KadService, now: Instant) -> Vec<RoutingBucketSummary> {
    let bucket_count = svc.routing.bucket_count();
    let mut buckets = vec![
        RoutingBucketSummary {
            index: 0,
            count: 0,
            verified: 0,
            unverified: 0,
            last_seen_min_secs: None,
            last_seen_max_secs: None,
            last_inbound_min_secs: None,
            last_inbound_max_secs: None,
            last_activity_secs_ago: None,
            last_refresh_secs_ago: None,
        };
        bucket_count
    ];

    for (i, b) in buckets.iter_mut().enumerate() {
        b.index = i;
        b.last_activity_secs_ago = age_secs(now, svc.routing.bucket_last_activity(i));
        b.last_refresh_secs_ago = age_secs(now, svc.routing.bucket_last_refresh(i));
    }

    for st in svc.routing.snapshot_states() {
        let Some(idx) = svc.routing.bucket_index_for(KadId(st.node.client_id)) else {
            continue;
        };
        let b = &mut buckets[idx];
        b.count += 1;
        if st.node.verified {
            b.verified += 1;
        } else {
            b.unverified += 1;
        }
        let seen_age = now.saturating_duration_since(st.last_seen).as_secs();
        b.last_seen_min_secs = Some(b.last_seen_min_secs.map_or(seen_age, |v| v.min(seen_age)));
        b.last_seen_max_secs = Some(b.last_seen_max_secs.map_or(seen_age, |v| v.max(seen_age)));
        if let Some(t) = st.last_inbound {
            let inbound_age = now.saturating_duration_since(t).as_secs();
            b.last_inbound_min_secs = Some(
                b.last_inbound_min_secs
                    .map_or(inbound_age, |v| v.min(inbound_age)),
            );
            b.last_inbound_max_secs = Some(
                b.last_inbound_max_secs
                    .map_or(inbound_age, |v| v.max(inbound_age)),
            );
        }
    }

    buckets
}

pub(super) fn build_routing_nodes(
    svc: &KadService,
    now: Instant,
    bucket: usize,
) -> Vec<RoutingNodeSummary> {
    let mut nodes = Vec::new();
    for st in svc.routing.snapshot_states() {
        let Some(idx) = svc.routing.bucket_index_for(KadId(st.node.client_id)) else {
            continue;
        };
        if idx != bucket {
            continue;
        }
        nodes.push(RoutingNodeSummary {
            kad_id_hex: KadId(st.node.client_id).to_hex_lower(),
            udp_dest_b64: st.dest_b64.clone(),
            udp_dest_short: crate::i2p::b64::short(&st.dest_b64).to_string(),
            kad_version: st.node.kad_version,
            verified: st.node.verified,
            failures: st.failures,
            last_seen_secs_ago: now.saturating_duration_since(st.last_seen).as_secs(),
            last_inbound_secs_ago: age_secs(now, st.last_inbound),
        });
    }
    nodes
}
