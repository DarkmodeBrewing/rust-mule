use crate::{kad::KadId, nodes::imule::ImuleNode};
use std::collections::{BTreeMap, HashMap};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NodeState {
    pub node: ImuleNode,
    pub dest_b64: String,
    pub last_seen: Instant,
    /// Optional vendor/agent string learned from Kad2 HELLO tags.
    ///
    /// This is not persisted to disk (nodes.dat is iMule format); it is a runtime hint to
    /// distinguish rust-mule peers from iMule/aMule peers for feature gating/debugging.
    pub peer_agent: Option<String>,
    /// Last time we received *any* packet from this destination.
    pub last_inbound: Option<Instant>,
    pub last_queried: Option<Instant>,
    pub last_bootstrap: Option<Instant>,
    pub last_hello: Option<Instant>,
    pub received_hello: bool,
    pub needs_hello: bool,
    pub failures: u32,
}

/// Minimal Kademlia routing table.
///
/// This is not a full bucketed Kademlia implementation yet; it is a stable in-memory set of
/// known nodes with helpers to pick the closest peers for lookups and to snapshot/persist nodes.
#[derive(Debug)]
pub struct RoutingTable {
    my_id: KadId,
    by_id: BTreeMap<KadId, NodeState>,
    by_dest: HashMap<String, KadId>,
    bucket_activity: Vec<Option<Instant>>,
    bucket_last_refresh: Vec<Option<Instant>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    Updated,
    IgnoredZeroId,
    IgnoredSelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerHealthClass {
    Unknown,
    Verified,
    Stable,
    Unreliable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeerHealthCounts {
    pub unknown: usize,
    pub verified: usize,
    pub stable: usize,
    pub unreliable: usize,
}

impl RoutingTable {
    pub fn new(my_id: KadId) -> Self {
        Self {
            my_id,
            by_id: BTreeMap::new(),
            by_dest: HashMap::new(),
            bucket_activity: vec![None; BUCKET_COUNT],
            bucket_last_refresh: vec![None; BUCKET_COUNT],
        }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn my_id(&self) -> KadId {
        self.my_id
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn live_count(&self) -> usize {
        self.by_id
            .values()
            .filter(|st| st.last_inbound.is_some())
            .count()
    }

    pub fn live_count_recent(&self, now: Instant, window: Duration) -> usize {
        self.by_id
            .values()
            .filter(|st| is_recent_inbound(st, now, window))
            .count()
    }

    pub fn peer_health_counts(&self, now: Instant) -> PeerHealthCounts {
        let mut counts = PeerHealthCounts::default();
        for st in self.by_id.values() {
            match classify_peer_health(st, now) {
                PeerHealthClass::Unknown => counts.unknown += 1,
                PeerHealthClass::Verified => counts.verified += 1,
                PeerHealthClass::Stable => counts.stable += 1,
                PeerHealthClass::Unreliable => counts.unreliable += 1,
            }
        }
        counts
    }

    pub fn peer_health_class_by_dest(
        &self,
        dest_b64: &str,
        now: Instant,
    ) -> Option<PeerHealthClass> {
        let st = self.get_by_dest(dest_b64)?;
        Some(classify_peer_health(st, now))
    }

    pub fn contains_id(&self, id: KadId) -> bool {
        self.by_id.contains_key(&id)
    }

    pub fn get_by_id(&self, id: KadId) -> Option<&NodeState> {
        self.by_id.get(&id)
    }

    pub fn get_by_dest(&self, dest_b64: &str) -> Option<&NodeState> {
        let id = self.by_dest.get(dest_b64)?;
        self.by_id.get(id)
    }

    pub fn id_for_dest(&self, dest_b64: &str) -> Option<KadId> {
        self.by_dest.get(dest_b64).copied()
    }

    pub fn get_mut_by_dest(&mut self, dest_b64: &str) -> Option<&mut NodeState> {
        let id = *self.by_dest.get(dest_b64)?;
        self.by_id.get_mut(&id)
    }

    pub fn upsert(&mut self, node: ImuleNode, now: Instant) -> UpsertOutcome {
        let id = KadId(node.client_id);
        if id.is_zero() {
            return UpsertOutcome::IgnoredZeroId;
        }
        if id == self.my_id {
            return UpsertOutcome::IgnoredSelf;
        }

        let dest_b64 = node.udp_dest_b64();
        let mut legacy_refresh_only = false;
        if let Some(old) = self.by_id.get(&id) {
            if old.node.verified && old.dest_b64 != dest_b64 {
                // Don't allow verified nodes to change destinations.
                return UpsertOutcome::Updated;
            }
            if old.dest_b64 != dest_b64 && node.kad_version < old.node.kad_version {
                // Don't let lower Kad versions overwrite newer endpoints.
                return UpsertOutcome::Updated;
            }
            if old.node.udp_key != 0 && node.udp_key != 0 && old.node.udp_key != node.udp_key {
                // Prevent updates with mismatched sender keys (anti-hijack).
                return UpsertOutcome::Updated;
            }
            if (1..6).contains(&old.node.kad_version) && old.received_hello {
                // Legacy Kad2 contacts are only allowed to refresh their timers once we've
                // seen a HELLO packet (avoid hijacks).
                if old.dest_b64 != dest_b64 || old.node.kad_version != node.kad_version {
                    return UpsertOutcome::Updated;
                }
                legacy_refresh_only = true;
            }
            if old.dest_b64 != dest_b64 {
                self.by_dest.remove(&old.dest_b64);
            }
        }
        self.by_dest.insert(dest_b64.clone(), id);

        let mut inserted = false;
        self.by_id
            .entry(id)
            .and_modify(|st| {
                if !legacy_refresh_only {
                    // Prefer newer/verified entries and keep UDP keys once learned.
                    if node.kad_version > st.node.kad_version {
                        st.node.kad_version = node.kad_version;
                    }
                    if node.udp_key != 0 && st.node.udp_key != node.udp_key {
                        // UDP keys are bound to the sender/receiver tuple. If we learn a new one,
                        // update it eagerly.
                        st.node.udp_key = node.udp_key;
                        st.node.udp_key_ip = node.udp_key_ip;
                    }
                    if st.node.udp_key == 0 && node.udp_key != 0 {
                        st.node.udp_key = node.udp_key;
                        st.node.udp_key_ip = node.udp_key_ip;
                    }
                    if node.verified {
                        st.node.verified = true;
                    }
                }
                st.last_seen = now;
                st.failures = 0;
            })
            .or_insert_with(|| {
                inserted = true;
                NodeState {
                    node,
                    dest_b64,
                    last_seen: now,
                    peer_agent: None,
                    last_inbound: None,
                    last_queried: None,
                    last_bootstrap: None,
                    last_hello: None,
                    received_hello: false,
                    needs_hello: true,
                    failures: 0,
                }
            });

        self.touch_bucket_by_id(id, now);
        if inserted {
            UpsertOutcome::Inserted
        } else {
            UpsertOutcome::Updated
        }
    }

    pub fn mark_seen_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.last_seen = now;
            st.last_inbound = Some(now);
            st.failures = 0;
            let id = KadId(st.node.client_id);
            self.touch_bucket_by_id(id, now);
        }
    }

    pub fn mark_queried_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.last_queried = Some(now);
        }
    }

