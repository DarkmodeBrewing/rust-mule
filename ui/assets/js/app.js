import { apiGet, openEventStream, getTheme, setTheme } from './helpers.js';

/**  
  Pages 
  Contains intial skeletal functions for Alpine init...
*/
// "ui/assets/js/app.js"
import {
  getToken,
  setToken,
  clearToken,
  tokenPresent,
  apiGet,
  apiPost,
  openEventStream,
  formatNumber,
} from './helpers.js';

window.indexApp = function indexApp() {
  return {
    apiBase: 'http://127.0.0.1', // purely for display; requests use same-origin paths
    loading: true,
    connected: false,
    error: null,

    overview: null,
    lastUpdateAt: null,

    logLines: [],
    logMaxLines: 400,
    sse: null,

    tokenPresent,

    formatNumber,
    formatTime(ts) {
      try {
        return new Date(ts).toLocaleTimeString();
      } catch {
        return '—';
      }
    },

    async init() {
      try {
        this.loading = true;
        this.error = null;

        // If you want auto-dev-login on first visit, uncomment:
        // if (!tokenPresent()) await this.devLogin()

        await this.loadOverview();
        this.startStream();
      } catch (e) {
        this.error = String(e?.message ?? e);
      } finally {
        this.loading = false;
      }
    },

    async refresh() {
      try {
        this.error = null;
        this.loading = true;
        await this.loadOverview();
      } catch (e) {
        this.error = String(e?.message ?? e);
      } finally {
        this.loading = false;
      }
    },

    async devLogin() {
      // Loopback-only endpoint on your API (recommended)
      // POST /api/auth/dev -> { token: "..." }
      try {
        this.error = null;
        this.loading = true;
        const res = await apiPost('/api/auth/dev', {});
        if (!res?.token)
          throw new Error('No token returned from /api/auth/dev');
        setToken(res.token);

        await this.loadOverview();
        this.startStream();
      } catch (e) {
        this.error = String(e?.message ?? e);
      } finally {
        this.loading = false;
      }
    },

    logout() {
      clearToken();
      this.stopStream();
      this.connected = false;
      this.overview = null;
      this.logLines = [];
    },

    async loadOverview() {
      // GET /api/overview -> your single “overview snapshot” payload
      const data = await apiGet('/api/overview');
      this.overview = data;
      this.lastUpdateAt = Date.now();
    },

    async action(path) {
      // Simple command endpoints
      try {
        this.error = null;
        await apiPost(path, {});
      } catch (e) {
        this.error = String(e?.message ?? e);
      }
    },

    startStream() {
      // Close previous stream if any
      this.stopStream();

      // NOTE: Native EventSource can't send Authorization headers.
      // For localhost iteration, the simplest approach is to let SSE accept a token query param.
      // Your backend can treat it as loopback-only and ensure logs don't print the full URL.
      //
      // Later (remote/TLS), switch SSE auth to cookie-based session or WebSocket.
      const token = getToken();
      if (!token) {
        this.connected = false;
        return;
      }

      this.sse = openEventStream(token, (msg) => this.handleEvent(msg));
      this.connected = true;
    },

    stopStream() {
      if (this.sse) {
        try {
          this.sse.close();
        } catch {}
        this.sse = null;
      }
    },

    handleEvent(msg) {
      // You define these messages; keep UI “dumb”.
      // Example:
      // { type: "telemetry", data: {...} }
      // { type: "log", level: "info", line: "..." }
      if (!msg || typeof msg !== 'object') return;

      if (msg.type === 'telemetry' && msg.data) {
        this.overview = msg.data;
        this.lastUpdateAt = Date.now();
      }

      if (msg.type === 'log') {
        const line = msg.line ?? JSON.stringify(msg);
        this.appendLog(line);
      }
    },

    appendLog(line) {
      this.logLines.push(line);
      if (this.logLines.length > this.logMaxLines) {
        this.logLines.splice(0, this.logLines.length - this.logMaxLines);
      }

      // auto-scroll
      queueMicrotask(() => {
        const el = this.$refs.log;
        if (!el) return;
        el.scrollTop = el.scrollHeight;
      });
    },

    clearLog() {
      this.logLines = [];
    },

    async copyLog() {
      const text = this.logLines.join('\n');
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        // fallback
        const ta = document.createElement('textarea');
        ta.value = text;
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        ta.remove();
      }
    },
  };
};

window.appSearch = function appSearch() {
  // search.html
  return {
    loading: true,
    overview: null,

    async init() {
      this.overview = await apiGet('/api/search');
      this.loading = false;

      this.stream = openEventStream((msg) => this.handleEvent(msg));
    },

    handleEvent(msg) {
      if (msg.type === 'telemetry') {
        this.overview = msg.data;
      }
    },
  };
};
