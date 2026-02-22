use crate::{
    i2p::sam::SamKadSocket,
    kad::{
        KadId,
        routing::RoutingTable,
        udp_crypto,
        wire::{
            I2P_DEST_LEN, KADEMLIA_HELLO_REQ_DEPRECATED, KADEMLIA_HELLO_RES_DEPRECATED,
            KADEMLIA_PUBLISH_NOTES_REQ_DEPRECATED, KADEMLIA_PUBLISH_REQ_DEPRECATED,
            KADEMLIA_PUBLISH_RES_DEPRECATED, KADEMLIA_REQ_DEPRECATED, KADEMLIA_RES_DEPRECATED,
            KADEMLIA_SEARCH_NOTES_REQ_DEPRECATED, KADEMLIA_SEARCH_REQ_DEPRECATED,
            KADEMLIA_SEARCH_RES_DEPRECATED, KADEMLIA2_BOOTSTRAP_REQ, KADEMLIA2_BOOTSTRAP_RES,
            KADEMLIA2_HELLO_REQ, KADEMLIA2_HELLO_RES, KADEMLIA2_HELLO_RES_ACK, KADEMLIA2_PING,
            KADEMLIA2_PONG, KADEMLIA2_PUBLISH_KEY_REQ, KADEMLIA2_PUBLISH_RES,
            KADEMLIA2_PUBLISH_SOURCE_REQ, KADEMLIA2_REQ, KADEMLIA2_RES, KADEMLIA2_SEARCH_KEY_REQ,
            KADEMLIA2_SEARCH_RES, KADEMLIA2_SEARCH_SOURCE_REQ, Kad2PublishRes, Kad2PublishResKey,
            Kad2SearchRes, KadPacket, TAG_KADMISCOPTIONS, decode_kad2_bootstrap_res,
            decode_kad2_hello, decode_kad2_publish_key_keyword_prefix,
            decode_kad2_publish_key_req_lenient, decode_kad2_publish_res,
            decode_kad2_publish_res_key, decode_kad2_publish_source_req_min, decode_kad2_req,
            decode_kad2_res, decode_kad2_search_key_req, decode_kad2_search_res,
            decode_kad2_search_source_req, encode_kad2_bootstrap_res, encode_kad2_hello,
            encode_kad2_hello_req, encode_kad2_publish_key_req, encode_kad2_publish_res_for_key,
            encode_kad2_publish_res_for_source, encode_kad2_publish_source_req, encode_kad2_req,
            encode_kad2_res, encode_kad2_search_key_req, encode_kad2_search_res_keyword,
            encode_kad2_search_res_sources, encode_kad2_search_source_req,
        },
    },
    nodes::imule::ImuleNode,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{Duration, Instant, MissedTickBehavior, interval};

mod inbound;
mod keyword;
mod lookup;
mod routing_view;
mod source_probe;
mod status;
#[cfg(test)]
mod tests;
mod types;

pub use types::{
    KadKeywordHit, KadKeywordHitOrigin, KadKeywordSearchInfo, KadPeerInfo, KadServiceCommand,
    KadServiceConfig, KadServiceCrypto, KadServiceStatus, KadSourceEntry, RoutingBucketSummary,
    RoutingNodeSummary, RoutingSummary,
};
use types::{KadServiceCumulative, KadServiceStats};

pub type Result<T> = std::result::Result<T, KadServiceError>;

#[derive(Debug)]
pub enum KadServiceError {
    Sam(crate::i2p::sam::SamError),
    Wire(crate::kad::wire::WireError),
    Crypto(crate::kad::udp_crypto::UdpCryptoError),
    Kad(crate::kad::KadError),
    Nodes(crate::nodes::imule::ImuleNodesError),
    Io(std::io::Error),
    Timeout(tokio::time::error::Elapsed),
    Recv(tokio::sync::oneshot::error::RecvError),
    InvalidState(String),
}

impl std::fmt::Display for KadServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sam(source) => write!(f, "{source}"),
            Self::Wire(source) => write!(f, "{source}"),
            Self::Crypto(source) => write!(f, "{source}"),
            Self::Kad(source) => write!(f, "{source}"),
            Self::Nodes(source) => write!(f, "{source}"),
            Self::Io(source) => write!(f, "{source}"),
            Self::Timeout(source) => write!(f, "{source}"),
            Self::Recv(source) => write!(f, "{source}"),
            Self::InvalidState(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for KadServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sam(source) => Some(source),
            Self::Wire(source) => Some(source),
            Self::Crypto(source) => Some(source),
            Self::Kad(source) => Some(source),
            Self::Nodes(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Timeout(source) => Some(source),
            Self::Recv(source) => Some(source),
            Self::InvalidState(_) => None,
        }
    }
}

impl From<crate::i2p::sam::SamError> for KadServiceError {
    fn from(value: crate::i2p::sam::SamError) -> Self {
        Self::Sam(value)
    }
}

impl From<crate::kad::wire::WireError> for KadServiceError {
    fn from(value: crate::kad::wire::WireError) -> Self {
        Self::Wire(value)
    }
}

impl From<crate::kad::udp_crypto::UdpCryptoError> for KadServiceError {
    fn from(value: crate::kad::udp_crypto::UdpCryptoError) -> Self {
        Self::Crypto(value)
    }
}

impl From<crate::kad::KadError> for KadServiceError {
    fn from(value: crate::kad::KadError) -> Self {
        Self::Kad(value)
    }
}

impl From<crate::nodes::imule::ImuleNodesError> for KadServiceError {
    fn from(value: crate::nodes::imule::ImuleNodesError) -> Self {
        Self::Nodes(value)
    }
}

impl From<std::io::Error> for KadServiceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tokio::time::error::Elapsed> for KadServiceError {
    fn from(value: tokio::time::error::Elapsed) -> Self {
        Self::Timeout(value)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for KadServiceError {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::Recv(value)
    }
}

const KEYWORD_JOB_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const KEYWORD_JOB_LOOKUP_EVERY: Duration = Duration::from_secs(45);
const KEYWORD_JOB_ACTION_EVERY: Duration = Duration::from_secs(45);
const KEYWORD_JOB_ACTION_BATCH: usize = 5;
const TRACKED_OUT_REQUEST_TTL: Duration = Duration::from_secs(180);
const TRACKED_IN_CLEANUP_EVERY: Duration = Duration::from_secs(12 * 60);
const TRACKED_IN_ENTRY_TTL: Duration = Duration::from_secs(15 * 60);
const SHAPER_PEER_STATE_TTL: Duration = Duration::from_secs(60 * 60);
const SHAPER_PEER_STATE_MAX: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OutboundClass {
    Query,
    Hello,
    Bootstrap,
    Response,
}

#[derive(Debug, Clone, Copy)]
struct ShaperClassPolicy {
    base_delay_ms: u64,
    jitter_ms: u64,
    global_min_interval_ms: u64,
    peer_min_interval_ms: u64,
    global_max_per_sec: u32,
    peer_max_per_sec: u32,
    drop_when_delayed: bool,
}

#[derive(Debug, Clone)]
struct TrackedOutRequest {
    request_id: u64,
    dest_b64: String,
    request_opcode: u8,
    trace_tag: Option<String>,
    inserted: Instant,
}

#[derive(Debug, Clone, Copy)]
struct TrackedInCounter {
    first_added: Instant,
    count: u32,
    warned: bool,
}

#[derive(Debug, Clone)]
struct UnmatchedResponseDiag {
    from_dest_b64: String,
    response_opcode: u8,
    expected_request_opcodes: Vec<u8>,
    peer_tracked_requests: usize,
    total_tracked_requests: usize,
}

pub struct KadService {
    routing: RoutingTable,
    // Minimal (in-memory) source index: file ID -> (source ID -> UDP dest).
    sources_by_file: BTreeMap<KadId, BTreeMap<KadId, [u8; I2P_DEST_LEN]>>,
    source_probe_by_file: HashMap<KadId, SourceProbeState>,
    // Minimal (in-memory) keyword index: keyword hash -> (file ID -> hit state).
    keyword_hits_by_keyword: BTreeMap<KadId, BTreeMap<KadId, KeywordHitState>>,
    keyword_hits_total: usize,
    keyword_interest: HashMap<KadId, Instant>,

    keyword_store_by_keyword: BTreeMap<KadId, BTreeMap<KadId, KeywordHitState>>,
    keyword_store_total: usize,

    keyword_jobs: HashMap<KadId, KeywordJob>,

    pending_reqs: HashMap<String, Instant>,
    tracked_out_requests: Vec<TrackedOutRequest>,
    next_tracked_out_request_id: u64,
    last_unmatched_response: Option<UnmatchedResponseDiag>,
    tracked_in_requests: HashMap<u32, HashMap<u8, TrackedInCounter>>,
    tracked_in_last_cleanup: Instant,
    shaper_window_started: Instant,
    shaper_global_sent_in_window: HashMap<OutboundClass, u32>,
    shaper_peer_sent_in_window: HashMap<String, u32>,
    shaper_last_global_send: HashMap<OutboundClass, Instant>,
    shaper_last_peer_send: HashMap<String, Instant>,
    shaper_jitter_seed: u64,
    crawl_round: u64,
    stats_window: KadServiceStats,
    stats_cumulative: KadServiceCumulative,
    publish_key_decode_fail_logged: HashSet<String>,

    lookup_queue: std::collections::VecDeque<LookupTask>,
    active_lookup: Option<LookupTask>,
    last_refresh_tick: Instant,
    last_underpopulated_refresh: Instant,

    cmd_rx: mpsc::Receiver<KadServiceCommand>,
}

#[derive(Debug, Clone)]
struct KeywordHitState {
    hit: KadKeywordHit,
    last_seen: Instant,
}

#[derive(Debug, Clone)]
struct SourceProbeState {
    first_publish_sent_at: Option<Instant>,
    first_search_sent_at: Option<Instant>,
    first_publish_res_at: Option<Instant>,
    first_search_res_at: Option<Instant>,
    search_result_events: u64,
    search_results_total: u64,
    last_search_results: u64,
    last_update: Instant,
}

#[derive(Debug, Clone)]
struct KeywordPublishSpec {
    file: KadId,
    filename: String,
    file_size: u64,
    file_type: Option<String>,
}

#[derive(Debug, Clone)]
struct KeywordJob {
    created_at: Instant,
    next_lookup_at: Instant,
    next_search_at: Instant,
    next_publish_at: Instant,
    sent_to_search: HashSet<String>,
    sent_to_publish: HashSet<String>,

