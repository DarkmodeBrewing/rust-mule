import {
  apiGet,
  bootstrapToken,
  buildPartGraphSegments,
  formatBytes,
  formatRate,
  formatUnixSecs,
  getToken,
  graphSegmentStyleValue,
  loadSearchThreads,
  sessionControlMixin,
} from '../app-core.js';

window.appDownloads = function appDownloads() {
  return {
    ...sessionControlMixin(),
    loading: false,
    error: '',
    notice: '',
    status: null,
    searchThreads: [],
    downloads: [],
    uploads: [],
    sharedFiles: [],
    sharedActions: [],
    sharedActionBusy: false,
    showDangerZone: false,
    dangerAcknowledged: false,

    get activeDownloadCount() {
      return this.downloads.filter((item) =>
        ['queued', 'downloading', 'paused', 'completing'].includes(item.state),
      ).length;
    },

    get activeSharedRequests() {
      return this.sharedFiles.filter((item) => item.active_request).length;
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
        await this.refreshData();
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

    async refreshData() {
      const [downloadsResp, uploadsResp, sharedResp] = await Promise.all([
        apiGet('/downloads'),
        apiGet('/uploads'),
        apiGet('/shared'),
      ]);
      this.downloads = Array.isArray(downloadsResp?.downloads)
        ? downloadsResp.downloads.map((item) => ({
            ...item,
            pretty_size: formatBytes(item.file_size),
            rate_label: `${formatRate(item.rate_bps_5s)} (5s) / ${formatRate(item.rate_bps_30s)} (30s)`,
            graph_segments: buildPartGraphSegments(
              item.file_size,
              item.missing_range_spans,
              item.inflight_range_spans,
              item.source_count,
            ),
          }))
        : [];
      this.uploads = Array.isArray(uploadsResp?.uploads)
        ? uploadsResp.uploads.map((item) => ({
            ...item,
            requested_bytes_total_pretty: formatBytes(item.requested_bytes_total || 0),
            rate_label: `${formatRate(item.rate_bps_5s)} (5s) / ${formatRate(item.rate_bps_30s)} (30s)`,
            zero_fill_rate_label: `${formatRate(item.zero_fill_rate_bps_5s)} (5s) / ${formatRate(item.zero_fill_rate_bps_30s)} (30s)`,
            zero_fill_requested_bytes_total_pretty: formatBytes(
              item.zero_fill_requested_bytes_total || 0,
            ),
            active_peer_ids_label: Array.isArray(item.active_peer_ids)
              ? item.active_peer_ids.join(', ')
              : '',
            held_ranges_label: (item.held_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            sending_ranges_label: (item.sending_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            last_requested_label: formatUnixSecs(item.last_requested_unix_secs),
            active_since_label: formatUnixSecs(item.active_since_unix_secs),
            payload_source_label: item.last_payload_source || 'unknown',
            sessions: Array.isArray(item.sessions)
              ? item.sessions.map((session) => ({
                  ...session,
                  bytes_total_pretty: formatBytes(session.bytes_total || 0),
                  started_label: formatUnixSecs(session.started_unix_secs),
                  updated_label: formatUnixSecs(session.last_updated_unix_secs),
                  payload_source_label: session.payload_source || 'unknown',
                  terminal_reason_label: session.terminal_reason || 'active',
                }))
              : [],
            recent_sessions: Array.isArray(item.recent_sessions)
              ? item.recent_sessions.map((session) => ({
                  ...session,
                  bytes_total_pretty: formatBytes(session.bytes_total || 0),
                  started_label: formatUnixSecs(session.started_unix_secs),
                  updated_label: formatUnixSecs(session.last_updated_unix_secs),
                  payload_source_label: session.payload_source || 'unknown',
                  terminal_reason_label: session.terminal_reason || 'unknown',
                  terminal_reason_class: this.uploadTerminalReasonClass(
                    session.terminal_reason,
                  ),
                }))
              : [],
            recent_session_groups: this.buildRecentSessionGroups(item.recent_sessions),
          }))
        : [];
      this.sharedFiles = Array.isArray(sharedResp?.files)
        ? sharedResp.files.map((item) => ({
            ...item,
            pretty_size: formatBytes(item.file_size),
            requested_bytes_total_pretty: formatBytes(item.requested_bytes_total || 0),
            local_source_label: item.local_source_cached ? 'cached' : 'not cached',
            source_publish_queue_label: item.source_publish_last_result
              ? `${item.source_publish_last_result} (${item.source_publish_attempts})`
              : 'not attempted',
            source_publish_response_label: item.source_publish_response_received
              ? `response seen${
                  typeof item.source_publish_first_response_latency_ms === 'number'
                    ? ` (${item.source_publish_first_response_latency_ms} ms)`
                    : ''
                }`
              : 'no response yet',
            keyword_publish_queue_label: item.keyword_publish_last_result
              ? `${item.keyword_publish_last_result} (${item.keyword_publish_queued}/${item.keyword_publish_attempts})`
              : 'not attempted',
            keyword_publish_ack_label:
              typeof item.keyword_publish_total === 'number' && item.keyword_publish_total > 0
                ? `${item.keyword_publish_acked}/${item.keyword_publish_total} acked`
                : 'no keyword publishes',
            queued_upload_ranges_label: (item.queued_upload_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
            inflight_upload_ranges_label: (item.inflight_upload_ranges || [])
              .map((range) => `${range.start}-${range.end}`)
              .join(', '),
          }))
        : [];
      await this.refreshSharedActions();
    },

    async refreshSharedActions() {
      const now = Math.floor(Date.now() / 1000);
      const actionsResp = await apiGet('/shared/actions');
      this.sharedActions = Array.isArray(actionsResp?.actions)
        ? actionsResp.actions
            .map((action) => ({
              ...action,
              cooldown_remaining_secs:
                typeof action.cooldown_until_unix_secs === 'number' &&
                action.cooldown_until_unix_secs > now
                  ? action.cooldown_until_unix_secs - now
                  : 0,
              summary:
                action.action === 'reindex'
                  ? `files=${action.library_files_total ?? 0}, reused=${action.reused_entries ?? 0}, hashed=${action.hashed_entries ?? 0}`
                  : `items=${action.items_total}, queued=${action.queued_total}, failed=${action.failed_total}`,
            }))
            .map((action) => ({
              ...action,
              summary:
                action.cooldown_remaining_secs > 0
                  ? `${action.summary}, cooldown=${action.cooldown_remaining_secs}s`
                  : action.summary,
            }))
        : [];
    },

    async runSharedAction(path, successNotice, confirmationText) {
      if (!this.dangerAcknowledged) {
        this.notice = 'acknowledge the danger zone before running shared maintenance actions';
        this.error = '';
        return;
      }
      if (!window.confirm(confirmationText)) {
        return;
      }
      this.sharedActionBusy = true;
      this.error = '';
      this.notice = '';
      try {
        const token = getToken();
        if (!token) {
          throw new Error('missing api token in sessionStorage');
        }
        const response = await fetch(`/api/v1${path}`, {
          method: 'POST',
          headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ confirm: true }),
        });
        const data = await response.json().catch(() => ({}));
        if (response.ok) {
          this.notice = data?.started
            ? successNotice
            : `${data?.status?.action || 'action'} is already running`;
        } else if (response.status === 409) {
          this.notice = `${data?.status?.action || 'action'} is already running`;
        } else if (response.status === 429) {
          this.notice = `${data?.status?.action || 'action'} is cooling down`;
        } else {
          this.error = data?.message || `${path}: ${response.status}`;
        }
        await this.refreshData();
      } catch (err) {
        this.error = String(err?.message || err);
      } finally {
        this.sharedActionBusy = false;
      }
    },

    async reindexSharedLibrary() {
      await this.runSharedAction(
        '/shared/actions/reindex',
        'reindex started',
        'Reindex Library will rescan all configured shared folders. Continue?',
      );
    },

    async republishSharedSources() {
      await this.runSharedAction(
        '/shared/actions/republish_sources',
        'source republish started',
        'Republish Sources will queue fresh source publish traffic for all indexed shared files. Continue?',
      );
    },

    async republishSharedKeywords() {
      await this.runSharedAction(
        '/shared/actions/republish_keywords',
        'keyword republish started',
        'Republish Keywords will queue fresh keyword publish traffic for all indexed shared files and may generate significant KAD traffic. Continue?',
      );
    },

    graphSegmentStyle(segment) {
      return graphSegmentStyleValue(segment);
    },
  };
};
