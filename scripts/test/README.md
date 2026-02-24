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
- `download_resume_soak.sh`: automated crash/restart resume-soak on top of stack runner (kills app mid-scenario, restarts in-place, verifies continued progress).
- `download_fixtures.example.json`: example fixture file format (`file_name`, `file_size`, `file_hash_md4_hex`).
- `gen_download_fixture.sh`: generates fixture JSON from local files (MD4 + size + name).
- `kad_publish_search_probe.sh`: publishes source on node A, triggers search on node B, and polls A/B counters + B sources until success/timeout.
- `kad_phase0_baseline.sh`: captures KAD Phase 0 timing/ordering baseline counters from `/api/v1/status` into TSV.
- `kad_phase0_longrun.sh`: wrapper for long baseline captures (default 6h) using `kad_phase0_baseline.sh`.
- `kad_phase0_compare.sh`: compares two Phase 0 baseline TSV files and prints before/after delta summary.
- `kad_phase0_gate.sh`: runs before/after baseline capture + compare + threshold gate in one command.
- `kad_phase0_ci_smoke.sh`: offline deterministic smoke check for baseline+compare scripts using synthetic TSV fixtures (CI-safe, no network/node runtime).
- `soak_triage.sh`: triage summary for soak tarball outputs.

## KAD Phase 0 Baseline Capture

Use this before and after KAD/wire changes to compare observable timing/ordering counters.

Run:
- `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token DURATION_SECS=1800 INTERVAL_SECS=5 bash scripts/test/kad_phase0_baseline.sh`

Or with flags:
- `bash scripts/test/kad_phase0_baseline.sh --base-url http://127.0.0.1:17835 --token-file data/api.token --duration-secs 1800 --interval-secs 5 --out-file /tmp/kad-phase0.tsv`

Captured TSV columns include:
- `pending_overdue`, `pending_max_overdue_ms`
- `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
- `outbound_shaper_delayed`, `outbound_shaper_drop_global_cap`, `outbound_shaper_drop_peer_cap`
- `dropped_legacy_kad1`, `dropped_unhandled_opcode`
- cumulative totals:
  - `sent_reqs_total`, `recv_ress_total`, `timeouts_total`
  - `tracked_out_matched_total`, `tracked_out_unmatched_total`, `tracked_out_expired_total`
  - `outbound_shaper_delayed_total`
  - `dropped_legacy_kad1_total`, `dropped_unhandled_opcode_total`
  - `sam_framing_desync_total`
- restart detection:
  - `restart_marker` (`1` when `uptime_secs` decreases from previous sample, else `0`)
- key throughput/error counters (`sent_reqs`, `recv_ress`, `timeouts`, batch send/fail counters)

Notes:
- `503` from `/api/v1/status` is treated as warmup/not-ready and skipped (not a script failure).
- script summary reports `samples`, `skipped_503`, `skipped_other`, and `restarts`.

Compare two baseline runs:
- `bash scripts/test/kad_phase0_compare.sh --before /tmp/kad-before.tsv --after /tmp/kad-after.tsv`
- output columns:
  - `metric`, `before_avg`, `after_avg`, `delta`, `pct_change`
  - min/max and sample counts for each side

Long-run baseline (default 6h):
- `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token bash scripts/test/kad_phase0_longrun.sh`
- optional overrides:
  - `DURATION_SECS=28800` (8h), `INTERVAL_SECS=5`, `OUT_FILE=/tmp/kad-longrun.tsv`
- output includes a post-run summary:
  - `restart_markers=<count>`
  - `sam_framing_desync_total_max=<max observed>`
  - `dropped_legacy_kad1_total_max=<max observed>`
  - `dropped_unhandled_opcode_total_max=<max observed>`

Automated before/after gate:
- manual binary/process swap between captures:
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=../../mule-a/data/api.token DURATION_SECS=1800 INTERVAL_SECS=5 bash scripts/test/kad_phase0_gate.sh`
- with setup hooks (script runs commands before each capture):
  - `BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=../../mule-a/data/api.token BEFORE_SETUP_CMD='bash scripts/test/switch_binary_main.sh' AFTER_SETUP_CMD='bash scripts/test/switch_binary_feature.sh' DURATION_SECS=1800 INTERVAL_SECS=5 bash scripts/test/kad_phase0_gate.sh`
- outputs:
  - `<OUT_DIR>/before.tsv`
  - `<OUT_DIR>/after.tsv`
  - `<OUT_DIR>/compare.tsv`
  - `<OUT_DIR>/gate.tsv`
- default gate thresholds (override via env):
  - `sent_reqs_total` after/before >= `0.90`
  - `recv_ress_total` after/before >= `0.90`
  - `tracked_out_matched_total` after/before >= `0.90`
  - `timeouts_total` after/before <= `1.10`
  - `outbound_shaper_delayed_total` after/before <= `1.25`
  - `tracked_out_matched_total/sent_reqs_total` after/before >= `0.90`
  - `timeouts_total/sent_reqs_total` after/before <= `1.10`
- gate metric mode:
  - gate compares `*_total` metrics as per-uptime rates derived from each run (`(last-first)/(uptime_last-uptime_first)`), not raw averages.
  - this avoids false fails from startup warmup/sample-window skew.
- enforcement:
  - `ENFORCE_THRESHOLDS=1` exits non-zero on failed checks
  - `ENFORCE_THRESHOLDS=0` prints failures but exits zero