    pub fn mark_bootstrap_sent_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.last_bootstrap = Some(now);
        }
    }

    pub fn mark_hello_sent_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.last_hello = Some(now);
            st.needs_hello = false;
        }
    }

    pub fn mark_received_hello_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.received_hello = true;
            st.needs_hello = false;
            st.last_seen = now;
            st.last_inbound = Some(now);
            st.failures = 0;
            let id = KadId(st.node.client_id);
            self.touch_bucket_by_id(id, now);
        }
    }

    pub fn mark_failure_by_dest(&mut self, dest_b64: &str) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.failures = st.failures.saturating_add(1);
        }
    }

    pub fn update_sender_keys_by_dest(
        &mut self,
        dest_b64: &str,
        now: Instant,
        sender_verify_key: u32,
        my_dest_hash: u32,
        verified: bool,
    ) {
        let id = match self.by_dest.get(dest_b64).copied() {
            Some(id) => id,
            None => return,
        };
        if let Some(st) = self.by_id.get_mut(&id) {
            st.last_seen = now;
            st.last_inbound = Some(now);
            st.failures = 0;
            if sender_verify_key != 0 && st.node.udp_key != sender_verify_key {
                st.node.udp_key = sender_verify_key;
                st.node.udp_key_ip = my_dest_hash;
            }
            if verified {
                st.node.verified = true;
            }
        }
        self.touch_bucket_by_id(id, now);
    }

    pub fn closest_to(&self, target: KadId, max: usize, exclude_dest_hash: u32) -> Vec<ImuleNode> {
        let mut out: Vec<ImuleNode> = self
            .by_id
            .values()
            .map(|st| st.node.clone())
            .filter(|n| n.kad_version != 0)
            .filter(|n| n.udp_dest_hash_code() != exclude_dest_hash)
            .collect();

        out.sort_by_key(|n| xor_distance(KadId(n.client_id), target));
        out.truncate(max);
        out
    }

    /// Like `closest_to`, but prefers nodes we have heard from recently.
    pub fn closest_to_prefer_live(
        &self,
        target: KadId,
        max: usize,
        exclude_dest_hash: u32,
        now: Instant,
        live_window: Duration,
        min_kad_version: u8,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= min_kad_version)
            .filter(|st| st.node.udp_dest_hash_code() != exclude_dest_hash)
            .collect();

        out.sort_by_key(|st| {
            (
                !is_recent_inbound(st, now, live_window),
                xor_distance(KadId(st.node.client_id), target),
                st.last_queried,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    /// Like `closest_to`, but uses "recently live" as a *tiebreaker* (distance is always primary).
    ///
    /// This is important for DHT correctness: publish/search should target the nodes closest to the
    /// key in XOR-space. Prioritizing liveness over distance can cause us to store/query far away
    /// nodes which other clients won't naturally query.
    pub fn closest_to_distance_first_prefer_live(
        &self,
        target: KadId,
        max: usize,
        exclude_dest_hash: u32,
        now: Instant,
        live_window: Duration,
        min_kad_version: u8,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= min_kad_version)
            .filter(|st| st.node.udp_dest_hash_code() != exclude_dest_hash)
            .collect();

        out.sort_by_key(|st| {
            (
                xor_distance(KadId(st.node.client_id), target),
                !is_recent_inbound(st, now, live_window),
                st.last_queried,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    pub fn snapshot_nodes(&self, max: usize) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self.by_id.values().collect();
        out.sort_by_key(|st| {
            (
                std::cmp::Reverse(st.node.verified),
                std::cmp::Reverse(st.node.udp_key != 0),
                std::cmp::Reverse(st.node.kad_version),
                std::cmp::Reverse(st.last_seen),
                st.failures,
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    pub fn snapshot_states(&self) -> Vec<NodeState> {
        self.by_id.values().cloned().collect()
    }

    pub fn bucket_count(&self) -> usize {
        BUCKET_COUNT
    }

    pub fn bucket_index_for(&self, id: KadId) -> Option<usize> {
        bucket_index(self.my_id, id)
    }

    pub fn bucket_last_activity(&self, bucket: usize) -> Option<Instant> {
        self.bucket_activity.get(bucket).copied().flatten()
    }

    pub fn bucket_last_refresh(&self, bucket: usize) -> Option<Instant> {
        self.bucket_last_refresh.get(bucket).copied().flatten()
    }

    pub fn mark_bucket_refreshed(&mut self, bucket: usize, now: Instant) {
        if let Some(entry) = self.bucket_last_refresh.get_mut(bucket) {
            *entry = Some(now);
        }
    }

    fn touch_bucket_by_id(&mut self, id: KadId, now: Instant) {
        let Some(idx) = bucket_index(self.my_id, id) else {
            return;
        };
        if let Some(entry) = self.bucket_activity.get_mut(idx) {
            *entry = Some(now);
        }
    }

    pub fn select_query_candidates(
        &self,
        max: usize,
        now: Instant,
        base_interval: Duration,
        max_failures: u32,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= 2)
            .filter(|st| st.failures < max_failures)
            .filter(|st| match st.last_queried {
                Some(t) => {
                    now.saturating_duration_since(t) >= backoff_interval(base_interval, st.failures)
                }
                None => true,
            })
            .collect();

        // Prefer nodes we've actually heard from, then oldest-queried, then most recently seen.
        out.sort_by_key(|st| {
            (
                st.last_inbound.is_none(),
                peer_health_rank(classify_peer_health(st, now)),
                st.last_queried,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    /// Select query candidates for a specific lookup target (Kademlia-style).
    ///
    /// We prefer nodes we've heard from (`last_inbound`), but we also bias selection by XOR
    /// distance to `target` so that repeated crawls explore different regions.
    pub fn select_query_candidates_for_target(
        &self,
        target: KadId,
        max: usize,
        now: Instant,
        base_interval: Duration,
        max_failures: u32,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= 2)
            .filter(|st| st.failures < max_failures)
            .filter(|st| match st.last_queried {
                Some(t) => {
                    now.saturating_duration_since(t) >= backoff_interval(base_interval, st.failures)
                }
                None => true,
            })
            .collect();

        out.sort_by_key(|st| {
            (
                st.last_inbound.is_none(),
                peer_health_rank(classify_peer_health(st, now)),
                xor_distance(KadId(st.node.client_id), target),
                st.last_queried,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    pub fn select_hello_candidates(
        &self,
        max: usize,
        now: Instant,
        base_interval: Duration,
        max_failures: u32,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= 2)
            .filter(|st| st.failures < max_failures)
            .filter(|st| {
                st.needs_hello
                    || match st.last_hello {
                        Some(t) => {
                            now.saturating_duration_since(t)
                                >= backoff_interval(base_interval, st.failures)
                        }
                        None => true,
                    }
            })
            .collect();

        // Prefer nodes which haven't been greeted yet, and among those, prefer nodes we haven't
        // heard from (to explore and establish keys with "cold" peers).
        out.sort_by_key(|st| {
            (
                !st.needs_hello,
                st.last_inbound.is_some(),
                st.last_hello,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    pub fn select_bootstrap_candidates(
        &self,
        max: usize,
        now: Instant,
        base_interval: Duration,
        recent_live_window: Duration,
    ) -> Vec<ImuleNode> {
        let mut out: Vec<&NodeState> = self
            .by_id
            .values()
            .filter(|st| st.node.kad_version >= 2)
            .filter(|st| match st.last_bootstrap {
                Some(t) => {
                    now.saturating_duration_since(t)
                        >= bootstrap_backoff_interval(base_interval, st.failures)
                }
                None => true,
            })
            .collect();

        // Discovery strategy:
        // - Prefer "cold" peers (no recent inbound) to diversify our view of the network.
        // - Rotate by oldest-bootstrapped to spread probes across the table.
        out.sort_by_key(|st| {
            (
                is_recent_inbound(st, now, recent_live_window),
                st.last_bootstrap,
                std::cmp::Reverse(st.last_seen),
            )
        });

        out.into_iter()
            .take(max)
            .map(|st| st.node.clone())
            .collect()
    }

    pub fn evict(&mut self, now: Instant, max_failures: u32, max_age: Duration) -> usize {
        let before = self.by_id.len();
        let mut to_remove: Vec<KadId> = Vec::new();
        for (id, st) in &self.by_id {
            if st.failures >= max_failures && now.saturating_duration_since(st.last_seen) >= max_age
            {
                to_remove.push(*id);
            }
        }
        for id in to_remove {
            if let Some(st) = self.by_id.remove(&id) {
                self.by_dest.remove(&st.dest_b64);
            }
        }
        before - self.by_id.len()
    }
}

fn xor_distance(a: KadId, b: KadId) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = a.0[i] ^ b.0[i];
    }
    out
}

const BUCKET_COUNT: usize = 128;

fn bucket_index(my_id: KadId, other: KadId) -> Option<usize> {
    if other == my_id || other.is_zero() {
        return None;
    }
    let dist = xor_distance(my_id, other);
    let lz = leading_zeros_128(&dist);
    if lz >= 128 {
        return None;
    }
    let idx = lz as usize;
    if idx >= BUCKET_COUNT { None } else { Some(idx) }
}

fn leading_zeros_128(bytes: &[u8; 16]) -> u32 {
    let mut count = 0u32;
    for b in bytes {
        if *b == 0 {
            count += 8;
        } else {
            count += b.leading_zeros();
            break;
        }
    }
    count
}

fn backoff_interval(base: Duration, failures: u32) -> Duration {
    // Exponential backoff (capped) to avoid repeatedly hammering dead/stale peers.
    let pow = failures.min(8);
    let mul = 1u32 << pow;
    base.checked_mul(mul)
        .unwrap_or(Duration::from_secs(24 * 60 * 60))
}

fn bootstrap_backoff_interval(base: Duration, failures: u32) -> Duration {
    // We don't want bootstrap refresh to become "effectively disabled" due to timeouts.
    // Keep the per-peer backoff bounded.
    let pow = failures.min(2);
    let mul = 1u32 << pow;
    base.checked_mul(mul)
        .unwrap_or(Duration::from_secs(24 * 60 * 60))
}

fn is_recent_inbound(st: &NodeState, now: Instant, window: Duration) -> bool {
    st.last_inbound
        .is_some_and(|t| now.saturating_duration_since(t) <= window)
}

fn classify_peer_health(st: &NodeState, now: Instant) -> PeerHealthClass {
    const STABLE_RECENT_WINDOW: Duration = Duration::from_secs(10 * 60);
    const UNRELIABLE_FAILURE_THRESHOLD: u32 = 3;

    if st.failures >= UNRELIABLE_FAILURE_THRESHOLD {
        return PeerHealthClass::Unreliable;
    }
    if !st.node.verified {
        return PeerHealthClass::Unknown;
    }
    if is_recent_inbound(st, now, STABLE_RECENT_WINDOW) && st.failures == 0 {
        PeerHealthClass::Stable
    } else {
        PeerHealthClass::Verified
    }
}

fn peer_health_rank(class: PeerHealthClass) -> u8 {
    match class {
        PeerHealthClass::Stable => 0,
        PeerHealthClass::Verified => 1,
        PeerHealthClass::Unknown => 2,
        PeerHealthClass::Unreliable => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_index_uses_msb_distance() {
        let my = KadId([0u8; 16]);
        let mut far = [0u8; 16];
        far[0] = 0x80; // MSB set
        let mut near = [0u8; 16];
        near[15] = 0x01; // LSB set

        assert_eq!(bucket_index(my, KadId(far)), Some(0));
        assert_eq!(bucket_index(my, KadId(near)), Some(127));
    }

    #[test]
    fn peer_health_counts_reflect_unknown_verified_stable_unreliable() {
        let my = KadId([0u8; 16]);
        let now = Instant::now();
        let mut rt = RoutingTable::new(my);

        let n1 = ImuleNode {
            kad_version: 6,
            client_id: [1u8; 16],
            udp_dest: [1u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: false,
        };
        let n2 = ImuleNode {
            kad_version: 6,
            client_id: [2u8; 16],
            udp_dest: [2u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: true,
        };
        let n3 = ImuleNode {
            kad_version: 6,
            client_id: [3u8; 16],
            udp_dest: [3u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: true,
        };
        let n4 = ImuleNode {
            kad_version: 6,
            client_id: [4u8; 16],
            udp_dest: [4u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: true,
        };

        let _ = rt.upsert(n1, now);
        let _ = rt.upsert(n2, now);
        let _ = rt.upsert(n3, now);
        let _ = rt.upsert(n4, now);

        let d2 = [2u8; 387];
        let d3 = [3u8; 387];
        let d4 = [4u8; 387];
        let d2_b64 = crate::i2p::b64::encode(&d2);
        let d3_b64 = crate::i2p::b64::encode(&d3);
        let d4_b64 = crate::i2p::b64::encode(&d4);

        rt.mark_seen_by_dest(&d3_b64, now); // stable candidate: verified + recent inbound + 0 failures
        rt.mark_seen_by_dest(&d4_b64, now); // unreliable candidate; then add failures
        rt.mark_failure_by_dest(&d4_b64);
        rt.mark_failure_by_dest(&d4_b64);
        rt.mark_failure_by_dest(&d4_b64);

        // verified-only candidate: older inbound
        if let Some(st) = rt.get_mut_by_dest(&d2_b64) {
            st.last_inbound = Some(now - Duration::from_secs(20 * 60));
        }

        let counts = rt.peer_health_counts(now);
        assert_eq!(counts.unknown, 1);
        assert_eq!(counts.verified, 1);
        assert_eq!(counts.stable, 1);
        assert_eq!(counts.unreliable, 1);
    }

    #[test]
    fn query_candidates_for_target_prefer_stable_over_unreliable() {
        let my = KadId([0u8; 16]);
        let now = Instant::now();
        let mut rt = RoutingTable::new(my);

        let n1 = ImuleNode {
            kad_version: 6,
            client_id: [1u8; 16],
            udp_dest: [1u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: true,
        };
        let n2 = ImuleNode {
            kad_version: 6,
            client_id: [2u8; 16],
            udp_dest: [2u8; 387],
            udp_key: 0,
            udp_key_ip: 0,
            verified: true,
        };
        let _ = rt.upsert(n1, now);
        let _ = rt.upsert(n2, now);

        let d1_b64 = rt
            .get_by_id(KadId([1u8; 16]))
            .expect("node 1 present")
            .dest_b64
            .clone();
        let d2_b64 = rt
            .get_by_id(KadId([2u8; 16]))
            .expect("node 2 present")
            .dest_b64
            .clone();

        // Mark both as live, then make node1 unreliable.
        rt.mark_seen_by_dest(&d1_b64, now);
        rt.mark_seen_by_dest(&d2_b64, now);
        rt.mark_failure_by_dest(&d1_b64);
        rt.mark_failure_by_dest(&d1_b64);
        rt.mark_failure_by_dest(&d1_b64);

        let target = KadId([0u8; 16]);
        let out = rt.select_query_candidates_for_target(target, 2, now, Duration::from_secs(1), 5);
        let ids: Vec<u8> = out.iter().map(|n| n.client_id[0]).collect();
        assert_eq!(ids, vec![2, 1]);
    }
}
