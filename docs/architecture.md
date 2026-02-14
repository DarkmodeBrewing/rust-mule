# Architecture Notes

## Goal

Keep `rust-mule` maintainable as we grow from “networking prototype” into a full iMule-like client:

- Backend: Rust networking core (SAM/I2P + KAD/DHT + downloads/search/publish logic).
- Frontend: Lightweight GUI that talks to the backend over a local HTTP API (REST + streaming).

This separation lets us iterate on the GUI without risking the networking core, and makes it
possible to run the backend headless (server/CLI) while still having a rich UI.

## Layers

1. **Core / Backend (crate)**
   - Modules under `src/i2p/`, `src/kad/`, `src/nodes/` implement the protocol and storage.
   - `src/app.rs` wires config + services together.

2. **Local HTTP API (control plane)**
   - `src/api/` implements a small HTTP server intended to be bound locally by default.
   - The API exposes:
     - **REST** for request/response actions (later: search, publish, add download, etc.).
     - **SSE** for live updates (status, peer events, search results, progress).

3. **GUI**
   - Initial skeleton is wired and served by the backend at `/` and `/ui/...`.
   - Current bootstrap flow fetches token via `GET /api/v1/auth/bootstrap` (loopback-only), creates an HTTP-only frontend session via `POST /api/v1/session`, stores the token in `sessionStorage` for API calls, then uses:
     - `GET /api/v1/status` for a snapshot
     - `GET /api/v1/events` for continuous updates

## HTTP API

### Config

`config.toml`:

- `[api].port` (default `17835`)
- `[api].enable_debug_endpoints` (default `true`)
- `[api].auth_mode` (default `local_ui`)
- `[api].rate_limit_enabled` (default `true`)
- `[api].rate_limit_window_secs` (default `60`)
- `[api].rate_limit_auth_bootstrap_max_per_window` (default `30`)
- `[api].rate_limit_session_max_per_window` (default `30`)
- `[api].rate_limit_token_rotate_max_per_window` (default `10`)

### Auth / Token

On first start, the backend creates a token file:

- `data/api.token`

Clients must send:

- `Authorization: Bearer <token>`

Additionally for browser frontend routes:

- UI pages and assets require a valid `rm_session` HTTP-only cookie.
- Session cookie is created via `POST /api/v1/session` (authenticated with bearer token).
- `GET /api/v1/events` (SSE) uses session-cookie auth (no token query parameter).

Notes:

- We do **not** print the token to logs. The GUI should read it from `data/api.token`.
- On Unix we attempt to set file permissions to `0600` best-effort; on Windows we skip this.
- CORS is restricted to loopback origins only (`localhost`, `127.0.0.1`, and loopback IPs), with
  `Authorization` and `Content-Type` as the allowed request headers.
- Logging policy:
  - `INFO` is concise operator status (startup/readiness/periodic summaries).
  - Detailed protocol and per-peer diagnostics are kept in `DEBUG`/`TRACE`.
  - Daily file logs are rotated with `prefix.YYYY-MM-DD.suffix` naming (default: `rust-mule.YYYY-MM-DD.log`).
  - Startup prunes matching log files older than 30 days.

### Endpoints

- `GET /api/v1/auth/bootstrap`
  - No auth.
  - Loopback-only.
  - Subject to API rate limiting when enabled.
  - Returns `{ "token": "<bearer token>" }` for local UI bootstrap.

- `POST /api/v1/session`
  - Bearer auth required.
  - Subject to API rate limiting when enabled.
  - Issues `Set-Cookie: rm_session=...; HttpOnly; SameSite=Strict; Path=/`.
  - Session TTL currently defaults to 8 hours.
  - Expired sessions are cleaned lazily on validation/create and by a periodic background sweep.
  - Used by browser UI bootstrap so frontend routes and SSE can be session-authenticated.

- `POST /api/v1/token/rotate`
  - Bearer auth required.
  - Subject to API rate limiting when enabled.
  - Rotates the bearer token persisted in `data/api.token`.
  - Updates in-memory API auth token and invalidates all active frontend sessions.
  - Returns the new token for immediate client-side session/token refresh.

- `GET /api/v1/session/check`
  - Session-cookie auth required.
  - Returns `{ "ok": true }` when session is valid.
  - Used by frontend to detect session expiry and redirect to `/auth`.

- `POST /api/v1/session/logout`
  - Session-cookie auth required.
  - Invalidates current session server-side and clears `rm_session` cookie.

- `GET /api/v1/health`
  - No auth.
  - Returns `{ "ok": true }`.

- `GET /api/v1/status`
  - Auth required.
  - Returns the latest KAD service status snapshot (or `503` until the service has started).

- `GET /api/v1/searches`
  - Auth required.
  - Returns currently active keyword-search jobs tracked by the KAD service.

- `GET /api/v1/searches/:search_id`
  - Auth required.
  - Returns one active keyword-search job plus its current in-memory hits.
  - `search_id` is the keyword hash hex used for the search job.