- suggested tuning-env for noisy networks:
  - `MIN_SENT_REQS_TOTAL_RATIO=0.60`
  - keep efficiency thresholds enabled (`MIN_MATCH_PER_SENT_RATIO`, `MAX_TIMEOUT_PER_SENT_RATIO`)

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

For real transfer/resume validation (not random synthetic hashes), provide fixtures:
- `DOWNLOAD_FIXTURES_FILE=/path/to/download_fixtures.json`
- `FIXTURES_ONLY=1` to fail fast if fixture-backed creates cannot be used.

Generate fixture JSON from local files:
- `scripts/test/gen_download_fixture.sh --out /tmp/download_fixtures.json /path/to/file1 /path/to/file2`
- Generate and publish those hashes to a specific node:
  - `scripts/test/gen_download_fixture.sh --out /tmp/download_fixtures.json --publish --base-url http://127.0.0.1:17835 --token-file data/api.token /path/to/file1 /path/to/file2`

Probe publish->search visibility between two nodes:
- `scripts/test/kad_publish_search_probe.sh --file-id-hex 52be4bd97ca58ba9507877d71858de96 --file-size 2097152 --a-base-url http://127.0.0.1:17866 --a-token-file ../../mule-a/data/api.token --b-base-url http://127.0.0.1:17835 --b-token-file data/api.token --timeout-secs 900 --poll-secs 5`
- If discovery is slow, keep source advertisements warm:
  - add `--republish-every 12` (with `--poll-secs 5`, this republishes every 60s).

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
- on external interruption (`SIGINT`/`SIGTERM`), the active scenario is stopped, a best-effort collect is attempted, and `results.tsv` gets an `interrupted` row.

Optional overrides:
- `OUT_DIR=/tmp/my-band`
- `INTEGRITY_SECS=3600`
- `SINGLE_E2E_SECS=3600`
- `CONCURRENCY_SECS=7200`
- `LONG_CHURN_SECS=7200`
- `CONCURRENCY_TARGET=20`
- `CHURN_MAX_QUEUE=25`
- `READY_TIMEOUT_SECS=300`
- `READY_PATH=/api/v1/downloads`
- `READY_HTTP_CODES=200`
- `API_CONNECT_TIMEOUT_SECS=3`
- `API_MAX_TIME_SECS=8`
- `DOWNLOAD_FIXTURES_FILE=/path/to/download_fixtures.json`
- `FIXTURES_ONLY=1`

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
- `DOWNLOAD_FIXTURES_FILE=/path/to/download_fixtures.json`
- `FIXTURES_ONLY=1`

Troubleshooting:
- If status is `failed` immediately, inspect `/tmp/rust-mule-download-stack/logs/stack.out`.
- The stack runner now attempts to add `~/.cargo/bin` to `PATH` automatically when `cargo` is not found.
- `stop` now kills the full stack process tree (runner group + per-scenario soak runners + run-dir processes) to avoid orphaned `rust-mule`/soak processes.

## Resume Soak Automation (Crash + Restart)

Automates a hard-crash resume test during download soak:
1. starts `download_soak_stack_bg.sh`
2. waits until `RESUME_SCENARIO` is running (default `concurrency`)
3. snapshots `/api/v1/downloads` (pre-crash)
4. `kill -9` on `rust-mule`
5. restarts `rust-mule` in the same staged run dir
6. snapshots `/api/v1/downloads` (post-restart)
7. verifies per-download monotonicity (`downloaded_bytes` never regresses post-restart)
8. verifies scenario status continues
9. waits for at least one completed download after restart
10. waits for stack terminal state
11. collects stack tarball and writes a resume report

Run:
- `bash scripts/test/download_resume_soak.sh`
- `DOWNLOAD_FIXTURES_FILE=/path/to/download_fixtures.json FIXTURES_ONLY=1 bash scripts/test/download_resume_soak.sh`

Common overrides:
- `RESUME_SCENARIO=concurrency`
- `STACK_ROOT=/tmp/rust-mule-download-stack`
- `API_PORT=17835`
- `WAIT_TIMEOUT_SECS=21600`
- `HEALTH_TIMEOUT_SECS=300`
- `ACTIVE_TRANSFER_TIMEOUT_SECS=1800`
- `COMPLETION_TIMEOUT_SECS=3600`
- `RESUME_OUT_DIR=/tmp/rust-mule-download-resume-<timestamp>`

Outputs:
- resume artifacts/report under `RESUME_OUT_DIR`:
  - `pre_downloads.json`, `post_downloads.json`
  - `pre_summary.txt`, `post_summary.txt`
  - `pre_violations.count`, `post_violations.count`
  - `resume_report.txt`
  - `stack_bundle.path`

Notes:
- Crash validation is process-based (killed app PID + no remaining run-dir `rust-mule` process), not strictly `health=000`.
- This avoids false failures when another process is already serving the same API port.
- Resume restart now hard-checks `/proc` for any remaining run-dir `rust-mule` process (`cwd`/`cmdline`), and fails early with PID details if single-instance lock would still be held.
- Crash step now force-kills all run-dir-owned `rust-mule` PIDs found via `/proc` (not only `control/app.pid`) to handle wrapper-PID vs child-PID mismatches.
- Resume now waits for an active transfer before crash (`downloaded_bytes>0` and `inflight_ranges>0`) so the run validates true in-flight resume behavior.
- Resume now fails if any pre-existing download has lower `downloaded_bytes` after restart.
- Resume now requires at least one completed download post-restart within `COMPLETION_TIMEOUT_SECS`.
