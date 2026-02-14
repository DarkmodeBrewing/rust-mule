import { createReadStream, existsSync } from 'node:fs';
import { stat } from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const uiRoot = path.resolve(__dirname, '..', '..');

const port = Number(process.env.UI_MOCK_PORT || 17835);
const host = '127.0.0.1';

const STATUS_PAYLOAD = {
  uptime_secs: 120,
  routing: 10,
  live: 3,
  live_10m: 2,
  pending: 1,
  recv_req: 1,
  recv_res: 1,
  sent_reqs: 1,
  recv_ress: 1,
  res_contacts: 0,
  dropped_undecipherable: 0,
  dropped_unparsable: 0,
  recv_hello_reqs: 0,
  sent_bootstrap_reqs: 0,
  recv_bootstrap_ress: 0,
  bootstrap_contacts: 0,
  sent_hellos: 0,
  recv_hello_ress: 0,
  sent_hello_acks: 0,
  recv_hello_acks: 0,
  hello_ack_skipped_no_sender_key: 0,
  timeouts: 0,
  new_nodes: 0,
  evicted: 0,
  sent_search_source_reqs: 1,
  recv_search_source_reqs: 1,
  recv_search_source_decode_failures: 0,
  source_search_hits: 1,
  source_search_misses: 0,
  source_search_results_served: 1,
  recv_search_ress: 1,
  search_results: 1,
  new_sources: 1,
  sent_search_key_reqs: 0,
  recv_search_key_reqs: 0,
  keyword_results: 1,
  new_keyword_results: 1,
  evicted_keyword_hits: 0,
  evicted_keyword_keywords: 0,
  keyword_keywords_tracked: 1,
  keyword_hits_total: 1,
  store_keyword_keywords: 1,
  store_keyword_hits_total: 1,
  source_store_files: 1,
  source_store_entries_total: 1,
  recv_publish_key_reqs: 0,
  recv_publish_key_decode_failures: 0,
  sent_publish_key_ress: 0,
  sent_publish_key_reqs: 0,
  recv_publish_key_ress: 0,
  new_store_keyword_hits: 0,
  evicted_store_keyword_hits: 0,
  evicted_store_keyword_keywords: 0,
  sent_publish_source_reqs: 1,
  recv_publish_source_reqs: 1,
  recv_publish_source_decode_failures: 0,
  sent_publish_source_ress: 1,
  new_store_source_entries: 1,
  recv_publish_ress: 1,
  source_search_batch_candidates: 8,
  source_search_batch_skipped_version: 1,
  source_search_batch_sent: 6,
  source_search_batch_send_fail: 1,
  source_publish_batch_candidates: 6,
  source_publish_batch_skipped_version: 1,
  source_publish_batch_sent: 4,
  source_publish_batch_send_fail: 1,
  source_probe_first_publish_responses: 1,
  source_probe_first_search_responses: 1,
  source_probe_search_results_total: 1,
  source_probe_publish_latency_ms_total: 10,
  source_probe_search_latency_ms_total: 20,
};

const SEARCH_ID = '00112233445566778899aabbccddeeff';
const SETTINGS_PAYLOAD = {
  settings: {
    sam: { host: '127.0.0.1', port: 7656, session_name: 'rust-mule' },
    api: { port: 17835 },
    general: {
      log_level: 'info',
      log_to_file: true,
      log_file_level: 'debug',
      auto_open_ui: false,
    },
  },
  restart_required: true,
};

function contentType(filePath) {
  if (filePath.endsWith('.html')) return 'text/html; charset=utf-8';
  if (filePath.endsWith('.css')) return 'text/css; charset=utf-8';
  if (filePath.endsWith('.js') || filePath.endsWith('.mjs')) {
    return 'text/javascript; charset=utf-8';
  }
  if (filePath.endsWith('.json')) return 'application/json; charset=utf-8';
  if (filePath.endsWith('.png')) return 'image/png';
  if (filePath.endsWith('.svg')) return 'image/svg+xml';
  return 'application/octet-stream';
}

function sendJson(res, code, payload) {
  res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8' });
  res.end(JSON.stringify(payload));
}

async function serveFile(res, filePath) {
  if (!existsSync(filePath)) {
    res.writeHead(404);
    res.end('not found');
    return;
  }
  const fileStat = await stat(filePath);
  if (!fileStat.isFile()) {
    res.writeHead(404);
    res.end('not found');
    return;
  }
  res.writeHead(200, { 'Content-Type': contentType(filePath) });
  createReadStream(filePath).pipe(res);
}

