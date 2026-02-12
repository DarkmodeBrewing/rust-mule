import {
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
  bootstrapToken,
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
  return {
    ...thread,
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

window.indexApp = function indexApp() {
  return {
    loading: false,
    connected: false,
    error: '',
    notice: '',
    token: '',
    status: null,
    sse: null,
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
      return this.activeThread?.search_id_hex || 'No active search selected';
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
        this.token = await bootstrapToken();
        await this.refreshStatus();
        await this.refreshThreads();
        this.selectInitialThread();
        this.startEvents();
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
        this.status = await apiGet('/status');
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
            this.status = status;
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

    startNewSearch() {
      goToSearchPage();
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
    loading: false,
    submitting: false,
    error: '',
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

    async init() {
      this.loading = true;
      this.error = '';
      try {
        await bootstrapToken();
        await this.refreshThreads();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async submitSearch() {
      this.submitting = true;
      this.error = '';
      try {
        const payload = this.buildPayload();
        this.searchResponse = await apiPost('/kad/search_keyword', payload);
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
      this.query = '';
      this.keywordIdHex = '';
      this.searchResponse = null;
      this.keywordResults = null;
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

    async init() {
      this.loading = true;
      this.error = '';
      this.searchId = parseSearchIdFromQuery();

      try {
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

window.appSettings = function appSettings() {
  return {
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

    async logoutSession() {
      this.error = '';
      this.notice = '';
      try {
        await apiPost('/session/logout', {});
      } catch (_err) {
        // continue with redirect even if backend already considers session invalid
      }
      window.location.replace('/auth');
    },
  };
};
