# Test Harness Scripts

Scenario and soak test scripts.

- `two_instance_dht_selftest.sh`: two-instance publish/search/source flow checks.
- `rust_mule_soak.sh`: long-running soak runner scaffold.
- `source_probe_soak_bg.sh`: timed background soak runner with PID/status/stop controls.
- `download_soak_bg.sh`: generic timed background soak runner for download API scenarios.
- `download_soak_single_e2e_bg.sh`: single-download lifecycle soak wrapper.
- `download_soak_long_churn_bg.sh`: long churn lifecycle soak wrapper.
- `download_soak_integrity_bg.sh`: API invariant/integrity soak wrapper.
- `download_soak_concurrency_bg.sh`: concurrent queue pressure soak wrapper.
- `download_soak_band.sh`: sequential runner that executes all four download soaks in one command and auto-collects tarballs.
- `download_soak_stack_bg.sh`: full background pipeline (build + staged run dir + config + app launch + health wait + band soak + collectable artifacts).
- `soak_triage.sh`: triage summary for soak tarball outputs.

## Timed Background Soak

Use `source_probe_soak_bg.sh` when you want the soak to keep running after terminal/session changes.

1. Start a timed run (example: 4 hours = 14400 seconds):
   - `bash scripts/test/source_probe_soak_bg.sh start 14400`
2. Check status:
   - `bash scripts/test/source_probe_soak_bg.sh status`
3. Stop early (optional):
   - `bash scripts/test/source_probe_soak_bg.sh stop`
4. Collect logs into a tarball:
   - `bash scripts/test/source_probe_soak_bg.sh collect`

Safety behavior:
- Fails fast if `A_URL` or `B_URL` ports are already occupied.
- Fails fast on repeated readiness `403` responses (likely token mismatch against an old process).
- Verifies spawned node PIDs are running from the expected per-run directories.
- `stop` also attempts to kill `rust-mule` listeners bound to `A_URL`/`B_URL` ports if PID files are stale.
- `stop` also scans `/proc` for soak-owned processes whose cwd/cmdline points into the current `RUN_ROOT` and force-stops them.
- Startup/readiness failures now mark runner state as `failed` and run cleanup immediately.
- `stop` clears `logs/a.pid` and `logs/b.pid` so `status` does not report stale node pid files.
- Each run now writes unique SAM `session_name` values for A/B (`rust-mule-a-soak-<tag>`, `rust-mule-b-soak-<tag>`).
- By default, copied `data/sam.keys` are removed per run (`SOAK_FRESH_IDENTITY=1`) so each soak run gets a fresh destination identity.

Defaults:
- binaries: `../../mule-a/rust-mule` and `../../mule-b/rust-mule`
- run root: `/tmp/rust-mule-soak-bg`
- API URLs: `127.0.0.1:17835` and `127.0.0.1:17836`
- miss recheck: `MISS_RECHECK_ATTEMPTS=1`, `MISS_RECHECK_DELAY=20`
- identity: `SOAK_FRESH_IDENTITY=1` (set to `0` to reuse copied `sam.keys`)
- detached runner stdout/stderr: `/tmp/rust-mule-soak-bg/logs/runner.out`

Override via env vars, for example:
- `A_SRC=/dist/mule-a B_SRC=/dist/mule-b RUN_ROOT=/tmp/my-soak bash scripts/test/source_probe_soak_bg.sh start 7200`
- `SOAK_FRESH_IDENTITY=0 SOAK_RUN_TAG=manual-debug bash scripts/test/source_probe_soak_bg.sh start 3600`

Miss recheck behavior:
- After each round's first `GET /api/v1/kad/sources/:file_id_hex` miss, the runner can recheck before classifying a miss.
- Tune with:
  - `MISS_RECHECK_ATTEMPTS` (how many additional polls to run after first miss)
  - `MISS_RECHECK_DELAY` (seconds between additional polls)

## Download Soak Plan (Run After Current Source Soak Completes)

These scripts target the current download API/control-plane behavior (`/api/v1/downloads` and pause/resume/cancel/delete actions).

Pre-check:
- Ensure one node API is reachable and token file is valid.
- Defaults:
  - `BASE_URL=http://127.0.0.1:17835`
  - `TOKEN_FILE=data/api.token`

