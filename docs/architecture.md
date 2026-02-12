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
   - Not implemented yet.
   - Expected to read the backend token from disk (or be launched with it) and then use:
     - `GET /api/v1/status` for a snapshot
     - `GET /api/v1/events` for continuous updates

## HTTP API

### Config

`config.toml`:

- `[api].host` (default `127.0.0.1`)
- `[api].port` (default `17835`)

### Auth / Token

On first start, the backend creates a token file:

- `data/api.token`

Clients must send:

- `Authorization: Bearer <token>`

Notes:

- We do **not** print the token to logs. The GUI should read it from `data/api.token`.
- On Unix we attempt to set file permissions to `0600` best-effort; on Windows we skip this.
- CORS is restricted to loopback origins only (`localhost`, `127.0.0.1`, and loopback IPs), with
  `Authorization` and `Content-Type` as the allowed request headers.

### Endpoints

- `GET /api/v1/dev/auth`
  - No auth.
  - Loopback-only.
  - Returns `{ "token": "<bearer token>" }` for local UI bootstrap.

- `GET /api/v1/health`
  - No auth.
  - Returns `{ "ok": true }`.

- `GET /api/v1/status`
  - Auth required.
  - Returns the latest KAD service status snapshot (or `503` until the service has started).

- `GET /api/v1/events`
  - Auth required.
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
curl -N  -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/events

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
