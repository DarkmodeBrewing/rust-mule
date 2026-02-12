import { apiGet, apiPost, bootstrapToken, openStatusEventStream } from "./helpers.js";

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
    loading: false,
    submitting: false,
    error: "",
    query: "",
    keywordIdHex: "",
    searchResponse: null,
    keywordResults: null,

    get activeKeywordIdHex() {
      return this.searchResponse?.keyword_id_hex || this.keywordIdHex.trim();
    },

    get prettySearchResponse() {
      if (!this.searchResponse) {
        return "{}";
      }
      return JSON.stringify(this.searchResponse, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = "";
      try {
        await bootstrapToken();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.loading = false;
      }
    },

    async submitSearch() {
      this.submitting = true;
      this.error = "";
      try {
        const payload = this.buildPayload();
        this.searchResponse = await apiPost("/kad/search_keyword", payload);
        await this.refreshResults();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.submitting = false;
      }
    },

    async refreshResults() {
      const keywordIdHex = this.activeKeywordIdHex;
      if (!keywordIdHex) {
        this.keywordResults = null;
        return;
      }
      this.keywordResults = await apiGet(`/kad/keyword_results/${keywordIdHex}`);
    },

    buildPayload() {
      const keywordIdHex = this.keywordIdHex.trim();
      if (keywordIdHex) {
        return { keyword_id_hex: keywordIdHex };
      }

      const query = this.query.trim();
      if (!query) {
        throw new Error("enter a keyword query or keyword id");
      }
      return { query };
    },
  };
};
