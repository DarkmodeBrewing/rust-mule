use crate::{
    i2p::sam::SamKadSocket,
    kad::{
        KadId,
        routing::RoutingTable,
        udp_crypto,
        wire::{
            I2P_DEST_LEN, KADEMLIA_HELLO_REQ_DEPRECATED, KADEMLIA_HELLO_RES_DEPRECATED,
            KADEMLIA_REQ_DEPRECATED, KADEMLIA_RES_DEPRECATED, KADEMLIA2_BOOTSTRAP_REQ,
            KADEMLIA2_BOOTSTRAP_RES, KADEMLIA2_HELLO_REQ, KADEMLIA2_HELLO_RES,
            KADEMLIA2_HELLO_RES_ACK, KADEMLIA2_PING, KADEMLIA2_PONG, KADEMLIA2_PUBLISH_RES,
            KADEMLIA2_PUBLISH_KEY_REQ, KADEMLIA2_PUBLISH_SOURCE_REQ, KADEMLIA2_REQ, KADEMLIA2_RES,
            KADEMLIA2_SEARCH_KEY_REQ, KADEMLIA2_SEARCH_RES, KADEMLIA2_SEARCH_SOURCE_REQ,
            Kad2PublishKeyReq, Kad2PublishRes, Kad2PublishResKey, Kad2SearchRes,
            KadPacket, TAG_KADMISCOPTIONS, decode_kad1_req, decode_kad2_bootstrap_res,
            decode_kad2_hello, decode_kad2_publish_key_req, decode_kad2_publish_res,
            decode_kad2_publish_res_key, decode_kad2_publish_source_req_min, decode_kad2_req,
            decode_kad2_res, decode_kad2_search_key_req, decode_kad2_search_res,
            decode_kad2_search_source_req, encode_kad1_res, encode_kad2_hello,
            encode_kad2_publish_key_req, encode_kad2_publish_res_for_key,
            encode_kad2_publish_res_for_source, encode_kad2_publish_source_req, encode_kad2_req,
            encode_kad2_res, encode_kad2_search_key_req, encode_kad2_search_res_keyword,
            encode_kad2_search_res_sources, encode_kad2_search_source_req,
        },
    },
    nodes::imule::ImuleNode,
};
use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::{Duration, Instant, MissedTickBehavior, interval};

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

    pub maintenance_every_secs: u64,
    pub status_every_secs: u64,
    pub max_failures: u32,
    pub evict_age_secs: u64,

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

const KEYWORD_JOB_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const KEYWORD_JOB_LOOKUP_EVERY: Duration = Duration::from_secs(60);
const KEYWORD_JOB_ACTION_EVERY: Duration = Duration::from_secs(60);
const KEYWORD_JOB_ACTION_BATCH: usize = 3;

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

            maintenance_every_secs: 5,
            status_every_secs: 60,
            max_failures: 5,
            // I2P peers can be very intermittent; don't aggressively evict by default.
            evict_age_secs: 24 * 60 * 60,

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

#[derive(Debug, Default, Clone, Copy)]
struct KadServiceStats {
    sent_reqs: u64,
    recv_ress: u64,
    res_contacts: u64,
    dropped_undecipherable: u64,
    dropped_unparsable: u64,
    sent_bootstrap_reqs: u64,
    recv_bootstrap_ress: u64,
    bootstrap_contacts: u64,
    sent_hellos: u64,
    recv_hello_ress: u64,
    timeouts: u64,
    new_nodes: u64,
    evicted: u64,

    sent_search_source_reqs: u64,
    recv_search_ress: u64,
    search_results: u64,
    new_sources: u64,

    sent_search_key_reqs: u64,
    keyword_results: u64,
    new_keyword_results: u64,
    evicted_keyword_hits: u64,
    evicted_keyword_keywords: u64,

    recv_publish_key_reqs: u64,
    sent_publish_key_ress: u64,
    sent_publish_key_reqs: u64,
    recv_publish_key_ress: u64,
    new_store_keyword_hits: u64,
    evicted_store_keyword_hits: u64,
    evicted_store_keyword_keywords: u64,