    want_search: bool,
    publish: Option<KeywordPublishSpec>,
    got_publish_ack: bool,
}

#[derive(Debug, Clone, Copy)]
enum LookupKind {
    Debug,
    Refresh { bucket: usize },
}

#[derive(Debug, Clone)]
struct LookupTask {
    kind: LookupKind,
    target: KadId,
    started_at: Instant,
    last_progress: Instant,
    iteration: u32,
    alpha_override: Option<usize>,
    queried: HashSet<String>,
    inflight: HashSet<String>,
    known: BTreeMap<KadId, ImuleNode>,
    new_nodes: u64,
}

impl KadService {
    pub fn new(my_id: KadId, cmd_rx: mpsc::Receiver<KadServiceCommand>) -> Self {
        let now = Instant::now();
        Self {
            routing: RoutingTable::new(my_id),
            sources_by_file: BTreeMap::new(),
            source_probe_by_file: HashMap::new(),
            keyword_hits_by_keyword: BTreeMap::new(),
            keyword_hits_total: 0,
            keyword_interest: HashMap::new(),
            keyword_store_by_keyword: BTreeMap::new(),
            keyword_store_total: 0,
            keyword_jobs: HashMap::new(),
            pending_reqs: HashMap::new(),
            tracked_out_requests: Vec::new(),
            next_tracked_out_request_id: 1,
            last_unmatched_response: None,
            tracked_in_requests: HashMap::new(),
            tracked_in_last_cleanup: now,
            shaper_window_started: now,
            shaper_global_sent_in_window: HashMap::new(),
            shaper_peer_sent_in_window: HashMap::new(),
            shaper_last_global_send: HashMap::new(),
            shaper_last_peer_send: HashMap::new(),
            shaper_jitter_seed: now.elapsed().as_nanos() as u64 ^ 0x9e37_79b9_7f4a_7c15,
            crawl_round: 0,
            stats_window: KadServiceStats::default(),
            stats_cumulative: KadServiceCumulative::default(),
            publish_key_decode_fail_logged: HashSet::new(),
            lookup_queue: std::collections::VecDeque::new(),
            active_lookup: None,
            last_refresh_tick: now,
            last_underpopulated_refresh: now,
            cmd_rx,
        }
    }

    pub fn routing(&self) -> &RoutingTable {
        &self.routing
    }

    pub fn routing_mut(&mut self) -> &mut RoutingTable {
        &mut self.routing
    }

    pub fn note_sam_framing_desync(&mut self) {
        self.stats_cumulative.sam_framing_desync =
            self.stats_cumulative.sam_framing_desync.saturating_add(1);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_service(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    initial_nodes: impl IntoIterator<Item = ImuleNode>,
    crypto: KadServiceCrypto,
    cfg: KadServiceConfig,
    persist_path: &std::path::Path,
    status_tx: Option<watch::Sender<Option<KadServiceStatus>>>,
    status_events_tx: Option<broadcast::Sender<KadServiceStatus>>,
) -> Result<()> {
    let started = Instant::now();
    let now = started;
    for n in initial_nodes {
        let _ = svc.routing.upsert(n, now);
    }
    tracing::info!(nodes = svc.routing.len(), "kad service started");
    publish_status(svc, started, &status_tx, &status_events_tx);

    let mut crawl_tick = interval(Duration::from_secs(cfg.crawl_every_secs.max(1)));
    let mut persist_tick = interval(Duration::from_secs(cfg.persist_every_secs.max(5)));
    // Don't fire immediately on service start; we already did an initial bootstrap.
    let mut bootstrap_tick = tokio::time::interval_at(
        Instant::now() + Duration::from_secs(cfg.bootstrap_every_secs.max(30)),
        Duration::from_secs(cfg.bootstrap_every_secs.max(30)),
    );
    let mut hello_tick = interval(Duration::from_secs(cfg.hello_every_secs.max(1)));
    let mut maintenance_tick = interval(Duration::from_secs(cfg.maintenance_every_secs.max(1)));
    let mut status_tick = interval(Duration::from_secs(cfg.status_every_secs.max(5)));

    // If the process is paused (sleep / container suspension / busy host), default Tokio
    // behavior is to "catch up" by running ticks in a tight loop. That's bad for network
    // friendliness (and can overwhelm SAM). Skip missed ticks instead.
    crawl_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    persist_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    bootstrap_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    hello_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    maintenance_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    status_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

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

            _ = bootstrap_tick.tick() => {
                send_bootstrap_batch(svc, sock, crypto, &cfg).await?;
            }

            _ = hello_tick.tick() => {
                send_hello_batch(svc, sock, crypto, &cfg).await?;
            }

            _ = maintenance_tick.tick() => {
                maintenance(svc, &cfg).await;
                tick_lookups(svc, sock, crypto, &cfg).await?;
                progress_keyword_jobs(svc, sock, crypto, &cfg).await?;
            }

            _ = status_tick.tick() => {
                publish_status(svc, started, &status_tx, &status_events_tx);
            }

            cmd = svc.cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    handle_command(svc, sock, crypto, &cfg, cmd).await?;
                }
            }

            recv = sock.recv() => {
                let recv = recv?;
                handle_inbound(svc, sock, recv.from_destination, recv.payload, crypto, &cfg).await?;
            }
        }
    }

    persist_snapshot(svc, persist_path, cfg.max_persist_nodes).await;
    tracing::info!("kad service stopped");
    Ok(())
}

fn stop_keyword_search(svc: &mut KadService, keyword: KadId) -> bool {
    let Some(job) = svc.keyword_jobs.get_mut(&keyword) else {
        return false;
    };

    job.want_search = false;
    job.publish = None;
    job.got_publish_ack = true;
    true
}

fn delete_keyword_search(svc: &mut KadService, keyword: KadId, purge_results: bool) -> bool {
    let removed = svc.keyword_jobs.remove(&keyword).is_some();
    if !removed {
        return false;
    }

    if purge_results {
        if let Some(hits) = svc.keyword_hits_by_keyword.remove(&keyword) {
            svc.keyword_hits_total = svc.keyword_hits_total.saturating_sub(hits.len());
        }
        svc.keyword_interest.remove(&keyword);

        if let Some(store) = svc.keyword_store_by_keyword.remove(&keyword) {
            svc.keyword_store_total = svc.keyword_store_total.saturating_sub(store.len());
        }
    }

    true
}

