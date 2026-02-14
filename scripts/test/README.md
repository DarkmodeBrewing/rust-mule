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

Defaults:
- binaries: `../../mule-a/rust-mule` and `../../mule-b/rust-mule`
- run root: `/tmp/rust-mule-soak-bg`
- API URLs: `127.0.0.1:17835` and `127.0.0.1:17836`

Override via env vars, for example:
- `A_SRC=/dist/mule-a B_SRC=/dist/mule-b RUN_ROOT=/tmp/my-soak bash scripts/test/source_probe_soak_bg.sh start 7200`
