# rust-mule

<p align="center">
  <img src="assets/this_is_fine_mule.png" alt="This is fine – rust-mule KAD bootstrap" width="420">
</p>

`rust-mule` is an experimental, I2P-only, iMule-compatible Kademlia (KAD) node and control plane.
It talks to I2P via SAM v3 datagram sessions and exposes a local HTTP API + embedded UI.

## Current Scope

- SAM v3 datagram transport (`tcp` or `udp_forward`)
- KAD bootstrap/routing/search/publish work in progress
- Local control-plane API under `/api/v1`
- Embedded static UI served from the Rust binary

## Quick Start

1. Configure `config.toml`:
- `[sam].host` / `[sam].port`
- `[sam].datagram_transport = "tcp" | "udp_forward"`
- `[api].port`

2. Run:

```bash
cargo run --bin rust-mule
```

On startup, after HTTP is ready, the app logs:

```text
rust-mule UI available at: http://localhost:<port>
```

## UI and API Access

- Root URL redirects to `/index.html`.
- UI routes require a frontend session cookie (`rm_session`).
- `/auth` bootstraps the session by calling:
  - `GET /api/v1/auth/bootstrap` (loopback-only token bootstrap)
  - `POST /api/v1/session` (sets `rm_session` cookie)
- REST API calls use bearer auth (`Authorization: Bearer <token>`).
- SSE (`GET /api/v1/events`) uses session-cookie auth.
- Optional endpoint toggles:
  - `[api].enable_debug_endpoints = true|false` for `/api/v1/debug/*`
  - `[api].auth_mode = "local_ui"|"headless_remote"` controls `/api/v1/auth/bootstrap` availability
- Optional API rate limiting:
  - `[api].rate_limit_enabled = true|false`
  - `[api].rate_limit_window_secs`
  - `[api].rate_limit_auth_bootstrap_max_per_window`
  - `[api].rate_limit_session_max_per_window`
  - `[api].rate_limit_token_rotate_max_per_window`

Quick API check:

```bash
TOKEN="$(cat data/api.token)"
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17835/api/v1/status | jq .
```

## Build

```bash
cargo build --release --locked --bin rust-mule
```

Packaging helper:

```bash
scripts/build/build_linux_release.sh
```

Tag-driven release builds are automated in GitHub Actions:
- Push a version tag (example: `git tag v0.1.0 && git push origin v0.1.0`)
- Workflow `.github/workflows/release.yml` builds Linux/macOS/Windows bundles
- Artifacts are attached to the GitHub Release for that tag

## Quality Gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
scripts/test/coverage.sh
```

## Two-Instance Local Test

Use separate `data_dir`, `sam.session_name`, and API ports per instance.

```bash
mkdir -p run-a run-b
cp target/release/rust-mule run-a/
cp target/release/rust-mule run-b/
```

Then run each instance from its own directory with its own `config.toml`.

## Data Files

Runtime state lives under `data/` (gitignored):

- `data/api.token`
- `data/nodes.dat`
- `data/nodes.initseed.dat`
- `data/nodes.fallback.dat`
- `data/preferencesKad.dat`
- `data/kad_udp_key_secret.dat`
- `data/sam.keys`
- `data/logs/` (optional daily-rolled logs)

## Documentation Map

- `docs/README.md`: documentation index
- `docs/dev.md`: developer setup notes
- `docs/architecture.md`: backend/API/UI architecture and auth model
- `docs/API_DESIGN.md`: API design (implemented + future direction)
- `docs/DOWNLOAD_DESIGN.md`: iMule-compatible download subsystem strategy and phased plan
- `docs/UI_DESIGN.md`: UI design and page model
- `docs/ui_api_contract_map.md`: UI page -> endpoint/field contract map
- `docs/api_curl.md`: curl examples for API testing
- `docs/TODO.md`: normalized backlog by subsystem
- `docs/TASKS.md`: current prioritized execution plan
- `docs/handoff.md`: rolling status log for continuation between sessions

## Utility Scripts

- `scripts/build/`: platform release bundle scripts (`linux`, `macos`, `windows`)
- `scripts/test/two_instance_dht_selftest.sh`: short two-instance validation
- `scripts/test/rust_mule_soak.sh`: long-running soak scaffold
- `scripts/test/soak_triage.sh`: soak archive triage summary
- `scripts/docs/*.sh`: endpoint-focused debug helpers
