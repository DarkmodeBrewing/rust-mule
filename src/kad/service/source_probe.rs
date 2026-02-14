use super::*;

const SOURCE_PROBE_MAX_TRACKED_FILES: usize = 2048;

pub(super) fn source_store_totals_impl(svc: &KadService) -> (usize, usize) {
    let files = svc.sources_by_file.len();
    let entries = svc.sources_by_file.values().map(BTreeMap::len).sum();
    (files, entries)
}

fn source_probe_state_mut_impl(
    svc: &mut KadService,
    file: KadId,
    now: Instant,
) -> &mut SourceProbeState {
    if svc.source_probe_by_file.len() >= SOURCE_PROBE_MAX_TRACKED_FILES
        && !svc.source_probe_by_file.contains_key(&file)
        && let Some(oldest_key) = svc
            .source_probe_by_file
            .iter()
            .min_by_key(|(_, st)| st.last_update)
            .map(|(k, _)| *k)
    {
        svc.source_probe_by_file.remove(&oldest_key);
    }
    svc.source_probe_by_file
        .entry(file)
        .or_insert_with(|| SourceProbeState {
            first_publish_sent_at: None,
            first_search_sent_at: None,
            first_publish_res_at: None,
            first_search_res_at: None,
            search_result_events: 0,
            search_results_total: 0,
            last_search_results: 0,
            last_update: now,
        })
}

pub(super) fn mark_source_publish_sent_impl(svc: &mut KadService, file: KadId, now: Instant) {
    let st = source_probe_state_mut_impl(svc, file, now);
    if st.first_publish_sent_at.is_none() {
        st.first_publish_sent_at = Some(now);
    }
    st.last_update = now;
}

pub(super) fn mark_source_search_sent_impl(svc: &mut KadService, file: KadId, now: Instant) {
    let st = source_probe_state_mut_impl(svc, file, now);
    if st.first_search_sent_at.is_none() {
        st.first_search_sent_at = Some(now);
    }
    st.last_update = now;
}

pub(super) fn on_source_publish_response_impl(
    svc: &mut KadService,
    file: KadId,
    from_dest_b64: &str,
    now: Instant,
) {
    let mut first_response = false;
    let mut first_latency_ms = None;
    {
        let st = source_probe_state_mut_impl(svc, file, now);
        if st.first_publish_res_at.is_none() {
            st.first_publish_res_at = Some(now);
            first_response = true;
            if let Some(sent_at) = st.first_publish_sent_at {
                first_latency_ms = Some(now.saturating_duration_since(sent_at).as_millis() as u64);
            }
        }
        st.last_update = now;
    }
    if first_response {
        svc.stats_window.source_probe_first_publish_responses += 1;
        if let Some(latency_ms) = first_latency_ms {
            svc.stats_window.source_probe_publish_latency_ms_total += latency_ms;
            tracing::info!(
                event = "source_probe_publish_first_response",
                from = %crate::i2p::b64::short(from_dest_b64),
                file = %crate::logging::redact_hex(&file.to_hex_lower()),
                latency_ms,
                "source publish first response observed"
            );
        }
    }
}

pub(super) fn on_source_search_response_impl(
    svc: &mut KadService,
    file: KadId,
    from_dest_b64: &str,
    returned_sources: u64,
    now: Instant,
) {
    let mut first_response = false;
    let mut first_latency_ms = None;
    {
        let st = source_probe_state_mut_impl(svc, file, now);
        if st.first_search_res_at.is_none() {
            st.first_search_res_at = Some(now);
            first_response = true;
            if let Some(sent_at) = st.first_search_sent_at {
                first_latency_ms = Some(now.saturating_duration_since(sent_at).as_millis() as u64);
            }
        }
        st.search_result_events = st.search_result_events.saturating_add(1);
        st.search_results_total = st.search_results_total.saturating_add(returned_sources);
        st.last_search_results = returned_sources;
        st.last_update = now;
    }
    if first_response {
        svc.stats_window.source_probe_first_search_responses += 1;
        if let Some(latency_ms) = first_latency_ms {
            svc.stats_window.source_probe_search_latency_ms_total += latency_ms;
            tracing::info!(
                event = "source_probe_search_first_response",
                from = %crate::i2p::b64::short(from_dest_b64),
                file = %crate::logging::redact_hex(&file.to_hex_lower()),
                latency_ms,
                returned_sources,
                "source search first response observed"
            );
        }
    }
    svc.stats_window.source_probe_search_results_total = svc
        .stats_window
        .source_probe_search_results_total
        .saturating_add(returned_sources);
}
