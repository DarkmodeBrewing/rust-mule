import {
  apiGet,
  apiPost,
  bootstrapToken,
  loadSearchThreads,
  sessionControlMixin,
} from '../app-core.js';

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
      this.keywordResults = await apiGet(`/kad/keyword_results/${keywordIdHex}`);
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