async fn handle_command(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    cmd: KadServiceCommand,
) -> Result<()> {
    let now = Instant::now();
    match cmd {
        KadServiceCommand::SearchSources { file, file_size } => {
            send_search_sources(svc, sock, crypto, cfg, file, file_size).await?;
        }
        KadServiceCommand::SearchKeyword { keyword } => {
            touch_keyword_interest(svc, cfg, keyword, now);
            start_keyword_job_search(svc, sock, crypto, cfg, keyword, now).await?;
        }
        KadServiceCommand::PublishKeyword {
            keyword,
            file,
            filename,
            file_size,
            file_type,
        } => {
            touch_keyword_interest(svc, cfg, keyword, now);
            // For UX/debugging: reflect our own publish locally as well, so it is visible via
            // `/kad/keyword_results/<keyword>` even if the network is empty/silent.
            upsert_keyword_hit_cache(
                svc,
                cfg,
                now,
                keyword,
                KadKeywordHit {
                    file_id: file,
                    filename: filename.clone(),
                    file_size,
                    file_type: file_type.clone(),
                    publish_info: None,
                    origin: KadKeywordHitOrigin::Local,
                },
            );

            start_keyword_job_publish(
                svc, sock, crypto, cfg, keyword, file, filename, file_size, file_type, now,
            )
            .await?;
        }
        KadServiceCommand::PublishSource { file, file_size } => {
            // Mirror local publish into in-memory source cache so incoming SEARCH_SOURCE_REQ
            // can return this source before broader network convergence.
            cache_local_published_source(svc, crypto, file);
            send_publish_source(svc, sock, crypto, cfg, file, file_size).await?;
        }
        KadServiceCommand::GetSources { file, respond_to } => {
            let sources = svc
                .sources_by_file
                .get(&file)
                .map(|m| {
                    m.iter()
                        .take(1024)
                        .map(|(sid, dest)| KadSourceEntry {
                            source_id: *sid,
                            udp_dest: *dest,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let _ = respond_to.send(sources);
        }
        KadServiceCommand::GetKeywordResults {
            keyword,
            respond_to,
        } => {
            touch_keyword_interest(svc, cfg, keyword, now);
            let hits = svc
                .keyword_hits_by_keyword
                .get(&keyword)
                .map(|m| {
                    m.values()
                        .take(1024)
                        .map(|s| s.hit.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let _ = respond_to.send(hits);
        }
        KadServiceCommand::GetKeywordSearches { respond_to } => {
            let mut searches = svc
                .keyword_jobs
                .iter()
                .map(|(keyword, job)| {
                    let hits = svc
                        .keyword_hits_by_keyword
                        .get(keyword)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    let publish_enabled = job.publish.is_some();
                    let state = if job.want_search || (publish_enabled && !job.got_publish_ack) {
                        "running"
                    } else {
                        "complete"
                    };
                    KadKeywordSearchInfo {
                        search_id_hex: keyword.to_hex_lower(),
                        keyword_id_hex: keyword.to_hex_lower(),
                        state: state.to_string(),
                        created_secs_ago: now.saturating_duration_since(job.created_at).as_secs(),
                        hits,
                        want_search: job.want_search,
                        publish_enabled,
                        got_publish_ack: job.got_publish_ack,
                    }
                })
                .collect::<Vec<_>>();
            searches.sort_by_key(|s| s.created_secs_ago);
            let _ = respond_to.send(searches);
        }
        KadServiceCommand::StopKeywordSearch {
            keyword,
            respond_to,
        } => {
            let stopped = stop_keyword_search(svc, keyword);
            let _ = respond_to.send(stopped);
        }
        KadServiceCommand::DeleteKeywordSearch {
            keyword,
            purge_results,
            respond_to,
        } => {
            let deleted = delete_keyword_search(svc, keyword, purge_results);
            let _ = respond_to.send(deleted);
        }
        KadServiceCommand::GetPeers { respond_to } => {
            let mut states = svc.routing.snapshot_states();
            states.sort_by_key(|st| {
                (
                    st.last_inbound.is_none(),
                    std::cmp::Reverse(st.last_inbound.unwrap_or(st.last_seen)),
                )
            });
            let peers = states
                .into_iter()
                .map(|st| KadPeerInfo {
                    kad_id_hex: KadId(st.node.client_id).to_hex_lower(),
                    udp_dest_b64: st.dest_b64.clone(),
                    udp_dest_short: crate::i2p::b64::short(&st.dest_b64).to_string(),
                    kad_version: st.node.kad_version,
                    verified: st.node.verified,
                    udp_key: st.node.udp_key,
                    udp_key_ip: st.node.udp_key_ip,
                    failures: st.failures,
                    peer_agent: st.peer_agent,
                    last_seen_secs_ago: now.saturating_duration_since(st.last_seen).as_secs(),
                    last_inbound_secs_ago: age_secs(now, st.last_inbound),
                    last_queried_secs_ago: age_secs(now, st.last_queried),
                    last_bootstrap_secs_ago: age_secs(now, st.last_bootstrap),
                    last_hello_secs_ago: age_secs(now, st.last_hello),
                })
                .collect::<Vec<_>>();
            let _ = respond_to.send(peers);
        }
        KadServiceCommand::GetRoutingSummary { respond_to } => {
            let summary = routing_view::build_routing_summary(svc, now);
            let _ = respond_to.send(summary);
        }
        KadServiceCommand::GetRoutingBuckets { respond_to } => {
            let buckets = routing_view::build_routing_buckets(svc, now);
            let _ = respond_to.send(buckets);
        }
        KadServiceCommand::GetRoutingNodes { bucket, respond_to } => {
            let nodes = routing_view::build_routing_nodes(svc, now, bucket);
            let _ = respond_to.send(nodes);
        }
        KadServiceCommand::StartDebugLookup { target, respond_to } => {
            let target = target.unwrap_or_else(|| KadId::random().unwrap_or(crypto.my_kad_id));
            start_lookup(svc, target, LookupKind::Debug, None, now);
            let _ = respond_to.send(target);
        }
        KadServiceCommand::DebugProbePeer {
            dest_b64,
            keyword,
            file,
            filename,
            file_size,
            file_type,
            respond_to,
        } => {
            let ok = match debug_probe_peer(
                svc,
                sock,
                crypto,
                cfg,
                &dest_b64,
                keyword,
                file,
                &filename,
                file_size,
                file_type.as_deref(),
                now,
            )
            .await
            {
                Ok(ok) => ok,
                Err(err) => {
                    tracing::debug!(error = %err, to = %crate::i2p::b64::short(&dest_b64), "debug probe failed");
                    false
                }
            };
            let _ = respond_to.send(ok);
        }
    }
    Ok(())
}

fn age_secs(now: Instant, t: Option<Instant>) -> Option<u64> {
    t.map(|ts| now.saturating_duration_since(ts).as_secs())
}

fn start_lookup(
    svc: &mut KadService,
    target: KadId,
    kind: LookupKind,
    alpha_override: Option<usize>,
    now: Instant,
) {
    lookup::start_lookup_impl(svc, target, kind, alpha_override, now);
}

async fn send_search_sources(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    file: KadId,
    file_size: u64,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let peers = closest_peers_with_fallback(svc, file, 8, 3, 0);
    let mut skipped_version = 0u64;
    let mut sent = 0u64;
    let mut send_fail = 0u64;
    let hello_min = Duration::from_secs(cfg.hello_min_interval_secs.max(60));
    for p in peers {
        // iMule uses Kad2 search source only for version >= 3.
        if p.kad_version < 3 {
            skipped_version += 1;
            continue;
        }
        maybe_send_hello_to_peer(svc, sock, crypto, cfg, &p, now, hello_min).await?;
        let payload = encode_kad2_search_source_req(file, 0, file_size);
        let trace_tag = format!(
            "source_search:{}",
            crate::logging::redact_hex(&file.to_hex_lower())
        );
        let request_id = match send_kad2_packet(
            svc,
            sock,
            cfg,
            &p,
            crypto,
            KADEMLIA2_SEARCH_SOURCE_REQ,
            &payload,
            Some(trace_tag.as_str()),
        )
        .await
        {
            Ok(Some(request_id)) => Some(request_id),
            Ok(None) => {
                send_fail += 1;
                continue;
            }
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    to = %crate::i2p::b64::short(&p.udp_dest_b64()),
                    "failed sending SEARCH_SOURCE_REQ"
                );
                send_fail += 1;
                continue;
            }
        };
        svc.stats_window.sent_search_source_reqs += 1;
        sent += 1;
        if let Some(request_id) = request_id {
            tracing::info!(
                event = "source_probe_request_sent",
                request_id,
                opcode = kad_opcode_name(KADEMLIA2_SEARCH_SOURCE_REQ),
                to = %crate::i2p::b64::short(&p.udp_dest_b64()),
                file = %crate::logging::redact_hex(&file.to_hex_lower()),
                "sent source search request"
            );
        }
        mark_source_search_sent(svc, file, now);
    }
    svc.stats_window.source_search_batch_candidates += sent + skipped_version + send_fail;
    svc.stats_window.source_search_batch_skipped_version += skipped_version;
    svc.stats_window.source_search_batch_sent += sent;
    svc.stats_window.source_search_batch_send_fail += send_fail;

    tracing::info!(
        event = "send_search_source_req_batch",
        file = %crate::logging::redact_hex(&file.to_hex_lower()),
        candidates = sent + skipped_version + send_fail,
        skipped_version,
        sent,
        send_fail,
        "sent SEARCH_SOURCE_REQ"
    );
    Ok(())
}

async fn send_publish_source(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    file: KadId,
    file_size: u64,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let peers = closest_peers_with_fallback(svc, file, 6, 4, 0);
    let mut skipped_version = 0u64;
    let mut sent = 0u64;
    let mut send_fail = 0u64;
    let hello_min = Duration::from_secs(cfg.hello_min_interval_secs.max(60));
    for p in peers {
        // iMule uses Kad2 publish source only for version >= 4.
        if p.kad_version < 4 {
            skipped_version += 1;
            continue;
        }
        maybe_send_hello_to_peer(svc, sock, crypto, cfg, &p, now, hello_min).await?;
        let payload = encode_kad2_publish_source_req(
            file,
            crypto.my_kad_id,
            &crypto.my_dest,
            Some(file_size),
        );
        let trace_tag = format!(
            "source_publish:{}",
            crate::logging::redact_hex(&file.to_hex_lower())
        );
        let request_id = match send_kad2_packet(
            svc,
            sock,
            cfg,
            &p,
            crypto,
            KADEMLIA2_PUBLISH_SOURCE_REQ,
            &payload,
            Some(trace_tag.as_str()),
        )
        .await
        {
            Ok(Some(request_id)) => Some(request_id),
            Ok(None) => {
                send_fail += 1;
                continue;
            }
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    to = %crate::i2p::b64::short(&p.udp_dest_b64()),
                    "failed sending PUBLISH_SOURCE_REQ"
                );
                send_fail += 1;
                continue;
            }
        };
        svc.stats_window.sent_publish_source_reqs += 1;
        sent += 1;
        if let Some(request_id) = request_id {
            tracing::info!(
                event = "source_probe_request_sent",
                request_id,
                opcode = kad_opcode_name(KADEMLIA2_PUBLISH_SOURCE_REQ),
                to = %crate::i2p::b64::short(&p.udp_dest_b64()),
                file = %crate::logging::redact_hex(&file.to_hex_lower()),
                "sent source publish request"
            );
        }
        mark_source_publish_sent(svc, file, now);
    }
    svc.stats_window.source_publish_batch_candidates += sent + skipped_version + send_fail;
    svc.stats_window.source_publish_batch_skipped_version += skipped_version;
    svc.stats_window.source_publish_batch_sent += sent;
    svc.stats_window.source_publish_batch_send_fail += send_fail;

    tracing::info!(
        event = "send_publish_source_req_batch",
        file = %crate::logging::redact_hex(&file.to_hex_lower()),
        candidates = sent + skipped_version + send_fail,
        skipped_version,
        sent,
        send_fail,
        "sent PUBLISH_SOURCE_REQ"
    );
    Ok(())
}

async fn start_keyword_job_search(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    keyword: KadId,
    now: Instant,
) -> Result<()> {
    let job = svc
        .keyword_jobs
        .entry(keyword)
        .or_insert_with(|| KeywordJob {
            created_at: now,
            next_lookup_at: now,
            next_search_at: now,
            next_publish_at: now,
            sent_to_search: HashSet::new(),
            sent_to_publish: HashSet::new(),
            want_search: true,
            publish: None,
            got_publish_ack: false,
        });

    job.want_search = true;
    job.created_at = now;
    job.next_lookup_at = now;
    job.next_search_at = now;
    job.sent_to_search.clear();

    // Kick off immediately (lookup + first action batch).
    progress_keyword_job(svc, sock, crypto, cfg, keyword, now).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_keyword_job_publish(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    keyword: KadId,
    file: KadId,
    filename: String,
    file_size: u64,
    file_type: Option<String>,
    now: Instant,
) -> Result<()> {
    let job = svc
        .keyword_jobs
        .entry(keyword)
        .or_insert_with(|| KeywordJob {
            created_at: now,
            next_lookup_at: now,
            next_search_at: now,
            next_publish_at: now,
            sent_to_search: HashSet::new(),
            sent_to_publish: HashSet::new(),
            want_search: false,
            publish: None,
            got_publish_ack: false,
        });

    job.publish = Some(KeywordPublishSpec {
        file,
        filename,
        file_size,
        file_type,
    });
    job.created_at = now;
    job.next_lookup_at = now;
    job.next_publish_at = now;
    job.got_publish_ack = false;
    job.sent_to_publish.clear();

    progress_keyword_job(svc, sock, crypto, cfg, keyword, now).await?;
    Ok(())
}

async fn progress_keyword_jobs(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    if svc.keyword_jobs.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let keys: Vec<KadId> = svc.keyword_jobs.keys().copied().collect();
    for k in keys {
        let expired = svc
            .keyword_jobs
            .get(&k)
            .is_some_and(|j| now.saturating_duration_since(j.created_at) > KEYWORD_JOB_TTL);
        if expired {
            svc.keyword_jobs.remove(&k);
            continue;
        }
        // Publish can stop early once any peer acks it; search may continue.
        if let Some(j) = svc.keyword_jobs.get_mut(&k)
            && j.got_publish_ack
        {
            j.publish = None;
            j.sent_to_publish.clear();
        }

        progress_keyword_job(svc, sock, crypto, cfg, k, now).await?;
    }
    Ok(())
}

async fn progress_keyword_job(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    keyword: KadId,
    now: Instant,
) -> Result<()> {
    let Some(mut job) = svc.keyword_jobs.remove(&keyword) else {
        return Ok(());
    };

    let needs_work = job.want_search || job.publish.is_some();
    if needs_work && now >= job.next_lookup_at {
        // Targeted lookup toward the keyword ID to discover closer nodes.
        let req_min = Duration::from_secs(cfg.req_min_interval_secs.max(1));
        let alpha = cfg.alpha.max(1);
        let mut candidates = svc.routing.select_query_candidates_for_target(
            keyword,
            alpha * 10,
            now,
            req_min,
            cfg.max_failures,
        );
        candidates.retain(|p| !svc.pending_reqs.contains_key(&p.udp_dest_b64()));
        for p in candidates.into_iter().take(alpha) {
            let requested_contacts = cfg.req_contacts.clamp(1, 31);
            let _ = send_kad2_req(svc, sock, crypto, cfg, requested_contacts, keyword, &p).await;
        }
        job.next_lookup_at = now + KEYWORD_JOB_LOOKUP_EVERY;
    }

    if !needs_work {
        // Nothing to do; keep the job around briefly in case the user immediately triggers more work.
        svc.keyword_jobs.insert(keyword, job);
        return Ok(());
    }

    if now < job.next_search_at && now < job.next_publish_at {
        svc.keyword_jobs.insert(keyword, job);
        return Ok(());
    }

    let peers = closest_peers_with_fallback(svc, keyword, 32, 3, 0);
    let hello_min = Duration::from_secs(cfg.hello_min_interval_secs.max(60));

    if job.want_search && now >= job.next_search_at {
        let mut sent = 0usize;
        let mut dests = Vec::<String>::new();
        for p in peers.iter() {
            if sent >= KEYWORD_JOB_ACTION_BATCH {
                break;
            }
            if p.kad_version < 3 {
                continue;
            }
            let dest = p.udp_dest_b64();
            if job.sent_to_search.contains(&dest) {
                continue;
            }

            maybe_send_hello_to_peer(svc, sock, crypto, cfg, p, now, hello_min).await?;
            let payload = encode_kad2_search_key_req(keyword, 0);
            if send_kad2_packet(
                svc,
                sock,
                cfg,
                p,
                crypto,
                KADEMLIA2_SEARCH_KEY_REQ,
                &payload,
                None,
            )
            .await
            .is_ok_and(|sent| sent.is_some())
            {
                svc.stats_window.sent_search_key_reqs += 1;
                job.sent_to_search.insert(dest.clone());
                dests.push(crate::i2p::b64::short(&dest).to_string());
                sent += 1;
            }
        }

        if sent > 0 {
            tracing::info!(
                event = "keyword_search_req_batch",
                keyword = %crate::logging::redact_hex(&keyword.to_hex_lower()),
                sent,
                to = %dests.join(","),
                "sent SEARCH_KEY_REQ (job)"
            );
        }
        job.next_search_at = now + KEYWORD_JOB_ACTION_EVERY;
    }

    if let Some(pubspec) = job.publish.as_ref()
        && !job.got_publish_ack
        && now >= job.next_publish_at
    {
        let mut sent = 0usize;
        let mut dests = Vec::<String>::new();
        for p in peers.iter() {
            if sent >= KEYWORD_JOB_ACTION_BATCH {
                break;
            }
            if p.kad_version < 2 {
                continue;
            }
            let dest = p.udp_dest_b64();
            if job.sent_to_publish.contains(&dest) {
                continue;
            }

            maybe_send_hello_to_peer(svc, sock, crypto, cfg, p, now, hello_min).await?;
            let entries = [(
                pubspec.file,
                pubspec.filename.as_str(),
                pubspec.file_size,
                pubspec.file_type.as_deref(),
            )];
            let payload = encode_kad2_publish_key_req(keyword, &entries);
            if send_kad2_packet(
                svc,
                sock,
                cfg,
                p,
                crypto,
                KADEMLIA2_PUBLISH_KEY_REQ,
                &payload,
                None,
            )
            .await
            .is_ok_and(|sent| sent.is_some())
            {
                svc.stats_window.sent_publish_key_reqs += 1;
                job.sent_to_publish.insert(dest.clone());
                dests.push(crate::i2p::b64::short(&dest).to_string());
                sent += 1;
            }
        }

        if sent > 0 {
            tracing::info!(
                event = "keyword_publish_req_batch",
                keyword = %crate::logging::redact_hex(&keyword.to_hex_lower()),
                file = %crate::logging::redact_hex(&pubspec.file.to_hex_lower()),
                sent,
                to = %dests.join(","),
                "sent PUBLISH_KEY_REQ (job)"
            );
        }
        job.next_publish_at = now + KEYWORD_JOB_ACTION_EVERY;
    }

    svc.keyword_jobs.insert(keyword, job);
    Ok(())
}

fn cache_local_published_source(svc: &mut KadService, crypto: KadServiceCrypto, file: KadId) {
    let entry = svc.sources_by_file.entry(file).or_default();
    if entry.insert(crypto.my_kad_id, crypto.my_dest).is_none() {
        svc.stats_window.new_store_source_entries =
            svc.stats_window.new_store_source_entries.saturating_add(1);
        let (source_store_files, source_store_entries_total) = source_store_totals(svc);
        tracing::info!(
            event = "source_store_local_publish_upsert",
            file = %crate::logging::redact_hex(&file.to_hex_lower()),
            source = %crate::logging::redact_hex(&crypto.my_kad_id.to_hex_lower()),
            source_store_files,
            source_store_entries_total,
            "cached local source entry for published file"
        );
    }
}

fn closest_peers_by_distance(
    svc: &KadService,
    target: KadId,
    max: usize,
    min_kad_version: u8,
    exclude_dest_hash: u32,
) -> Vec<ImuleNode> {
    let mut peers = svc.routing.closest_to(target, max * 2, exclude_dest_hash);
    peers.retain(|p| p.kad_version >= min_kad_version);
    peers.truncate(max);
    peers
}

fn closest_peers_with_fallback(
    svc: &KadService,
    target: KadId,
    max: usize,
    min_kad_version: u8,
    exclude_dest_hash: u32,
) -> Vec<ImuleNode> {
    let mut peers = svc.routing.closest_to(target, max * 4, exclude_dest_hash);
    peers.retain(|p| p.kad_version >= min_kad_version);
    let now = Instant::now();
    peers.sort_by_key(|p| {
        let dest = p.udp_dest_b64();
        (
            peer_health_rank(
                svc.routing
                    .peer_health_class_by_dest(&dest, now)
                    .unwrap_or(crate::kad::routing::PeerHealthClass::Unknown),
            ),
            xor_distance(KadId(p.client_id), target),
        )
    });
    peers
}

fn peer_health_rank(class: crate::kad::routing::PeerHealthClass) -> u8 {
    match class {
        crate::kad::routing::PeerHealthClass::Stable => 0,
        crate::kad::routing::PeerHealthClass::Verified => 1,
        crate::kad::routing::PeerHealthClass::Unknown => 2,
        crate::kad::routing::PeerHealthClass::Unreliable => 3,
    }
}

fn xor_distance(a: KadId, b: KadId) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = a.0[i] ^ b.0[i];
    }
    out
}

async fn maybe_send_hello_to_peer(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    peer: &ImuleNode,
    now: Instant,
    min_interval: Duration,
) -> Result<()> {
    let dest = peer.udp_dest_b64();
    let Some(st) = svc.routing.get_by_dest(&dest) else {
        return Ok(());
    };
    if !should_send_hello(st, now, min_interval) {
        return Ok(());
    }

    let hello_plain_payload = encode_kad2_hello_req(1, crypto.my_kad_id, &crypto.my_dest);
    let hello_plain = KadPacket::encode(KADEMLIA2_HELLO_REQ, &hello_plain_payload);

    let receiver_verify_key = if peer.udp_key_ip == crypto.my_dest_hash {
        peer.udp_key
    } else {
        0
    };

    let out = hello_plain.clone();

    match shaper_send(
        svc,
        sock,
        cfg,
        OutboundClass::Hello,
        &dest,
        &out,
        KADEMLIA2_HELLO_REQ,
    )
    .await
    {
        Err(err) => {
            tracing::debug!(error = %err, to = %dest, "failed sending KAD2 HELLO_REQ (preflight)");
        }
        Ok(false) => {}
        Ok(true) => {
            track_outgoing_request(svc, &dest, KADEMLIA2_HELLO_REQ, now, None);
            svc.stats_window.sent_hellos += 1;
            svc.routing.mark_hello_sent_by_dest(&dest, now);
            tracing::debug!(
                to = %crate::i2p::b64::short(&dest),
                kad_version = peer.kad_version,
                encrypted = false,
                receiver_key = receiver_verify_key != 0,
                "sent HELLO_REQ"
            );
        }
    }

    if cfg.hello_dual_obfuscated && peer.kad_version >= 6 && receiver_verify_key != 0 {
        let target_kad_id = KadId(peer.client_id);
        let sender_verify_key =
            udp_crypto::udp_verify_key(crypto.udp_key_secret, peer.udp_dest_hash_code());
        if let Ok(hello) = udp_crypto::encrypt_kad_packet(
            &hello_plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        ) {
            match shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Hello,
                &dest,
                &hello,
                KADEMLIA2_HELLO_REQ,
            )
            .await
            {
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        to = %dest,
                        "failed sending KAD2 HELLO_REQ (dual obfuscated)"
                    );
                }
                Ok(false) => {}
                Ok(true) => {
                    svc.stats_window.sent_hellos += 1;
                    tracing::debug!(
                        to = %crate::i2p::b64::short(&dest),
                        "sent HELLO_REQ (dual obfuscated)"
                    );
                }
            }
        }
    }

    Ok(())
}

