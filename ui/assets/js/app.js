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

const THEME_KEY = 'ui_theme';
const ALLOWED_THEMES = ['dark', 'light', 'hc'];

function parseSearchIdFromQuery() {
  const params = new URLSearchParams(window.location.search);
  return (params.get('searchId') || '').trim();
}

function stateClass(state) {
  if (state === 'running') {
    return 'state-running';
  }
  if (state === 'complete') {
    return 'state-done';
  }
  return 'state-idle';
}

function normalizeSearchThread(thread) {
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

function nodeState(peer) {
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

function nodeStateClass(state) {
  if (state === 'active') {
    return 'state-running';
  }
  if (state === 'live') {
    return 'state-done';
  }
  return 'state-idle';
}

async function loadSearchThreads(ctx) {
  const data = await apiGet('/searches');
  const threads = Array.isArray(data?.searches) ? data.searches : [];
  ctx.searchThreads = threads.map(normalizeSearchThread);
  if ('searchReady' in ctx) {
    ctx.searchReady = Boolean(data?.ready);
  }
}

function downloadJson(filename, data) {
  const blob = new Blob([JSON.stringify(data, null, 2)], {
    type: 'application/json',
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function goToSearchPage() {
  window.location.href = '/ui/search';
}

function currentTheme() {
  const t = document.documentElement.getAttribute('data-theme') || 'dark';
  return ALLOWED_THEMES.includes(t) ? t : 'dark';
}

function applyThemeValue(theme) {
  const next = ALLOWED_THEMES.includes(theme) ? theme : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  try {
    localStorage.setItem(THEME_KEY, next);
  } catch (_err) {
    // best-effort local preference persistence
  }
  return next;
}

function formatBytes(bytes) {
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

function formatUnixSecs(unixSecs) {
  const value = Number(unixSecs);
  if (!Number.isFinite(value) || value <= 0) {
    return 'never';
  }
  return new Date(value * 1000).toLocaleString();
}

function formatRate(bytesPerSec) {
  const value = Number(bytesPerSec || 0);
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B/s';
  }
  return `${formatBytes(value)}/s`;
}

function enrichOverviewStatus(status, previous = null) {
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

function normalizeRanges(ranges) {
  return Array.isArray(ranges)
    ? ranges
        .map((range) => ({
          start: Number(range?.start || 0),
          end: Number(range?.end || 0),
        }))
        .filter((range) => Number.isFinite(range.start) && Number.isFinite(range.end) && range.end >= range.start)
        .sort((a, b) => a.start - b.start)
    : [];
}

function buildPartGraphSegments(fileSize, missingRanges, inflightRanges, sourceCount) {
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

function graphSegmentStyleValue(segment) {
  const colors = {
    downloaded: '#4caf50',
    inflight: '#ff9800',
    pending: '#607d8b',
    no_source: '#ef5350',
  };
  return `width:${segment.pct}%;background:${colors[segment.kind] || '#607d8b'};height:100%;`;
}

function splitLines(text) {
  return String(text || '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function sessionControlMixin() {
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

window.indexApp = function indexApp() {
  return {
    ...sessionControlMixin(),
    loading: false,
    connected: false,
    error: '',
    notice: '',
    token: '',
    status: null,
    sse: null,
    statusPollTimer: null,
    searchThreads: [],
    selectedSearchId: '',

    get activeThread() {
      if (!this.selectedSearchId) {
        return null;
      }
      return (
        this.searchThreads.find(
          (t) => t.search_id_hex === this.selectedSearchId,
        ) || null
      );
    },

    get activeThreadTitle() {
      return this.activeThread?.display_label || 'No active search selected';
    },

    get activeThreadState() {
      return this.activeThread?.state || 'idle';
    },

    get activeThreadStateClass() {
      return this.activeThread?.state_class || stateClass('idle');
    },

    get prettyStatus() {
      if (!this.status) {
        return '{}';
      }
      return JSON.stringify(this.status, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = '';
      this.notice = '';

      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        this.token = await bootstrapToken();
        await this.refreshStatus();
        await this.refreshThreads();
        this.selectInitialThread();
        this.startEvents();
        this.startStatusPolling();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    selectInitialThread() {
      const fromQuery = parseSearchIdFromQuery();
      if (
        fromQuery &&
        this.searchThreads.some((t) => t.search_id_hex === fromQuery)
      ) {
        this.selectedSearchId = fromQuery;
        return;
      }
      this.selectedSearchId = this.searchThreads[0]?.search_id_hex || '';
    },

    async refreshStatus() {
      try {
        this.error = '';
        const status = await apiGet('/status');
        this.status = enrichOverviewStatus(status, this.status);
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    async refreshThreads() {
      try {
        await loadSearchThreads(this);
        if (
          this.selectedSearchId &&
          !this.searchThreads.some(
            (t) => t.search_id_hex === this.selectedSearchId,
          )
        ) {
          this.selectedSearchId = this.searchThreads[0]?.search_id_hex || '';
        }
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    startEvents() {
      this.stopEvents();
      try {
        this.sse = openStatusEventStream(
          (status) => {
            this.status = enrichOverviewStatus(status, this.status);
            this.connected = true;
          },
          (message) => {
            this.connected = false;
            this.error = message;
          },
        );
      } catch (err) {
        this.connected = false;
        this.error = String(err?.message || err);
      }
    },

    stopEvents() {
      if (this.sse) {
        this.sse.close();
        this.sse = null;
      }
      this.connected = false;
    },

    startStatusPolling() {
      this.stopStatusPolling();
      this.statusPollTimer = setInterval(() => {
        this.refreshStatus();
      }, 15000);
      window.addEventListener('beforeunload', () => {
        this.stopStatusPolling();
        this.stopEvents();
      });
    },

    stopStatusPolling() {
      if (this.statusPollTimer) {
        clearInterval(this.statusPollTimer);
        this.statusPollTimer = null;
      }
    },

    async stopActiveSearch() {
      if (!this.activeThread) {
        this.notice = 'No active search selected to stop.';
        return;
      }
      try {
        const id = this.activeThread.search_id_hex;
        const resp = await apiPost(`/searches/${id}/stop`, {});
        if (resp?.stopped) {
          this.notice = `Stopped search ${id}.`;
        } else {
          this.notice = `Search ${id} was not active.`;
        }
        await this.refreshThreads();
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    async exportActiveSearch() {
      if (!this.activeThread) {
        this.notice = 'No active search selected to export.';
        return;
      }
      try {
        const details = await apiGet(
          `/searches/${this.activeThread.search_id_hex}`,
        );
        downloadJson(`search-${this.activeThread.search_id_hex}.json`, details);
        this.notice = `Exported search ${this.activeThread.search_id_hex}.`;
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    async deleteActiveSearch() {
      if (!this.activeThread) {
        this.notice = 'No active search selected to remove from view.';
        return;
      }
      try {
        const id = this.activeThread.search_id_hex;
        const resp = await apiDelete(`/searches/${id}`);
        if (resp?.deleted) {
          this.notice = `Deleted search ${id}.`;
        } else {
          this.notice = `Search ${id} was not found.`;
        }
        await this.refreshThreads();
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },
  };
};

window.appSearch = function appSearch() {
  return {
    ...sessionControlMixin(),
    loading: false,
    submitting: false,
    searchReady: false,
    error: '',
    notice: '',
    query: '',
    keywordIdHex: '',
    searchResponse: null,
    keywordResults: null,
    searchThreads: [],

    get activeKeywordIdHex() {
      return this.searchResponse?.keyword_id_hex || this.keywordIdHex.trim();
    },

    get prettySearchResponse() {
      if (!this.searchResponse) {
        return '{}';
      }
      return JSON.stringify(this.searchResponse, null, 2);
    },

    get keywordHits() {
      const hits = this.keywordResults?.hits;
      return Array.isArray(hits) ? hits : [];
    },

    async init() {
      this.loading = true;
      this.error = '';
      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async submitSearch() {
      if (!this.searchReady) {
        this.error = 'Search service is still starting. Wait for KAD readiness.';
        this.focusQueryInput();
        return;
      }
      this.submitting = true;
      this.error = '';
      this.notice = '';
      try {
        const payload = this.buildPayload();
        this.searchResponse = await apiPost('/kad/search_keyword', payload);
        this.clearSearchInputs();
        await this.refreshResults();
        await this.refreshThreads();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.submitting = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async refreshResults() {
      if (!this.searchReady) {
        this.notice = 'Search results are unavailable until KAD search is ready.';
        return;
      }
      const keywordIdHex = this.activeKeywordIdHex;
      if (!keywordIdHex) {
        this.keywordResults = null;
        return;
      }
      this.keywordResults = await apiGet(
        `/kad/keyword_results/${keywordIdHex}`,
      );
    },

    startNewSearch() {
      this.notice = '';
      this.query = '';
      this.keywordIdHex = '';
      this.searchResponse = null;
      this.keywordResults = null;
      this.focusQueryInput();
    },

    clearSearchInputs() {
      this.query = '';
      this.keywordIdHex = '';
      this.focusQueryInput();
    },

    focusQueryInput() {
      const queryInput = document.getElementById('query');
      if (queryInput) {
        queryInput.focus();
      }
    },

    buildPayload() {
      const keywordIdHex = this.keywordIdHex.trim();
      if (keywordIdHex) {
        return { keyword_id_hex: keywordIdHex };
      }

      const query = this.query.trim();
      if (!query) {
        throw new Error('enter a keyword query or keyword id');
      }
      return { query };
    },
  };
};

window.appSearchDetails = function appSearchDetails() {
  return {
    ...sessionControlMixin(),
    loading: false,
    error: '',
    searchId: '',
    details: null,
    searchThreads: [],

    get hits() {
      return this.details?.hits || [];
    },

    get prettyDetails() {
      if (!this.details) {
        return '{}';
      }
      return JSON.stringify(this.details, null, 2);
    },

    get detailsStateClass() {
      const state = this.details?.search?.state || 'idle';
      return stateClass(state);
    },

    get searchIdLabel() {
      return this.searchId || '(missing)';
    },

    async init() {
      this.loading = true;
      this.error = '';
      this.searchId = parseSearchIdFromQuery();

      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
        if (!this.searchId) {
          throw new Error('missing searchId query parameter');
        }
        await this.loadDetails();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async loadDetails() {
      if (!this.searchId) {
        return;
      }
      this.details = await apiGet(`/searches/${this.searchId}`);
    },
  };
};

window.appNodeStats = function appNodeStats() {
  return {
    ...sessionControlMixin(),
    loading: false,
    connected: false,
    error: '',
    status: null,
    peers: [],
    searchThreads: [],
    sse: null,
    refreshTimer: null,
    charts: null,
    historyMaxPoints: 360,
    historyWindow: 60,
    historyPaused: false,
    historyWindows: [20, 60, 180, 360],
    history: {
      labels: [],
      totalHits: [],
      requestRate: [],
      responseRate: [],
      livePeers: [],
      idlePeers: [],
    },
    lastRateSample: null,

    get totalNodes() {
      return this.peers.length;
    },

    get liveNodes() {
      return this.peers.filter((p) => p.ui_state !== 'idle').length;
    },

    get activeNodes() {
      return this.peers.filter((p) => p.ui_state === 'active').length;
    },

    async init() {
      this.loading = true;
      this.error = '';
      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refresh();
        this.initCharts();
        this.captureSnapshot();
        this.startEvents();
        this.startPolling();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async refreshStatus() {
      this.status = await apiGet('/status');
    },

    async refreshPeers() {
      const peersResp = await apiGet('/kad/peers');
      const rawPeers = Array.isArray(peersResp?.peers) ? peersResp.peers : [];
      const normalizedPeers = rawPeers.map((peer) => {
        const state = nodeState(peer);
        const inbound = peer?.last_inbound_secs_ago;
        return {
          ...peer,
          ui_state: state,
          ui_state_class: nodeStateClass(state),
          inbound_label: typeof inbound === 'number' ? `${inbound}s` : '-',
        };
      });
      this.peers = normalizedPeers.slice().sort((a, b) => {
        const sa = a.ui_state;
        const sb = b.ui_state;
        const rank = { active: 0, live: 1, idle: 2 };
        if (rank[sa] !== rank[sb]) {
          return rank[sa] - rank[sb];
        }
        return (
          (a.last_seen_secs_ago ?? Number.MAX_SAFE_INTEGER) -
          (b.last_seen_secs_ago ?? Number.MAX_SAFE_INTEGER)
        );
      });
    },

    async refresh() {
      try {
        this.error = '';
        await Promise.all([
          this.refreshStatus(),
          this.refreshPeers(),
          this.refreshThreads(),
        ]);
        this.captureSnapshot();
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    startEvents() {
      this.stopEvents();
      try {
        this.sse = openStatusEventStream(
          (status) => {
            this.status = status;
            this.connected = true;
            this.captureSnapshot();
          },
          (message) => {
            this.connected = false;
            this.error = message;
          },
        );
      } catch (err) {
        this.connected = false;
        this.error = String(err?.message || err);
      }
    },

    stopEvents() {
      if (this.sse) {
        this.sse.close();
        this.sse = null;
      }
      this.connected = false;
    },

    startPolling() {
      this.stopPolling();
      this.refreshTimer = setInterval(() => {
        this.refresh();
      }, 15000);
      window.addEventListener('beforeunload', () => {
        this.stopPolling();
        this.stopEvents();
        this.destroyCharts();
      });
    },

    stopPolling() {
      if (this.refreshTimer) {
        clearInterval(this.refreshTimer);
        this.refreshTimer = null;
      }
    },

    captureSnapshot() {
      if (!this.status) {
        return;
      }
      const now = Date.now();
      const label = new Date(now).toLocaleTimeString();

      const totalHits = this.searchThreads.reduce((acc, thread) => {
        const hits = Number(thread?.hits ?? 0);
        return acc + (Number.isFinite(hits) ? hits : 0);
      }, 0);

      const req = Number(this.status?.recv_req ?? 0);
      const res = Number(this.status?.recv_res ?? 0);
      let requestRate = 0;
      let responseRate = 0;
      if (this.lastRateSample) {
        const dt = (now - this.lastRateSample.ts) / 1000;
        if (dt > 0) {
          requestRate = Math.max(0, (req - this.lastRateSample.req) / dt);
          responseRate = Math.max(0, (res - this.lastRateSample.res) / dt);
        }
      }
      this.lastRateSample = { ts: now, req, res };

      if (this.historyPaused) {
        return;
      }

      const livePeers = this.peers.filter((p) => p.ui_state !== 'idle').length;
      const idlePeers = this.peers.length - livePeers;

      this.pushHistoryPoint(
        label,
        totalHits,
        requestRate,
        responseRate,
        livePeers,
        idlePeers,
      );
      this.updateCharts();
    },

    pushHistoryPoint(
      label,
      totalHits,
      requestRate,
      responseRate,
      livePeers,
      idlePeers,
    ) {
      this.history.labels.push(label);
      this.history.totalHits.push(totalHits);
      this.history.requestRate.push(Number(requestRate.toFixed(2)));
      this.history.responseRate.push(Number(responseRate.toFixed(2)));
      this.history.livePeers.push(livePeers);
      this.history.idlePeers.push(idlePeers);

      while (this.history.labels.length > this.historyMaxPoints) {
        this.history.labels.shift();
        this.history.totalHits.shift();
        this.history.requestRate.shift();
        this.history.responseRate.shift();
        this.history.livePeers.shift();
        this.history.idlePeers.shift();
      }
    },

    chartColor(varName, fallback) {
      const value = getComputedStyle(document.documentElement)
        .getPropertyValue(varName)
        .trim();
      return value || fallback;
    },

    initCharts() {
      if (!window.Chart) {
        this.error = 'chart.js is not available';
        return;
      }
      this.destroyCharts();
      const ChartRef = window.Chart;
      const signal = this.chartColor('--signal', '#4aa3ff');
      const running = this.chartColor('--state-running', '#38b000');
      const done = this.chartColor('--state-done', '#f4a261');
      const idle = this.chartColor('--state-idle', '#6c757d');

      const commonOptions = {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
      };

      this.charts = {};
      this.charts.hits = new ChartRef(this.$refs.hitsChart, {
        type: 'line',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Total Hits',
              data: this.history.totalHits,
              borderColor: signal,
              backgroundColor: signal,
              tension: 0.25,
            },
          ],
        },
        options: commonOptions,
      });

      this.charts.rate = new ChartRef(this.$refs.rateChart, {
        type: 'line',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Requests / sec',
              data: this.history.requestRate,
              borderColor: running,
              backgroundColor: running,
              tension: 0.25,
            },
            {
              label: 'Responses / sec',
              data: this.history.responseRate,
              borderColor: done,
              backgroundColor: done,
              tension: 0.25,
            },
          ],
        },
        options: commonOptions,
      });

      this.charts.peers = new ChartRef(this.$refs.peersChart, {
        type: 'bar',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Live',
              data: this.history.livePeers,
              backgroundColor: done,
              stack: 'peers',
            },
            {
              label: 'Idle',
              data: this.history.idlePeers,
              backgroundColor: idle,
              stack: 'peers',
            },
          ],
        },
        options: {
          ...commonOptions,
          scales: {
            x: { stacked: true },
            y: { stacked: true, beginAtZero: true },
          },
        },
      });
    },

    updateCharts() {
      if (!this.charts) {
        return;
      }
      const n = Math.max(1, Number(this.historyWindow || 60));
      const labels = this.history.labels.slice(-n);
      this.charts.hits.data.labels = labels;
      this.charts.hits.data.datasets[0].data = this.history.totalHits.slice(-n);
      this.charts.rate.data.labels = labels;
      this.charts.rate.data.datasets[0].data =
        this.history.requestRate.slice(-n);
      this.charts.rate.data.datasets[1].data =
        this.history.responseRate.slice(-n);
      this.charts.peers.data.labels = labels;
      this.charts.peers.data.datasets[0].data =
        this.history.livePeers.slice(-n);
      this.charts.peers.data.datasets[1].data =
        this.history.idlePeers.slice(-n);
      this.charts.hits.update();
      this.charts.rate.update();
      this.charts.peers.update();
    },

    destroyCharts() {
      if (!this.charts) {
        return;
      }
      Object.values(this.charts).forEach((chart) => chart.destroy());
      this.charts = null;
    },

    toggleHistoryPause() {
      this.historyPaused = !this.historyPaused;
    },

    resetHistory() {
      this.history.labels = [];
      this.history.totalHits = [];
      this.history.requestRate = [];
      this.history.responseRate = [];
      this.history.livePeers = [];
      this.history.idlePeers = [];
      this.lastRateSample = null;
      this.updateCharts();
    },
  };
};

window.appLogs = function appLogs() {
  return {
    ...sessionControlMixin(),
    loading: false,
    connected: false,
    error: '',
    status: null,
    sse: null,
    searchThreads: [],
    logEntries: [],

    get prettyStatus() {
      if (!this.status) {
        return '{}';
      }
      return JSON.stringify(this.status, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = '';
      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
        await this.refreshStatus();
        this.startEvents();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async refreshStatus() {
      try {
        this.status = await apiGet('/status');
        this.appendStatusLog(this.status, 'snapshot');
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },

    startEvents() {
      this.stopEvents();
      try {
        this.sse = openStatusEventStream(
          (status) => {
            this.status = status;
            this.connected = true;
            this.appendStatusLog(status, 'event');
          },
          (message) => {
            this.connected = false;
            this.error = message;
            this.appendTextLog(`events stream: ${message}`);
          },
        );
      } catch (err) {
        this.connected = false;
        this.error = String(err?.message || err);
      }
    },

    stopEvents() {
      if (this.sse) {
        this.sse.close();
        this.sse = null;
      }
      this.connected = false;
    },

    appendStatusLog(status, source) {
      const ts = new Date().toISOString();
      const summary =
        `[${source}] routing=${status?.routing ?? 0} ` +
        `live=${status?.live ?? 0} live_10m=${status?.live_10m ?? 0}`;
      this.logEntries.unshift({
        ts,
        summary,
        payload: JSON.stringify(status, null, 2),
      });
      this.logEntries = this.logEntries.slice(0, 200);
    },

    appendTextLog(text) {
      const ts = new Date().toISOString();
      this.logEntries.unshift({
        ts,
        summary: text,
        payload: '',
      });
      this.logEntries = this.logEntries.slice(0, 200);
    },
  };
};

window.appDownloads = function appDownloads() {
  return {
    ...sessionControlMixin(),
    loading: false,
    error: '',
    notice: '',
    status: null,
    searchThreads: [],
    downloads: [],
    uploads: [],
    sharedFiles: [],
    sharedActions: [],
    sharedActionBusy: false,
    showDangerZone: false,
    dangerAcknowledged: false,

    get activeDownloadCount() {
      return this.downloads.filter((item) =>
        ['queued', 'downloading', 'paused', 'completing'].includes(item.state),
      ).length;
    },

    get activeSharedRequests() {
      return this.sharedFiles.filter((item) => item.active_request).length;
    },

    async init() {
      this.loading = true;
      this.error = '';
      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
        await this.refreshStatus();
        await this.refreshData();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async refreshStatus() {
      this.status = await apiGet('/status');
    },

    async refreshData() {
      const [downloadsResp, uploadsResp, sharedResp] = await Promise.all([
        apiGet('/downloads'),
        apiGet('/uploads'),
        apiGet('/shared'),
      ]);
      this.downloads = Array.isArray(downloadsResp?.downloads)
        ? downloadsResp.downloads.map((item) => ({
            ...item,
            pretty_size: formatBytes(item.file_size),
            rate_label: `${formatRate(item.rate_bps_5s)} (5s) / ${formatRate(item.rate_bps_30s)} (30s)`,
            graph_segments: buildPartGraphSegments(
              item.file_size,
              item.missing_range_spans,
              item.inflight_range_spans,
              item.source_count,
            ),
          }))
        : [];
      this.uploads = Array.isArray(uploadsResp?.uploads)
        ? uploadsResp.uploads.map((item) => ({
            ...item,
            requested_bytes_total_pretty: formatBytes(item.requested_bytes_total || 0),
            rate_label: `${formatRate(item.rate_bps_5s)} (5s) / ${formatRate(item.rate_bps_30s)} (30s)`,
            zero_fill_rate_label: `${formatRate(item.zero_fill_rate_bps_5s)} (5s) / ${formatRate(item.zero_fill_rate_bps_30s)} (30s)`,
            zero_fill_requested_bytes_total_pretty: formatBytes(
              item.zero_fill_requested_bytes_total || 0,
            ),
            active_peer_ids_label: Array.isArray(item.active_peer_ids)
              ? item.active_peer_ids.join(', ')
              : '',
            held_ranges_label: (item.held_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            sending_ranges_label: (item.sending_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            last_requested_label: formatUnixSecs(item.last_requested_unix_secs),
            active_since_label: formatUnixSecs(item.active_since_unix_secs),
            payload_source_label: item.last_payload_source || 'unknown',
            sessions: Array.isArray(item.sessions)
              ? item.sessions.map((session) => ({
                  ...session,
                  bytes_total_pretty: formatBytes(session.bytes_total || 0),
                  started_label: formatUnixSecs(session.started_unix_secs),
                  updated_label: formatUnixSecs(session.last_updated_unix_secs),
                  payload_source_label: session.payload_source || 'unknown',
                  terminal_reason_label: session.terminal_reason || 'active',
                }))
              : [],
            recent_sessions: Array.isArray(item.recent_sessions)
              ? item.recent_sessions.map((session) => ({
                  ...session,
                  bytes_total_pretty: formatBytes(session.bytes_total || 0),
                  started_label: formatUnixSecs(session.started_unix_secs),
                  updated_label: formatUnixSecs(session.last_updated_unix_secs),
                  payload_source_label: session.payload_source || 'unknown',
                  terminal_reason_label: session.terminal_reason || 'unknown',
                  terminal_reason_class: this.uploadTerminalReasonClass(
                    session.terminal_reason,
                  ),
                }))
              : [],
            recent_session_groups: this.buildRecentSessionGroups(item.recent_sessions),
          }))
        : [];
      this.sharedFiles = Array.isArray(sharedResp?.files)
        ? sharedResp.files.map((item) => ({
            ...item,
            pretty_size: formatBytes(item.file_size),
            requested_bytes_total_pretty: formatBytes(item.requested_bytes_total || 0),
            local_source_label: item.local_source_cached ? 'cached' : 'not cached',
            source_publish_queue_label: item.source_publish_last_result
              ? `${item.source_publish_last_result} (${item.source_publish_attempts})`
              : 'not attempted',
            source_publish_response_label: item.source_publish_response_received
              ? `response seen${
                  typeof item.source_publish_first_response_latency_ms === 'number'
                    ? ` (${item.source_publish_first_response_latency_ms} ms)`
                    : ''
                }`
              : 'no response yet',
            keyword_publish_queue_label: item.keyword_publish_last_result
              ? `${item.keyword_publish_last_result} (${item.keyword_publish_queued}/${item.keyword_publish_attempts})`
              : 'not attempted',
            keyword_publish_ack_label:
              typeof item.keyword_publish_total === 'number' && item.keyword_publish_total > 0
                ? `${item.keyword_publish_acked}/${item.keyword_publish_total} acked`
                : 'no keyword publishes',
            queued_upload_ranges_label: (item.queued_upload_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            inflight_upload_ranges_label: (item.inflight_upload_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
          }))
        : [];
      await this.refreshSharedActions();
    },

    async refreshSharedActions() {
      const now = Math.floor(Date.now() / 1000);
      const actionsResp = await apiGet('/shared/actions');
      this.sharedActions = Array.isArray(actionsResp?.actions)
        ? actionsResp.actions.map((action) => ({
            ...action,
            cooldown_remaining_secs:
              typeof action.cooldown_until_unix_secs === 'number' &&
              action.cooldown_until_unix_secs > now
                ? action.cooldown_until_unix_secs - now
                : 0,
            summary:
              action.action === 'reindex'
                ? `files=${action.library_files_total ?? 0}, reused=${action.reused_entries ?? 0}, hashed=${action.hashed_entries ?? 0}`
                : `items=${action.items_total}, queued=${action.queued_total}, failed=${action.failed_total}`,
          }))
            .map((action) => ({
              ...action,
              summary:
                action.cooldown_remaining_secs > 0
                  ? `${action.summary}, cooldown=${action.cooldown_remaining_secs}s`
                  : action.summary,
            }))
        : [];
    },

    async runSharedAction(path, successNotice, confirmationText) {
      if (!this.dangerAcknowledged) {
        this.notice = 'acknowledge the danger zone before running shared maintenance actions';
        this.error = '';
        return;
      }
      if (!window.confirm(confirmationText)) {
        return;
      }
      this.sharedActionBusy = true;
      this.error = '';
      this.notice = '';
      try {
        const token = getToken();
        if (!token) {
          throw new Error('missing api token in sessionStorage');
        }
        const response = await fetch(`/api/v1${path}`, {
          method: 'POST',
          headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ confirm: true }),
        });
        const data = await response.json().catch(() => ({}));
        if (response.ok) {
          this.notice = data?.started
            ? successNotice
            : `${data?.status?.action || 'action'} is already running`;
        } else if (response.status === 409) {
          this.notice = `${data?.status?.action || 'action'} is already running`;
        } else if (response.status === 429) {
          this.notice = `${data?.status?.action || 'action'} is cooling down`;
        } else {
          this.error = data?.message || `${path}: ${response.status}`;
        }
        await this.refreshData();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.sharedActionBusy = false;
      }
    },

    async reindexSharedLibrary() {
      await this.runSharedAction(
        '/shared/actions/reindex',
        'reindex started',
        'Reindex Library will rescan all configured shared folders. Continue?',
      );
    },

    async republishSharedSources() {
      await this.runSharedAction(
        '/shared/actions/republish_sources',
        'source republish started',
        'Republish Sources will queue fresh source publish traffic for all indexed shared files. Continue?',
      );
    },

    async republishSharedKeywords() {
      await this.runSharedAction(
        '/shared/actions/republish_keywords',
        'keyword republish started',
        'Republish Keywords will queue fresh keyword publish traffic for all indexed shared files and may generate significant KAD traffic. Continue?',
      );
    },

    graphSegmentStyle(segment) {
      return graphSegmentStyleValue(segment);
    },
  };
};

window.appSettings = function appSettings() {
  return {
    ...sessionControlMixin(),
    loading: false,
    saving: false,
    error: '',
    notice: '',
    searchThreads: [],
    status: null,
    theme: currentTheme(),
    settings: null,
    restartRequired: true,
    form: {
      samHost: '',
      samPort: 0,
      samSessionName: '',
      apiHost: '',
      apiPort: 0,
      logLevel: '',
      logToFile: true,
      logFileLevel: '',
      autoOpenUi: true,
      shareRootsText: '',
    },

    get prettyStatus() {
      if (!this.status) {
        return '{}';
      }
      return JSON.stringify(this.status, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = '';
      this.notice = '';
      try {
        this.theme = currentTheme();
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
        this.status = await apiGet('/status');
        await this.loadSettings();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    applyTheme() {
      this.theme = applyThemeValue(this.theme);
    },

    async loadSettings() {
      const resp = await apiGet('/settings');
      this.settings = resp?.settings || null;
      this.restartRequired = Boolean(resp?.restart_required);
      this.form.samHost = this.settings?.sam?.host || '';
      this.form.samPort = this.settings?.sam?.port || 0;
      this.form.samSessionName = this.settings?.sam?.session_name || '';
      this.form.apiHost = this.settings?.api?.host || '';
      this.form.apiPort = this.settings?.api?.port || 0;
      this.form.logLevel = this.settings?.general?.log_level || 'info';
      this.form.logToFile = Boolean(this.settings?.general?.log_to_file);
      this.form.logFileLevel =
        this.settings?.general?.log_file_level || 'debug';
      this.form.autoOpenUi = this.settings?.general?.auto_open_ui !== false;
      this.form.shareRootsText = Array.isArray(this.settings?.sharing?.share_roots)
        ? this.settings.sharing.share_roots.join('\n')
        : '';
    },

    async saveSettings() {
      this.saving = true;
      this.error = '';
      this.notice = '';
      try {
        const payload = {
          general: {
            log_level: this.form.logLevel,
            log_to_file: this.form.logToFile,
            log_file_level: this.form.logFileLevel,
            auto_open_ui: this.form.autoOpenUi,
          },
          sam: {
            host: this.form.samHost,
            port: Number(this.form.samPort),
            session_name: this.form.samSessionName,
          },
          api: {
            host: this.form.apiHost,
            port: Number(this.form.apiPort),
          },
          sharing: {
            share_roots: splitLines(this.form.shareRootsText),
          },
        };
        const resp = await apiPatch('/settings', payload);
        this.settings = resp?.settings || null;
        this.restartRequired = Boolean(resp?.restart_required);
        this.notice = this.restartRequired
          ? 'Settings saved. Restart required for full effect.'
          : 'Settings saved.';
        await this.loadSettings();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.saving = false;
      }
    },

    async rotateApiToken() {
      this.error = '';
      this.notice = '';
      try {
        const resp = await apiPost('/token/rotate', {});
        const token = resp?.token || '';
        if (!token) {
          throw new Error('token rotate response missing token');
        }
        setToken(token);
        const sessionResp = await fetch('/api/v1/session', {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}` },
        });
        if (!sessionResp.ok) {
          throw new Error(`session refresh failed: ${sessionResp.status}`);
        }
        this.notice = 'API token rotated and session refreshed.';
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },
  };
};