    sent_publish_source_reqs: u64,
    recv_publish_ress: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct KadServiceStatus {
    pub uptime_secs: u64,
    pub routing: usize,
    pub live: usize,
    pub live_10m: usize,
    pub pending: usize,

    pub sent_reqs: u64,
    pub recv_ress: u64,
    pub res_contacts: u64,
    pub dropped_undecipherable: u64,
    pub dropped_unparsable: u64,
    pub sent_bootstrap_reqs: u64,
    pub recv_bootstrap_ress: u64,
    pub bootstrap_contacts: u64,
    pub sent_hellos: u64,
    pub recv_hello_ress: u64,
    pub timeouts: u64,
    pub new_nodes: u64,
    pub evicted: u64,

    pub sent_search_source_reqs: u64,
    pub recv_search_ress: u64,
    pub search_results: u64,
    pub new_sources: u64,

    pub sent_search_key_reqs: u64,
    pub keyword_results: u64,
    pub new_keyword_results: u64,
    pub evicted_keyword_hits: u64,
    pub evicted_keyword_keywords: u64,
    pub keyword_keywords_tracked: usize,
    pub keyword_hits_total: usize,

    pub store_keyword_keywords: usize,
    pub store_keyword_hits_total: usize,

    pub recv_publish_key_reqs: u64,
    pub sent_publish_key_ress: u64,
    pub sent_publish_key_reqs: u64,
    pub recv_publish_key_ress: u64,
    pub new_store_keyword_hits: u64,
    pub evicted_store_keyword_hits: u64,
    pub evicted_store_keyword_keywords: u64,

    pub sent_publish_source_reqs: u64,
    pub recv_publish_ress: u64,
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
}

pub struct KadService {
    routing: RoutingTable,
    // Minimal (in-memory) source index: file ID -> (source ID -> UDP dest).
    sources_by_file: BTreeMap<KadId, BTreeMap<KadId, [u8; I2P_DEST_LEN]>>,
    // Minimal (in-memory) keyword index: keyword hash -> (file ID -> hit state).
    keyword_hits_by_keyword: BTreeMap<KadId, BTreeMap<KadId, KeywordHitState>>,
    keyword_hits_total: usize,
    keyword_interest: HashMap<KadId, Instant>,

    keyword_store_by_keyword: BTreeMap<KadId, BTreeMap<KadId, KeywordHitState>>,
    keyword_store_total: usize,

    keyword_jobs: HashMap<KadId, KeywordJob>,

    pending_reqs: HashMap<String, Instant>,
    crawl_round: u64,
    stats_window: KadServiceStats,

    cmd_rx: mpsc::Receiver<KadServiceCommand>,
}

#[derive(Debug, Clone)]
struct KeywordHitState {
    hit: KadKeywordHit,
    last_seen: Instant,
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

impl KadService {
    pub fn new(my_id: KadId, cmd_rx: mpsc::Receiver<KadServiceCommand>) -> Self {
        Self {
            routing: RoutingTable::new(my_id),
            sources_by_file: BTreeMap::new(),
            keyword_hits_by_keyword: BTreeMap::new(),
            keyword_hits_total: 0,
            keyword_interest: HashMap::new(),
            keyword_store_by_keyword: BTreeMap::new(),
            keyword_store_total: 0,
            keyword_jobs: HashMap::new(),
            pending_reqs: HashMap::new(),
            crawl_round: 0,
            stats_window: KadServiceStats::default(),
            cmd_rx,
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
            send_search_sources(svc, sock, crypto, file, file_size).await?;
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
                },
            );

