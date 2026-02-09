use crate::{kad::KadId, nodes::imule::ImuleNode};
use std::collections::{BTreeMap, HashMap};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NodeState {
    pub node: ImuleNode,
    pub dest_b64: String,
    pub last_seen: Instant,
    /// Last time we received *any* packet from this destination.
    pub last_inbound: Option<Instant>,
    pub last_queried: Option<Instant>,
    pub last_bootstrap: Option<Instant>,
    pub last_hello: Option<Instant>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    Updated,
    IgnoredZeroId,
    IgnoredSelf,
}

impl RoutingTable {
    pub fn new(my_id: KadId) -> Self {
        Self {
            my_id,
            by_id: BTreeMap::new(),
            by_dest: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
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
        if let Some(old) = self.by_id.get(&id)
            && old.dest_b64 != dest_b64
        {
            self.by_dest.remove(&old.dest_b64);
        }
        self.by_dest.insert(dest_b64.clone(), id);

        let mut inserted = false;
        self.by_id
            .entry(id)
            .and_modify(|st| {
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
                st.last_seen = now;
                st.failures = 0;
            })
            .or_insert_with(|| {
                inserted = true;
                NodeState {
                    node,
                    dest_b64,
                    last_seen: now,
                    last_inbound: None,
                    last_queried: None,
                    last_bootstrap: None,
                    last_hello: None,
                    needs_hello: true,
                    failures: 0,
                }
            });

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
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
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
            .filter(|st| st.node.kad_version >= 6)
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
            .filter(|st| st.node.kad_version >= 6)
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
            .filter(|st| st.node.kad_version >= 6)
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
            .filter(|st| st.node.kad_version >= 6)
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
