import {
  apiGet,
  bootstrapToken,
  loadSearchThreads,
  openStatusEventStream,
  sessionControlMixin,
} from '../app-core.js';

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
