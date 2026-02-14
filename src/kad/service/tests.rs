use super::*;
use crate::nodes::imule::ImuleNode;
use tokio::sync::mpsc;

fn make_node(id_byte: u8, kad_version: u8) -> ImuleNode {
    let mut client_id = [0u8; 16];
    client_id[15] = id_byte;
    let mut udp_dest = [0u8; 387];
    udp_dest[0] = id_byte;
    ImuleNode {
        kad_version,
        client_id,
        udp_dest,
        udp_key: 0,
        udp_key_ip: 0,
        verified: true,
    }
}

#[test]
fn closest_peers_with_fallback_keeps_distance_order() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();

    let nodes = vec![
        make_node(1, 3),
        make_node(2, 3),
        make_node(3, 3),
        make_node(4, 3),
    ];
    for n in nodes {
        let _ = svc.routing_mut().upsert(n, now);
    }

    let target = KadId([0u8; 16]);
    let peers = closest_peers_with_fallback(&svc, target, 2, 3, 0);
    let ids: Vec<u8> = peers.iter().map(|p| p.client_id[15]).collect();

    assert_eq!(ids, vec![1, 2, 3, 4]);
}

#[test]
fn closest_peers_with_fallback_filters_kad_version() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();

    let nodes = vec![make_node(1, 2), make_node(2, 3), make_node(3, 4)];
    for n in nodes {
        let _ = svc.routing_mut().upsert(n, now);
    }

    let target = KadId([0u8; 16]);
    let peers = closest_peers_with_fallback(&svc, target, 2, 3, 0);
    let ids: Vec<u8> = peers.iter().map(|p| p.client_id[15]).collect();

    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn stop_keyword_search_disables_active_job() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();
    let keyword = KadId([1u8; 16]);

    svc.keyword_jobs.insert(
        keyword,
        KeywordJob {
            created_at: now,
            next_lookup_at: now,
            next_search_at: now,
            next_publish_at: now,
            sent_to_search: HashSet::new(),
            sent_to_publish: HashSet::new(),
            want_search: true,
            publish: Some(KeywordPublishSpec {
                file: KadId([2u8; 16]),
                filename: "f.bin".to_string(),
                file_size: 1,
                file_type: None,
            }),
            got_publish_ack: false,
        },
    );

    assert!(stop_keyword_search(&mut svc, keyword));
    let job = svc.keyword_jobs.get(&keyword).expect("job must exist");
    assert!(!job.want_search);
    assert!(job.publish.is_none());
    assert!(job.got_publish_ack);
}

#[test]
fn delete_keyword_search_purges_cached_results() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();
    let keyword = KadId([3u8; 16]);
    let file = KadId([4u8; 16]);

    svc.keyword_jobs.insert(
        keyword,
        KeywordJob {
            created_at: now,
            next_lookup_at: now,
            next_search_at: now,
            next_publish_at: now,
            sent_to_search: HashSet::new(),
            sent_to_publish: HashSet::new(),
            want_search: true,
            publish: None,
            got_publish_ack: false,
        },
    );

    let mut hits = BTreeMap::new();
    hits.insert(
        file,
        KeywordHitState {
            hit: KadKeywordHit {
                file_id: file,
                filename: "x.bin".to_string(),
                file_size: 1,
                file_type: None,
                publish_info: None,
                origin: KadKeywordHitOrigin::Local,
            },
            last_seen: now,
        },
    );
    svc.keyword_hits_by_keyword.insert(keyword, hits);
    svc.keyword_hits_total = 1;
    svc.keyword_interest.insert(keyword, now);

    let mut store_hits = BTreeMap::new();
    store_hits.insert(
        file,
        KeywordHitState {
            hit: KadKeywordHit {
                file_id: file,
                filename: "x.bin".to_string(),
                file_size: 1,
                file_type: None,
                publish_info: None,
                origin: KadKeywordHitOrigin::Network,
            },
            last_seen: now,
        },
    );
    svc.keyword_store_by_keyword.insert(keyword, store_hits);
    svc.keyword_store_total = 1;

    assert!(delete_keyword_search(&mut svc, keyword, true));
    assert!(!svc.keyword_jobs.contains_key(&keyword));
    assert!(!svc.keyword_hits_by_keyword.contains_key(&keyword));
    assert!(!svc.keyword_interest.contains_key(&keyword));
    assert!(!svc.keyword_store_by_keyword.contains_key(&keyword));
    assert_eq!(svc.keyword_hits_total, 0);
    assert_eq!(svc.keyword_store_total, 0);
}