async fn maybe_hello_on_inbound(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    from_dest_b64: &str,
    now: Instant,
) -> Result<()> {
    let Some(st) = svc.routing.get_by_dest(from_dest_b64) else {
        return Ok(());
    };
    let hello_min = Duration::from_secs(cfg.hello_min_interval_secs.max(60));
    let peer = st.node.clone();
    maybe_send_hello_to_peer(svc, sock, crypto, cfg, &peer, now, hello_min).await
}

fn should_send_hello(
    st: &crate::kad::routing::NodeState,
    now: Instant,
    min_interval: Duration,
) -> bool {
    if st.needs_hello {
        return true;
    }
    match st.last_hello {
        Some(t) => now.saturating_duration_since(t) >= min_interval,
        None => true,
    }
}

fn canonical_request_opcode(opcode: u8) -> Option<u8> {
    match opcode {
        KADEMLIA2_BOOTSTRAP_REQ => Some(KADEMLIA2_BOOTSTRAP_REQ),
        KADEMLIA_HELLO_REQ_DEPRECATED | KADEMLIA2_HELLO_REQ => Some(KADEMLIA2_HELLO_REQ),
        KADEMLIA2_HELLO_RES => Some(KADEMLIA2_HELLO_RES),
        KADEMLIA_REQ_DEPRECATED | KADEMLIA2_REQ => Some(KADEMLIA2_REQ),
        KADEMLIA2_SEARCH_KEY_REQ => Some(KADEMLIA2_SEARCH_KEY_REQ),
        KADEMLIA2_SEARCH_SOURCE_REQ => Some(KADEMLIA2_SEARCH_SOURCE_REQ),
        KADEMLIA2_PUBLISH_KEY_REQ => Some(KADEMLIA2_PUBLISH_KEY_REQ),
        KADEMLIA2_PUBLISH_SOURCE_REQ => Some(KADEMLIA2_PUBLISH_SOURCE_REQ),
        KADEMLIA2_PING => Some(KADEMLIA2_PING),
        _ => None,
    }
}

