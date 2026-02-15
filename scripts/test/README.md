# Test Harness Scripts

Scenario and soak test scripts.

- `two_instance_dht_selftest.sh`: two-instance publish/search/source flow checks.
- `rust_mule_soak.sh`: long-running soak runner scaffold.
- `source_probe_soak_bg.sh`: timed background soak runner with PID/status/stop controls.
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

Defaults:
- binaries: `../../mule-a/rust-mule` and `../../mule-b/rust-mule`
- run root: `/tmp/rust-mule-soak-bg`
- API URLs: `127.0.0.1:17835` and `127.0.0.1:17836`
- miss recheck: `MISS_RECHECK_ATTEMPTS=1`, `MISS_RECHECK_DELAY=20`
- detached runner stdout/stderr: `/tmp/rust-mule-soak-bg/logs/runner.out`

Override via env vars, for example:
- `A_SRC=/dist/mule-a B_SRC=/dist/mule-b RUN_ROOT=/tmp/my-soak bash scripts/test/source_probe_soak_bg.sh start 7200`

Miss recheck behavior:
- After each round's first `GET /api/v1/kad/sources/:file_id_hex` miss, the runner can recheck before classifying a miss.
- Tune with:
  - `MISS_RECHECK_ATTEMPTS` (how many additional polls to run after first miss)
  - `MISS_RECHECK_DELAY` (seconds between additional polls)