### 1) Single File E2E Lifecycle Soak

- Start:
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token bash scripts/test/download_soak_single_e2e_bg.sh start 3600`
- Status:
  - `bash scripts/test/download_soak_single_e2e_bg.sh status`
- Collect:
  - `bash scripts/test/download_soak_single_e2e_bg.sh collect`

Pass signals:
- Runner state ends `completed`.
- No repeated API hard failures in `logs/runner.log`.

### 2) Long Churn Soak

- Start:
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token CHURN_MAX_QUEUE=25 bash scripts/test/download_soak_long_churn_bg.sh start 7200`
- Status:
  - `bash scripts/test/download_soak_long_churn_bg.sh status`
- Collect:
  - `bash scripts/test/download_soak_long_churn_bg.sh collect`

Pass signals:
- Runner state ends `completed`.
- Queue remains bounded by churn cleanup logic (no unbounded growth).

### 3) Integrity/Invariants Soak

- Start:
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token bash scripts/test/download_soak_integrity_bg.sh start 3600`
- Status:
  - `bash scripts/test/download_soak_integrity_bg.sh status`
- Collect:
  - `bash scripts/test/download_soak_integrity_bg.sh collect`

Pass signals:
- Runner state ends `completed` (not `failed`).
- No `ERROR: integrity_violations` or `ERROR: duplicate_part_numbers` in `logs/runner.log`.

### 4) Concurrency Soak

- Start:
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token CONCURRENCY_TARGET=20 bash scripts/test/download_soak_concurrency_bg.sh start 7200`
- Status:
  - `bash scripts/test/download_soak_concurrency_bg.sh status`
- Collect:
  - `bash scripts/test/download_soak_concurrency_bg.sh collect`

Pass signals:
- Runner state ends `completed`.
- API continues to respond without repeated readiness/list failures under queue pressure.

Stop any scenario early:
- `bash scripts/test/download_soak_<scenario>_bg.sh stop`
  - where `<scenario>` is `single_e2e`, `long_churn`, `integrity`, or `concurrency`.

## Download Soak In-Band Runner

Run all four download scenarios sequentially with automatic status polling, stop, and collect:

- `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token bash scripts/test/download_soak_band.sh`

Precondition:
- rust-mule API must already be running at `BASE_URL` (`/api/v1/health` must return `200`).

Outputs:
- per-scenario tarballs copied into `OUT_DIR` (default `/tmp/rust-mule-download-soak-band-<timestamp>`)
- `results.tsv` with final state/result per scenario
- `status.tsv` with polling snapshots

Optional overrides:
- `OUT_DIR=/tmp/my-band`
- `INTEGRITY_SECS=3600`
- `SINGLE_E2E_SECS=3600`
- `CONCURRENCY_SECS=7200`
- `LONG_CHURN_SECS=7200`
- `CONCURRENCY_TARGET=20`
- `CHURN_MAX_QUEUE=25`

## Full Background Pipeline (Build + Run + Soak)

Use this when you want one command to:
1. build latest rust-mule
2. stage run dir in `/tmp/rustmule-run-<timestamp>`
3. write run-specific `config.toml`
4. start rust-mule
5. health-check and wait for `data/api.token`
6. run `download_soak_band.sh`

Start:
- `bash scripts/test/download_soak_stack_bg.sh start`

Status:
- `bash scripts/test/download_soak_stack_bg.sh status`

Stop:
- `bash scripts/test/download_soak_stack_bg.sh stop`

Collect:
- `bash scripts/test/download_soak_stack_bg.sh collect`

Common overrides:
- `API_PORT=17835`
- `SAM_HOST=10.99.0.2`
- `SAM_PORT=7656`
- `LOG_LEVEL=info`
- `BUILD_CMD='cargo build --release'`
- `INTEGRITY_SECS=3600`
- `SINGLE_E2E_SECS=3600`
- `CONCURRENCY_SECS=7200`
- `LONG_CHURN_SECS=7200`

Troubleshooting:
- If status is `failed` immediately, inspect `/tmp/rust-mule-download-stack/logs/stack.out`.
- The stack runner now attempts to add `~/.cargo/bin` to `PATH` automatically when `cargo` is not found.