fn response_expected_request_opcodes(
    response_opcode: u8,
    payload_len: usize,
) -> Option<&'static [u8]> {
    const EXPECT_BOOTSTRAP: &[u8] = &[KADEMLIA2_BOOTSTRAP_REQ];
    const EXPECT_HELLO_RES: &[u8] = &[KADEMLIA2_HELLO_REQ];
    const EXPECT_HELLO_ACK: &[u8] = &[KADEMLIA2_HELLO_RES];
    const EXPECT_RES: &[u8] = &[KADEMLIA2_REQ];
    const EXPECT_SEARCH_RES: &[u8] = &[KADEMLIA2_SEARCH_KEY_REQ, KADEMLIA2_SEARCH_SOURCE_REQ];
    const EXPECT_PONG: &[u8] = &[KADEMLIA2_PING];
    const EXPECT_PUBLISH_KEY: &[u8] = &[KADEMLIA2_PUBLISH_KEY_REQ];
    const EXPECT_PUBLISH_SRC_OR_KEY: &[u8] =
        &[KADEMLIA2_PUBLISH_SOURCE_REQ, KADEMLIA2_PUBLISH_KEY_REQ];

    match response_opcode {
        KADEMLIA2_BOOTSTRAP_RES => Some(EXPECT_BOOTSTRAP),
        KADEMLIA2_HELLO_RES => Some(EXPECT_HELLO_RES),
        KADEMLIA2_HELLO_RES_ACK => Some(EXPECT_HELLO_ACK),
        KADEMLIA_RES_DEPRECATED | KADEMLIA2_RES => Some(EXPECT_RES),
        KADEMLIA2_SEARCH_RES => Some(EXPECT_SEARCH_RES),
        KADEMLIA2_PONG => Some(EXPECT_PONG),
        KADEMLIA2_PUBLISH_RES => {
            if payload_len == 17 {
                Some(EXPECT_PUBLISH_KEY)
            } else {
                Some(EXPECT_PUBLISH_SRC_OR_KEY)
            }
        }
        _ => None,
    }
}

fn kad_opcode_name(opcode: u8) -> &'static str {
    match opcode {
        KADEMLIA_HELLO_REQ_DEPRECATED => "KADEMLIA_HELLO_REQ",
        KADEMLIA_HELLO_RES_DEPRECATED => "KADEMLIA_HELLO_RES",
        KADEMLIA_REQ_DEPRECATED => "KADEMLIA_REQ",
        KADEMLIA_RES_DEPRECATED => "KADEMLIA_RES",
        KADEMLIA_SEARCH_REQ_DEPRECATED => "KADEMLIA_SEARCH_REQ",
        KADEMLIA_SEARCH_RES_DEPRECATED => "KADEMLIA_SEARCH_RES",
        KADEMLIA_SEARCH_NOTES_REQ_DEPRECATED => "KADEMLIA_SEARCH_NOTES_REQ",
        KADEMLIA_PUBLISH_REQ_DEPRECATED => "KADEMLIA_PUBLISH_REQ",
        KADEMLIA_PUBLISH_RES_DEPRECATED => "KADEMLIA_PUBLISH_RES",
        KADEMLIA_PUBLISH_NOTES_REQ_DEPRECATED => "KADEMLIA_PUBLISH_NOTES_REQ",
        KADEMLIA2_BOOTSTRAP_REQ => "KADEMLIA2_BOOTSTRAP_REQ",
        KADEMLIA2_BOOTSTRAP_RES => "KADEMLIA2_BOOTSTRAP_RES",
        KADEMLIA2_HELLO_REQ => "KADEMLIA2_HELLO_REQ",
        KADEMLIA2_HELLO_RES => "KADEMLIA2_HELLO_RES",
        KADEMLIA2_REQ => "KADEMLIA2_REQ",
        KADEMLIA2_HELLO_RES_ACK => "KADEMLIA2_HELLO_RES_ACK",
        KADEMLIA2_RES => "KADEMLIA2_RES",
        KADEMLIA2_SEARCH_KEY_REQ => "KADEMLIA2_SEARCH_KEY_REQ",
        KADEMLIA2_SEARCH_SOURCE_REQ => "KADEMLIA2_SEARCH_SOURCE_REQ",
        KADEMLIA2_SEARCH_RES => "KADEMLIA2_SEARCH_RES",
        KADEMLIA2_PUBLISH_KEY_REQ => "KADEMLIA2_PUBLISH_KEY_REQ",
        KADEMLIA2_PUBLISH_SOURCE_REQ => "KADEMLIA2_PUBLISH_SOURCE_REQ",
        KADEMLIA2_PUBLISH_RES => "KADEMLIA2_PUBLISH_RES",
        KADEMLIA2_PING => "KADEMLIA2_PING",
        KADEMLIA2_PONG => "KADEMLIA2_PONG",
        _ => "UNKNOWN",
    }
}

fn kad_dispatch_target(opcode: u8) -> &'static str {
    match opcode {
        KADEMLIA_HELLO_REQ_DEPRECATED => "handle_kad1_hello_req",
        KADEMLIA_SEARCH_REQ_DEPRECATED
        | KADEMLIA_SEARCH_RES_DEPRECATED
        | KADEMLIA_SEARCH_NOTES_REQ_DEPRECATED
        | KADEMLIA_PUBLISH_REQ_DEPRECATED
        | KADEMLIA_PUBLISH_RES_DEPRECATED
        | KADEMLIA_PUBLISH_NOTES_REQ_DEPRECATED => "drop_legacy_kad1_opcode",
        KADEMLIA2_BOOTSTRAP_REQ => "handle_bootstrap_req",
        KADEMLIA2_BOOTSTRAP_RES => "handle_bootstrap_res",
        KADEMLIA2_HELLO_REQ => "handle_hello_req",
        KADEMLIA2_HELLO_RES => "handle_hello_res",
        KADEMLIA2_HELLO_RES_ACK => "handle_hello_res_ack",
        KADEMLIA_REQ_DEPRECATED | KADEMLIA2_REQ => "handle_req",
        KADEMLIA_RES_DEPRECATED | KADEMLIA2_RES => "handle_res",
        KADEMLIA2_PING => "handle_ping",
        KADEMLIA2_PONG => "handle_pong",
        KADEMLIA2_PUBLISH_KEY_REQ => "handle_publish_key_req",
        KADEMLIA2_PUBLISH_SOURCE_REQ => "handle_publish_source_req",
        KADEMLIA2_SEARCH_KEY_REQ => "handle_search_key_req",
        KADEMLIA2_SEARCH_SOURCE_REQ => "handle_search_source_req",
        KADEMLIA2_SEARCH_RES => "handle_search_res",
        KADEMLIA2_PUBLISH_RES => "handle_publish_res",
        _ => "drop_unhandled_opcode",
    }
}

fn track_outgoing_request(
    svc: &mut KadService,
    dest_b64: &str,
    opcode: u8,
    now: Instant,
    trace_tag: Option<&str>,
) -> Option<u64> {
    let request_opcode = canonical_request_opcode(opcode)?;
    cleanup_tracked_out_requests(svc, now);
    let request_id = svc.next_tracked_out_request_id;
    svc.next_tracked_out_request_id = svc.next_tracked_out_request_id.saturating_add(1);
    svc.tracked_out_requests.push(TrackedOutRequest {
        request_id,
        dest_b64: dest_b64.to_string(),
        request_opcode,
        trace_tag: trace_tag.map(std::string::ToString::to_string),
        inserted: now,
    });
    Some(request_id)
}

fn cleanup_tracked_out_requests(svc: &mut KadService, now: Instant) {
    let before = svc.tracked_out_requests.len();
    svc.tracked_out_requests
        .retain(|t| now.saturating_duration_since(t.inserted) <= TRACKED_OUT_REQUEST_TTL);
    let removed = before.saturating_sub(svc.tracked_out_requests.len());
    svc.stats_window.tracked_out_expired += removed as u64;
}

