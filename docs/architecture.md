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
     - `GET /status` for a snapshot
     - `GET /events` for continuous updates

## HTTP API

### Config

`config.toml`:

- `[api].enabled` (default `false`)
- `[api].host` (default `127.0.0.1`)
- `[api].port` (default `17835`)

### Auth / Token

On first start with `api.enabled=true`, the backend creates a token file:

- `data/api.token`

Clients must send:

- `Authorization: Bearer <token>`

Notes:

- We do **not** print the token to logs. The GUI should read it from `data/api.token`.
- On Unix we attempt to set file permissions to `0600` best-effort; on Windows we skip this.

### Endpoints

- `GET /health`
  - No auth.
  - Returns `{ "ok": true }`.

- `GET /status`
  - Auth required.
  - Returns the latest KAD service status snapshot (or `503` until the service has started).

- `GET /events`
  - Auth required.
  - SSE stream of live status updates.
  - Events are sent as:
    - `event: status`
    - `data: <json KadServiceStatus>`

Example:

```bash
TOKEN="$(cat data/api.token)"
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/status | jq .
curl -N  -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/events
```

## TLS (Future)

For local-only GUI usage, the token + loopback binding is usually sufficient.

If we later want to support remote GUI connections, we should add TLS:

- Generate a self-signed cert/key on first launch and store under `data/` (or allow users to
  provide their own).
- Keep token-based auth even with TLS (defense in depth).
