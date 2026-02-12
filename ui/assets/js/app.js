import {
  apiGet,
  apiPost,
  bootstrapToken,
  openStatusEventStream,
} from './helpers.js';

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

function nodeStateClass(peer) {
  const state = nodeState(peer);
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
  ctx.searchThreads = Array.isArray(data?.searches) ? data.searches : [];
}

window.indexApp = function indexApp() {
  return {
    loading: false,
    connected: false,
    error: '',
    token: '',
    status: null,
    sse: null,
    searchThreads: [],

    threadStateClass(state) {
      return stateClass(state);
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

      try {
        this.token = await bootstrapToken();
        await this.refreshStatus();
        await this.refreshThreads();
        this.startEvents();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
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

    threadStateClass(state) {
      return stateClass(state);
    },

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

    threadStateClass(state) {
      return stateClass(state);
    },

    get hits() {
      return this.details?.hits || [];
    },

    get prettyDetails() {
      if (!this.details) {
        return '{}';
      }
      return JSON.stringify(this.details, null, 2);
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

    threadStateClass(state) {
      return stateClass(state);
    },

    nodeState(peer) {
      return nodeState(peer);
    },

    nodeStateClass(peer) {
      return nodeStateClass(peer);
    },

    get totalNodes() {
      return this.peers.length;
    },

    get liveNodes() {
      return this.peers.filter((p) => this.nodeState(p) !== 'idle').length;
    },

    get activeNodes() {
      return this.peers.filter((p) => this.nodeState(p) === 'active').length;
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
        this.peers = rawPeers.slice().sort((a, b) => {
          const sa = this.nodeState(a);
          const sb = this.nodeState(b);
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

    threadStateClass(state) {
      return stateClass(state);
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
