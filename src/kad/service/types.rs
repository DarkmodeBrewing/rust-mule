use crate::kad::{KadId, wire::I2P_DEST_LEN};
use serde::Serialize;
use tokio::sync::oneshot;

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
    /// Requested number of contacts in `KADEMLIA2_REQ` (1..=31).
    pub req_contacts: u8,
    pub max_persist_nodes: usize,

    pub req_timeout_secs: u64,
    pub req_min_interval_secs: u64,

    pub bootstrap_every_secs: u64,
    pub bootstrap_batch: usize,
    pub bootstrap_min_interval_secs: u64,

    pub hello_every_secs: u64,
    pub hello_batch: usize,
    pub hello_min_interval_secs: u64,
    /// Optional: send a second obfuscated HELLO_REQ if we already have a receiver key.
    /// This diverges from iMule's plain-HELLO behavior and is experimental.
    pub hello_dual_obfuscated: bool,

    pub maintenance_every_secs: u64,
    pub status_every_secs: u64,
    pub max_failures: u32,
    pub evict_age_secs: u64,

    // Bucket refresh / lookup tuning.
    pub refresh_interval_secs: u64,
    pub refresh_buckets_per_tick: usize,
    pub refresh_underpopulated_min_contacts: usize,
    pub refresh_underpopulated_every_secs: u64,
    pub refresh_underpopulated_buckets_per_tick: usize,
    pub refresh_underpopulated_alpha: usize,

    // Keyword search result caching (in-memory)
    /// If true, only accept/retain keyword results for keywords we are "interested in"
    /// (i.e. we initiated a search for them, or requested their results via API).
    pub keyword_require_interest: bool,
    /// How long a keyword stays "active" (tracked) since last interest touch.
    pub keyword_interest_ttl_secs: u64,
    /// Drop keyword hits not re-seen for this long.
    pub keyword_results_ttl_secs: u64,
    /// Maximum number of active keywords to track.
    pub keyword_max_keywords: usize,
    /// Maximum total keyword hits across all keywords.
    pub keyword_max_total_hits: usize,
    /// Maximum hits to keep per keyword.
    pub keyword_max_hits_per_keyword: usize,

    // DHT keyword storage (what we accept from inbound PUBLISH_KEY requests).
    pub store_keyword_max_keywords: usize,
    pub store_keyword_max_total_hits: usize,
    pub store_keyword_evict_age_secs: u64,
}