- `POST /api/v1/searches/:search_id/stop`
  - Auth required.
  - Stops an active keyword-search job from continuing search/publish activity.

- `DELETE /api/v1/searches/:search_id`
  - Auth required.
  - Deletes an active keyword-search job.
  - Supports `?purge_results=true|false` (default `true`) to control whether cached keyword hits are removed.

- `GET /api/v1/settings`
  - Returns API-backed settings snapshot (`general`, `sam`, `api`) read from in-memory config state.
  - `general` currently includes: `log_level`, `log_to_file`, `log_file_level`, `auto_open_ui`.
  - Intended for settings page bootstrapping and refresh.

- `PATCH /api/v1/settings`
  - Persists selected settings updates into `config.toml`.
  - Supports toggling `general.auto_open_ui` for headless operation.
  - Validates host/port/log-filter inputs.
  - Response includes updated snapshot and `restart_required=true`.

- `GET /api/v1/events`
  - Session-cookie auth required.
  - SSE stream of live status updates.
  - Events are sent as:
    - `event: status`
    - `data: <json KadServiceStatus>`

- `POST /api/v1/kad/search_sources`
  - Auth required.
  - Body: `{ "file_id_hex": "<32 hex chars>", "file_size": 123 }`
  - Enqueues a conservative Kad2 `KADEMLIA2_SEARCH_SOURCE_REQ` against a few closest known peers.

- `GET /api/v1/kad/sources/:file_id_hex`
  - Auth required.
  - Returns sources learned so far for that fileID (in-memory, not yet persisted).

- `POST /api/v1/kad/publish_source`
  - Auth required.
  - Body: `{ "file_id_hex": "<32 hex chars>", "file_size": 123 }`
  - Enqueues a conservative Kad2 `KADEMLIA2_PUBLISH_SOURCE_REQ` advertising *this node* as a source.

- `POST /api/v1/kad/search_keyword`
  - Auth required.
  - Body: `{ "query": "some words" }`
  - Enqueues a conservative Kad2 `KADEMLIA2_SEARCH_KEY_REQ` for an iMule-style keyword hash.
  - Currently uses the **first extracted keyword word** (iMule behavior).

- `POST /api/v1/kad/publish_keyword`
  - Auth required.
  - Body: `{ "query": "some words", "file_id_hex": "<32 hex chars>", "filename": "...", "file_size": 123, "file_type": "Pro" }`
  - Enqueues a conservative Kad2 `KADEMLIA2_PUBLISH_KEY_REQ` publishing keyword->file metadata to the DHT.

- `GET /api/v1/kad/keyword_results/:keyword_id_hex`
  - Auth required.
  - Returns keyword hits learned so far for that keyword hash (in-memory, not yet persisted).

- `GET /api/v1/kad/peers`
  - Auth required.
  - Returns a routing table snapshot (peer IDs, liveness ages, failures, and optional agent string).

Example:

```bash
TOKEN="$(cat data/api.token)"
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/status | jq .
curl -i -sS -X POST -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/session
curl -N  -sS --cookie "rm_session=<session-id>" http://127.0.0.1:17835/api/v1/events

# File/source actions (hex is 16 bytes / 32 hex chars).
curl -sS -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"file_id_hex":"00112233445566778899aabbccddeeff","file_size":0}' \
  http://127.0.0.1:17835/api/v1/kad/search_sources
```

For a maintained collection of `curl` commands, see `docs/api_curl.md`.

## TLS (Future)

For local-only GUI usage, the token + loopback binding is usually sufficient.

If we later want to support remote GUI connections, we should add TLS:

- Generate a self-signed cert/key on first launch and store under `data/` (or allow users to
  provide their own).
- Keep token-based auth even with TLS (defense in depth).

TODOs:

- Add a `rust-mule` startup mode that prints the API bind address and a one-liner for tunneling
  (SSH `-L`) to make remote dev easier without opening firewalls.
- Add TLS support (self-signed by default) for remote GUI scenarios.
- Add finer-grained auth scopes (read-only vs control) if we later expose mutating endpoints
  (downloads/search/publish).

## Data Storage (Future)

Right now runtime state is stored in simple files under `data/` (e.g. `nodes.dat`, `sam.keys`).
This is intentionally easy to inspect, portable, and matches iMule/aMule conventions where it
makes sense.

As we add "client features" (search history, file hashes/metadata, downloads, sources, and
possibly cached publish/search results), it may be worth introducing SQLite:

- Pros: structured queries, indexing, transactions/atomicity, and easier schema evolution.
- Cons: adds a dependency + migrations, and can complicate “just inspect the state” workflows.

If/when we adopt SQLite, we should keep compatibility files like `data/nodes.dat` alongside it,
and store higher-level application state (downloads/search/etc.) in the database.

## Caching / Memory (Notes)

- Keyword search hits (discovering `file_id_hex`) are treated as a bounded, TTL’d in-memory cache.
- File sources are expected to be more “intermittent”; plan is to keep them longer and track `last_seen`,
  plus bounds/persistence as we implement downloads.