fn consume_tracked_out_request(
    svc: &mut KadService,
    from_dest_b64: &str,
    response_opcode: u8,
    response_payload_len: usize,
    now: Instant,
) -> bool {
    cleanup_tracked_out_requests(svc, now);
    let Some(expected) = response_expected_request_opcodes(response_opcode, response_payload_len)
    else {
        return true;
    };

    let Some(pos) = svc
        .tracked_out_requests
        .iter()
        .position(|t| t.dest_b64 == from_dest_b64 && expected.contains(&t.request_opcode))
    else {
        let peer_tracked = svc
            .tracked_out_requests
            .iter()
            .filter(|t| t.dest_b64 == from_dest_b64)
            .count();
        svc.last_unmatched_response = Some(UnmatchedResponseDiag {
            from_dest_b64: from_dest_b64.to_string(),
            response_opcode,
            expected_request_opcodes: expected.to_vec(),
            peer_tracked_requests: peer_tracked,
            total_tracked_requests: svc.tracked_out_requests.len(),
        });
        if response_opcode == KADEMLIA2_SEARCH_RES || response_opcode == KADEMLIA2_PUBLISH_RES {
            tracing::info!(
                event = "source_probe_response_unmatched",
                from = %crate::i2p::b64::short(from_dest_b64),
                response_opcode = kad_opcode_name(response_opcode),
                expected_request_opcodes = ?expected.iter().map(|op| kad_opcode_name(*op)).collect::<Vec<_>>(),
                peer_tracked_requests = peer_tracked,
                total_tracked_requests = svc.tracked_out_requests.len(),
                "inbound source response did not match tracked outbound request"
            );
        }
        svc.stats_window.tracked_out_unmatched += 1;
        return false;
    };
    let tracked = svc.tracked_out_requests.remove(pos);
    svc.stats_window.tracked_out_matched += 1;
    svc.last_unmatched_response = None;
    if response_opcode == KADEMLIA2_SEARCH_RES || response_opcode == KADEMLIA2_PUBLISH_RES {
        tracing::info!(
            event = "source_probe_response_matched",
            request_id = tracked.request_id,
            from = %crate::i2p::b64::short(from_dest_b64),
            request_opcode = kad_opcode_name(tracked.request_opcode),
            response_opcode = kad_opcode_name(response_opcode),
            age_ms = now.saturating_duration_since(tracked.inserted).as_millis() as u64,
            trace_tag = tracked.trace_tag.as_deref().unwrap_or(""),
            "matched inbound source response to tracked outbound request"
        );
    }
    true
}

fn inbound_request_limit_per_minute(opcode: u8) -> Option<(u8, u32)> {
    let canonical = canonical_request_opcode(opcode)?;
    let limit = match canonical {
        KADEMLIA2_BOOTSTRAP_REQ => 2,
        KADEMLIA2_HELLO_REQ => 3,
        KADEMLIA2_REQ => 30,
        KADEMLIA2_SEARCH_KEY_REQ | KADEMLIA2_SEARCH_SOURCE_REQ => 30,
        KADEMLIA2_PUBLISH_KEY_REQ | KADEMLIA2_PUBLISH_SOURCE_REQ => 30,
        KADEMLIA2_PING => 2,
        _ => return None,
    };
    Some((canonical, limit))
}

fn inbound_request_allowed(svc: &mut KadService, from_hash: u32, opcode: u8, now: Instant) -> bool {
    let Some((canonical_opcode, limit_per_minute)) = inbound_request_limit_per_minute(opcode)
    else {
        return true;
    };

    if now.saturating_duration_since(svc.tracked_in_last_cleanup) >= TRACKED_IN_CLEANUP_EVERY {
        svc.tracked_in_requests.retain(|_, op_map| {
            op_map.retain(|_, c| {
                now.saturating_duration_since(c.first_added) <= TRACKED_IN_ENTRY_TTL
            });
            !op_map.is_empty()
        });
        svc.tracked_in_last_cleanup = now;
    }

    let per_dest = svc.tracked_in_requests.entry(from_hash).or_default();
    let counter = per_dest
        .entry(canonical_opcode)
        .or_insert(TrackedInCounter {
            first_added: now,
            count: 0,
            warned: false,
        });

    if now.saturating_duration_since(counter.first_added) >= Duration::from_secs(60) {
        counter.first_added = now;
        counter.count = 0;
        counter.warned = false;
    }

    counter.count = counter.count.saturating_add(1);
    if counter.count > limit_per_minute {
        if !counter.warned {
            tracing::debug!(
                from_hash,
                opcode = format_args!("0x{canonical_opcode:02x}"),
                count = counter.count,
                limit_per_minute,
                "dropping inbound KAD request due to flood limit"
            );
            counter.warned = true;
        }
        return false;
    }

    true
}

fn shaper_roll_window(svc: &mut KadService, now: Instant) {
    if now.saturating_duration_since(svc.shaper_window_started) >= Duration::from_secs(1) {
        svc.shaper_window_started = now;
        svc.shaper_global_sent_in_window.clear();
        svc.shaper_peer_sent_in_window.clear();
        shaper_cleanup_stale_peers(svc, now);
    }
}

fn shaper_peer_key(class: OutboundClass, dest_b64: &str) -> String {
    let cls = match class {
        OutboundClass::Query => "q",
        OutboundClass::Hello => "h",
        OutboundClass::Bootstrap => "b",
        OutboundClass::Response => "r",
    };
    format!("{cls}:{dest_b64}")
}

fn shaper_cleanup_stale_peers(svc: &mut KadService, now: Instant) {
    svc.shaper_last_peer_send
        .retain(|_, seen_at| now.saturating_duration_since(*seen_at) <= SHAPER_PEER_STATE_TTL);
    if svc.shaper_last_peer_send.len() <= SHAPER_PEER_STATE_MAX {
        return;
    }

    let mut by_age = svc
        .shaper_last_peer_send
        .iter()
        .map(|(peer, seen_at)| (peer.clone(), *seen_at))
        .collect::<Vec<_>>();
    by_age.sort_by_key(|(_, seen_at)| std::cmp::Reverse(*seen_at));
    let keep = by_age
        .into_iter()
        .take(SHAPER_PEER_STATE_MAX)
        .map(|(peer, _)| peer)
        .collect::<HashSet<_>>();
    svc.shaper_last_peer_send
        .retain(|peer, _| keep.contains(peer));
}

fn shaper_jitter_ms(svc: &mut KadService, max_ms: u64) -> u64 {
    if max_ms == 0 {
        return 0;
    }
    // Deterministic per-process jitter without external RNG dependency.
    svc.shaper_jitter_seed = svc
        .shaper_jitter_seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    svc.shaper_jitter_seed % (max_ms + 1)
}

fn shaper_policy(cfg: &KadServiceConfig, class: OutboundClass) -> ShaperClassPolicy {
    fn scaled_cap_or_disabled(base: u32, factor: u32, min_if_enabled: u32) -> u32 {
        if base == 0 {
            0
        } else {
            base.saturating_mul(factor).max(min_if_enabled)
        }
    }

    let query = ShaperClassPolicy {
        base_delay_ms: cfg.outbound_shaper_base_delay_ms,
        jitter_ms: cfg.outbound_shaper_jitter_ms,
        global_min_interval_ms: cfg.outbound_shaper_global_min_interval_ms,
        peer_min_interval_ms: cfg.outbound_shaper_peer_min_interval_ms,
        global_max_per_sec: cfg.outbound_shaper_global_max_per_sec,
        peer_max_per_sec: cfg.outbound_shaper_peer_max_per_sec,
        drop_when_delayed: true,
    };

    match class {
        OutboundClass::Query => query,
        OutboundClass::Hello => ShaperClassPolicy {
            base_delay_ms: query.base_delay_ms / 2,
            jitter_ms: query.jitter_ms / 2,
            global_min_interval_ms: query.global_min_interval_ms,
            peer_min_interval_ms: query.peer_min_interval_ms / 2,
            global_max_per_sec: scaled_cap_or_disabled(query.global_max_per_sec, 2, 32),
            peer_max_per_sec: scaled_cap_or_disabled(query.peer_max_per_sec, 2, 8),
            drop_when_delayed: true,
        },
        OutboundClass::Bootstrap => ShaperClassPolicy {
            base_delay_ms: query.base_delay_ms / 2,
            jitter_ms: query.jitter_ms / 2,
            global_min_interval_ms: query.global_min_interval_ms / 2,
            peer_min_interval_ms: query.peer_min_interval_ms / 2,
            global_max_per_sec: scaled_cap_or_disabled(query.global_max_per_sec, 2, 32),
            peer_max_per_sec: scaled_cap_or_disabled(query.peer_max_per_sec, 2, 8),
            drop_when_delayed: true,
        },
        OutboundClass::Response => ShaperClassPolicy {
            base_delay_ms: 0,
            jitter_ms: 0,
            global_min_interval_ms: 0,
            peer_min_interval_ms: 0,
            global_max_per_sec: scaled_cap_or_disabled(query.global_max_per_sec, 4, 128),
            peer_max_per_sec: scaled_cap_or_disabled(query.peer_max_per_sec, 4, 24),
            drop_when_delayed: false,
        },
    }
}

fn outbound_class_for_opcode(opcode: u8) -> OutboundClass {
    match opcode {
        KADEMLIA2_HELLO_REQ | KADEMLIA_HELLO_REQ_DEPRECATED => OutboundClass::Hello,
        KADEMLIA2_BOOTSTRAP_REQ => OutboundClass::Bootstrap,
        _ => OutboundClass::Query,
    }
}

fn shaper_schedule_send(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    class: OutboundClass,
    dest_b64: &str,
    now: Instant,
) -> Option<Instant> {
    shaper_roll_window(svc, now);
    let policy = shaper_policy(cfg, class);

    let global_sent = svc
        .shaper_global_sent_in_window
        .get(&class)
        .copied()
        .unwrap_or(0);
    if policy.global_max_per_sec > 0 && global_sent >= policy.global_max_per_sec {
        svc.stats_window.outbound_shaper_drop_global_cap = svc
            .stats_window
            .outbound_shaper_drop_global_cap
            .saturating_add(1);
        return None;
    }

    let peer_key = shaper_peer_key(class, dest_b64);
    let peer_sent = svc
        .shaper_peer_sent_in_window
        .get(&peer_key)
        .copied()
        .unwrap_or(0);
    if policy.peer_max_per_sec > 0 && peer_sent >= policy.peer_max_per_sec {
        svc.stats_window.outbound_shaper_drop_peer_cap = svc
            .stats_window
            .outbound_shaper_drop_peer_cap
            .saturating_add(1);
        return None;
    }

    let global_min_interval = Duration::from_millis(policy.global_min_interval_ms);
    let peer_min_interval = Duration::from_millis(policy.peer_min_interval_ms);

    let mut target = if policy.drop_when_delayed {
        now
    } else {
        let base_delay = Duration::from_millis(policy.base_delay_ms);
        let jitter_delay = Duration::from_millis(shaper_jitter_ms(svc, policy.jitter_ms));
        now + base_delay + jitter_delay
    };
    if let Some(last) = svc.shaper_last_global_send.get(&class).copied() {
        target = std::cmp::max(target, last + global_min_interval);
    }
    if let Some(last) = svc.shaper_last_peer_send.get(&peer_key).copied() {
        target = std::cmp::max(target, last + peer_min_interval);
    }
    Some(target)
}

