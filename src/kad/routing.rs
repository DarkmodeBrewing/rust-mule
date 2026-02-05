use crate::{kad::KadId, nodes::imule::ImuleNode};
use std::collections::{BTreeMap, HashMap};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct NodeState {
    pub node: ImuleNode,
    pub dest_b64: String,
    pub last_seen: Instant,
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

    pub fn upsert(&mut self, node: ImuleNode, now: Instant) {
        let id = KadId(node.client_id);
        if id.is_zero() {
            return;
        }
        if id == self.my_id {
            return;
        }

        let dest_b64 = node.udp_dest_b64();
        if let Some(old) = self.by_id.get(&id)
            && old.dest_b64 != dest_b64
        {
            self.by_dest.remove(&old.dest_b64);
        }
        self.by_dest.insert(dest_b64.clone(), id);

        self.by_id
            .entry(id)
            .and_modify(|st| {
                // Prefer newer/verified entries and keep UDP keys once learned.
                if node.kad_version > st.node.kad_version {
                    st.node.kad_version = node.kad_version;
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
            .or_insert(NodeState {
                node,
                dest_b64,
                last_seen: now,
                failures: 0,
            });
    }

    pub fn mark_seen_by_dest(&mut self, dest_b64: &str, now: Instant) {
        if let Some(st) = self.get_mut_by_dest(dest_b64) {
            st.last_seen = now;
            st.failures = 0;
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
            st.failures = 0;
            if sender_verify_key != 0 && st.node.udp_key == 0 {
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

    pub fn snapshot_nodes(&self, max: usize) -> Vec<ImuleNode> {
        let mut out: Vec<ImuleNode> = self.by_id.values().map(|st| st.node.clone()).collect();
        out.sort_by_key(|n| (std::cmp::Reverse(n.verified), std::cmp::Reverse(n.kad_version)));
        out.truncate(max);
        out
    }
}

fn xor_distance(a: KadId, b: KadId) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = a.0[i] ^ b.0[i];
    }
    out
}