            start_keyword_job_publish(
                svc,
                sock,
                crypto,
                cfg,
                keyword,
                file,
                filename,
                file_size,
                file_type,
                now,
            )
            .await?;
        }
        KadServiceCommand::PublishSource { file, file_size } => {
            send_publish_source(svc, sock, crypto, file, file_size).await?;
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
    }
    Ok(())
}

async fn send_search_sources(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    file: KadId,
    file_size: u64,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    // Ask a few peers, but prefer ones we've actually heard from recently.
    // (Search requests are user-initiated, so it is OK to be a bit less conservative than
    // the background crawl loop.)
    let now = Instant::now();
    let peers = svc.routing.closest_to_prefer_live(
        file,
        8,
        0,
        now,
        Duration::from_secs(10 * 60),
        3,
    );
    for p in peers {
        // iMule uses Kad2 search source only for version >= 3.
        if p.kad_version < 3 {
            continue;
        }
        let payload = encode_kad2_search_source_req(file, 0, file_size);
        if let Err(err) =
            send_kad2_packet(sock, &p, crypto, KADEMLIA2_SEARCH_SOURCE_REQ, &payload).await
        {
            tracing::debug!(
                error = %err,
                to = %p.udp_dest_b64(),
                "failed sending SEARCH_SOURCE_REQ"
            );
            continue;
        }
        svc.stats_window.sent_search_source_reqs += 1;
    }

    tracing::info!(file = %file.to_hex_lower(), "sent SEARCH_SOURCE_REQ");
    Ok(())
}

async fn send_publish_source(
    svc: &mut KadService,
    sock: &mut SamKadSocket,
    crypto: KadServiceCrypto,
    file: KadId,
    file_size: u64,
) -> Result<()> {
    if svc.routing.is_empty() {
        return Ok(());
    }

    // Publish to a few peers, prefer recently-live.
    let now = Instant::now();
    let peers = svc.routing.closest_to_prefer_live(
        file,
        6,
        0,
        now,
        Duration::from_secs(10 * 60),
        4,
    );
    for p in peers {
        // iMule uses Kad2 publish source only for version >= 4.
        if p.kad_version < 4 {
            continue;
        }
        let payload = encode_kad2_publish_source_req(
            file,
            crypto.my_kad_id,
            &crypto.my_dest,
            Some(file_size),
        );
        if let Err(err) =
            send_kad2_packet(sock, &p, crypto, KADEMLIA2_PUBLISH_SOURCE_REQ, &payload).await
        {
            tracing::debug!(
                error = %err,
                to = %p.udp_dest_b64(),
                "failed sending PUBLISH_SOURCE_REQ"
            );
            continue;
        }
        svc.stats_window.sent_publish_source_reqs += 1;
    }

    tracing::info!(file = %file.to_hex_lower(), "sent PUBLISH_SOURCE_REQ");
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
    let job = svc.keyword_jobs.entry(keyword).or_insert_with(|| KeywordJob {
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

    // Kick off immediately (lookup + first action batch).
    progress_keyword_job(svc, sock, crypto, cfg, keyword, now).await?;
    Ok(())
}

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
    let job = svc.keyword_jobs.entry(keyword).or_insert_with(|| KeywordJob {
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

    let peers = svc.routing.closest_to_prefer_live(
        keyword,
        32,
        0,
        now,
        Duration::from_secs(30 * 60),
        3,
    );

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

            let payload = encode_kad2_search_key_req(keyword, 0);
            if send_kad2_packet(sock, p, crypto, KADEMLIA2_SEARCH_KEY_REQ, &payload)
                .await
                .is_ok()
            {
                svc.stats_window.sent_search_key_reqs += 1;
                job.sent_to_search.insert(dest.clone());
                dests.push(crate::i2p::b64::short(&dest).to_string());
                sent += 1;
            }
        }

        if sent > 0 {
            tracing::info!(
                keyword = %keyword.to_hex_lower(),
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

            let entries = [(
                pubspec.file,
                pubspec.filename.as_str(),
                pubspec.file_size,
                pubspec.file_type.as_deref(),
            )];
            let payload = encode_kad2_publish_key_req(keyword, &entries);
            if send_kad2_packet(sock, p, crypto, KADEMLIA2_PUBLISH_KEY_REQ, &payload)
                .await
                .is_ok()
            {
                svc.stats_window.sent_publish_key_reqs += 1;
                job.sent_to_publish.insert(dest.clone());
                dests.push(crate::i2p::b64::short(&dest).to_string());
                sent += 1;
            }
        }

        if sent > 0 {
            tracing::info!(
                keyword = %keyword.to_hex_lower(),
                file = %pubspec.file.to_hex_lower(),
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

async fn send_kad2_packet(
    sock: &mut SamKadSocket,
    node: &ImuleNode,
    crypto: KadServiceCrypto,
    opcode: u8,
    payload: &[u8],
) -> Result<()> {
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

    sock.send_to(&dest, &out).await?;
    Ok(())
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

    if !have_cold {
        if let Some(cold) = candidates.iter().find(|c| {
            let id = KadId(c.client_id);
            svc.routing
                .get_by_id(id)
                .and_then(|st| st.last_inbound)
                .is_none()
        }) {
            if !peers.is_empty() {
                peers.pop();
            }
            peers.push(cold.clone());
        }
    }

    let requested_contacts = cfg.req_contacts.clamp(1, 31);

    for p in peers {
        if let Err(err) =
            send_kad2_req(svc, sock, crypto, cfg, requested_contacts, target, &p).await
        {
            tracing::debug!(error = %err, to = %p.udp_dest_b64(), "failed sending KAD2 REQ (crawl)");
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
    send_kad2_packet(sock, peer, crypto, KADEMLIA2_REQ, &req_payload).await?;

    svc.stats_window.sent_reqs += 1;
    svc.pending_reqs.insert(
        dest.clone(),
        Instant::now() + Duration::from_secs(cfg.req_timeout_secs.max(5)),
    );
    svc.routing.mark_queried_by_dest(&dest, Instant::now());
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

        let out = udp_crypto::encrypt_kad_packet(
            &plain,
            target_kad_id,
            receiver_verify_key,
            sender_verify_key,
        )?;

        if let Err(err) = sock.send_to(&dest, &out).await {
            tracing::debug!(error = %err, to = %dest, "failed sending KAD2 BOOTSTRAP_REQ (service)");
        } else {
            svc.stats_window.sent_bootstrap_reqs += 1;
            tracing::info!(
                to = %crate::i2p::b64::short(&dest),
                "sent periodic KAD2 BOOTSTRAP_REQ (refresh)"
            );
            svc.routing.mark_bootstrap_sent_by_dest(&dest, now);
        }
    }

    Ok(())
}

fn touch_keyword_interest(svc: &mut KadService, cfg: &KadServiceConfig, keyword: KadId, now: Instant) {
    svc.keyword_interest.insert(keyword, now);
    enforce_keyword_interest_limit(svc, cfg);
}

fn enforce_keyword_interest_limit(svc: &mut KadService, cfg: &KadServiceConfig) {
    let max_keywords = cfg.keyword_max_keywords;
    if max_keywords == 0 {
        // Treat 0 as "disable caching": drop everything.
        let removed = drop_all_keywords(svc);
        if removed.keywords > 0 || removed.hits > 0 {
            svc.stats_window.evicted_keyword_keywords += removed.keywords as u64;
            svc.stats_window.evicted_keyword_hits += removed.hits as u64;
        }
        return;
    }

    if svc.keyword_interest.len() <= max_keywords {
        return;
    }

    let mut v = svc
        .keyword_interest
        .iter()
        .map(|(k, t)| (*k, *t))
        .collect::<Vec<_>>();
    v.sort_by_key(|(_, t)| *t);

    let mut idx = 0usize;
    while svc.keyword_interest.len() > max_keywords && idx < v.len() {
        let k = v[idx].0;
        idx += 1;
        svc.keyword_interest.remove(&k);
        let removed_hits = drop_keyword_hits_only(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }
}

fn enforce_keyword_size_limits(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    enforce_keyword_per_keyword_caps_all(svc, cfg);
    enforce_keyword_total_cap(svc, cfg, now);
}

fn upsert_keyword_hit_cache(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
    keyword: KadId,
    hit: KadKeywordHit,
) {
    if cfg.keyword_require_interest && !svc.keyword_interest.contains_key(&keyword) {
        return;
    }

    let m = svc.keyword_hits_by_keyword.entry(keyword).or_default();
    match m.get_mut(&hit.file_id) {
        Some(st) => {
            st.hit = hit;
            st.last_seen = now;
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
        }
    }

    enforce_keyword_size_limits(svc, cfg, now);
}

fn enforce_keyword_per_keyword_caps_all(svc: &mut KadService, cfg: &KadServiceConfig) {
    let per = cfg.keyword_max_hits_per_keyword;
    if per == 0 {
        return;
    }
    let keys = svc.keyword_hits_by_keyword.keys().copied().collect::<Vec<_>>();
    for k in keys {
        prune_keyword_hits_per_keyword(svc, k, per);
    }
}

fn enforce_keyword_per_keyword_cap(svc: &mut KadService, cfg: &KadServiceConfig, keyword: KadId) {
    let per = cfg.keyword_max_hits_per_keyword;
    if per == 0 {
        return;
    }
    prune_keyword_hits_per_keyword(svc, keyword, per);
}

fn enforce_keyword_total_cap(svc: &mut KadService, cfg: &KadServiceConfig, _now: Instant) {
    // Total cap. Evict oldest keywords by interest time first.
    let max_total = cfg.keyword_max_total_hits;
    if max_total == 0 || svc.keyword_hits_total <= max_total {
        return;
    }

    let mut v = svc
        .keyword_interest
        .iter()
        .map(|(k, t)| (*k, *t))
        .collect::<Vec<_>>();
    v.sort_by_key(|(_, t)| *t);

    for (k, _) in v {
        if svc.keyword_hits_total <= max_total {
            break;
        }
        svc.keyword_interest.remove(&k);
        let removed_hits = drop_keyword_hits_only(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }
}

fn prune_keyword_hits_per_keyword(svc: &mut KadService, keyword: KadId, max: usize) {
    let Some(m) = svc.keyword_hits_by_keyword.get_mut(&keyword) else {
        return;
    };
    if m.len() <= max {
        return;
    }

    let mut entries = m
        .iter()
        .map(|(file_id, st)| (*file_id, st.last_seen))
        .collect::<Vec<_>>();
    entries.sort_by_key(|(_, t)| *t);

    let to_remove = m.len().saturating_sub(max);
    for (file_id, _) in entries.into_iter().take(to_remove) {
        if m.remove(&file_id).is_some() {
            svc.keyword_hits_total = svc.keyword_hits_total.saturating_sub(1);
            svc.stats_window.evicted_keyword_hits += 1;
        }
    }

    if m.is_empty() {
        svc.keyword_hits_by_keyword.remove(&keyword);
    }
}

fn drop_keyword_hits_only(svc: &mut KadService, keyword: KadId) -> usize {
    if let Some(m) = svc.keyword_hits_by_keyword.remove(&keyword) {
        let n = m.len();
        svc.keyword_hits_total = svc.keyword_hits_total.saturating_sub(n);
        return n;
    }
    0
}

#[derive(Debug, Default, Clone, Copy)]
struct KeywordDropCount {
    keywords: usize,
    hits: usize,
}

fn drop_all_keywords(svc: &mut KadService) -> KeywordDropCount {
    let keywords = svc.keyword_hits_by_keyword.len();
    let hits = svc.keyword_hits_total;
    svc.keyword_hits_by_keyword.clear();
    svc.keyword_hits_total = 0;
    svc.keyword_interest.clear();
    KeywordDropCount { keywords, hits }
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
        tracing::info!(
            evicted,
            remaining = svc.routing.len(),
            "evicted stale peers"
        );
    }

    maintain_keyword_cache(svc, cfg, now);
    maintain_keyword_store(svc, cfg, now);
}

fn maintain_keyword_cache(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    // Expire keyword interest.
    let interest_ttl = Duration::from_secs(cfg.keyword_interest_ttl_secs.max(60));
    let mut expired_keywords = Vec::<KadId>::new();
    for (k, t) in &svc.keyword_interest {
        if now.saturating_duration_since(*t) >= interest_ttl {
            expired_keywords.push(*k);
        }
    }
    for k in expired_keywords {
        svc.keyword_interest.remove(&k);
        let removed_hits = drop_keyword_hits_only(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }

    // Expire stale hits (per-hit TTL).
    let results_ttl = Duration::from_secs(cfg.keyword_results_ttl_secs.max(60));
    let keys = svc.keyword_hits_by_keyword.keys().copied().collect::<Vec<_>>();
    for k in keys {
        // If we're requiring interest and we've dropped interest, drop any remaining hits.
        if cfg.keyword_require_interest && !svc.keyword_interest.contains_key(&k) {
            let removed_hits = drop_keyword_hits_only(svc, k);
            svc.stats_window.evicted_keyword_keywords += 1;
            svc.stats_window.evicted_keyword_hits += removed_hits as u64;
            continue;
        }

        let Some(m) = svc.keyword_hits_by_keyword.get_mut(&k) else {
            continue;
        };
        let mut to_remove = Vec::<KadId>::new();
        for (file_id, st) in m.iter() {
            if now.saturating_duration_since(st.last_seen) >= results_ttl {
                to_remove.push(*file_id);
            }
        }
        if !to_remove.is_empty() {
            for file_id in to_remove {
                if m.remove(&file_id).is_some() {
                    svc.keyword_hits_total = svc.keyword_hits_total.saturating_sub(1);
                    svc.stats_window.evicted_keyword_hits += 1;
                }
            }
        }
        if m.is_empty() {
            svc.keyword_hits_by_keyword.remove(&k);
        }
    }

    // Finally, enforce size caps.
    enforce_keyword_interest_limit(svc, cfg);
    enforce_keyword_size_limits(svc, cfg, now);
}

fn maintain_keyword_store(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    // TTL pruning: drop keyword->file entries we haven't seen for a while.
    let ttl = Duration::from_secs(cfg.store_keyword_evict_age_secs.max(60));
    let keys = svc
        .keyword_store_by_keyword
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for k in keys {
        let Some(m) = svc.keyword_store_by_keyword.get_mut(&k) else {
            continue;
        };
        let mut to_remove = Vec::<KadId>::new();
        for (file_id, st) in m.iter() {
            if now.saturating_duration_since(st.last_seen) >= ttl {
                to_remove.push(*file_id);
            }
        }
        if !to_remove.is_empty() {
            for file_id in to_remove {
                if m.remove(&file_id).is_some() {
                    svc.keyword_store_total = svc.keyword_store_total.saturating_sub(1);
                    svc.stats_window.evicted_store_keyword_hits += 1;
                }
            }
        }
        if m.is_empty() {
            svc.keyword_store_by_keyword.remove(&k);
            svc.stats_window.evicted_store_keyword_keywords += 1;
        }
    }

    enforce_keyword_store_limits(svc, cfg, now);
}

fn enforce_keyword_store_limits(svc: &mut KadService, cfg: &KadServiceConfig, now: Instant) {
    let max_keywords = cfg.store_keyword_max_keywords;
    let max_total = cfg.store_keyword_max_total_hits;

    if max_keywords == 0 || max_total == 0 {
        // Treat any 0 as "disable store".
        if !svc.keyword_store_by_keyword.is_empty() {
            svc.stats_window.evicted_store_keyword_keywords += svc.keyword_store_by_keyword.len() as u64;
            svc.stats_window.evicted_store_keyword_hits += svc.keyword_store_total as u64;
        }
        svc.keyword_store_by_keyword.clear();
        svc.keyword_store_total = 0;
        return;
    }

    // If we exceed max keywords, evict oldest keyword buckets by "last seen" of any entry.
    if svc.keyword_store_by_keyword.len() > max_keywords {
        let mut v = svc
            .keyword_store_by_keyword
            .iter()
            .map(|(k, m)| {
                let last = m.values().map(|st| st.last_seen).max().unwrap_or(now);
                (*k, last)
            })
            .collect::<Vec<_>>();
        v.sort_by_key(|(_, t)| *t);

        for (k, _) in v {
            if svc.keyword_store_by_keyword.len() <= max_keywords {
                break;
            }
            if let Some(m) = svc.keyword_store_by_keyword.remove(&k) {
                svc.keyword_store_total = svc.keyword_store_total.saturating_sub(m.len());
                svc.stats_window.evicted_store_keyword_keywords += 1;
                svc.stats_window.evicted_store_keyword_hits += m.len() as u64;
            }
        }
    }

    // If we exceed max total hits, evict oldest keyword buckets until under cap.
    if svc.keyword_store_total > max_total {
        let mut v = svc
            .keyword_store_by_keyword
            .iter()
            .map(|(k, m)| {
                let last = m.values().map(|st| st.last_seen).max().unwrap_or(now);
                (*k, last)
            })
            .collect::<Vec<_>>();
        v.sort_by_key(|(_, t)| *t);

        for (k, _) in v {
            if svc.keyword_store_total <= max_total {
                break;
            }
            if let Some(m) = svc.keyword_store_by_keyword.remove(&k) {
                svc.keyword_store_total = svc.keyword_store_total.saturating_sub(m.len());
                svc.stats_window.evicted_store_keyword_keywords += 1;
                svc.stats_window.evicted_store_keyword_hits += m.len() as u64;
            }
        }
    }
}

fn build_status(svc: &mut KadService, started: Instant) -> KadServiceStatus {
    let routing = svc.routing.len();
    let now = Instant::now();
    let live = svc.routing.live_count();
    let live_10m = svc
        .routing
        .live_count_recent(now, Duration::from_secs(10 * 60));
    let pending = svc.pending_reqs.len();
    let keyword_keywords_tracked = svc.keyword_hits_by_keyword.len();
    let keyword_hits_total = svc.keyword_hits_total;
    let store_keyword_keywords = svc.keyword_store_by_keyword.len();
    let store_keyword_hits_total = svc.keyword_store_total;
    let w = svc.stats_window;
    svc.stats_window = KadServiceStats::default();

    KadServiceStatus {
        uptime_secs: started.elapsed().as_secs(),
        routing,
        live,
        live_10m,
        pending,
        sent_reqs: w.sent_reqs,
        recv_ress: w.recv_ress,
        res_contacts: w.res_contacts,
        dropped_undecipherable: w.dropped_undecipherable,
        dropped_unparsable: w.dropped_unparsable,
        sent_bootstrap_reqs: w.sent_bootstrap_reqs,
        recv_bootstrap_ress: w.recv_bootstrap_ress,
        bootstrap_contacts: w.bootstrap_contacts,
        sent_hellos: w.sent_hellos,
        recv_hello_ress: w.recv_hello_ress,
        timeouts: w.timeouts,
        new_nodes: w.new_nodes,
        evicted: w.evicted,

        sent_search_source_reqs: w.sent_search_source_reqs,
        recv_search_ress: w.recv_search_ress,
        search_results: w.search_results,
        new_sources: w.new_sources,

        sent_search_key_reqs: w.sent_search_key_reqs,
        keyword_results: w.keyword_results,
        new_keyword_results: w.new_keyword_results,
        evicted_keyword_hits: w.evicted_keyword_hits,
        evicted_keyword_keywords: w.evicted_keyword_keywords,
        keyword_keywords_tracked,
        keyword_hits_total,

        store_keyword_keywords,
        store_keyword_hits_total,

        recv_publish_key_reqs: w.recv_publish_key_reqs,
        sent_publish_key_ress: w.sent_publish_key_ress,
        sent_publish_key_reqs: w.sent_publish_key_reqs,
        recv_publish_key_ress: w.recv_publish_key_ress,
        new_store_keyword_hits: w.new_store_keyword_hits,
        evicted_store_keyword_hits: w.evicted_store_keyword_hits,
        evicted_store_keyword_keywords: w.evicted_store_keyword_keywords,

        sent_publish_source_reqs: w.sent_publish_source_reqs,
        recv_publish_ress: w.recv_publish_ress,
    }
}

fn publish_status(
    svc: &mut KadService,
    started: Instant,
    status_tx: &Option<watch::Sender<Option<KadServiceStatus>>>,
    status_events_tx: &Option<broadcast::Sender<KadServiceStatus>>,
) {
    let st = build_status(svc, started);
    tracing::info!(
        uptime_secs = st.uptime_secs,
        routing = st.routing,
        live = st.live,
        live_10m = st.live_10m,
        pending = st.pending,
        sent_reqs = st.sent_reqs,
        recv_ress = st.recv_ress,
        res_contacts = st.res_contacts,
        sent_bootstrap_reqs = st.sent_bootstrap_reqs,
        recv_bootstrap_ress = st.recv_bootstrap_ress,
        bootstrap_contacts = st.bootstrap_contacts,
        sent_hellos = st.sent_hellos,
        recv_hello_ress = st.recv_hello_ress,
        timeouts = st.timeouts,
        new_nodes = st.new_nodes,
        evicted = st.evicted,
        sent_search_source_reqs = st.sent_search_source_reqs,
        recv_search_ress = st.recv_search_ress,
        search_results = st.search_results,
        new_sources = st.new_sources,
        sent_search_key_reqs = st.sent_search_key_reqs,
        keyword_results = st.keyword_results,
        new_keyword_results = st.new_keyword_results,
        evicted_keyword_hits = st.evicted_keyword_hits,
        evicted_keyword_keywords = st.evicted_keyword_keywords,
        keyword_keywords_tracked = st.keyword_keywords_tracked,
        keyword_hits_total = st.keyword_hits_total,
        store_keyword_keywords = st.store_keyword_keywords,
        store_keyword_hits_total = st.store_keyword_hits_total,
        recv_publish_key_reqs = st.recv_publish_key_reqs,
        sent_publish_key_ress = st.sent_publish_key_ress,
        sent_publish_key_reqs = st.sent_publish_key_reqs,
        recv_publish_key_ress = st.recv_publish_key_ress,
        new_store_keyword_hits = st.new_store_keyword_hits,
        evicted_store_keyword_hits = st.evicted_store_keyword_hits,
        evicted_store_keyword_keywords = st.evicted_store_keyword_keywords,
        sent_publish_source_reqs = st.sent_publish_source_reqs,
        recv_publish_ress = st.recv_publish_ress,
        "kad service status"
    );

    if let Some(tx) = status_tx {
        let _ = tx.send(Some(st.clone()));
    }
    if let Some(tx) = status_events_tx {
        let _ = tx.send(st);
    }
}

async fn handle_inbound(
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
        Some(b) if b.len() >= 4 => u32::from_le_bytes(b[0..4].try_into().unwrap()),
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
            let _ = sock.send_to(&from_dest_b64, &res).await;
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
                udp_crypto::encrypt_kad_packet(
                    &res_plain,
                    hello.node_id,
                    receiver_verify_key,
                    sender_verify_key,
                )?
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
            let target = res.target;
            let contacts = res.contacts;
            tracing::trace!(from = %from_dest_b64, contacts = contacts.len(), "got KAD2 RES");
            svc.pending_reqs.remove(&from_dest_b64);
            svc.stats_window.recv_ress += 1;
            svc.stats_window.res_contacts += contacts.len() as u64;
            let before = svc.routing.len();
            for c in contacts {
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
                    "learned new nodes from KAD2 RES"
                );
            }

            // If this response was part of a user-initiated keyword lookup/publish, nudge the
            // job forward immediately instead of waiting for the next maintenance tick.
            if svc.keyword_jobs.contains_key(&target) {
                progress_keyword_job(svc, sock, crypto, cfg, target, now).await?;
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

        KADEMLIA2_PUBLISH_KEY_REQ => {
            let req: Kad2PublishKeyReq = match decode_kad2_publish_key_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 PUBLISH_KEY_REQ payload");
                    return Ok(());
                }
            };

            svc.stats_window.recv_publish_key_reqs += 1;

            let m = svc.keyword_store_by_keyword.entry(req.keyword).or_default();
            let mut inserted = 0u64;
            for e in req.entries {
                let (Some(filename), Some(file_size)) = (e.filename, e.file_size) else {
                    continue;
                };
                if filename.is_empty() {
                    continue;
                }
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
            let res_payload = encode_kad2_publish_res_for_key(req.keyword, 0);
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
            svc.stats_window.sent_publish_key_ress += 1;
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
                svc.sources_by_file
                    .entry(req.file)
                    .or_default()
                    .insert(req.source, udp_dest);
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

        KADEMLIA2_SEARCH_KEY_REQ => {
            let req = match decode_kad2_search_key_req(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(error = %err, from = %from_dest_b64, "failed to decode KAD2 SEARCH_KEY_REQ payload");
                    return Ok(());
                }
            };

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

        KADEMLIA2_SEARCH_RES => {
            let res: Kad2SearchRes = match decode_kad2_search_res(&pkt.payload) {
                Ok(r) => r,
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        from = %from_dest_b64,
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

            for r in res.results {
                if let Some(dest) = r.tags.best_udp_dest() {
                    // Source-style result: key = file ID, answer = source ID.
                    let m = svc.sources_by_file.entry(res.key).or_default();
                    if m.insert(r.answer, dest).is_none() {
                        inserted_sources += 1;
                    }
                    continue;
                }

                if let (Some(filename), Some(file_size)) = (r.tags.filename.clone(), r.tags.file_size)
                {
                    // Keyword-style result: key = keyword hash, answer = file ID.
                    if cfg.keyword_require_interest && !svc.keyword_interest.contains_key(&res.key) {
                        continue;
                    }
                    keyword_entries += 1;
                    let hit = KadKeywordHit {
                        file_id: r.answer,
                        filename,
                        file_size,
                        file_type: r.tags.file_type.clone(),
                        publish_info: r.tags.publish_info,
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

        KADEMLIA2_PUBLISH_RES => {
            // Publish results can be different shapes (key/source/notes).
            // - Key:   <u128 key><u8 load>
            // - Source: <u128 file><u32 sources><u32 complete><u8 load>
            match pkt.payload.len() {
                17 => {
                    let res: Kad2PublishResKey = match decode_kad2_publish_res_key(&pkt.payload) {
                        Ok(r) => r,
                        Err(err) => {
                            tracing::trace!(
                                error = %err,
                                from = %from_dest_b64,
                                len = pkt.payload.len(),
                                "unparsed KAD2 PUBLISH_RES (key)"
                            );
                            return Ok(());
                        }
                    };
                    svc.stats_window.recv_publish_key_ress += 1;
                    if let Some(job) = svc.keyword_jobs.get_mut(&res.key) {
                        job.got_publish_ack = true;
                    }
                    tracing::debug!(
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        key = %res.key.to_hex_lower(),
                        load = res.load,
                        "got PUBLISH_RES (key)"
                    );
                }
                25.. => {
                    let res: Kad2PublishRes = match decode_kad2_publish_res(&pkt.payload) {
                        Ok(r) => r,
                        Err(err) => {
                            tracing::trace!(
                                error = %err,
                                from = %from_dest_b64,
                                len = pkt.payload.len(),
                                "unparsed KAD2 PUBLISH_RES (source)"
                            );
                            return Ok(());
                        }
                    };
                    svc.stats_window.recv_publish_ress += 1;
                    tracing::debug!(
                        from = %crate::i2p::b64::short(&from_dest_b64),
                        file = %res.file.to_hex_lower(),
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
