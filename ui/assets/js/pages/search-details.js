import {
  apiGet,
  bootstrapToken,
  loadSearchThreads,
  parseSearchIdFromQuery,
  sessionControlMixin,
  stateClass,
} from '../app-core.js';

window.appSearchDetails = function appSearchDetails() {
  return {
    ...sessionControlMixin(),
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

    get searchIdLabel() {
      return this.searchId || '(missing)';
    },

    async init() {
      this.loading = true;
      this.error = '';
      this.searchId = parseSearchIdFromQuery();

      try {
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
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
