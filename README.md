# rust-mule

<p align="center">
  <img src="assets/this_is_fine_mule.png" alt="This is fine – rust-mule KAD bootstrap" width="420">
</p>

<p align="center">
  <em>
    Bootstrapping a decentralized KAD network over I2P.<br>
    Sometimes it works immediately. Sometimes it sets the room on fire.
  </em>
</p>

`rust-mule` is an experimental, **I2P-only** iMule-compatible Kademlia (KAD) crawler/overlay.
It talks to the local (or remote) I2P router via **SAM v3** using `STYLE=DATAGRAM` sessions.

This repo is currently focused on:

- Getting the SAM datagram client stable (TCP datagram framing or UDP forwarding).
- Running a long-lived KAD service loop to discover peers and maintain a routing table.
- Persisting a reusable bootstrap seed pool (`data/nodes.dat`).

## Quick Start

1. Edit `config.toml`:

- `sam.host` / `sam.port`: where the SAM bridge is
- `sam.datagram_transport`: `tcp` (default) or `udp_forward`

2. Run:

```bash
cargo run --bin rust-mule
```

Logs:

- Stdout is controlled by `[general].log_level` (or `RUST_LOG`)
- File logs roll daily under `data/logs/` when `[general].log_to_file=true`

## Build A Linux Release Binary

Build the optimized binary:

```bash
cargo build --release --locked --bin rust-mule
ls -lah target/release/rust-mule
```

Optional (shrinks the binary):

```bash
strip target/release/rust-mule || true
```

Packaging helper:

```bash
docs/scripts/build_linux_release.sh
```

## Running Two Instances (Same Router)

To run two instances on the same machine/router, you must ensure:

- different `[general].data_dir` (so `sam.keys` + lock file do not clash)
- different `[sam].session_name` (so SAM session IDs do not clash)
- different `[api].port` if the API is enabled in both instances

Example layout:

```bash
mkdir -p run-a run-b
cp target/release/rust-mule run-a/
cp target/release/rust-mule run-b/
```

Create `run-a/config.toml`:

```toml
[general]
data_dir = "data"

[sam]
session_name = "rust-mule-a"

[api]
enabled = true
port = 17835
```

Create `run-b/config.toml`:

```toml
[general]
data_dir = "data"

[sam]
session_name = "rust-mule-b"

[api]
enabled = true
port = 17836
```

Then run each instance from its own directory (so it picks up that directory’s `config.toml`):

```bash
(cd run-a && ./rust-mule)
(cd run-b && ./rust-mule)
```

## Local HTTP API (For Future GUI)

There is a local HTTP API (REST + SSE) intended for the control plane and a future GUI.

- Config: `[api]` in `config.toml`
- Auth: bearer token stored in `data/api.token`
- Docs: `docs/architecture.md`

Quick curl test:

```bash
TOKEN="$(cat data/api.token)"
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/status
curl -N  -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/events

# Enqueue a Kad2 search for sources of a fileID (16 bytes / 32 hex chars).
curl -sS -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"file_id_hex":"00112233445566778899aabbccddeeff","file_size":0}' \
  http://127.0.0.1:17835/api/v1/kad/search_sources

# Read sources discovered so far (in-memory).
curl -sS -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:17835/api/v1/kad/sources/00112233445566778899aabbccddeeff | jq .
```

UI bootstrap (dev):

```bash
open http://127.0.0.1:17835/
```

The overview page bootstraps a token via `GET /api/v1/dev/auth`, stores it in
`sessionStorage`, fetches `/api/v1/status`, and subscribes to `/api/v1/events`.

## Data Files

Runtime state lives under `data/` (gitignored):

- `data/nodes.dat`: primary persisted bootstrap pool (iMule `nodes.dat` v2 format)
- `data/nodes.initseed.dat`: initial seed snapshot (created from embedded initseed on first run)
- `data/nodes.fallback.dat`: fallback seed snapshot (currently same as initseed)
- `data/preferencesKad.dat`: your KadID (stable node identity)
- `data/kad_udp_key_secret.dat`: UDP obfuscation secret (iMule-style verify key seed; generated on first run)
- `data/sam.keys`: SAM destination keys (`PUB=...` / `PRIV=...`)

For more details and current development notes, see `docs/handoff.md`.
