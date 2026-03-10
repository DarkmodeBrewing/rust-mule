import {
  apiGet,
  apiPatch,
  apiPost,
  applyThemeValue,
  bootstrapToken,
  currentTheme,
  loadSearchThreads,
  sessionControlMixin,
  setToken,
  splitLines,
} from '../app-core.js';

window.appSettings = function appSettings() {
  return {
    ...sessionControlMixin(),
    loading: false,
    saving: false,
    error: '',
    notice: '',
    searchThreads: [],
    status: null,
    theme: currentTheme(),
    settings: null,
    restartRequired: true,
    form: {
      samHost: '',
      samPort: 0,
      samSessionName: '',
      apiHost: '',
      apiPort: 0,
      logLevel: '',
      logToFile: true,
      logFileLevel: '',
      autoOpenUi: true,
      shareRootsText: '',
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
        this.theme = currentTheme();
        const ok = await this.checkSession();
        if (!ok) {
          window.location.replace('/auth');
          return;
        }
        await bootstrapToken();
        await this.refreshThreads();
        this.status = await apiGet('/status');
        await this.loadSettings();
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

    async loadSettings() {
      const resp = await apiGet('/settings');
      this.settings = resp?.settings || null;
      this.restartRequired = Boolean(resp?.restart_required);
      this.form.samHost = this.settings?.sam?.host || '';
      this.form.samPort = this.settings?.sam?.port || 0;
      this.form.samSessionName = this.settings?.sam?.session_name || '';
      this.form.apiHost = this.settings?.api?.host || '';
      this.form.apiPort = this.settings?.api?.port || 0;
      this.form.logLevel = this.settings?.general?.log_level || 'info';
      this.form.logToFile = Boolean(this.settings?.general?.log_to_file);
      this.form.logFileLevel = this.settings?.general?.log_file_level || 'debug';
      this.form.autoOpenUi = this.settings?.general?.auto_open_ui !== false;
      this.form.shareRootsText = Array.isArray(this.settings?.sharing?.share_roots)
        ? this.settings.sharing.share_roots.join('\n')
        : '';
    },

    async saveSettings() {
      this.saving = true;
      this.error = '';
      this.notice = '';
      try {
        const payload = {
          general: {
            log_level: this.form.logLevel,
            log_to_file: this.form.logToFile,
            log_file_level: this.form.logFileLevel,
            auto_open_ui: this.form.autoOpenUi,
          },
          sam: {
            host: this.form.samHost,
            port: Number(this.form.samPort),
            session_name: this.form.samSessionName,
          },
          api: {
            host: this.form.apiHost,
            port: Number(this.form.apiPort),
          },
          sharing: {
            share_roots: splitLines(this.form.shareRootsText),
          },
        };
        const resp = await apiPatch('/settings', payload);
        this.settings = resp?.settings || null;
        this.restartRequired = Boolean(resp?.restart_required);
        this.notice = this.restartRequired
          ? 'Settings saved. Restart required for full effect.'
          : 'Settings saved.';
        await this.loadSettings();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.saving = false;
      }
    },

    async rotateApiToken() {
      this.error = '';
      this.notice = '';
      try {
        const resp = await apiPost('/token/rotate', {});
        const token = resp?.token || '';
        if (!token) {
          throw new Error('token rotate response missing token');
        }
        setToken(token);
        const sessionResp = await fetch('/api/v1/session', {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}` },
        });
        if (!sessionResp.ok) {
          throw new Error(`session refresh failed: ${sessionResp.status}`);
        }
        this.notice = 'API token rotated and session refreshed.';
      } catch (err) {
        this.error = String(err?.message || err);
      }
    },
  };
};
