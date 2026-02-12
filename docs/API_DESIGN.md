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
- currently implemented API behavior (`/api/v1` as of 2026-02-12), and
- forward-looking endpoint ideas for later milestones.

For executable endpoint usage, prefer `docs/api_curl.md`.

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
- UI bootstrap gets a local bearer token from `GET /api/v1/dev/auth` (loopback-only).
- Browser session is established via `POST /api/v1/session` and `rm_session` HTTP-only cookie.
- REST endpoints use `Authorization: Bearer <token>`.
- SSE (`GET /api/v1/events`) uses session-cookie auth.
- Token query parameters for SSE are not used.

### Endpoints
- `GET /api/v1/dev/auth`
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

### Health & Meta

- `GET /api/v1/health`
  - returns `{ "status": "ok" }`

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

### Downloads (Future)

If/when rust-mule manages downloads:

- `GET /api/v1/downloads`
- `POST /api/v1/downloads` (start download from search result)
- `GET /api/v1/downloads/{id}`
- `POST /api/v1/downloads/{id}/pause`
- `POST /api/v1/downloads/{id}/resume`
- `POST /api/v1/downloads/{id}/cancel`

---

### Routing / Nodes (Operational)

- `GET /api/v1/routing/summary`
  - buckets count, occupied, stale, refresh age distribution

- `GET /api/v1/nodes`
  - known peers list (paged)

- `POST /api/v1/bootstrap`
  - triggers bootstrap cycle

- `POST /api/v1/routing/refresh`
  - triggers bucket refresh cycle

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
  - localhost v1: allow `?token=...` (since `EventSource` can’t send headers)

### Event envelope
All events follow a consistent envelope:

```json
{
  "type": "search_progress",
  "ts": "2026-02-11T21:15:00Z",
  "search_id": "abc123",
  "data": { }
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
  - 403 forbidden (non-loopback dev auth)
  - 404 not found
  - 409 conflict (already running, etc.)
  - 429 rate limit (optional)
  - 500 internal error

- Error response shape (consistent):
```json
{
  "error": {
    "code": "SEARCH_NOT_FOUND",
    "message": "Search id does not exist",
    "details": { }
  }
}
```

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
- `GET /api/v1/dev/auth` must be loopback-only.
- Avoid putting tokens in URLs except for localhost SSE; do not log full query strings.
- For headless deployments:
  - keep API loopback-only and expose via VPN or reverse proxy with TLS + auth.

---

## Minimal v1 Checklist

- [ ] Health/version endpoints
- [ ] Create/list/get/delete searches
- [ ] Start/stop searches
- [ ] Search detail snapshot includes progress + results summary
- [ ] SSE events with search_id and telemetry/log events
- [ ] Routing summary endpoint (optional but valuable)