fn shaper_mark_sent(svc: &mut KadService, class: OutboundClass, dest_b64: &str, sent_at: Instant) {
    shaper_roll_window(svc, sent_at);
    let global_counter = svc.shaper_global_sent_in_window.entry(class).or_insert(0);
    *global_counter = global_counter.saturating_add(1);
    let peer_key = shaper_peer_key(class, dest_b64);
    let peer_counter = svc
        .shaper_peer_sent_in_window
        .entry(peer_key.clone())
        .or_insert(0);
    *peer_counter = peer_counter.saturating_add(1);
    svc.shaper_last_global_send.insert(class, sent_at);
    svc.shaper_last_peer_send.insert(peer_key, sent_at);
}

async fn shaper_send(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    cfg: &KadServiceConfig,
    class: OutboundClass,
    dest_b64: &str,
    payload: &[u8],
    opcode: u8,
) -> Result<bool> {
    let now = Instant::now();
    let policy = shaper_policy(cfg, class);
    let Some(send_at) = shaper_schedule_send(svc, cfg, class, dest_b64, now) else {
        tracing::debug!(
            to = %crate::i2p::b64::short(dest_b64),
            opcode = kad_opcode_name(opcode),
            "outbound packet dropped by shaper cap"
        );
        return Ok(false);
    };

    if send_at > now && policy.drop_when_delayed {
        svc.stats_window.outbound_shaper_delayed =
            svc.stats_window.outbound_shaper_delayed.saturating_add(1);
        return Ok(false);
    }
    sock.send_to(dest_b64, payload).await?;
    shaper_mark_sent(svc, class, dest_b64, Instant::now());
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
async fn send_kad2_packet(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    cfg: &KadServiceConfig,
    node: &ImuleNode,
    crypto: KadServiceCrypto,
    opcode: u8,
    payload: &[u8],
    trace_tag: Option<&str>,
) -> Result<Option<u64>> {
    let dest = node.udp_dest_b64();
    let target_kad_id = KadId(node.client_id);
    let plain = KadPacket::encode(opcode, payload);

    let sender_verify_key =
        udp_crypto::udp_verify_key(crypto.udp_key_secret, node.udp_dest_hash_code());
    let receiver_verify_key = if node.udp_key_ip == crypto.my_dest_hash {
        node.udp_key
    } else {
        0
    };

    let out = if node.kad_version >= 6 {
        udp_crypto::encrypt_kad_packet(
            &plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        )?
    } else {
        plain
    };

    let class = outbound_class_for_opcode(opcode);
    if !shaper_send(svc, sock, cfg, class, &dest, &out, opcode).await? {
        return Ok(None);
    }
    Ok(track_outgoing_request(
        svc,
        &dest,
        opcode,
        Instant::now(),
        trace_tag,
    ))
}

async fn persist_snapshot(svc: &KadService, path: &std::path::Path, max_nodes: usize) {
    // Merge the current routing snapshot into whatever is already persisted on disk.
    //
    // This prevents `nodes.dat` from shrinking to a tiny set if we temporarily evict dead entries;
    // we still want a broader seed pool across restarts.
    let mut merged = std::collections::BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();
    if let Ok(existing) = crate::nodes::imule::nodes_dat_contacts(path).await {
        for n in existing {
            merged.insert(n.udp_dest_b64(), n);
        }
    }

    for n in svc.routing.snapshot_nodes(max_nodes) {
        let k = n.udp_dest_b64();
        merged
            .entry(k)
            .and_modify(|old| {
                if n.kad_version > old.kad_version {
                    old.kad_version = n.kad_version;
                }
                if n.verified {
                    old.verified = true;
                }
                if old.udp_key == 0 && n.udp_key != 0 {
                    old.udp_key = n.udp_key;
                    old.udp_key_ip = n.udp_key_ip;
                }
                if old.client_id == [0u8; 16] && n.client_id != [0u8; 16] {
                    old.client_id = n.client_id;
                }
            })
            .or_insert(n);
    }

    let mut nodes: Vec<_> = merged.into_values().collect();
    nodes.sort_by_key(|n| {
        (
            std::cmp::Reverse(n.verified),
            std::cmp::Reverse(n.udp_key != 0),
            std::cmp::Reverse(n.kad_version),
        )
    });
    nodes.truncate(max_nodes);

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

    // Exploration strategy:
    // Prefer "live" peers, but always try to poke at least one "cold" peer as well.
    let alpha = cfg.alpha.max(1);
    let mut candidates = svc.routing.select_query_candidates_for_target(
        target,
        alpha * 10, // enough to pick a mix
        now,
        req_min,
        cfg.max_failures,
    );
    candidates.retain(|p| !svc.pending_reqs.contains_key(&p.udp_dest_b64()));
    if candidates.is_empty() {
        return Ok(());
    }

    let mut peers: Vec<ImuleNode> = Vec::with_capacity(alpha);
    let mut have_cold = false;
    for c in &candidates {
        if peers.len() >= alpha {
            break;
        }
        let id = KadId(c.client_id);
        let is_cold = svc
            .routing
            .get_by_id(id)
            .and_then(|st| st.last_inbound)
            .is_none();
        if is_cold {
            have_cold = true;
        }
        peers.push(c.clone());
    }

    if !have_cold
        && let Some(cold) = candidates.iter().find(|c| {
            let id = KadId(c.client_id);
            svc.routing
                .get_by_id(id)
                .and_then(|st| st.last_inbound)
                .is_none()
        })
    {
        if !peers.is_empty() {
            peers.pop();
        }
        peers.push(cold.clone());
    }

    let requested_contacts = cfg.req_contacts.clamp(1, 31);

    for p in peers {
        if let Err(err) =
            send_kad2_req(svc, sock, crypto, cfg, requested_contacts, target, &p).await
        {
            tracing::debug!(
                error = %err,
                to = %crate::i2p::b64::short(&p.udp_dest_b64()),
                "failed sending KAD2 REQ (crawl)"
            );
        }
    }

    Ok(())
}

async fn send_kad2_req(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    requested_contacts: u8,
    target: KadId,
    peer: &ImuleNode,
) -> Result<()> {
    let dest = peer.udp_dest_b64();
    let target_kad_id = KadId(peer.client_id);

    // NOTE: iMule's `KADEMLIA2_REQ` includes a `check` field which must match the *receiver's*
    // KadID (used to discard packets not intended for this node). If we put our own KadID here,
    // peers will silently ignore the request and we'll never get `KADEMLIA2_RES`.
    let req_payload = encode_kad2_req(requested_contacts, target, target_kad_id, crypto.my_kad_id);
    if send_kad2_packet(
        svc,
        sock,
        cfg,
        peer,
        crypto,
        KADEMLIA2_REQ,
        &req_payload,
        None,
    )
    .await?
    .is_some()
    {
        svc.stats_window.sent_reqs += 1;
        svc.pending_reqs.insert(
            dest.clone(),
            Instant::now() + Duration::from_secs(cfg.req_timeout_secs.max(5)),
        );
        svc.routing.mark_queried_by_dest(&dest, Instant::now());
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

    let hello_plain_payload = encode_kad2_hello_req(1, crypto.my_kad_id, &crypto.my_dest);
    let hello_plain = KadPacket::encode(KADEMLIA2_HELLO_REQ, &hello_plain_payload);

    for p in peers.drain(..) {
        let dest = p.udp_dest_b64();

        let out = hello_plain.clone();

        match shaper_send(
            svc,
            sock,
            cfg,
            OutboundClass::Hello,
            &dest,
            &out,
            KADEMLIA2_HELLO_REQ,
        )
        .await
        {
            Err(err) => {
                tracing::debug!(error = %err, to = %dest, "failed sending KAD2 HELLO_REQ (service)");
            }
            Ok(false) => {}
            Ok(true) => {
                track_outgoing_request(svc, &dest, KADEMLIA2_HELLO_REQ, now, None);
                tracing::trace!(to = %dest, "sent KAD2 HELLO_REQ (service)");
                svc.stats_window.sent_hellos += 1;
                svc.routing.mark_hello_sent_by_dest(&dest, now);
            }
        }

        if cfg.hello_dual_obfuscated
            && p.kad_version >= 6
            && p.udp_key_ip == crypto.my_dest_hash
            && p.udp_key != 0
        {
            let target_kad_id = KadId(p.client_id);
            let sender_verify_key =
                udp_crypto::udp_verify_key(crypto.udp_key_secret, p.udp_dest_hash_code());
            let receiver_verify_key = p.udp_key;
            if let Ok(hello) = udp_crypto::encrypt_kad_packet(
                &hello_plain,
                target_kad_id,
                receiver_verify_key,
                sender_verify_key,
            ) {
                match shaper_send(
                    svc,
                    sock,
                    cfg,
                    OutboundClass::Hello,
                    &dest,
                    &hello,
                    KADEMLIA2_HELLO_REQ,
                )
                .await
                {
                    Err(err) => {
                        tracing::debug!(
                            error = %err,
                            to = %dest,
                            "failed sending KAD2 HELLO_REQ (dual obfuscated)"
                        );
                    }
                    Ok(false) => {}
                    Ok(true) => {
                        tracing::trace!(to = %dest, "sent KAD2 HELLO_REQ (dual obfuscated)");
                        svc.stats_window.sent_hellos += 1;
                    }
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn debug_probe_peer(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
    dest_b64: &str,
    keyword: KadId,
    file: KadId,
    filename: &str,
    file_size: u64,
    file_type: Option<&str>,
    now: Instant,
) -> Result<bool> {
    let Some(st) = svc.routing.get_by_dest(dest_b64) else {
        return Ok(false);
    };
    let peer = st.node.clone();

    let hello_payload = encode_kad2_hello_req(1, crypto.my_kad_id, &crypto.my_dest);
    let hello_plain = KadPacket::encode(KADEMLIA2_HELLO_REQ, &hello_payload);
    if shaper_send(
        svc,
        sock,
        cfg,
        OutboundClass::Hello,
        dest_b64,
        &hello_plain,
        KADEMLIA2_HELLO_REQ,
    )
    .await?
    {
        track_outgoing_request(svc, dest_b64, KADEMLIA2_HELLO_REQ, now, None);
        svc.stats_window.sent_hellos += 1;
        svc.routing.mark_hello_sent_by_dest(dest_b64, now);
    }

    if cfg.hello_dual_obfuscated
        && peer.kad_version >= 6
        && peer.udp_key_ip == crypto.my_dest_hash
        && peer.udp_key != 0
    {
        let target_kad_id = KadId(peer.client_id);
        let sender_verify_key =
            udp_crypto::udp_verify_key(crypto.udp_key_secret, peer.udp_dest_hash_code());
        let receiver_verify_key = peer.udp_key;
        if let Ok(hello) = udp_crypto::encrypt_kad_packet(
            &hello_plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        ) {
            match shaper_send(
                svc,
                sock,
                cfg,
                OutboundClass::Hello,
                dest_b64,
                &hello,
                KADEMLIA2_HELLO_REQ,
            )
            .await
            {
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        to = %crate::i2p::b64::short(dest_b64),
                        "debug probe failed to send HELLO_REQ (dual obfuscated)"
                    );
                }
                Ok(false) => {}
                Ok(true) => {
                    svc.stats_window.sent_hellos += 1;
                }
            }
        }
    }

    let search_payload = encode_kad2_search_key_req(keyword, 0);
    match send_kad2_packet(
        svc,
        sock,
        cfg,
        &peer,
        crypto,
        KADEMLIA2_SEARCH_KEY_REQ,
        &search_payload,
        None,
    )
    .await
    {
        Err(err) => {
            tracing::debug!(
                error = %err,
                to = %crate::i2p::b64::short(dest_b64),
                "debug probe failed to send SEARCH_KEY_REQ"
            );
        }
        Ok(Some(_)) => {
            svc.stats_window.sent_search_key_reqs += 1;
        }
        Ok(None) => {}
    }

    let publish_payload =
        encode_kad2_publish_key_req(keyword, &[(file, filename, file_size, file_type)]);
    match send_kad2_packet(
        svc,
        sock,
        cfg,
        &peer,
        crypto,
        KADEMLIA2_PUBLISH_KEY_REQ,
        &publish_payload,
        None,
    )
    .await
    {
        Err(err) => {
            tracing::debug!(
                error = %err,
                to = %crate::i2p::b64::short(dest_b64),
                "debug probe failed to send PUBLISH_KEY_REQ"
            );
        }
        Ok(Some(_)) => {
            svc.stats_window.sent_publish_key_reqs += 1;
        }
        Ok(None) => {}
    }

    // Force source-path probing against a known peer to isolate source opcode handling.
    let mut sent_search_source = false;
    let mut sent_publish_source = false;

    if peer.kad_version >= 3 {
        let search_source_payload = encode_kad2_search_source_req(file, 0, file_size);
        let trace_tag = format!(
            "source_search:{}",
            crate::logging::redact_hex(&file.to_hex_lower())
        );
        match send_kad2_packet(
            svc,
            sock,
            cfg,
            &peer,
            crypto,
            KADEMLIA2_SEARCH_SOURCE_REQ,
            &search_source_payload,
            Some(trace_tag.as_str()),
        )
        .await
        {
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    to = %crate::i2p::b64::short(dest_b64),
                    "debug probe failed to send SEARCH_SOURCE_REQ"
                );
            }
            Ok(Some(_)) => {
                sent_search_source = true;
                svc.stats_window.sent_search_source_reqs += 1;
                mark_source_search_sent(svc, file, now);
            }
            Ok(None) => {}
        }
    }

    if peer.kad_version >= 4 {
        let publish_source_payload = encode_kad2_publish_source_req(
            file,
            crypto.my_kad_id,
            &crypto.my_dest,
            Some(file_size),
        );
        let trace_tag = format!(
            "source_publish:{}",
            crate::logging::redact_hex(&file.to_hex_lower())
        );
        match send_kad2_packet(
            svc,
            sock,
            cfg,
            &peer,
            crypto,
            KADEMLIA2_PUBLISH_SOURCE_REQ,
            &publish_source_payload,
            Some(trace_tag.as_str()),
        )
        .await
        {
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    to = %crate::i2p::b64::short(dest_b64),
                    "debug probe failed to send PUBLISH_SOURCE_REQ"
                );
            }
            Ok(Some(_)) => {
                sent_publish_source = true;
                svc.stats_window.sent_publish_source_reqs += 1;
                mark_source_publish_sent(svc, file, now);
            }
            Ok(None) => {}
        }
    }

    tracing::debug!(
        to = %crate::i2p::b64::short(dest_b64),
        keyword = %crate::logging::redact_hex(&keyword.to_hex_lower()),
        file = %crate::logging::redact_hex(&file.to_hex_lower()),
        sent_search_source,
        sent_publish_source,
        "debug probe sent HELLO/SEARCH_KEY/PUBLISH_KEY with source probes"
    );

    Ok(true)
}

async fn send_bootstrap_batch(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let min_interval = Duration::from_secs(cfg.bootstrap_min_interval_secs.max(60));
    let recent_live_window = Duration::from_secs(10 * 60);
    let mut peers = svc.routing.select_bootstrap_candidates(
        cfg.bootstrap_batch.max(1),
        now,
        min_interval,
        recent_live_window,
    );
    if peers.is_empty() {
        tracing::debug!(
            routing = svc.routing.len(),
            live = svc.routing.live_count(),
            live_10m = svc.routing.live_count_recent(now, recent_live_window),
            "no eligible peers for periodic BOOTSTRAP_REQ"
        );
        return Ok(());
    }

    let plain = KadPacket::encode(KADEMLIA2_BOOTSTRAP_REQ, &[]);

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

        let out = if p.kad_version >= 6 {
            udp_crypto::encrypt_kad_packet(
                &plain,
                target_kad_id,
                receiver_verify_key,
                sender_verify_key,
            )?
        } else {
            plain.clone()
        };

        match shaper_send(
            svc,
            sock,
            cfg,
            OutboundClass::Bootstrap,
            &dest,
            &out,
            KADEMLIA2_BOOTSTRAP_REQ,
        )
        .await
        {
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    to = %dest,
                    "failed sending KAD2 BOOTSTRAP_REQ (service)"
                );
            }
            Ok(false) => {}
            Ok(true) => {
                track_outgoing_request(svc, &dest, KADEMLIA2_BOOTSTRAP_REQ, now, None);
                svc.stats_window.sent_bootstrap_reqs += 1;
                tracing::info!(
                    to = %crate::i2p::b64::short(&dest),
                    kad_version = p.kad_version,
                    encrypted = p.kad_version >= 6,
                    "sent periodic KAD2 BOOTSTRAP_REQ (refresh)"
                );
                svc.routing.mark_bootstrap_sent_by_dest(&dest, now);
            }
        }
    }

    Ok(())
}

