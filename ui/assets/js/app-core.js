import {
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
  bootstrapToken,
  getToken,
  setToken,
  openStatusEventStream,
} from './helpers.js';

export {
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
  bootstrapToken,
  getToken,
  setToken,
  openStatusEventStream,
};

const THEME_KEY = 'ui_theme';
const ALLOWED_THEMES = ['dark', 'light', 'hc'];

export function parseSearchIdFromQuery() {
  const params = new URLSearchParams(window.location.search);
  return (params.get('searchId') || '').trim();
}

export function stateClass(state) {
  if (state === 'running') {
    return 'state-running';
  }
  if (state === 'complete') {
    return 'state-done';
  }
  return 'state-idle';
}

export function normalizeSearchThread(thread) {
  const nextState = typeof thread?.state === 'string' ? thread.state : 'idle';
  const keywordLabel =
    typeof thread?.keyword_label === 'string' ? thread.keyword_label.trim() : '';
  const displayLabel = keywordLabel || thread?.search_id_hex || 'unknown search';
  return {
    ...thread,
    keyword_label: keywordLabel || null,
    display_label: displayLabel,
    display_hash: thread?.search_id_hex || '',
    has_distinct_label: Boolean(keywordLabel && keywordLabel !== thread?.search_id_hex),
    state: nextState,
    state_class: stateClass(nextState),
  };
}

export function nodeState(peer) {
  const inbound = peer?.last_inbound_secs_ago;
  const seen = peer?.last_seen_secs_ago;
  if (typeof inbound === 'number' && inbound <= 600) {
    return 'active';
  }
  if (typeof seen === 'number' && seen <= 600) {
    return 'live';
  }
  return 'idle';
}

export function nodeStateClass(state) {
  if (state === 'active') {
    return 'state-running';
  }
  if (state === 'live') {
    return 'state-done';
  }
  return 'state-idle';
}

export async function loadSearchThreads(ctx) {
  const data = await apiGet('/searches');
  const threads = Array.isArray(data?.searches) ? data.searches : [];
  ctx.searchThreads = threads.map(normalizeSearchThread);
  if ('searchReady' in ctx) {
    ctx.searchReady = Boolean(data?.ready);
  }
}

export function currentTheme() {
  const t = document.documentElement.getAttribute('data-theme') || 'dark';
  return ALLOWED_THEMES.includes(t) ? t : 'dark';
}

export function applyThemeValue(theme) {
  const next = ALLOWED_THEMES.includes(theme) ? theme : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  try {
    localStorage.setItem(THEME_KEY, next);
  } catch (_err) {
    // best-effort local preference persistence
  }
  return next;
}

export function formatBytes(bytes) {
  const value = Number(bytes || 0);
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B';
  }
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let size = value;
  let unit = units[0];
  for (let i = 1; i < units.length && size >= 1024; i += 1) {
    size /= 1024;
    unit = units[i];
  }
  return `${size >= 10 ? size.toFixed(0) : size.toFixed(1)} ${unit}`;
}

export function formatUnixSecs(unixSecs) {
  const value = Number(unixSecs);
  if (!Number.isFinite(value) || value <= 0) {
    return 'never';
  }
  return new Date(value * 1000).toLocaleString();
}

export function formatRate(bytesPerSec) {
  const value = Number(bytesPerSec || 0);
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B/s';
  }
  return `${formatBytes(value)}/s`;
}

export function enrichOverviewStatus(status, previous = null) {
  const next = {
    ...(previous || {}),
    ...(status || {}),
  };
  next.download_rate_label = formatRate(next.download_rate_bps_5s || 0);
  next.upload_rate_label = formatRate(next.upload_rate_bps_5s || 0);
  next.zero_fill_upload_rate_label = formatRate(
    next.zero_fill_upload_rate_bps_5s || 0,
  );
  return next;
}

export function normalizeRanges(ranges) {
  return Array.isArray(ranges)
    ? ranges
        .map((range) => ({
          start: Number(range?.start || 0),
          end: Number(range?.end || 0),
        }))
        .filter(
          (range) =>
            Number.isFinite(range.start) &&
            Number.isFinite(range.end) &&
            range.end >= range.start,
        )
        .sort((a, b) => a.start - b.start)
    : [];
}

