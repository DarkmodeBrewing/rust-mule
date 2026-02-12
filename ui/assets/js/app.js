import { apiGet, apiPost, bootstrapToken, openStatusEventStream } from "./helpers.js";

function parseSearchIdFromQuery() {
  const params = new URLSearchParams(window.location.search);
  return (params.get("searchId") || "").trim();
}

function stateClass(state) {
  if (state === "running") {
    return "state-running";
  }
  if (state === "complete") {
    return "state-done";
  }
  return "state-idle";
}

async function loadSearchThreads(ctx) {
  const data = await apiGet("/searches");
  ctx.searchThreads = Array.isArray(data?.searches) ? data.searches : [];
}

window.indexApp = function indexApp() {
  return {
    loading: false,
    connected: false,
    error: "",
    token: "",
    status: null,
    sse: null,
    searchThreads: [],

    threadStateClass(state) {
      return stateClass(state);
    },

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
        this.error = "";
        this.status = await apiGet("/status");
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
    error: "",
    query: "",
    keywordIdHex: "",
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
        return "{}";
      }
      return JSON.stringify(this.searchResponse, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = "";
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
      this.error = "";
      try {
        const payload = this.buildPayload();
        this.searchResponse = await apiPost("/kad/search_keyword", payload);
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

window.appSearchDetails = function appSearchDetails() {
  return {
    loading: false,
    error: "",
    searchId: "",
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
        return "{}";
      }
      return JSON.stringify(this.details, null, 2);
    },

    async init() {
      this.loading = true;
      this.error = "";
      this.searchId = parseSearchIdFromQuery();

      try {
        await bootstrapToken();
        await this.refreshThreads();
        if (!this.searchId) {
          throw new Error("missing searchId query parameter");
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
