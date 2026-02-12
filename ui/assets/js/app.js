import { apiGet, bootstrapToken, getToken, openStatusEventStream } from "./helpers.js";

window.indexApp = function indexApp() {
  return {
    loading: false,
    connected: false,
    error: "",
    token: "",
    status: null,
    sse: null,

    get prettyStatus() {
      if (!this.status) {
        return "{}";
      }
      return JSON.stringify(this.status, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = "";

      try {
        this.token = await bootstrapToken();
        await this.refreshStatus();
        this.startEvents();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async refreshStatus() {
      try {
        this.error = "";
        this.status = await apiGet("/status");
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
    message: "Search UI is next. Bootstrap is active on the overview page.",
    init() {
      // Keep token warm for future search views.
      bootstrapToken().catch(() => {
        this.message = "Unable to bootstrap token. Check /api/v1/dev/auth.";
      });
      if (getToken()) {
        this.message = "Token ready. Search page skeleton is in place.";
      }
    },
  };
};