export function buildPartGraphSegments(
  fileSize,
  missingRanges,
  inflightRanges,
  sourceCount,
) {
  const total = Math.max(Number(fileSize || 0), 1);
  const missing = normalizeRanges(missingRanges);
  const inflight = normalizeRanges(inflightRanges);
  const points = new Set([0, total]);
  for (const range of [...missing, ...inflight]) {
    points.add(Math.max(0, Math.min(total, range.start)));
    points.add(Math.max(0, Math.min(total, range.end + 1)));
  }
  const edges = Array.from(points).sort((a, b) => a - b);
  const segments = [];
  for (let i = 0; i < edges.length - 1; i += 1) {
    const start = edges[i];
    const endExclusive = edges[i + 1];
    if (endExclusive <= start) {
      continue;
    }
    const coveredByInflight = inflight.some(
      (range) => start >= range.start && start <= range.end,
    );
    const coveredByMissing = missing.some(
      (range) => start >= range.start && start <= range.end,
    );
    let kind = 'downloaded';
    if (coveredByInflight) {
      kind = 'inflight';
    } else if (coveredByMissing) {
      kind = sourceCount > 0 ? 'pending' : 'no_source';
    }
    segments.push({
      kind,
      bytes: endExclusive - start,
      pct: ((endExclusive - start) / total) * 100,
      title: `${kind} ${formatBytes(endExclusive - start)}`,
    });
  }
  return segments.filter((segment) => segment.pct > 0);
}

export function graphSegmentStyleValue(segment) {
  const colors = {
    downloaded: '#4caf50',
    inflight: '#ff9800',
    pending: '#607d8b',
    no_source: '#ef5350',
  };
  return `width:${segment.pct}%;background:${colors[segment.kind] || '#607d8b'};height:100%;`;
}

export function splitLines(text) {
  return String(text || '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function goToSearchPage() {
  window.location.href = '/ui/search';
}

export function sessionControlMixin() {
  return {
    sessionActive: null,
    sessionChecking: false,
    sessionStateLabel: 'session: unknown',
    sessionStateClass: '',

    startNewSearch() {
      goToSearchPage();
    },

    uploadTerminalReasonClass(reason) {
      switch (reason) {
        case 'completed':
          return 'state-done';
        case 'dropped':
          return 'state-failed';
        case 'expired':
          return 'state-idle';
        default:
          return 'state-idle';
      }
    },

    buildRecentSessionGroups(recentSessions) {
      if (!Array.isArray(recentSessions)) {
        return [];
      }
      const counts = new Map();
      for (const session of recentSessions) {
        const reason = session.terminal_reason || 'unknown';
        counts.set(reason, (counts.get(reason) || 0) + 1);
      }
      return Array.from(counts.entries())
        .map(([reason, count]) => ({
          reason,
          count,
          reason_class: this.uploadTerminalReasonClass(reason),
        }))
        .sort((a, b) => a.reason.localeCompare(b.reason));
    },

    updateSessionStateUi() {
      if (this.sessionChecking) {
        this.sessionStateLabel = 'session: checking';
        this.sessionStateClass = 'state-running';
        return;
      }
      if (this.sessionActive === true) {
        this.sessionStateLabel = 'session: active';
        this.sessionStateClass = 'state-done';
        return;
      }
      if (this.sessionActive === false) {
        this.sessionStateLabel = 'session: expired';
        this.sessionStateClass = 'state-idle';
        return;
      }
      this.sessionStateLabel = 'session: unknown';
      this.sessionStateClass = '';
    },

    async checkSession() {
      this.sessionChecking = true;
      this.updateSessionStateUi();
      try {
        const resp = await fetch('/api/v1/session/check');
        if (resp.ok) {
          this.sessionActive = true;
          this.updateSessionStateUi();
          return true;
        }
        if (resp.status === 401 || resp.status === 403) {
          this.sessionActive = false;
          this.updateSessionStateUi();
          return false;
        }
        throw new Error(`session check failed: ${resp.status}`);
      } catch (err) {
        this.sessionActive = false;
        this.updateSessionStateUi();
        if ('error' in this) {
          this.error = String(err?.message || err);
        }
        return false;
      } finally {
        this.sessionChecking = false;
        this.updateSessionStateUi();
      }
    },

    async logoutSession() {
      try {
        await fetch('/api/v1/session/logout', { method: 'POST' });
      } catch (_err) {
        // continue with redirect even if session is already invalid
      }
      window.location.replace('/auth');
    },
  };
}
