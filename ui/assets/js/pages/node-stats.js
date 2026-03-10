import {
  apiGet,
  bootstrapToken,
  loadSearchThreads,
  nodeState,
  nodeStateClass,
  openStatusEventStream,
  sessionControlMixin,
} from '../app-core.js';

window.appNodeStats = function appNodeStats() {
  return {
    ...sessionControlMixin(),
    loading: false,
    connected: false,
    error: '',
    status: null,
    peers: [],
    searchThreads: [],
    sse: null,
    refreshTimer: null,
    charts: null,
    historyMaxPoints: 360,
    historyWindow: 60,
    historyPaused: false,
    historyWindows: [20, 60, 180, 360],
    history: {
      labels: [],
      totalHits: [],
      requestRate: [],
      responseRate: [],
      livePeers: [],
      idlePeers: [],
    },
    lastRateSample: null,

    get totalNodes() {
      return this.peers.length;
    },

    get liveNodes() {
      return this.peers.filter((p) => p.ui_state !== 'idle').length;
    },

    get activeNodes() {
      return this.peers.filter((p) => p.ui_state === 'active').length;
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
        await this.refresh();
        this.initCharts();
        this.captureSnapshot();
        this.startEvents();
        this.startPolling();
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
      this.status = await apiGet('/status');
    },

    async refreshPeers() {
      const peersResp = await apiGet('/kad/peers');
      const rawPeers = Array.isArray(peersResp?.peers) ? peersResp.peers : [];
      const normalizedPeers = rawPeers.map((peer) => {
        const state = nodeState(peer);
        const inbound = peer?.last_inbound_secs_ago;
        return {
          ...peer,
          ui_state: state,
          ui_state_class: nodeStateClass(state),
          inbound_label: typeof inbound === 'number' ? `${inbound}s` : '-',
        };
      });
      this.peers = normalizedPeers.slice().sort((a, b) => {
        const sa = a.ui_state;
        const sb = b.ui_state;
        const rank = { active: 0, live: 1, idle: 2 };
        if (rank[sa] !== rank[sb]) {
          return rank[sa] - rank[sb];
        }
        return (
          (a.last_seen_secs_ago ?? Number.MAX_SAFE_INTEGER) -
          (b.last_seen_secs_ago ?? Number.MAX_SAFE_INTEGER)
        );
      });
    },

    async refresh() {
      try {
        this.error = '';
        await Promise.all([
          this.refreshStatus(),
          this.refreshPeers(),
          this.refreshThreads(),
        ]);
        this.captureSnapshot();
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
            this.captureSnapshot();
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

    startPolling() {
      this.stopPolling();
      this.refreshTimer = setInterval(() => {
        this.refresh();
      }, 15000);
      window.addEventListener('beforeunload', () => {
        this.stopPolling();
        this.stopEvents();
        this.destroyCharts();
      });
    },

    stopPolling() {
      if (this.refreshTimer) {
        clearInterval(this.refreshTimer);
        this.refreshTimer = null;
      }
    },

    captureSnapshot() {
      if (!this.status) {
        return;
      }
      const now = Date.now();
      const label = new Date(now).toLocaleTimeString();

      const totalHits = this.searchThreads.reduce((acc, thread) => {
        const hits = Number(thread?.hits ?? 0);
        return acc + (Number.isFinite(hits) ? hits : 0);
      }, 0);

      const req = Number(this.status?.recv_req ?? 0);
      const res = Number(this.status?.recv_res ?? 0);
      let requestRate = 0;
      let responseRate = 0;
      if (this.lastRateSample) {
        const dt = (now - this.lastRateSample.ts) / 1000;
        if (dt > 0) {
          requestRate = Math.max(0, (req - this.lastRateSample.req) / dt);
          responseRate = Math.max(0, (res - this.lastRateSample.res) / dt);
        }
      }
      this.lastRateSample = { ts: now, req, res };

      if (this.historyPaused) {
        return;
      }

      const livePeers = this.peers.filter((p) => p.ui_state !== 'idle').length;
      const idlePeers = this.peers.length - livePeers;

      this.pushHistoryPoint(
        label,
        totalHits,
        requestRate,
        responseRate,
        livePeers,
        idlePeers,
      );
      this.updateCharts();
    },

    pushHistoryPoint(
      label,
      totalHits,
      requestRate,
      responseRate,
      livePeers,
      idlePeers,
    ) {
      this.history.labels.push(label);
      this.history.totalHits.push(totalHits);
      this.history.requestRate.push(Number(requestRate.toFixed(2)));
      this.history.responseRate.push(Number(responseRate.toFixed(2)));
      this.history.livePeers.push(livePeers);
      this.history.idlePeers.push(idlePeers);

      while (this.history.labels.length > this.historyMaxPoints) {
        this.history.labels.shift();
        this.history.totalHits.shift();
        this.history.requestRate.shift();
        this.history.responseRate.shift();
        this.history.livePeers.shift();
        this.history.idlePeers.shift();
      }
    },

    chartColor(varName, fallback) {
      const value = getComputedStyle(document.documentElement)
        .getPropertyValue(varName)
        .trim();
      return value || fallback;
    },

    initCharts() {
      if (!window.Chart) {
        this.error = 'chart.js is not available';
        return;
      }
      this.destroyCharts();
      const ChartRef = window.Chart;
      const signal = this.chartColor('--signal', '#4aa3ff');
      const running = this.chartColor('--state-running', '#38b000');
      const done = this.chartColor('--state-done', '#f4a261');
      const idle = this.chartColor('--state-idle', '#6c757d');

      const commonOptions = {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
      };

      this.charts = {};
      this.charts.hits = new ChartRef(this.$refs.hitsChart, {
        type: 'line',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Total Hits',
              data: this.history.totalHits,
              borderColor: signal,
              backgroundColor: signal,
              tension: 0.25,
            },
          ],
        },
        options: commonOptions,
      });

      this.charts.rate = new ChartRef(this.$refs.rateChart, {
        type: 'line',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Requests / sec',
              data: this.history.requestRate,
              borderColor: running,
              backgroundColor: running,
              tension: 0.25,
            },
            {
              label: 'Responses / sec',
              data: this.history.responseRate,
              borderColor: done,
              backgroundColor: done,
              tension: 0.25,
            },
          ],
        },
        options: commonOptions,
      });

      this.charts.peers = new ChartRef(this.$refs.peersChart, {
        type: 'bar',
        data: {
          labels: this.history.labels,
          datasets: [
            {
              label: 'Live',
              data: this.history.livePeers,
              backgroundColor: done,
              stack: 'peers',
            },
            {
              label: 'Idle',
              data: this.history.idlePeers,
              backgroundColor: idle,
              stack: 'peers',
            },
          ],
        },
        options: {
          ...commonOptions,
          scales: {
            x: { stacked: true },
            y: { stacked: true, beginAtZero: true },
          },
        },
      });
    },

    updateCharts() {
      if (!this.charts) {
        return;
      }
      const n = Math.max(1, Number(this.historyWindow || 60));
      const labels = this.history.labels.slice(-n);
      this.charts.hits.data.labels = labels;
      this.charts.hits.data.datasets[0].data = this.history.totalHits.slice(-n);
      this.charts.rate.data.labels = labels;
      this.charts.rate.data.datasets[0].data = this.history.requestRate.slice(-n);
      this.charts.rate.data.datasets[1].data = this.history.responseRate.slice(-n);
      this.charts.peers.data.labels = labels;
      this.charts.peers.data.datasets[0].data = this.history.livePeers.slice(-n);
      this.charts.peers.data.datasets[1].data = this.history.idlePeers.slice(-n);
      this.charts.hits.update();
      this.charts.rate.update();
      this.charts.peers.update();
    },

    destroyCharts() {
      if (!this.charts) {
        return;
      }
      Object.values(this.charts).forEach((chart) => chart.destroy());
      this.charts = null;
    },

    toggleHistoryPause() {
      this.historyPaused = !this.historyPaused;
    },

    resetHistory() {
      this.history.labels = [];
      this.history.totalHits = [];
      this.history.requestRate = [];
      this.history.responseRate = [];
      this.history.livePeers = [];
      this.history.idlePeers = [];
      this.lastRateSample = null;
      this.updateCharts();
    },
  };
};