impl Default for KadServiceConfig {
    fn default() -> Self {
        Self {
            runtime_secs: 0,
            crawl_every_secs: 3,
            persist_every_secs: 300,
            alpha: 3,
            req_contacts: 31,
            max_persist_nodes: 5000,

            req_timeout_secs: 45,
            req_min_interval_secs: 15,

            // Keep this conservative: the iMule I2P-KAD network is small.
            // This is a different query path than KADEMLIA2_REQ and sometimes yields contacts
            // when lookups don't.
            bootstrap_every_secs: 30 * 60,
            bootstrap_batch: 1,
            bootstrap_min_interval_secs: 6 * 60 * 60,

            hello_every_secs: 10,
            hello_batch: 2,
            hello_min_interval_secs: 900,
            hello_dual_obfuscated: false,

            maintenance_every_secs: 5,
            status_every_secs: 60,
            max_failures: 5,
            // I2P peers can be very intermittent; don't aggressively evict by default.
            evict_age_secs: 24 * 60 * 60,

            refresh_interval_secs: 45 * 60,
            refresh_buckets_per_tick: 1,
            refresh_underpopulated_min_contacts: 60,
            refresh_underpopulated_every_secs: 60,
            refresh_underpopulated_buckets_per_tick: 2,
            refresh_underpopulated_alpha: 5,

            keyword_require_interest: true,
            keyword_interest_ttl_secs: 24 * 60 * 60,
            keyword_results_ttl_secs: 24 * 60 * 60,
            keyword_max_keywords: 64,
            keyword_max_total_hits: 50_000,
            keyword_max_hits_per_keyword: 2_000,

            store_keyword_max_keywords: 1024,
            store_keyword_max_total_hits: 200_000,
            store_keyword_evict_age_secs: 14 * 24 * 60 * 60,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KadServiceStatus {
    pub uptime_secs: u64,
    pub routing: usize,
    pub live: usize,
    pub live_10m: usize,
    pub pending: usize,

    // UI compatibility aliases:
    // - recv_req maps to sent_reqs (requests issued by this node)
    // - recv_res maps to recv_ress (responses received by this node)
    pub recv_req: u64,
    pub recv_res: u64,
    pub sent_reqs: u64,
    pub recv_ress: u64,
    pub res_contacts: u64,
    pub dropped_undecipherable: u64,
    pub dropped_unparsable: u64,
    pub recv_hello_reqs: u64,
    pub sent_bootstrap_reqs: u64,
    pub recv_bootstrap_ress: u64,
    pub bootstrap_contacts: u64,
    pub sent_hellos: u64,
    pub recv_hello_ress: u64,
    pub sent_hello_acks: u64,
    pub recv_hello_acks: u64,
    pub hello_ack_skipped_no_sender_key: u64,
    pub timeouts: u64,
    pub new_nodes: u64,
    pub evicted: u64,

    pub sent_search_source_reqs: u64,
    pub recv_search_source_reqs: u64,
    pub recv_search_source_decode_failures: u64,
    pub source_search_hits: u64,
    pub source_search_misses: u64,
    pub source_search_results_served: u64,
    pub recv_search_ress: u64,
    pub search_results: u64,
    pub new_sources: u64,

    pub sent_search_key_reqs: u64,
    pub recv_search_key_reqs: u64,
    pub keyword_results: u64,
    pub new_keyword_results: u64,
    pub evicted_keyword_hits: u64,
    pub evicted_keyword_keywords: u64,
    pub keyword_keywords_tracked: usize,
    pub keyword_hits_total: usize,

    pub store_keyword_keywords: usize,
    pub store_keyword_hits_total: usize,
    pub source_store_files: usize,
    pub source_store_entries_total: usize,

    pub recv_publish_key_reqs: u64,
    pub recv_publish_key_decode_failures: u64,
    pub sent_publish_key_ress: u64,
    pub sent_publish_key_reqs: u64,
    pub recv_publish_key_ress: u64,
    pub new_store_keyword_hits: u64,
    pub evicted_store_keyword_hits: u64,
    pub evicted_store_keyword_keywords: u64,

    pub sent_publish_source_reqs: u64,
    pub recv_publish_source_reqs: u64,
    pub recv_publish_source_decode_failures: u64,
    pub sent_publish_source_ress: u64,
    pub new_store_source_entries: u64,
    pub recv_publish_ress: u64,

    pub source_search_batch_candidates: u64,
    pub source_search_batch_skipped_version: u64,
    pub source_search_batch_sent: u64,
    pub source_search_batch_send_fail: u64,
    pub source_publish_batch_candidates: u64,
    pub source_publish_batch_skipped_version: u64,
    pub source_publish_batch_sent: u64,
    pub source_publish_batch_send_fail: u64,

    pub source_probe_first_publish_responses: u64,
    pub source_probe_first_search_responses: u64,
    pub source_probe_search_results_total: u64,
    pub source_probe_publish_latency_ms_total: u64,
    pub source_probe_search_latency_ms_total: u64,
}

#[derive(Debug)]
pub enum KadServiceCommand {
    SearchSources {
        file: KadId,
        file_size: u64,
    },
    SearchKeyword {
        keyword: KadId,
    },
    PublishKeyword {
        keyword: KadId,
        file: KadId,
        filename: String,
        file_size: u64,
        file_type: Option<String>,
    },
    PublishSource {
        file: KadId,
        file_size: u64,
    },
    GetSources {
        file: KadId,
        respond_to: oneshot::Sender<Vec<KadSourceEntry>>,
    },
    GetKeywordResults {
        keyword: KadId,
        respond_to: oneshot::Sender<Vec<KadKeywordHit>>,
    },
    GetKeywordSearches {
        respond_to: oneshot::Sender<Vec<KadKeywordSearchInfo>>,
    },
    StopKeywordSearch {
        keyword: KadId,
        respond_to: oneshot::Sender<bool>,
    },
    DeleteKeywordSearch {
        keyword: KadId,
        purge_results: bool,
        respond_to: oneshot::Sender<bool>,
    },
    GetPeers {
        respond_to: oneshot::Sender<Vec<KadPeerInfo>>,
    },
    GetRoutingSummary {
        respond_to: oneshot::Sender<RoutingSummary>,
    },
    GetRoutingBuckets {
        respond_to: oneshot::Sender<Vec<RoutingBucketSummary>>,
    },
    GetRoutingNodes {
        bucket: usize,
        respond_to: oneshot::Sender<Vec<RoutingNodeSummary>>,
    },
    StartDebugLookup {
        target: Option<KadId>,
        respond_to: oneshot::Sender<KadId>,
    },
    DebugProbePeer {
        dest_b64: String,
        keyword: KadId,
        file: KadId,
        filename: String,
        file_size: u64,
        file_type: Option<String>,
        respond_to: oneshot::Sender<bool>,
    },
}

#[derive(Debug, Clone)]
pub struct KadSourceEntry {
    pub source_id: KadId,
    pub udp_dest: [u8; I2P_DEST_LEN],
}

#[derive(Debug, Clone)]
pub struct KadKeywordHit {
    pub file_id: KadId,
    pub filename: String,
    pub file_size: u64,
    pub file_type: Option<String>,
    pub publish_info: Option<u32>,
    pub origin: KadKeywordHitOrigin,
}

#[derive(Debug, Clone, Serialize)]
pub struct KadKeywordSearchInfo {
    pub search_id_hex: String,
    pub keyword_id_hex: String,
    pub state: String,
    pub created_secs_ago: u64,
    pub hits: usize,
    pub want_search: bool,
    pub publish_enabled: bool,
    pub got_publish_ack: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct KadPeerInfo {
    pub kad_id_hex: String,
    pub udp_dest_b64: String,
    pub udp_dest_short: String,
    pub kad_version: u8,
    pub verified: bool,
    pub udp_key: u32,
    pub udp_key_ip: u32,
    pub failures: u32,
    pub peer_agent: Option<String>,
    pub last_seen_secs_ago: u64,
    pub last_inbound_secs_ago: Option<u64>,
    pub last_queried_secs_ago: Option<u64>,
    pub last_bootstrap_secs_ago: Option<u64>,
    pub last_hello_secs_ago: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KadKeywordHitOrigin {
    Local,
    Network,
}

impl KadKeywordHitOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            KadKeywordHitOrigin::Local => "local",
            KadKeywordHitOrigin::Network => "network",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingSummary {
    pub bucket_count: usize,
    pub total_nodes: usize,
    pub verified_nodes: usize,
    pub unverified_nodes: usize,
    pub buckets_empty: usize,
    pub bucket_fill_min: usize,
    pub bucket_fill_median: usize,
    pub bucket_fill_max: usize,
    pub last_seen_min_secs: Option<u64>,
    pub last_seen_max_secs: Option<u64>,
    pub last_inbound_min_secs: Option<u64>,
    pub last_inbound_max_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingBucketSummary {
    pub index: usize,
    pub count: usize,
    pub verified: usize,
    pub unverified: usize,
    pub last_seen_min_secs: Option<u64>,
    pub last_seen_max_secs: Option<u64>,
    pub last_inbound_min_secs: Option<u64>,
    pub last_inbound_max_secs: Option<u64>,
    pub last_activity_secs_ago: Option<u64>,
    pub last_refresh_secs_ago: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingNodeSummary {
    pub kad_id_hex: String,
    pub udp_dest_b64: String,
    pub udp_dest_short: String,
    pub kad_version: u8,
    pub verified: bool,
    pub failures: u32,
    pub last_seen_secs_ago: u64,
    pub last_inbound_secs_ago: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct KadServiceStats {
    pub(super) sent_reqs: u64,
    pub(super) recv_ress: u64,
    pub(super) res_contacts: u64,
    pub(super) dropped_undecipherable: u64,
    pub(super) dropped_unparsable: u64,
    pub(super) recv_hello_reqs: u64,
    pub(super) sent_bootstrap_reqs: u64,
    pub(super) recv_bootstrap_ress: u64,
    pub(super) bootstrap_contacts: u64,
    pub(super) sent_hellos: u64,
    pub(super) recv_hello_ress: u64,
    pub(super) sent_hello_acks: u64,
    pub(super) recv_hello_acks: u64,
    pub(super) hello_ack_skipped_no_sender_key: u64,
    pub(super) timeouts: u64,
    pub(super) new_nodes: u64,
    pub(super) evicted: u64,

    pub(super) sent_search_source_reqs: u64,
    pub(super) recv_search_source_reqs: u64,
    pub(super) recv_search_source_decode_failures: u64,
    pub(super) source_search_hits: u64,
    pub(super) source_search_misses: u64,
    pub(super) source_search_results_served: u64,
    pub(super) recv_search_ress: u64,
    pub(super) search_results: u64,
    pub(super) new_sources: u64,

    pub(super) sent_search_key_reqs: u64,
    pub(super) recv_search_key_reqs: u64,
    pub(super) keyword_results: u64,
    pub(super) new_keyword_results: u64,
    pub(super) evicted_keyword_hits: u64,
    pub(super) evicted_keyword_keywords: u64,

    pub(super) recv_publish_key_reqs: u64,
    pub(super) recv_publish_key_decode_failures: u64,
    pub(super) sent_publish_key_ress: u64,
    pub(super) sent_publish_key_reqs: u64,
    pub(super) recv_publish_key_ress: u64,
    pub(super) new_store_keyword_hits: u64,
    pub(super) evicted_store_keyword_hits: u64,
    pub(super) evicted_store_keyword_keywords: u64,

    pub(super) sent_publish_source_reqs: u64,
    pub(super) recv_publish_source_reqs: u64,
    pub(super) recv_publish_source_decode_failures: u64,
    pub(super) sent_publish_source_ress: u64,
    pub(super) new_store_source_entries: u64,
    pub(super) recv_publish_ress: u64,

    pub(super) source_search_batch_candidates: u64,
    pub(super) source_search_batch_skipped_version: u64,
    pub(super) source_search_batch_sent: u64,
    pub(super) source_search_batch_send_fail: u64,
    pub(super) source_publish_batch_candidates: u64,
    pub(super) source_publish_batch_skipped_version: u64,
    pub(super) source_publish_batch_sent: u64,
    pub(super) source_publish_batch_send_fail: u64,

    pub(super) source_probe_first_publish_responses: u64,
    pub(super) source_probe_first_search_responses: u64,
    pub(super) source_probe_search_results_total: u64,
    pub(super) source_probe_publish_latency_ms_total: u64,
    pub(super) source_probe_search_latency_ms_total: u64,
}
