// ==============================
// Auth
// ==============================

export function getToken() {
  return sessionStorage.getItem('token');
}

export function setToken(token) {
  sessionStorage.setItem('token', token);
}

export function clearToken() {
  sessionStorage.removeItem('token');
}

export function authHeaders(extra = {}) {
  const token = getToken();
  return token ? { ...extra, Authorization: `Bearer ${token}` } : extra;
}

// ==============================
// API
// ==============================

async function handleResponse(res) {
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${res.status} ${text}`);
  }
  return res.json();
}

export async function apiGet(path) {
  const res = await fetch(path, {
    headers: authHeaders(),
  });
  return handleResponse(res);
}

export async function apiPost(path, body) {
  const res = await fetch(path, {
    method: 'POST',
    headers: authHeaders({ 'Content-Type': 'application/json' }),
    body: JSON.stringify(body),
  });
  return handleResponse(res);
}

export async function apiPut(path, body) {
  const res = await fetch(path, {
    method: 'PUT',
    headers: authHeaders({ 'Content-Type': 'application/json' }),
    body: JSON.stringify(body),
  });
  return handleResponse(res);
}

export async function apiPatch(path, body) {
  const res = await fetch(path, {
    method: 'PATCH',
    headers: authHeaders({ 'Content-Type': 'application/json' }),
    body: JSON.stringify(body),
  });
  return handleResponse(res);
}

// ==============================
// SSE
// ==============================

export function openEventStream(onEvent) {
  const es = new EventSource('/api/events');

  es.onmessage = (e) => {
    try {
      onEvent(JSON.parse(e.data));
    } catch (err) {
      console.warn('Bad SSE payload', err);
    }
  };

  es.onerror = () => {
    console.warn('SSE connection lost');
  };

  return es;
}

// ==============================
// Utils
// ==============================

export function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

export function formatNumber(n) {
  return new Intl.NumberFormat().format(n);
}

export function formatDuration(ms) {
  if (ms < 1000) return `${ms} ms`;
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return m > 0 ? `${m}m ${s % 60}s` : `${s}s`;
}

export function setTheme(theme) {
  localStorage.setItem('theme', theme);
}

export function getTheme() {
  theme = localStorage.getItem('theme');

  if (stringNotEmptyAndDefined(theme)) {
    return 'dark';
  }

  return theme;
}

export function changeTheme(theme) {
  document.documentElement.dataset.theme = stringNotEmptyAndDefined(theme)
    ? theme
    : 'dark';
}

function stringNotEmptyAndDefined(string) {
  return !string || string == null || string == '';
}