function mapUiPath(urlPath) {
  if (urlPath === '/' || urlPath === '/index.html' || urlPath === '/ui' || urlPath === '/ui/') {
    return path.join(uiRoot, 'index.html');
  }
  if (urlPath === '/ui/search') return path.join(uiRoot, 'search.html');
  if (urlPath === '/ui/search_details') return path.join(uiRoot, 'search_details.html');
  if (urlPath === '/ui/node_stats') return path.join(uiRoot, 'node_stats.html');
  if (urlPath === '/ui/log') return path.join(uiRoot, 'log.html');
  if (urlPath === '/ui/settings') return path.join(uiRoot, 'settings.html');
  if (urlPath.startsWith('/ui/assets/')) {
    return path.join(uiRoot, urlPath.replace('/ui/', ''));
  }
  if (urlPath.startsWith('/assets/')) {
    return path.join(uiRoot, urlPath.slice(1));
  }
  if (urlPath === '/search.html') return path.join(uiRoot, 'search.html');
  if (urlPath === '/search_details.html') return path.join(uiRoot, 'search_details.html');
  if (urlPath === '/node_stats.html') return path.join(uiRoot, 'node_stats.html');
  if (urlPath === '/log.html') return path.join(uiRoot, 'log.html');
  if (urlPath === '/settings.html') return path.join(uiRoot, 'settings.html');
  return null;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || '/', `http://${host}:${port}`);
  const p = url.pathname;

  if (p === '/auth') {
    res.writeHead(302, { Location: '/index.html' });
    res.end();
    return;
  }

  if (p === '/api/v1/events') {
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    });
    res.write(`event: status\ndata: ${JSON.stringify(STATUS_PAYLOAD)}\n\n`);
    const timer = setInterval(() => {
      res.write(`event: status\ndata: ${JSON.stringify(STATUS_PAYLOAD)}\n\n`);
    }, 1000);
    req.on('close', () => clearInterval(timer));
    return;
  }

  if (p === '/api/v1/auth/bootstrap') return sendJson(res, 200, { token: 'test-token' });
  if (p === '/api/v1/session/check') return sendJson(res, 200, { ok: true });
  if (p === '/api/v1/session/logout' && req.method === 'POST') return sendJson(res, 200, { ok: true });
  if (p === '/api/v1/session' && req.method === 'POST') return sendJson(res, 200, { ok: true });
  if (p === '/api/v1/token/rotate' && req.method === 'POST') return sendJson(res, 200, { token: 'rotated-token', sessions_cleared: true });
  if (p === '/api/v1/status') return sendJson(res, 200, STATUS_PAYLOAD);
  if (p === '/api/v1/searches') {
    return sendJson(res, 200, {
      searches: [
        {
          search_id_hex: SEARCH_ID,
          keyword_id_hex: SEARCH_ID,
          state: 'running',
          created_secs_ago: 1,
          hits: 1,
          want_search: true,
          publish_enabled: false,
          got_publish_ack: false,
        },
      ],
    });
  }
  if (p === `/api/v1/searches/${SEARCH_ID}`) {
    return sendJson(res, 200, {
      search: {
        search_id_hex: SEARCH_ID,
        keyword_id_hex: SEARCH_ID,
        state: 'running',
        created_secs_ago: 1,
        hits: 1,
      },
      hits: [],
    });
  }
  if (p === '/api/v1/kad/search_keyword' && req.method === 'POST') {
    return sendJson(res, 200, { keyword_id_hex: SEARCH_ID });
  }
  if (p.startsWith('/api/v1/kad/keyword_results/')) {
    const id = p.split('/').pop() || SEARCH_ID;
    return sendJson(res, 200, { keyword_id_hex: id, hits: [] });
  }
  if (p === '/api/v1/kad/peers') return sendJson(res, 200, { peers: [] });
  if (p === '/api/v1/settings') return sendJson(res, 200, SETTINGS_PAYLOAD);
  if (p === '/api/v1/settings' && req.method === 'PATCH') return sendJson(res, 200, SETTINGS_PAYLOAD);

  const filePath = mapUiPath(p);
  if (filePath) {
    await serveFile(res, filePath);
    return;
  }

  res.writeHead(302, { Location: '/index.html' });
  res.end();
});

server.listen(port, host, () => {
  process.stdout.write(`mock ui server listening at http://${host}:${port}\n`);
});