#[test]
fn tracked_out_request_requires_matching_response() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();
    let dest = "peer-a".to_string();

    track_outgoing_request(&mut svc, &dest, KADEMLIA2_SEARCH_KEY_REQ, now);
    assert!(consume_tracked_out_request(
        &mut svc,
        &dest,
        KADEMLIA2_SEARCH_RES,
        0,
        now
    ));
    assert!(!consume_tracked_out_request(
        &mut svc,
        &dest,
        KADEMLIA2_SEARCH_RES,
        0,
        now
    ));

    track_outgoing_request(&mut svc, &dest, KADEMLIA2_PUBLISH_SOURCE_REQ, now);
    assert!(!consume_tracked_out_request(
        &mut svc,
        &dest,
        KADEMLIA2_HELLO_RES,
        0,
        now
    ));
}

#[test]
fn inbound_request_limit_drops_flood_after_threshold() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let now = Instant::now();
    let from_hash = 42u32;

    for _ in 0..30 {
        assert!(inbound_request_allowed(
            &mut svc,
            from_hash,
            KADEMLIA2_REQ,
            now
        ));
    }
    assert!(!inbound_request_allowed(
        &mut svc,
        from_hash,
        KADEMLIA2_REQ,
        now
    ));
    assert!(inbound_request_allowed(
        &mut svc,
        from_hash,
        KADEMLIA2_REQ,
        now + Duration::from_secs(61)
    ));
}

#[test]
fn build_status_reports_source_store_totals() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let started = Instant::now();

    let file_a = KadId([1u8; 16]);
    let file_b = KadId([2u8; 16]);
    let source_a = KadId([3u8; 16]);
    let source_b = KadId([4u8; 16]);
    let source_c = KadId([5u8; 16]);

    svc.sources_by_file
        .entry(file_a)
        .or_default()
        .insert(source_a, [11u8; I2P_DEST_LEN]);
    svc.sources_by_file
        .entry(file_a)
        .or_default()
        .insert(source_b, [12u8; I2P_DEST_LEN]);
    svc.sources_by_file
        .entry(file_b)
        .or_default()
        .insert(source_c, [13u8; I2P_DEST_LEN]);

    let st = build_status(&mut svc, started);
    assert_eq!(st.source_store_files, 2);
    assert_eq!(st.source_store_entries_total, 3);
}

#[test]
fn source_probe_tracks_first_send_response_latency_and_results() {
    let (_tx, rx) = mpsc::channel(1);
    let mut svc = KadService::new(KadId([0u8; 16]), rx);
    let file = KadId([9u8; 16]);
    let t0 = Instant::now();
    let t1 = t0 + Duration::from_millis(25);
    let t2 = t0 + Duration::from_millis(40);
    let t3 = t0 + Duration::from_millis(75);

    mark_source_publish_sent(&mut svc, file, t0);
    mark_source_search_sent(&mut svc, file, t1);
    on_source_publish_response(&mut svc, file, "peer-a", t2);
    on_source_search_response(&mut svc, file, "peer-a", 3, t3);

    let st = svc
        .source_probe_by_file
        .get(&file)
        .expect("probe state exists");
    assert_eq!(st.search_result_events, 1);
    assert_eq!(st.search_results_total, 3);
    assert_eq!(st.last_search_results, 3);
    assert!(st.first_publish_sent_at.is_some());
    assert!(st.first_search_sent_at.is_some());
    assert!(st.first_publish_res_at.is_some());
    assert!(st.first_search_res_at.is_some());

    let status = build_status(&mut svc, t0);
    assert_eq!(status.source_probe_first_publish_responses, 1);
    assert_eq!(status.source_probe_first_search_responses, 1);
    assert_eq!(status.source_probe_search_results_total, 3);
    assert_eq!(status.source_probe_publish_latency_ms_total, 40);
    assert_eq!(status.source_probe_search_latency_ms_total, 50);
}
