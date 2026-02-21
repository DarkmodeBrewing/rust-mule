use super::*;

pub(super) fn build_status_impl(svc: &mut KadService, started: Instant) -> KadServiceStatus {
    let routing = svc.routing.len();
    let now = Instant::now();
    let live = svc.routing.live_count();
    let live_10m = svc
        .routing
        .live_count_recent(now, Duration::from_secs(10 * 60));
    let pending = svc.pending_reqs.len();
    let pending_overdue = svc
        .pending_reqs
        .values()
        .filter(|deadline| **deadline <= now)
        .count();
    let pending_max_overdue_ms = svc
        .pending_reqs
        .values()
        .filter(|deadline| **deadline <= now)
        .map(|deadline| now.saturating_duration_since(*deadline).as_millis() as u64)
        .max()
        .unwrap_or(0);
    let tracked_out_requests = svc.tracked_out_requests.len();
    let keyword_keywords_tracked = svc.keyword_hits_by_keyword.len();
    let keyword_hits_total = svc.keyword_hits_total;
    let store_keyword_keywords = svc.keyword_store_by_keyword.len();
    let store_keyword_hits_total = svc.keyword_store_total;
    let (source_store_files, source_store_entries_total) =
        source_probe::source_store_totals_impl(svc);
    let w = svc.stats_window;
    svc.stats_cumulative.sent_reqs = svc.stats_cumulative.sent_reqs.saturating_add(w.sent_reqs);
    svc.stats_cumulative.recv_ress = svc.stats_cumulative.recv_ress.saturating_add(w.recv_ress);
    svc.stats_cumulative.timeouts = svc.stats_cumulative.timeouts.saturating_add(w.timeouts);
    svc.stats_cumulative.tracked_out_matched = svc
        .stats_cumulative
        .tracked_out_matched
        .saturating_add(w.tracked_out_matched);
    svc.stats_cumulative.tracked_out_unmatched = svc
        .stats_cumulative
        .tracked_out_unmatched
        .saturating_add(w.tracked_out_unmatched);
    svc.stats_cumulative.tracked_out_expired = svc
        .stats_cumulative
        .tracked_out_expired
        .saturating_add(w.tracked_out_expired);
    svc.stats_cumulative.outbound_shaper_delayed = svc
        .stats_cumulative
        .outbound_shaper_delayed
        .saturating_add(w.outbound_shaper_delayed);
    let c = svc.stats_cumulative;
    svc.stats_window = KadServiceStats::default();

    KadServiceStatus {
        uptime_secs: started.elapsed().as_secs(),
        routing,
        live,
        live_10m,
        pending,
        pending_overdue,
        pending_max_overdue_ms,
        tracked_out_requests,
        recv_req: w.sent_reqs,
        recv_res: w.recv_ress,
        sent_reqs: w.sent_reqs,
        recv_ress: w.recv_ress,
        res_contacts: w.res_contacts,
        dropped_undecipherable: w.dropped_undecipherable,
        dropped_unparsable: w.dropped_unparsable,
        recv_hello_reqs: w.recv_hello_reqs,
        sent_bootstrap_reqs: w.sent_bootstrap_reqs,
        recv_bootstrap_ress: w.recv_bootstrap_ress,
        bootstrap_contacts: w.bootstrap_contacts,
        sent_hellos: w.sent_hellos,
        recv_hello_ress: w.recv_hello_ress,
        sent_hello_acks: w.sent_hello_acks,
        recv_hello_acks: w.recv_hello_acks,
        hello_ack_skipped_no_sender_key: w.hello_ack_skipped_no_sender_key,
        timeouts: w.timeouts,
        new_nodes: w.new_nodes,
        evicted: w.evicted,

        sent_search_source_reqs: w.sent_search_source_reqs,
        recv_search_source_reqs: w.recv_search_source_reqs,
        recv_search_source_decode_failures: w.recv_search_source_decode_failures,
        source_search_hits: w.source_search_hits,
        source_search_misses: w.source_search_misses,
        source_search_results_served: w.source_search_results_served,
        recv_search_ress: w.recv_search_ress,
        search_results: w.search_results,
        new_sources: w.new_sources,

        sent_search_key_reqs: w.sent_search_key_reqs,
        recv_search_key_reqs: w.recv_search_key_reqs,
        keyword_results: w.keyword_results,
        new_keyword_results: w.new_keyword_results,
        evicted_keyword_hits: w.evicted_keyword_hits,
        evicted_keyword_keywords: w.evicted_keyword_keywords,
        keyword_keywords_tracked,
        keyword_hits_total,

        store_keyword_keywords,
        store_keyword_hits_total,
        source_store_files,
        source_store_entries_total,

        recv_publish_key_reqs: w.recv_publish_key_reqs,
        recv_publish_key_decode_failures: w.recv_publish_key_decode_failures,
        sent_publish_key_ress: w.sent_publish_key_ress,
        sent_publish_key_reqs: w.sent_publish_key_reqs,
        recv_publish_key_ress: w.recv_publish_key_ress,
        new_store_keyword_hits: w.new_store_keyword_hits,
        evicted_store_keyword_hits: w.evicted_store_keyword_hits,
        evicted_store_keyword_keywords: w.evicted_store_keyword_keywords,

        sent_publish_source_reqs: w.sent_publish_source_reqs,
        recv_publish_source_reqs: w.recv_publish_source_reqs,
        recv_publish_source_decode_failures: w.recv_publish_source_decode_failures,
        sent_publish_source_ress: w.sent_publish_source_ress,
        new_store_source_entries: w.new_store_source_entries,
        recv_publish_ress: w.recv_publish_ress,

        source_search_batch_candidates: w.source_search_batch_candidates,
        source_search_batch_skipped_version: w.source_search_batch_skipped_version,
        source_search_batch_sent: w.source_search_batch_sent,
        source_search_batch_send_fail: w.source_search_batch_send_fail,
        source_publish_batch_candidates: w.source_publish_batch_candidates,
        source_publish_batch_skipped_version: w.source_publish_batch_skipped_version,
        source_publish_batch_sent: w.source_publish_batch_sent,
        source_publish_batch_send_fail: w.source_publish_batch_send_fail,

        source_probe_first_publish_responses: w.source_probe_first_publish_responses,
        source_probe_first_search_responses: w.source_probe_first_search_responses,
        source_probe_search_results_total: w.source_probe_search_results_total,
        source_probe_publish_latency_ms_total: w.source_probe_publish_latency_ms_total,
        source_probe_search_latency_ms_total: w.source_probe_search_latency_ms_total,
        tracked_out_matched: w.tracked_out_matched,
        tracked_out_unmatched: w.tracked_out_unmatched,
        tracked_out_expired: w.tracked_out_expired,
        outbound_shaper_delayed: w.outbound_shaper_delayed,
        outbound_shaper_drop_global_cap: w.outbound_shaper_drop_global_cap,
        outbound_shaper_drop_peer_cap: w.outbound_shaper_drop_peer_cap,
        recv_req_total: c.sent_reqs,
        recv_res_total: c.recv_ress,
        sent_reqs_total: c.sent_reqs,
        recv_ress_total: c.recv_ress,
        timeouts_total: c.timeouts,
        tracked_out_matched_total: c.tracked_out_matched,
        tracked_out_unmatched_total: c.tracked_out_unmatched,
        tracked_out_expired_total: c.tracked_out_expired,
        outbound_shaper_delayed_total: c.outbound_shaper_delayed,
        sam_framing_desync_total: c.sam_framing_desync,
    }
}

