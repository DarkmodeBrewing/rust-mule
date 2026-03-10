import {
  apiGet,
  bootstrapToken,
  enrichOverviewStatus,
  loadSearchThreads,
  openStatusEventStream,
  sessionControlMixin,
} from '../app-core.js';

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
        this.startEvents();
        this.startStatusPolling();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
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
  };
};
