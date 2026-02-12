import {
  apiDelete,
  apiGet,
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
    error: '',
    status: null,
    peers: [],
    searchThreads: [],

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
        await this.refreshThreads();
        await this.refresh();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshThreads() {
      await loadSearchThreads(this);
    },

    async refresh() {
      try {
        this.error = '';
        const [statusResp, peersResp] = await Promise.all([
          apiGet('/status'),
          apiGet('/kad/peers'),
        ]);
        this.status = statusResp;
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
      } catch (err) {
        this.error = String(err?.message || err);
      }
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
    error: '',
    searchThreads: [],
    status: null,
    theme: currentTheme(),

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
        this.theme = currentTheme();
        await bootstrapToken();
        await this.refreshThreads();
        this.status = await apiGet('/status');
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
  };
};
