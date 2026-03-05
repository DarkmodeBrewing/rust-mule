Status: ACTIVE
Last Reviewed: 2026-03-04

# rust-mule API Design Specification (High Level)

**Search-centric API for a Chat-style UI**

## Overview

This document describes a high-level HTTP API for rust-mule that supports:

- persistent **keyword searches** (UI “chat threads”)
- starting/stopping/rerunning searches
- reporting **progress, telemetry, and results**
- a global **event stream** for live UI updates (SSE)
- simple operational endpoints (health, version, routing, logs)

The API is designed for:

- localhost-first usage (127.0.0.1)
- later headless deployments behind TLS/reverse proxy/VPN
- a lightweight UI (HTML + Alpine.js) with minimal client logic

This document includes both:

- currently implemented API behavior (`/api/v1`, source of truth: `src/api/router.rs`), and
- forward-looking endpoint ideas for later milestones.

For executable endpoint usage, prefer `docs/30_operations/api_curl.md`.

---

## Design Principles

- **Stable IDs**: all primary entities use stable identifiers.
- **Snapshot + Stream**: UI fetches snapshots (REST) and receives updates (SSE).
- **Simple writes**: commands are `POST` actions; state is read via `GET`.
- **Idempotence where possible**: repeated commands should not corrupt state.
- **Server computes**: progress and derived metrics are computed server-side.

---

## Base URL & Versioning

- Base: `/api`
- Versioning options:
  - v1 in path: `/api/v1/...` (recommended for public releases)
  - or implicit v1 with `X-Api-Version: 1`

This spec assumes **v1 in path**.

---

## Authentication (Localhost v1)

### Goals

- Keep it simple for localhost.
- Avoid exposing mint-token endpoints to remote callers.

### Token/session model (implemented)

- UI bootstrap gets a local bearer token from `GET /api/v1/auth/bootstrap` (loopback-only).
- Browser session is established via `POST /api/v1/session` and `rm_session` HTTP-only cookie.
- REST endpoints use `Authorization: Bearer <token>`.
- SSE (`GET /api/v1/events`) uses session-cookie auth.
- Token query parameters for SSE are not used.

### Debug endpoint hardening (planned)

- Debug endpoints remain behind `[api].enable_debug_endpoints = true`.
- Add a second-factor debug secret gate:
  - config: `api.debug_token`
  - header: `X-Debug-Token`
- Expected behavior:
  - debug disabled: `404`
  - debug enabled + missing/invalid debug token: `403`
- Keep standard auth/session checks in place; debug token is additive defense-in-depth.
- Debug token lifecycle policy:
  - do not auto-delete/rotate `api.debug_token` when debug endpoints are disabled
  - when debug is disabled, token is inert (ignored at request path) and all debug routes stay `404`
  - token rotation should be explicit admin action, not startup side-effect
  - token must be redacted from logs and from any future `--print-effective-config` output

### Endpoints

- `GET /api/v1/auth/bootstrap`
  - **Loopback-only**
  - returns `{ "token": "..." }`
  - used by UI to bootstrap a local session

### Notes for later (headless)

For remote/headless deployments prefer:

- reverse proxy auth (Basic/OIDC) + upstream to rust-mule
- VPN-only access with API bound to loopback
- cookie sessions and/or upstream auth gateways

---

## Core Resources

### Search (primary UI entity)

Represents a keyword search and its current/latest run.

**Search fields (conceptual)**

- `id` (string/uuid)
- `keyword` (string)
- `state` (`idle | running | complete | stopped | error`)
- `created_at`, `updated_at`
- `active_run_id` (optional)
- `last_run_summary` (optional)

### SearchRun (optional v1, useful later)

Represents a single run of a search with its own timeline and results.

---

## Endpoints

### Implemented Endpoint Surface (Current `/api/v1`)

- Auth/session:
  - `GET /api/v1/auth/bootstrap` (local-ui mode only)
  - `POST /api/v1/session`
  - `GET /api/v1/session/check`
  - `POST /api/v1/session/logout`
  - `POST /api/v1/token/rotate`