fn touch_keyword_interest(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    keyword: KadId,
    now: Instant,
) {
    keyword::touch_keyword_interest_impl(svc, cfg, keyword, now);
}

fn upsert_keyword_hit_cache(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
    keyword: KadId,
    hit: KadKeywordHit,
) {
    keyword::upsert_keyword_hit_cache_impl(svc, cfg, now, keyword, hit);
}

fn enforce_keyword_per_keyword_cap(svc: &mut KadService, cfg: &KadServiceConfig, keyword: KadId) {
    keyword::enforce_keyword_per_keyword_cap_impl(svc, cfg, keyword);
}

fn enforce_keyword_total_cap(svc: &mut KadService, cfg: &KadServiceConfig, _now: Instant) {
    keyword::enforce_keyword_total_cap_impl(svc, cfg, _now);
}

async fn maintenance(svc: &mut KadService, cfg: &KadServiceConfig) {
    let now = Instant::now();
    cleanup_tracked_out_requests(svc, now);

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
        tracing::info!(
            evicted,
            remaining = svc.routing.len(),
            "evicted stale peers"
        );
    }

    maintain_keyword_cache(svc, cfg, now);
    maintain_keyword_store(svc, cfg, now);

    tick_refresh(svc, cfg, now);
}

fn maintain_keyword_cache(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    keyword::maintain_keyword_cache_impl(svc, cfg, now);
}

fn maintain_keyword_store(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    keyword::maintain_keyword_store_impl(svc, cfg, now);
}

fn tick_refresh(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    lookup::tick_refresh_impl(svc, cfg, now);
}

async fn tick_lookups(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    lookup::tick_lookups_impl(svc, sock, crypto, cfg).await
}

fn handle_lookup_response(
    svc: &mut KadService,
    now: Instant,
    target: KadId,
    from_dest: &str,
    contacts: &[KadId],
    inserted: u64,
) {
    lookup::handle_lookup_response_impl(svc, now, target, from_dest, contacts, inserted);
}

fn enforce_keyword_store_limits(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    keyword::enforce_keyword_store_limits_impl(svc, cfg, now);
}

fn source_store_totals(svc: &KadService) -> (usize, usize) {
    source_probe::source_store_totals_impl(svc)
}

fn mark_source_publish_sent(svc: &mut KadService, file: KadId, now: Instant) {
    source_probe::mark_source_publish_sent_impl(svc, file, now);
}

fn mark_source_search_sent(svc: &mut KadService, file: KadId, now: Instant) {
    source_probe::mark_source_search_sent_impl(svc, file, now);
}

fn on_source_publish_response(
    svc: &mut KadService,
    file: KadId,
    from_dest_b64: &str,
    now: Instant,
) {
    source_probe::on_source_publish_response_impl(svc, file, from_dest_b64, now);
}

fn on_source_search_response(
    svc: &mut KadService,
    file: KadId,
    from_dest_b64: &str,
    returned_sources: u64,
    now: Instant,
) {
    source_probe::on_source_search_response_impl(svc, file, from_dest_b64, returned_sources, now);
}

fn publish_status(
    svc: &mut KadService,
    started: Instant,
    status_tx: &Option<watch::Sender<Option<KadServiceStatus>>>,
    status_events_tx: &Option<broadcast::Sender<KadServiceStatus>>,
) {
    status::publish_status_impl(svc, started, status_tx, status_events_tx);
}

fn hex_head(b: &[u8], max: usize) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (i, v) in b.iter().take(max).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(&mut out, "{v:02x}");
    }
    out
}

async fn handle_inbound(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    from_dest_b64: String,
    payload: Vec<u8>,
    crypto: KadServiceCrypto,
    cfg: &KadServiceConfig,
) -> Result<()> {
    inbound::handle_inbound_impl(svc, sock, from_dest_b64, payload, crypto, cfg).await
}
