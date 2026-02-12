const API_BASE = '/api/v1';
const TOKEN_KEY = 'api_token';

export function getToken() {
  return sessionStorage.getItem(TOKEN_KEY) || '';
}

export function setToken(token) {
  sessionStorage.setItem(TOKEN_KEY, token);
}

export async function bootstrapToken() {
  const existing = getToken();
  if (existing) {
    return existing;
  }

  const res = await fetch(`${API_BASE}/dev/auth`);
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`dev auth failed: ${res.status} ${body}`);
  }
  const data = await res.json();
  if (!data?.token) {
    throw new Error('dev auth response missing token');
  }

  setToken(data.token);
  return data.token;
}

function authHeaders() {
  const token = getToken();
  if (!token) {
    throw new Error('missing api token in sessionStorage');
  }
  return { Authorization: `Bearer ${token}` };
}

export async function apiGet(path) {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: authHeaders(),
  });
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`${path}: ${res.status} ${body}`);
  }
  return res.json();
}

export async function apiPost(path, body) {
  const res = await fetch(`${API_BASE}${path}`, {
    method: 'POST',
    headers: {
      ...authHeaders(),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${path}: ${res.status} ${text}`);
  }
  return res.json();
}

export async function apiDelete(path) {
  const res = await fetch(`${API_BASE}${path}`, {
    method: 'DELETE',
    headers: authHeaders(),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${path}: ${res.status} ${text}`);
  }
  return res.json();
}

export async function apiPatch(path, body) {
  const res = await fetch(`${API_BASE}${path}`, {
    method: 'PATCH',
    headers: {
      ...authHeaders(),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${path}: ${res.status} ${text}`);
  }
  return res.json();
}

export function openStatusEventStream(onStatus, onError) {
  const es = new EventSource(`${API_BASE}/events`);
  es.addEventListener('status', (event) => {
    try {
      onStatus(JSON.parse(event.data));
    } catch (err) {
      onError?.(`bad status payload: ${String(err)}`);
    }
  });
  es.onerror = async () => {
    try {
      const resp = await fetch(`${API_BASE}/session/check`);
      if (resp.status === 401 || resp.status === 403) {
        window.location.replace('/auth');
        return;
      }
    } catch (_err) {
      // ignore probe failures; surface generic disconnect below
    }
    onError?.('events stream disconnected');
  };
  return es;
}