- Core:
  - `GET /api/v1/health`
  - `GET /api/v1/status`
  - `GET /api/v1/events` (SSE; session-cookie auth)
  - `GET /api/v1/settings`
  - `PATCH /api/v1/settings`
- Searches:
  - `GET /api/v1/searches`
  - `GET /api/v1/searches/{search_id}`
  - `POST /api/v1/searches/{search_id}/stop`
  - `DELETE /api/v1/searches/{search_id}?purge_results=true|false`
- Downloads:
  - `GET /api/v1/downloads`
  - `POST /api/v1/downloads`
  - `POST /api/v1/downloads/{part_number}/pause`
  - `POST /api/v1/downloads/{part_number}/resume`
  - `POST /api/v1/downloads/{part_number}/cancel`
  - `DELETE /api/v1/downloads/{part_number}`
- KAD:
  - `GET /api/v1/kad/peers`
  - `GET /api/v1/kad/sources/{file_id_hex}`
  - `GET /api/v1/kad/keyword_results/{keyword_id_hex}`
  - `POST /api/v1/kad/search_sources`
  - `POST /api/v1/kad/search_keyword`
  - `POST /api/v1/kad/publish_source`
  - `POST /api/v1/kad/publish_keyword`
- Debug (only when `[api].enable_debug_endpoints = true`):
  - `GET /api/v1/debug/routing/summary`
  - `GET /api/v1/debug/routing/buckets`
  - `GET /api/v1/debug/routing/nodes?bucket=N`
  - `POST /api/v1/debug/lookup_once`
  - `POST /api/v1/debug/probe_peer`
  - `POST /api/v1/debug/trace_lookup` (planned; see `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`)
  - `POST /api/v1/debug/bootstrap/restart` (planned; see `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`)

Everything below in this document should be read as design guidance and future expansion, unless it appears in the implemented list above.

---

### Health & Meta

- `GET /api/v1/health`
  - returns `{ "ok": true }`

- `GET /api/v1/version` (future)
  - candidate endpoint for build/version metadata.

---

### Searches (CRUD-ish)

- `GET /api/v1/searches`
  - returns list for sidebar
  - minimal fields: id, keyword, state, updated_at, counters summary

- `POST /api/v1/searches`
  - create a new search (optionally auto-start)
  - request:
    ```json
    { "keyword": "string", "autostart": true }
    ```
  - response: full Search snapshot

- `GET /api/v1/searches/{id}`
  - returns a full snapshot used to render main panel
  - includes:
    - search metadata
    - progress/telemetry snapshot
    - results summary + (optionally) top results
    - recent activity summary

- `PATCH /api/v1/searches/{id}`
  - update mutable properties
  - request example:
    ```json
    { "keyword": "new name", "pinned": true }
    ```

- `DELETE /api/v1/searches/{id}`
  - remove search and associated data (confirm in UI)

---

### Search Actions (Commands)

Commands are modeled as `POST` to action endpoints.

- `POST /api/v1/searches/{id}/start`
  - starts or resumes a search
  - request example:
    ```json
    { "mode": "full" }
    ```

- `POST /api/v1/searches/{id}/stop`
  - stops a running search

- `POST /api/v1/searches/{id}/rerun`
  - resets run state and starts new run (optional v1)

- `POST /api/v1/searches/{id}/export`
  - returns export payload (json)
  - or returns `{ "download_url": "..." }` if you prefer a file endpoint

---

### Results (Optional v1, recommended if results can be large)

If search details could get large, split results into separate endpoints:

- `GET /api/v1/searches/{id}/results`
  - query params:
    - `limit`, `offset`
    - `sort` (e.g. `sources`, `size`, `confidence`)
    - `q` (filter text)
  - returns:
    - list of result items
    - paging info

- `GET /api/v1/searches/{id}/results/{result_id}`
  - detailed view of one hit
  - sources list, metadata, hashes

---

### Downloads (Implemented - Queue/Lifecycle)

Implemented endpoints:

- `GET /api/v1/downloads`
  - list queue entries with current state and recovery counters
- `POST /api/v1/downloads`
  - create a new queued download entry (`.part` + `.part.met`)
- `POST /api/v1/downloads/{part_number}/pause`
- `POST /api/v1/downloads/{part_number}/resume`
- `POST /api/v1/downloads/{part_number}/cancel`
- `DELETE /api/v1/downloads/{part_number}`

Current phase scope:

- queue lifecycle + persistence/recovery are implemented
- transfer wire path (block request/receive pipeline) is pending

---

### Routing / Nodes (Operational)

- `GET /api/v1/kad/peers`
  - known peers list
- `GET /api/v1/debug/routing/summary` (debug endpoint)
  - routing summary counters
- `GET /api/v1/debug/routing/buckets` (debug endpoint)
  - per-bucket occupancy and recency distribution
- `GET /api/v1/debug/routing/nodes?bucket=N` (debug endpoint)
  - node-level snapshot for one bucket

---

### Logs (Operational)

Two approaches:

**A) Stream logs via SSE only (simplest)**

- no REST logs endpoint

**B) Provide a paged log endpoint**

- `GET /api/v1/logs?level=info&limit=200&cursor=...`

---

## Event Stream (SSE)

- `GET /api/v1/events`
  - SSE stream emitting JSON events
  - authenticated by `rm_session` HTTP-only cookie (session-cookie auth)

### Event envelope

All events follow a consistent envelope:

```json
{
  "type": "search_progress",
  "ts": "2026-02-11T21:15:00Z",
  "search_id": "abc123",
  "data": {}
}
```

### Recommended event types (v1)

- `search_created`
- `search_updated` (keyword/flags)
- `search_state_changed`
- `search_progress`
- `search_stats`
- `search_hit_found`
- `search_completed`
- `routing_stats`
- `traffic_stats`
- `log`

### Delivery rules

- Events should be **append-only**, never requiring the client to infer missing state.
- UI should be able to recover by re-fetching snapshots at any time.

---

## Snapshot DTOs (High Level)

### SearchListItem (sidebar)

- `id`
- `keyword`
- `state`
- `updated_at`
- `summary`:
  - `hits`
  - `peers_contacted`
  - `progress` (0..1 or phase)
  - `errors` (count)

### SearchDetail (main panel)

- `search` (metadata)
- `telemetry`:
  - peers, requests, responses, drops, rates
- `progress`:
  - phase, percent, started_at, eta (optional)
- `results`:
  - count + top N items
- `activity`:
  - last N events summary

---

## Error Handling

- Use standard HTTP codes:
  - 400 invalid request
  - 401 unauthorized
  - 403 forbidden (non-loopback token bootstrap)
  - 404 not found
  - 409 conflict (already running, etc.)
  - 429 rate limit (optional)
  - 500 internal error

- Error response shape (current default envelope):

```json
{
  "code": 404,
  "message": "not found"
}
```

Handler-specific endpoints may include richer payloads, but non-2xx responses are normalized to the envelope above.

---

## Concurrency & Idempotence (Guidelines)

- `POST /searches/{id}/start`:
  - if already running, return 200 with current state (idempotent)
- `POST /searches/{id}/stop`:
  - if already stopped, return 200 with current state (idempotent)
- Writes should be serialized per search to avoid state races.

---

## Security Notes (Pragmatic)

- Bind API to `127.0.0.1` by default.
- `GET /api/v1/auth/bootstrap` must be loopback-only.
- Do not put bearer tokens in URL query parameters; use header auth for REST and session cookies for SSE/UI.
- For headless deployments:
  - keep API loopback-only and expose via VPN or reverse proxy with TLS + auth.

---

## Minimal v1 Checklist

- [x] Health endpoint
- [ ] Version endpoint
- [ ] Create search endpoint
- [x] List/get/delete searches
- [x] Stop search endpoint
- [x] Download lifecycle endpoints
- [x] SSE events
- [x] KAD peer/source/keyword endpoints
- [x] Debug routing summary/buckets/nodes endpoints
