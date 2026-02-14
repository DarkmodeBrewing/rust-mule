use super::*;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct KeywordDropCount {
    pub(super) keywords: usize,
    pub(super) hits: usize,
}

pub(super) fn touch_keyword_interest_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    keyword: KadId,
    now: Instant,
) {
    svc.keyword_interest.insert(keyword, now);
    enforce_keyword_interest_limit_impl(svc, cfg);
}

pub(super) fn enforce_keyword_interest_limit_impl(svc: &mut KadService, cfg: &KadServiceConfig) {
    let max_keywords = cfg.keyword_max_keywords;
    if max_keywords == 0 {
        let removed = drop_all_keywords_impl(svc);
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
        let removed_hits = drop_keyword_hits_only_impl(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }
}

pub(super) fn enforce_keyword_size_limits_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
) {
    enforce_keyword_per_keyword_caps_all_impl(svc, cfg);
    enforce_keyword_total_cap_impl(svc, cfg, now);
}

pub(super) fn upsert_keyword_hit_cache_impl(
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

    enforce_keyword_size_limits_impl(svc, cfg, now);
}

pub(super) fn enforce_keyword_per_keyword_caps_all_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
) {
    let per = cfg.keyword_max_hits_per_keyword;
    if per == 0 {
        return;
    }
    let keys = svc
        .keyword_hits_by_keyword
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for k in keys {
        prune_keyword_hits_per_keyword_impl(svc, k, per);
    }
}

pub(super) fn enforce_keyword_per_keyword_cap_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    keyword: KadId,
) {
    let per = cfg.keyword_max_hits_per_keyword;
    if per == 0 {
        return;
    }
    prune_keyword_hits_per_keyword_impl(svc, keyword, per);
}

pub(super) fn enforce_keyword_total_cap_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    _now: Instant,
) {
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
        let removed_hits = drop_keyword_hits_only_impl(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }
}

pub(super) fn prune_keyword_hits_per_keyword_impl(
    svc: &mut KadService,
    keyword: KadId,
    max: usize,
) {
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

pub(super) fn drop_keyword_hits_only_impl(svc: &mut KadService, keyword: KadId) -> usize {
    if let Some(m) = svc.keyword_hits_by_keyword.remove(&keyword) {
        let n = m.len();
        svc.keyword_hits_total = svc.keyword_hits_total.saturating_sub(n);
        return n;
    }
    0
}

pub(super) fn drop_all_keywords_impl(svc: &mut KadService) -> KeywordDropCount {
    let keywords = svc.keyword_hits_by_keyword.len();
    let hits = svc.keyword_hits_total;
    svc.keyword_hits_by_keyword.clear();
    svc.keyword_hits_total = 0;
    svc.keyword_interest.clear();
    KeywordDropCount { keywords, hits }
}

pub(super) fn maintain_keyword_cache_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
) {
    let interest_ttl = Duration::from_secs(cfg.keyword_interest_ttl_secs.max(60));
    let mut expired_keywords = Vec::<KadId>::new();
    for (k, t) in &svc.keyword_interest {
        if now.saturating_duration_since(*t) >= interest_ttl {
            expired_keywords.push(*k);
        }
    }
    for k in expired_keywords {
        svc.keyword_interest.remove(&k);
        let removed_hits = drop_keyword_hits_only_impl(svc, k);
        svc.stats_window.evicted_keyword_keywords += 1;
        svc.stats_window.evicted_keyword_hits += removed_hits as u64;
    }

    let results_ttl = Duration::from_secs(cfg.keyword_results_ttl_secs.max(60));
    let keys = svc
        .keyword_hits_by_keyword
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for k in keys {
        if cfg.keyword_require_interest && !svc.keyword_interest.contains_key(&k) {
            let removed_hits = drop_keyword_hits_only_impl(svc, k);
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

    enforce_keyword_interest_limit_impl(svc, cfg);
    enforce_keyword_size_limits_impl(svc, cfg, now);
}

pub(super) fn maintain_keyword_store_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
) {
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

    enforce_keyword_store_limits_impl(svc, cfg, now);
}

pub(super) fn enforce_keyword_store_limits_impl(
    svc: &mut KadService,
    cfg: &KadServiceConfig,
    now: Instant,
) {
    let max_keywords = cfg.store_keyword_max_keywords;
    let max_total = cfg.store_keyword_max_total_hits;

    if max_keywords == 0 || max_total == 0 {
        if !svc.keyword_store_by_keyword.is_empty() {
            svc.stats_window.evicted_store_keyword_keywords +=
                svc.keyword_store_by_keyword.len() as u64;
            svc.stats_window.evicted_store_keyword_hits += svc.keyword_store_total as u64;
        }
        svc.keyword_store_by_keyword.clear();
        svc.keyword_store_total = 0;
        return;
    }

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