pub(super) fn publish_status_impl(
    svc: &mut KadService,
    started: Instant,
    status_tx: &Option<watch::Sender<Option<KadServiceStatus>>>,
    status_events_tx: &Option<broadcast::Sender<KadServiceStatus>>,
) {
    let st = build_status_impl(svc, started);
    let summary = routing_view::build_routing_summary(svc, Instant::now());
    let verified_pct = if summary.total_nodes > 0 {
        (summary.verified_nodes * 100) / summary.total_nodes
    } else {
        0
    };
    tracing::info!(
        event = "kad_status",
        uptime_secs = st.uptime_secs,
        routing = st.routing,
        live = st.live,
        live_10m = st.live_10m,
        pending = st.pending,
        pending_overdue = st.pending_overdue,
        pending_max_overdue_ms = st.pending_max_overdue_ms,
        tracked_out_requests = st.tracked_out_requests,
        sent_reqs = st.sent_reqs,
        recv_ress = st.recv_ress,
        sent_reqs_total = st.sent_reqs_total,
        recv_ress_total = st.recv_ress_total,
        timeouts = st.timeouts,
        timeouts_total = st.timeouts_total,
        new_nodes = st.new_nodes,
        evicted = st.evicted,
        search_results = st.search_results,
        keyword_results = st.keyword_results,
        keyword_keywords_tracked = st.keyword_keywords_tracked,
        keyword_hits_total = st.keyword_hits_total,
        store_keyword_keywords = st.store_keyword_keywords,
        store_keyword_hits_total = st.store_keyword_hits_total,
        source_store_files = st.source_store_files,
        source_store_entries_total = st.source_store_entries_total,
        verified_pct,
        buckets_empty = summary.buckets_empty,
        bucket_fill_min = summary.bucket_fill_min,
        bucket_fill_median = summary.bucket_fill_median,
        bucket_fill_max = summary.bucket_fill_max,
        "kad service status"
    );
    tracing::debug!(
        event = "kad_status_detail",
        uptime_secs = st.uptime_secs,
        routing = st.routing,
        live = st.live,
        live_10m = st.live_10m,
        pending = st.pending,
        pending_overdue = st.pending_overdue,
        pending_max_overdue_ms = st.pending_max_overdue_ms,
        tracked_out_requests = st.tracked_out_requests,
        sent_reqs = st.sent_reqs,
        recv_ress = st.recv_ress,
        res_contacts = st.res_contacts,
        dropped_undecipherable = st.dropped_undecipherable,
        dropped_unparsable = st.dropped_unparsable,
        recv_hello_reqs = st.recv_hello_reqs,
        sent_bootstrap_reqs = st.sent_bootstrap_reqs,
        recv_bootstrap_ress = st.recv_bootstrap_ress,
        bootstrap_contacts = st.bootstrap_contacts,
        sent_hellos = st.sent_hellos,
        recv_hello_ress = st.recv_hello_ress,
        sent_hello_acks = st.sent_hello_acks,
        recv_hello_acks = st.recv_hello_acks,
        hello_ack_skipped_no_sender_key = st.hello_ack_skipped_no_sender_key,
        timeouts = st.timeouts,
        new_nodes = st.new_nodes,
        evicted = st.evicted,
        sent_search_source_reqs = st.sent_search_source_reqs,
        recv_search_source_reqs = st.recv_search_source_reqs,
        recv_search_source_decode_failures = st.recv_search_source_decode_failures,
        source_search_hits = st.source_search_hits,
        source_search_misses = st.source_search_misses,
        source_search_results_served = st.source_search_results_served,
        recv_search_ress = st.recv_search_ress,
        search_results = st.search_results,
        new_sources = st.new_sources,
        sent_search_key_reqs = st.sent_search_key_reqs,
        recv_search_key_reqs = st.recv_search_key_reqs,
        keyword_results = st.keyword_results,
        new_keyword_results = st.new_keyword_results,
        evicted_keyword_hits = st.evicted_keyword_hits,
        evicted_keyword_keywords = st.evicted_keyword_keywords,
        keyword_keywords_tracked = st.keyword_keywords_tracked,
        keyword_hits_total = st.keyword_hits_total,
        store_keyword_keywords = st.store_keyword_keywords,
        store_keyword_hits_total = st.store_keyword_hits_total,
        source_store_files = st.source_store_files,
        source_store_entries_total = st.source_store_entries_total,
        recv_publish_key_reqs = st.recv_publish_key_reqs,
        recv_publish_key_decode_failures = st.recv_publish_key_decode_failures,
        sent_publish_key_ress = st.sent_publish_key_ress,
        sent_publish_key_reqs = st.sent_publish_key_reqs,
        recv_publish_key_ress = st.recv_publish_key_ress,
        new_store_keyword_hits = st.new_store_keyword_hits,
        evicted_store_keyword_hits = st.evicted_store_keyword_hits,
        evicted_store_keyword_keywords = st.evicted_store_keyword_keywords,
        sent_publish_source_reqs = st.sent_publish_source_reqs,
        recv_publish_source_reqs = st.recv_publish_source_reqs,
        recv_publish_source_decode_failures = st.recv_publish_source_decode_failures,
        sent_publish_source_ress = st.sent_publish_source_ress,
        new_store_source_entries = st.new_store_source_entries,
        recv_publish_ress = st.recv_publish_ress,
        source_search_batch_candidates = st.source_search_batch_candidates,
        source_search_batch_skipped_version = st.source_search_batch_skipped_version,
        source_search_batch_sent = st.source_search_batch_sent,
        source_search_batch_send_fail = st.source_search_batch_send_fail,
        source_publish_batch_candidates = st.source_publish_batch_candidates,
        source_publish_batch_skipped_version = st.source_publish_batch_skipped_version,
        source_publish_batch_sent = st.source_publish_batch_sent,
        source_publish_batch_send_fail = st.source_publish_batch_send_fail,
        source_probe_first_publish_responses = st.source_probe_first_publish_responses,
        source_probe_first_search_responses = st.source_probe_first_search_responses,
        source_probe_search_results_total = st.source_probe_search_results_total,
        source_probe_publish_latency_ms_total = st.source_probe_publish_latency_ms_total,
        source_probe_search_latency_ms_total = st.source_probe_search_latency_ms_total,
        tracked_out_matched = st.tracked_out_matched,
        tracked_out_unmatched = st.tracked_out_unmatched,
        tracked_out_expired = st.tracked_out_expired,
        outbound_shaper_delayed = st.outbound_shaper_delayed,
        outbound_shaper_drop_global_cap = st.outbound_shaper_drop_global_cap,
        outbound_shaper_drop_peer_cap = st.outbound_shaper_drop_peer_cap,
        sent_reqs_total = st.sent_reqs_total,
        recv_ress_total = st.recv_ress_total,
        timeouts_total = st.timeouts_total,
        tracked_out_matched_total = st.tracked_out_matched_total,
        tracked_out_unmatched_total = st.tracked_out_unmatched_total,
        tracked_out_expired_total = st.tracked_out_expired_total,
        outbound_shaper_delayed_total = st.outbound_shaper_delayed_total,
        sam_framing_desync_total = st.sam_framing_desync_total,
        verified_pct,
        buckets_empty = summary.buckets_empty,
        bucket_fill_min = summary.bucket_fill_min,
        bucket_fill_median = summary.bucket_fill_median,
        bucket_fill_max = summary.bucket_fill_max,
        "kad service status detail"
    );
    if st.routing > 0
        && st.evicted as usize >= st.routing / 5
        && st.evicted > 0
        && crate::logging::warn_throttled("kad_contacts_decayed_fast", Duration::from_secs(300))
    {
        tracing::warn!(
            evicted = st.evicted,
            routing = st.routing,
            "contacts decayed fast"
        );
    }

    if let Some(tx) = status_tx {
        let _ = tx.send(Some(st.clone()));
    }
    if let Some(tx) = status_events_tx {
        let _ = tx.send(st);
    }
}
