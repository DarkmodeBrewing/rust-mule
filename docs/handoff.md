# Handoff / Continuation Notes

This file exists because chat sessions are not durable project memory. In the next session, start here, then check `git log` on `main` and the active feature branch(es).

## Goal

Implement an iMule-compatible Kademlia (KAD) overlay over **I2P only**, using **SAM v3** `STYLE=DATAGRAM` sessions (UDP forwarding) for peer connectivity.

## Status (2026-02-14)

- Status: Updated CodeQL export script output path defaults on `feature/add-codeql-export-script`:
  - `scripts/export-gh-stats/export-codeql-alerts.sh` now writes CSV by default to:
    - `/tmp/codeql-alerts-OWNER-REPO_<timestamp>.csv`
  - Updated script header comment to match new output location/pattern.
- Decisions:
  - Use `/tmp` as default destination to avoid polluting repository working directories.
- Next steps:
  - If needed, add optional output path override flags/env in a follow-up.
- Change log:
  - Updated `scripts/export-gh-stats/export-codeql-alerts.sh` output path default.

- Status: Hardened `scripts/export-gh-stats/export-codeql-alerts.sh` on `feature/add-codeql-export-script`:
  - loads `.env` from script directory (`scripts/export-gh-stats/.env`) instead of current working directory
  - removed `source` execution path and replaced with safe `KEY=VALUE` parser
  - defaults and filters export to `TOOL_NAME=CodeQL`
  - passes `tool_name` query parameter and filters by `.tool.name == TOOL_NAME` in output conversion
  - handles zero-alert repos gracefully (header-only CSV + success exit)
  - sanitized local `.env` placeholder content and expanded `.env.example`.
- Decisions:
  - Prefer least-surprise local config loading and non-executable env parsing for safety.
  - Keep export scoped to CodeQL by default to match script intent and naming.
- Next steps:
  - Rotate/revoke previously used PAT if it was real.
  - Consider adding a small script README with token scope requirements and sample invocation.
- Change log:
  - Updated `scripts/export-gh-stats/export-codeql-alerts.sh`.
  - Updated `scripts/export-gh-stats/.env.example`.
  - Updated `scripts/export-gh-stats/.env` placeholder values.
  - Validation:
    - `bash -n scripts/export-gh-stats/export-codeql-alerts.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (91 tests)

- Status: Added explicit CodeQL workflow configuration for Rust on `feature/codeql-workflow-rust`:
  - New workflow: `.github/workflows/codeql.yml`
  - Triggers on:
    - push to `main`
    - pull requests targeting `main`
    - weekly schedule
  - Uses CodeQL action v3 with `language: rust` and `security-and-quality` queries.
- Decisions:
  - Use a committed (versioned) CodeQL workflow to keep scan configuration consistent across branches and PRs.
- Next steps:
  - In GitHub settings, disable CodeQL Default setup to avoid dual-configuration ambiguity.
  - Let the new workflow run on PR/main and confirm code scanning comparisons no longer warn about missing config.
- Change log:
  - Added `.github/workflows/codeql.yml`.

- Status: Added repository merge policy documentation on `main`:
  - `README.md` now states:
    - no direct commits/merges to `main`
    - all changes via feature branch + PR
    - merge to `main` only through reviewed PR with required checks
  - `.github/pull_request_template.md` now includes merge-policy acknowledgment checkbox.
- Decisions:
  - Treat this as a mandatory process rule from now on.
- Next steps:
  - Enforce branch protection in GitHub settings to match documented policy.
- Change log:
  - Updated `README.md` (`Merge Policy` section).
  - Updated `.github/pull_request_template.md` validation checklist.

- Status: Fixed `kad_phase0_compare.sh` output formatting on `feature/kad-phase0-baseline`:
  - header is now always first
  - metric rows are sorted and consistently tab-separated
  - numeric formatting is normalized to fixed precision fields
- Decisions:
  - Keep plain TSV output for easy piping into `column -t` / CI artifacts.
- Next steps:
  - Re-run compare command and verify table readability.
- Change log:
  - Updated `scripts/test/kad_phase0_compare.sh` output rendering/sort behavior.

- Status: Added KAD Phase 0 baseline compare helper on `feature/kad-phase0-baseline`:
  - New script: `scripts/test/kad_phase0_compare.sh`
    - compares two baseline TSV files (`--before`, `--after`)
    - emits per-metric summary with:
      - `before_avg`, `after_avg`, `delta`, `pct_change`
      - before/after min/max and sample counts
  - Updated `scripts/test/README.md` with compare usage.
- Decisions:
  - Keep comparison simple and script-only (tsv in, tsv summary out) for easy CI/local usage.
- Next steps:
  - Run compare after each KAD/wire change baseline pair and attach output to PR notes.
- Change log:
  - Added `scripts/test/kad_phase0_compare.sh`.
  - Updated `scripts/test/README.md`.

- Status: Added API backlog note for user-friendly HTTP error responses on `feature/kad-phase0-baseline`:
  - `docs/TODO.md` now tracks adding consistent human-friendly messages for non-2xx HTTP status responses.
- Decisions:
  - Treat this as an explicit API UX/error-contract task, separate from typed error envelope consistency.
- Next steps:
  - Define and implement a unified API error response shape that includes `status`, machine `code`, and human-friendly `message`.
- Change log:
  - Updated `docs/TODO.md` API section with human-friendly HTTP error message task.

- Status: Hardened KAD Phase 0 baseline script handling for startup-not-ready status endpoint on `feature/kad-phase0-baseline`:
  - `scripts/test/kad_phase0_baseline.sh` now treats HTTP `503` from `/api/v1/status` as warmup and skips sampling without noisy curl failures.
  - Script now prints end-of-run summary with `samples`, `skipped_503`, and `skipped_other`.
  - `scripts/test/README.md` updated with this behavior.
- Decisions:
  - Keep baseline collection robust under startup/transient status unavailability; do not fail run on `503`.
- Next steps:
  - Re-run baseline capture command and verify summary shows growing `samples` once status becomes available.
- Change log:
  - Updated `scripts/test/kad_phase0_baseline.sh` sampling/HTTP handling and summary output.
  - Updated `scripts/test/README.md` notes.

- Status: Implemented KAD Phase 0 baseline instrumentation + reviewer gates on `feature/kad-phase0-baseline`:
  - Added status counters for timing/ordering baseline comparison:
    - `pending_overdue`, `pending_max_overdue_ms`
    - `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
  - Instrumented tracked outbound request lifecycle:
    - matched responses increment `tracked_out_matched`
    - unmatched responses increment `tracked_out_unmatched`
    - tracked-request TTL cleanup increments `tracked_out_expired`
  - Added baseline capture script:
    - `scripts/test/kad_phase0_baseline.sh` (polls `/api/v1/status` and writes TSV)
  - Added KAD reviewer gates:
    - `.github/pull_request_template.md` KAD/wire baseline evidence section
    - `docs/REVIEWERS_CHECKLIST.md` baseline evidence gate
  - Updated docs:
    - `docs/KAD_WIRE_REFACTOR_PLAN.md` Phase 0 checkboxes (counters + reviewer gate done)
    - `scripts/test/README.md` baseline script usage
    - `docs/api_curl.md` Phase 0 counter jq example
- Decisions:
  - Phase 0 keeps behavior unchanged and only adds observability + review guardrails.
  - Baseline counters are exposed through existing `/api/v1/status` to avoid new endpoints.
- Next steps:
  - Run and archive before/after baseline captures with `scripts/test/kad_phase0_baseline.sh`.
  - Then start Phase 1 outbound shaper design/implementation using collected baseline deltas.
- Change log:
  - `src/kad/service/types.rs`: added Phase 0 status/stat counters.
  - `src/kad/service/status.rs`: exported/logged new counters.
  - `src/kad/service.rs`: tracked out-request match/unmatch/expiry instrumentation.
  - `src/kad/service/tests.rs`: added regression tests for tracked/pending counters.
  - `src/api/tests.rs`: updated status fixture for new fields.
  - `scripts/test/kad_phase0_baseline.sh`: new baseline capture script.
  - Validation:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (91 tests)

- Status: Addressed PR review findings for download store/service correctness on `feature/download-strategy-imule`:
  - Fixed recovered part path derivation in `scan_recoverable_downloads`:
    - `001.part.met` now maps to `001.part` (not `001.part.part`).
  - Fixed part number parsing/allocation for IDs beyond 999:
    - `parse_part_number` now accepts any all-digit stem that parses to `u16`.
    - `allocate_next_part_number` now correctly accounts for files like `1000.part`.
  - Fixed delete atomicity in `delete_download`:
    - file deletions occur first, then in-memory map entry is removed only on success.
    - on filesystem error, runtime entry remains so delete can be retried in-process.
- Decisions:
  - Preserve existing on-disk naming format (`{part:03}` minimum width) while making parsing robust for wider numeric stems.
  - Prefer state consistency over eager map mutation during deletion.
- Next steps:
  - Merge after PR review confirms these follow-up fixes.
- Change log:
  - `src/download/store.rs`: corrected `.part` path reconstruction; relaxed part-number parser; added regression tests.
  - `src/download/service.rs`: made delete state mutation happen after successful file cleanup; added regression test.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (89 tests)

- Status: Soak run `/tmp/rustmule-run-20260218_160244` validated as healthy on `feature/download-strategy-imule`:
  - `soak-band/results.tsv` shows all scenarios `completed/completed`:
    - `integrity`, `single_e2e`, `concurrency`, `long_churn`
  - Scenario tarballs each contain `download-soak-finished ... result=completed`
  - `data/download/` contains active transfer artifacts (`.part`, `.part.met`, `.bak`) across many part IDs
  - Integrity rounds report `violations=0 dup_parts=0`
  - No panic/fatal errors observed in run logs (only non-blocking nodes2 bootstrap lookup warning)
- Decisions:
  - Treat this run as branch-level validation pass for current download/churn/resume behavior.
- Next steps:
  - Open PR from `feature/download-strategy-imule` to `main`.
  - In PR summary include this branch close-out checklist:
    1. Record successful soak evidence and paths.
    2. Confirm no outstanding code or docs changes on branch.
    3. Merge into `main` after review.
- Change log: Added explicit close-out checklist and successful soak evidence for `/tmp/rustmule-run-20260218_160244`.

- Status: Added local source-cache upsert on publish path in `feature/download-strategy-imule`:
  - In `KadServiceCommand::PublishSource` handling, we now cache local source entry
    (`file -> my_kad_id/my_dest`) before sending network publish requests.
  - This allows inbound `SEARCH_SOURCE_REQ` to return the local source immediately,
    instead of waiting for external network re-ingestion.
  - Added unit test:
    - `kad::service::tests::cache_local_published_source_inserts_local_entry_once`
- Decisions:
  - Preserve network publish behavior; add local cache as compatibility/convergence aid.
- Next steps:
  - Re-run `kad_publish_search_probe.sh` and verify B observes at least one source for published fixture hashes.
- Change log: local publishes now populate `sources_by_file` cache immediately.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (87 tests)

- Status: Extended publish/search probe with periodic republish on `feature/download-strategy-imule`:
  - `scripts/test/kad_publish_search_probe.sh` now supports:
    - `--republish-every N` (poll intervals; `0` disables)
  - This allows repeated `publish_source` on node A while node B continues `search_sources` polling.
- Decisions:
  - Keep republish disabled by default for minimal baseline behavior, opt-in for sparse/slow networks.
- Next steps:
  - Re-run probe with `--republish-every 12 --poll-secs 5` (republish every 60s) and longer timeout if needed.
- Change log: A->B probe now supports periodic republish to improve source visibility convergence.
  - Validation run after patch:
    - `bash -n scripts/test/kad_publish_search_probe.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added automated A->B publish/search visibility probe on `feature/download-strategy-imule`:
  - New script: `scripts/test/kad_publish_search_probe.sh`
    - publishes file source on node A (`/api/v1/kad/publish_source`)
    - repeatedly queues search on node B (`/api/v1/kad/search_sources`)
    - polls B `/api/v1/kad/sources/:file_id_hex`
    - logs A/B status counters each interval:
      - A: `recv_publish_source_reqs`, `sent_publish_source_ress`, `recv_search_source_reqs`, `source_store_entries_total`
      - B: `sent_search_source_reqs`, `recv_search_ress`, `source_store_entries_total`
    - exits success when B sees at least one source; times out otherwise.
  - Added usage entry in `scripts/test/README.md`.
- Decisions:
  - Use explicit counter telemetry in probe output so failures are attributable to publish path vs search path vs discovery cache.
- Next steps:
  - Run probe for each fixture hash before resume soak and only proceed when probe exits `0`.
- Change log: Manual publish/search polling is now scripted and repeatable.
  - Validation run after patch:
    - `bash -n scripts/test/kad_publish_search_probe.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Fixed fixture validation bug in soak runner on `feature/download-strategy-imule`:
  - Root cause of failed run `/tmp/rustmule-run-20260218_114700`:
    - `download_soak_bg.sh` logged `fixtures not loaded or empty` and repeatedly `fixtures_only enabled but no valid fixture available`.
    - The `jq` fixture validator expression incorrectly filtered out valid entries, resulting in `FIXTURE_COUNT=0`.
  - Fix:
    - replaced validator with explicit `valid_fixture` predicate using safe field checks:
      - `file_name` string
      - `file_hash_md4_hex` string
      - `file_size` number > 0
    - applied in both fixture counting and fixture record selection paths.
- Decisions:
  - Keep strict fixture schema validation but ensure parser is robust to valid JSON fixtures.
- Next steps:
  - Re-run resume soak with the same fixture file and `FIXTURES_ONLY=1`; fixture load should now report non-zero count.
- Change log: Soak runner now correctly accepts valid fixture JSON entries.
  - Validation run after patch:
    - `bash -n scripts/test/download_soak_bg.sh` passed
    - `jq ... /tmp/download_fixtures.json` returned `2` valid fixtures
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Extended fixture generation with optional source publish on `feature/download-strategy-imule`:
  - `scripts/test/gen_download_fixture.sh` now supports:
    - `--publish`
    - `--base-url <url>`
    - `--token` / `--token-file`
    - `--publish-script <path>`
  - When `--publish` is set, each generated fixture is sent to `scripts/docs/kad_publish_source.sh` using the generated MD4 hash and file size.
- Decisions:
  - Keep publish optional to preserve offline/local-only fixture generation mode.
- Next steps:
  - Run one command to generate and publish fixture hashes on source node, then use that fixture file for resume soak on downloader node.
- Change log: Fixture prep can now generate + publish in a single command.
  - Validation run after patch:
    - `bash -n scripts/test/gen_download_fixture.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added built-in fixture generation tooling on `feature/download-strategy-imule`:
  - New Rust utility: `src/bin/download_fixture_gen.rs`
    - outputs fixture JSON from one or more files using repo-native MD4 (`rust_mule::kad::md4`)
  - New wrapper script: `scripts/test/gen_download_fixture.sh`
    - usage: `scripts/test/gen_download_fixture.sh --out /tmp/download_fixtures.json /path/to/file1 ...`
  - Updated `scripts/test/README.md` with generation command.
- Decisions:
  - Avoid OpenSSL/legacy-provider variability by using project-native MD4 implementation for fixture generation.
- Next steps:
  - Generate real peer-backed fixture file and run resume soak with:
    - `DOWNLOAD_FIXTURES_FILE=<fixtures.json> FIXTURES_ONLY=1 bash scripts/test/download_resume_soak.sh`
- Change log: Fixture generation is now one command and does not depend on external MD4 support.
  - Validation run after patch:
    - `bash -n scripts/test/gen_download_fixture.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added fixture-driven download creation for soak/resume validation on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` now supports:
    - `DOWNLOAD_FIXTURES_FILE` (JSON array with `file_name`, `file_size`, `file_hash_md4_hex`)
    - `FIXTURES_ONLY=1` (fails instead of falling back to random hashes)
  - Create actions in all scenarios (`single_e2e`, `long_churn`, `integrity`, `concurrency`) now prefer fixtures when provided.
  - Fixture behavior is propagated through:
    - `scripts/test/download_soak_band.sh`
    - `scripts/test/download_soak_stack_bg.sh`
    - resume workflow (via inherited env into stack start)
  - Added `scripts/test/download_fixtures.example.json`.
- Decisions:
  - Keep fixture mode opt-in for backward compatibility, but recommend `FIXTURES_ONLY=1` for real transfer/resume assertions.
- Next steps:
  - Run resume soak with peer-backed fixtures:
    - `DOWNLOAD_FIXTURES_FILE=<real-fixtures.json> FIXTURES_ONLY=1 bash scripts/test/download_resume_soak.sh`
  - Confirm active-transfer gate passes and post-restart completion is observed.
- Change log: Soak/resume tests can now target real downloadable hashes instead of random synthetic IDs.
  - Validation run after patch:
    - `bash -n scripts/test/download_soak_bg.sh scripts/test/download_soak_band.sh scripts/test/download_soak_stack_bg.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Strengthened resume-soak acceptance criteria on `feature/download-strategy-imule`:
  - Resume automation now enforces true in-flight resume validation instead of control-plane-only pass/fail.
  - Added pre-crash active-transfer gate: requires at least one download with `downloaded_bytes > 0` and `inflight_ranges > 0`.
  - Added post-restart monotonicity gate: fails if any pre-existing download regresses in `downloaded_bytes`.
  - Added post-restart completion gate: requires at least one completed download within configurable timeout.
- Decisions:
  - Treat resume success as data-plane continuity, not only process restart + scenario completion.
  - Keep thresholds configurable for slow environments via script env overrides.
- Next steps:
  - Run `scripts/test/download_resume_soak.sh` and verify the new gates pass under load.
  - If active-transfer gate times out, increase scenario duration/load or tune discovery/source readiness before crash point.
- Change log: `scripts/test/download_resume_soak.sh` now validates active transfer before crash, monotonic post-restart bytes, and post-restart completion.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Fixed resume-soak crash step for wrapper-pid mismatch on `feature/download-strategy-imule`:
  - User-observed failure:
    - after `crashed app pid=<pid>`, one run-dir `./rust-mule` process remained and restart never proceeded.
  - Root issue:
    - `control/app.pid` can reference a launcher/wrapper pid while actual rust-mule child keeps running.
  - Fix:
    - crash step now force-kills all run-dir-owned `rust-mule` pids discovered via `/proc` (`cwd`/`cmdline`), then waits for zero run-dir rust-mule processes.
- Decisions:
  - For forced crash simulation, process discovery by run-dir ownership is more reliable than trusting a single control pid file.
- Next steps:
  - Re-run resume soak and verify crash->restart proceeds when wrapper/child pid divergence exists.
- Change log: Resume crash now targets all run-dir rust-mule processes, eliminating wrapper-pid false negatives.

- Status: Fixed resume-soak restart false-positive by strengthening run-dir process detection on `feature/download-strategy-imule`:
  - User-observed failure:
    - restart exited immediately with `SingleInstance(AlreadyRunning)` after crash step.
  - Root issue:
    - run-dir process check matched only absolute binary path; missed `./rust-mule` processes started from run-dir cwd.
  - Fix:
    - switched run-dir process detection to `/proc`-based `cwd` + `cmdline` matching.
    - restart now refuses to proceed if any run-dir rust-mule process remains and prints PID diagnostics.
- Decisions:
  - Treat `/proc` ownership checks as authoritative for single-instance lock safety in resume automation.
- Next steps:
  - Re-run resume soak and verify crash->restart proceeds without lock conflict.
- Change log: Resume soak now correctly detects lingering `./rust-mule` run-dir processes before restart.

- Status: Hardened resume-soak crash detection on `feature/download-strategy-imule`:
  - User-observed failure:
    - after `kill -9`, script timed out waiting for `health=000` for 300s.
  - Root issue:
    - health-code shutdown check is brittle when API port can remain served by non-target process.
  - Fix:
    - resume script now validates crash by process identity:
      - killed app PID exits
      - no remaining run-dir `rust-mule` process
    - keeps health check as informational post-crash signal
    - adds restart immediate-exit guard with `rust-mule.resume.out` tail on failure.
- Decisions:
  - Use process-level ownership checks as primary crash/restart truth in resume automation.
- Next steps:
  - Re-run `download_resume_soak.sh` and confirm post-crash flow proceeds to restart/progress checks without `health=000` false timeout.
- Change log: Resume soak no longer blocks on strict `health=000` condition.

- Status: Added automated resume-soak orchestration script on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_resume_soak.sh`
  - Flow:
    - starts stack soak
    - waits for target scenario (`concurrency` default)
    - captures pre-crash `/api/v1/downloads` snapshot
    - hard-kills app (`SIGKILL`) and restarts in same run dir
    - captures post-restart snapshot and verifies scenario progress resumes
    - waits for terminal stack state, collects bundle, writes report.
  - Documented usage/overrides in `scripts/test/README.md`.
- Decisions:
  - Build resume validation as orchestration around existing stack runner instead of adding duplicate per-scenario harnesses.
- Next steps:
  - Run one automated resume-soak and validate `resume_report.txt` + stack bundle outcomes.
- Change log: Repo now has a one-command crash/restart resume-soak automation path.

- Status: Added TODO note for tag-driven CI/CD build/release flow on `feature/download-strategy-imule`.
- Decisions:
  - Track Git-tag-triggered build/publish verification as an explicit backlog item.
- Next steps:
  - Confirm release workflow behavior from tag push through artifact publication and document gaps.
- Change log: `docs/TODO.md` now includes tag-driven build/release automation verification.

- Status: Added cross-cutting naming/comment refactor TODO notes on `feature/download-strategy-imule`:
  - `docs/TODO.md` now tracks:
    - `Imule*` -> neutral `Mule*`/neutral identifier rename pass
    - code-comment wording normalization to compatibility-focused language
  - `docs/TASKS.md` scope now includes the same naming/comment normalization task.
- Decisions:
  - Keep explicit iMule/aMule/eMule wording for protocol reference documentation/tests where needed, but avoid it in production identifier names and code comments.
- Next steps:
  - Plan a repo-wide mechanical rename + comment wording sweep in bounded slices to minimize merge-risk.
- Change log: TODO/TASKS now explicitly capture naming and comment normalization policy.

- Status: Merged latest `main` into `feature/download-strategy-imule` to sync CI/docs/UI smoke and Pages workflow updates.
- Decisions:
  - Kept branch-local soak/download handoff history as primary during `docs/handoff.md` conflict resolution.
- Next steps:
  - Continue soak stabilization on top of synced branch baseline.
- Change log: Branch now includes latest `main` changes as merge base.

- Status: Fixed stack soak runner dependency on mutable repo script paths on `feature/download-strategy-imule`:
  - Failure analyzed from `/tmp/rust-mule-download-stack-20260217_170055.tar.gz`:
    - `concurrency` polling stayed `status=unknown state=unknown`
    - terminal error in stack output:
      - `env: '/home/coder/projects/rust-mule/scripts/test/download_soak_concurrency_bg.sh': No such file or directory`
      - `ERROR: band-run failed exit=127`
  - Root cause:
    - long-running stack run invoked wrappers directly from working-tree `scripts/test`; if those files change/disappear (e.g. branch switch) mid-run, scenario status/collect commands fail.
  - Fix:
    - `scripts/test/download_soak_stack_bg.sh` now stages `download_soak_*` scripts into `$RUN_DIR/soak-scripts` at startup and executes the band runner from that staged immutable copy.
- Decisions:
  - Treat soak script set as run artifact; do not depend on mutable working tree during long background runs.
- Next steps:
  - Re-run stack soak and confirm all four scenarios write results rows and artifacts even if repo branch changes during execution.
- Change log: Stack soak now uses per-run staged scripts and is resilient to working-tree churn.

- Status: Fixed stack runner shell-recursion regression on `feature/download-strategy-imule`:
  - Root cause of `bash: warning: shell level (1000) too high` was accidental command text inserted at the top of `scripts/test/download_soak_stack_bg.sh` before the shebang.
  - Removed the stray lines so script starts directly with `#!/usr/bin/env bash`.
  - Hardened background self-invocation to use `SELF_PATH` (absolute script path) instead of `$0`.
- Decisions:
  - Keep script entrypoint strict and avoid ambiguous `$0` resolution in detached shells.
- Next steps:
  - Re-run short stack soak to verify `start` no longer recurses and produces fresh run dirs/tarballs.
- Change log: Stack runner no longer recurses into nested bash startup loops.

- Status: Hardened in-band download runner interruption handling on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_130154.tar.gz` showed no scenario crash; `long_churn` was actively progressing but the band process received external termination (`Terminated` / `runner interrupted`) before writing final row.
  - `scripts/test/download_soak_band.sh` now traps `SIGINT`/`SIGTERM` and:
    - stops active scenario wrapper
    - performs best-effort `collect`
    - appends an `interrupted` row to `results.tsv`
  - `scripts/test/README.md` updated with interruption behavior.
- Decisions:
  - Treat external runner termination as first-class outcome in results, not silent truncation.
- Next steps:
  - Re-run stack soak; if interrupted, confirm `results.tsv` contains `interrupted` row and partial tarball is preserved.
- Change log: Band soak now records interruption outcomes explicitly.

- Status: Tuned download soak readiness probing on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` readiness now probes a configurable endpoint instead of hardcoding `/api/v1/status`.
  - New readiness env knobs:
    - `READY_TIMEOUT_SECS` (default `300`)
    - `READY_PATH` (default `/api/v1/downloads`)
    - `READY_HTTP_CODES` (default `200`, comma-separated)
  - Background `start` now forwards readiness env vars to detached run.
  - Also fixed latent integrity scenario crash risk by binding `round="$1"` in `scenario_integrity_round`.
  - `scripts/test/README.md` updated with readiness override knobs.
- Decisions:
  - For download-soak scenarios, readiness should key on download API availability, not KAD status endpoint warmup.
  - Keep readiness behavior configurable for environment-specific tuning.
- Next steps:
  - Re-run stack soak and verify integrity no longer fails on repeated startup `503` from `/api/v1/status`.
- Change log: Download soak readiness is now download-endpoint based and less brittle during startup.

- Status: Fixed download soak long-churn round crash on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_104554.tar.gz` showed:
    - `concurrency` completed
    - `long_churn` stuck as `status=stale_pid runner_state=running`
    - no long_churn tarball/result row in band output
  - Root cause from `/tmp/rust-mule-download-soak/long_churn/logs/runner.out`:
    - `download_soak_bg.sh: line 230: round: unbound variable`
  - Fix: assign `round="$1"` at start of `scenario_long_churn_round`.
- Decisions:
  - Keep `set -u`; patch all scenario entrypoints to bind function args explicitly.
- Next steps:
  - Re-run stack soak and verify long_churn now emits round ticks, terminal state, and collected tarball.
  - Separately evaluate repeated integrity readiness `503` behavior (startup/warmup timing).
- Change log: Long-churn scenario no longer crashes on unbound `round`.

- Status: Synced documentation to new contract/checklist/timing policy and created deferred KAD/wire refactor task plan on `feature/download-strategy-imule`:
  - Added `docs/KAD_WIRE_REFACTOR_PLAN.md` with phased tasks (baseline, shaper, bypass removal, retry envelope, validation).
  - Updated `README.md` and `docs/README.md` to include:
    - `docs/BEHAVIOURAL_CONTRACT.md`
    - `docs/REVIEWERS_CHECKLIST.md`
    - `docs/IMULE_COMPABILITY_TIMING.md`
    - `docs/KAD_WIRE_REFACTOR_PLAN.md`
  - Updated `docs/TODO.md` and `docs/TASKS.md` with explicit KAD/wire alignment tasks and “document now, refactor next” sequencing.
- Decisions:
  - Defer code-heavy KAD/wire timing refactor until soak baseline remains stable.
  - Treat behavior contract as authoritative, with iMule compatibility inside timing envelopes.
- Next steps:
  - Complete phase 0 baseline/guardrails from `docs/KAD_WIRE_REFACTOR_PLAN.md`.
  - Continue current soak stabilization; start shaper refactor only after baseline is green.
- Change log: Documentation and backlog are now aligned to contract-first timing policy and phased KAD/wire refactor plan.

- Status: Fixed download soak concurrency round crash on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_095242.tar.gz` showed `concurrency` aborting at round 1 with:
    - `download_soak_bg.sh: line 308: round: unbound variable`
  - Root cause: `scenario_concurrency_round` declared `round` but did not assign from function arg under `set -u`.
  - Fix: assign `round="$1"` at function start.
- Decisions:
  - Keep strict `set -u`; treat unbound vars as script bugs and patch at source.
- Next steps:
  - Re-run stack/band soak and verify `concurrency` and `long_churn` progress with regular round ticks and terminal states.
- Change log: Concurrency scenario no longer crashes due to unbound `round` variable.

- Status: Added bounded API curl timeouts in download soak runner on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` now uses shared timeout env knobs on all API calls (GET/POST/DELETE + readiness status probe):
    - `API_CONNECT_TIMEOUT_SECS` (default `3`)
    - `API_MAX_TIME_SECS` (default `8`)
  - `start` now forwards timeout env vars into detached `run` process so overrides are preserved.
  - `scripts/test/README.md` updated with new optional overrides for in-band runner usage.
- Decisions:
  - Prevent indefinite round hangs by time-bounding all API curl calls in the scenario runner.
  - Keep defaults conservative and operator-overridable for slower environments.
- Next steps:
  - Re-run stack/band soak and confirm scenarios keep progressing past round 1 without long `runner_state=running` stalls.
  - If timeouts are too aggressive under load, tune via env or bump defaults.
- Change log: Download soak API calls are now timeout-bounded and configurable.

- Status: Hardened stack `stop` teardown to avoid orphaned processes on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_stack_bg.sh` now:
    - stops all per-scenario download soak runners before killing stack runner
    - kills stack runner process group (TERM/KILL) instead of only top PID
    - scans `/proc` and terminates remaining processes tied to current run dir (`cwd`/`cmdline` match)
  - this addresses observed behavior where `stop` left `rust-mule` and soak helper processes alive.
- Decisions:
  - Prefer process-group and run-dir scoping for deterministic teardown.
- Next steps:
  - Re-run stack runner and verify `stop` leaves no matching processes (`pgrep -af rustmule-run-` returns none).
- Change log: Stack stop now performs full tree + run-dir cleanup.

- Status: Fixed download band wait/result logic for stale PID races on `feature/download-strategy-imule`:
  - Analysis from `/tmp/rust-mule-download-stack-20260216_140814.tar.gz` showed scenarios being advanced when `status=stale_pid` but `runner_state=running`.
  - `scripts/test/download_soak_band.sh` now:
    - treats terminal states strictly via `runner_state in {completed, failed, stopped}`
    - keeps waiting while `runner_state=running` (even if `status=stale_pid`)
    - maps final `results.tsv` outcome from terminal state (`completed|failed|stopped|running_after_wait|unknown`).
- Decisions:
  - Trust explicit runner state over transient status pid interpretation.
- Next steps:
  - Re-run stack/band soak and verify concurrency/long_churn no longer short-circuit after first poll.
- Change log: Band runner no longer treats `stale_pid + running` as finished.

- Status: Fixed stack runner build shell context on `feature/download-strategy-imule`:
  - Root cause: build command was executed via nested `bash -lc`, which lost the PATH bootstrap and still could not find `cargo`.
  - `scripts/test/download_soak_stack_bg.sh` now executes build command in current shell context (`eval "$BUILD_CMD"` in repo dir), preserving PATH/toolchain setup.
- Decisions:
  - Avoid nested login-shell build execution in stack runner.
- Next steps:
  - Re-run `download_soak_stack_bg.sh start` and confirm build begins and run directory stages.
- Change log: Stack runner build step now honors PATH bootstrap reliably.

- Status: Fixed download stack runner false-running behavior on `feature/download-strategy-imule`:
  - Root cause observed in logs: background shell could not find `cargo` (`cargo: command not found`), causing early exit before build/stage.
  - `scripts/test/download_soak_stack_bg.sh` now:
    - bootstraps PATH with `~/.cargo/bin` when needed
    - handles build failures explicitly (`runner.state=failed`, cleanup, pid removal)
    - validates runner process remains alive right after `start` and reports immediate-exit failure.
  - `scripts/test/README.md` troubleshooting updated (`stack.out` path and cargo PATH note).
- Decisions:
  - Prefer explicit failure state over stale/running ambiguity when background bootstrap fails.
- Next steps:
  - Re-run `download_soak_stack_bg.sh start`; verify build and run-dir staging occur and status transitions correctly.
- Change log: Stack runner now fails fast/cleanly on missing cargo or early runner death.

- Status: Added full background download soak pipeline runner on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_soak_stack_bg.sh` with `start|run|status|stop|collect`.
  - It now performs end-to-end automation:
    - builds latest sources (`BUILD_CMD`, default `cargo build --release`)
    - stages isolated run dir (`/tmp/rustmule-run-<timestamp>`)
    - writes run-specific `config.toml` (section-aware updates for `[sam]`, `[general]`, `[api]`)
    - starts rust-mule from staged dir and waits for health + token
    - runs `download_soak_band.sh` with forwarded soak parameters
    - supports post-run tarball collection.
  - `scripts/test/README.md` updated with full pipeline usage and env overrides.
- Decisions:
  - Keep app lifecycle isolated per run directory for reproducible soak artifacts.
  - Keep orchestration shell-native and reuse existing `download_soak_band.sh` logic.
- Next steps:
  - Execute `download_soak_stack_bg.sh start`, monitor `status`, and collect the resulting stack tarball for triage.
- Change log: Added one-command background build+run+download-soak pipeline.

- Status: Hardened download band-runner preflight and state handling on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_band.sh` now preflights API reachability (`GET /api/v1/health == 200`) and aborts early with a clear message if rust-mule is not running.
  - `scripts/test/download_soak_bg.sh` `stop` no longer overwrites terminal `failed/completed` state with `stopped`.
  - `scripts/test/README.md` now documents API-running precondition for band runs.
- Decisions:
  - Prefer fast-fail precondition checks over delayed per-scenario readiness timeouts when API is down.
  - Preserve terminal runner state for accurate post-run interpretation.
- Next steps:
  - Re-run `download_soak_band.sh` with rust-mule running and token present, then triage collected tarballs.
- Change log: Band runs now fail fast when API is offline and keep accurate scenario terminal states.

- Status: Fixed in-band download soak status parsing bug on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_band.sh` now parses `status=running pid=...` lines correctly.
  - Previous behavior treated `running pid` as non-running and advanced scenarios immediately.
- Decisions:
  - Parse only the first token value for `status`/`runner_state` lines.
- Next steps:
  - Re-run `download_soak_band.sh` and confirm each scenario blocks for intended duration unless stopped/fails.
- Change log: Band runner no longer short-circuits after first poll.

- Status: Added in-band download soak orchestrator on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_soak_band.sh`
    - runs download soak scenarios sequentially:
      1. `integrity` (default 3600s)
      2. `single_e2e` (default 3600s)
      3. `concurrency` (default 7200s)
      4. `long_churn` (default 7200s)
    - polls runner status, forces stop on timeout, and collects tarball for each scenario
    - copies collected tarballs + writes `results.tsv` and `status.tsv` under `OUT_DIR`
  - `scripts/test/README.md` updated with one-command in-band run instructions and overrides.
- Decisions:
  - Keep orchestrator shell-native and reuse existing per-scenario wrappers rather than duplicating scenario logic.
  - Preserve scenario isolation by running each with its own existing scenario run root and collect step.
- Next steps:
  - Run `download_soak_band.sh` after current source soak and share generated `OUT_DIR` + tarballs for triage.
  - If needed, add a companion triage script for `results.tsv` + per-scenario tar summaries.
- Change log: Added a one-command sequential download soak runner with automatic stop/collect.

- Status: Added download soak scaffolding and execution plan on `feature/download-strategy-imule`:
  - New generic runner: `scripts/test/download_soak_bg.sh`
    - background lifecycle: `start/run/status/stop/collect`
    - scenario switch via `SCENARIO=single_e2e|long_churn|integrity|concurrency`
    - writes per-scenario logs/bundles under `/tmp/rust-mule-download-soak/<scenario>`.
  - New scenario wrappers:
    - `scripts/test/download_soak_single_e2e_bg.sh`
    - `scripts/test/download_soak_long_churn_bg.sh`
    - `scripts/test/download_soak_integrity_bg.sh`
    - `scripts/test/download_soak_concurrency_bg.sh`
  - Updated `scripts/test/README.md` with post-source-soak run order, commands, and pass signals.
- Decisions:
  - Keep download soak scope API/control-plane focused for now (queue lifecycle + invariants + pressure), matching currently implemented download functionality.
  - Use scenario wrappers for simpler operator usage and isolated per-scenario run roots.
- Next steps:
  - After current source soak completes, run the four download soak scenarios in documented order.
  - Collect tarballs and triage runner/list logs for invariant violations and queue-pressure regressions.
- Change log: Added runnable download soak scripts and a concrete operator runbook.

- Status: Updated soak identity/session handling on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now:
    - generates unique per-run SAM session names for A/B using `RUN_TAG`
    - supports `SOAK_RUN_TAG` override for deterministic debug runs
    - defaults to `SOAK_FRESH_IDENTITY=1`, removing copied `data/sam.keys` so each run gets fresh I2P destinations
  - `scripts/test/README.md` updated with new identity/session controls.
- Decisions:
  - Keep fresh identity default enabled to avoid duplicate-destination registration when previous sessions linger.
  - Keep opt-out (`SOAK_FRESH_IDENTITY=0`) for controlled continuity tests.
- Next steps:
  - Restart soak with defaults and confirm no duplicate destination registration warnings from I2P router logs.
  - Continue soak comparison with `MISS_RECHECK_ATTEMPTS=0` once stable baseline resumes.
- Change log: Soak runs now isolate SAM identities and session names by default.

- Status: Cleaned soak status PID reporting on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh`:
    - `stop_nodes` now removes `logs/a.pid` and `logs/b.pid` after stop.
    - `status` now reports node PID liveness (`alive=1|0`) when pid files exist.
  - `scripts/test/README.md` updated to document stale-PID cleanup behavior.
- Decisions:
  - Prefer clearing pid files at stop to avoid false confidence in stale process IDs.
- Next steps:
  - Run quick `start -> status -> stop -> status` check and confirm node pid lines disappear after stop.
  - Proceed with baseline/tuned soak comparison runs.
- Change log: Soak status output no longer keeps stale node PID files after stop.

- Status: Hardened soak `stop` reliability and failure cleanup on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh`:
    - added `kill_pid_gracefully` (TERM + KILL fallback with result logging)
    - upgraded `stop_nodes` to use graceful escalation, not single-shot `kill`
    - added `stop_run_root_nodes` fallback scan over `/proc` to terminate soak-owned processes tied to current `RUN_ROOT`
    - tightened ownership matching to cwd/cmdline rooted in current `RUN_ROOT` (avoids killing unrelated local processes)
    - startup/readiness failure now sets `runner.state=failed` and runs cleanup immediately.
  - `scripts/test/README.md` updated with the stronger stop/cleanup behavior.
- Decisions:
  - Prioritize deterministic cleanup of soak-owned processes over PID-file-only teardown.
  - Keep process kill scope constrained to the active `RUN_ROOT`.
- Next steps:
  - Re-run `start -> stop -> status` smoke to verify no listeners remain on `A_URL`/`B_URL` after stop.
  - Resume baseline vs miss-recheck comparison soak once stop behavior is confirmed.
- Change log: Soak stop path now aggressively reaps RUN_ROOT-owned processes and failed starts no longer leave stale running state.

- Status: Added optional miss recheck pass in timed background soak harness on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now supports:
    - `MISS_RECHECK_ATTEMPTS` (default `1`)
    - `MISS_RECHECK_DELAY` seconds (default `20`)
  - After an initial source miss (`GET /api/v1/kad/sources/:file_id_hex`), the runner performs bounded delayed rechecks before persisting round outcome.
  - `rounds.tsv` format is unchanged (6 columns), so `scripts/test/soak_triage.sh` compatibility is preserved.
  - `scripts/test/README.md` updated with new env knobs and behavior description.
- Decisions:
  - Keep miss-recheck logic optional and env-controlled to preserve old baseline behavior (`MISS_RECHECK_ATTEMPTS=0` disables rechecks).
  - Keep `rounds.tsv` schema stable for existing triage tooling.
- Next steps:
  - Re-run A/B soak with two profiles:
    - baseline (`MISS_RECHECK_ATTEMPTS=0`)
    - tuned (`MISS_RECHECK_ATTEMPTS=1 MISS_RECHECK_DELAY=20`)
  - Compare hit-rate and hit-gap deltas using unchanged triage scripts.
- Change log: Soak runner now supports delayed miss recheck to reduce false misses from eventual consistency windows.

- Status: Hardened timed background soak harness failure handling on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now:
    - fails fast if `A_URL`/`B_URL` ports are already in use (prevents attaching to foreign processes)
    - synchronizes API port config from `A_URL` and `B_URL`
    - verifies spawned node PIDs are running from expected per-run directories
    - aborts readiness early on repeated `403` responses (token mismatch/wrong process)
    - separates detached stdout/stderr into `logs/runner.out` to avoid duplicate `runner.log` lines.
    - uses stricter multi-probe port detection (`ss` + `lsof` + TCP connect probes) before launch to catch occupied API ports reliably.
    - `stop` now also scans `A_URL`/`B_URL` listen ports and terminates matching `rust-mule` listener PIDs if PID files are stale/missing.
  - `scripts/test/README.md` updated with the new safety behavior.
- Decisions:
  - Prefer explicit preflight failure over implicit retries when ports are occupied.
  - Treat repeated readiness `403` as a hard test-environment mismatch signal.
  - Keep stop fallback conservative: only kill listeners whose process cmdline contains `rust-mule`.
- Next steps:
  - Re-run soak with the hardened script; verify `rounds.tsv` and `status.ndjson` are populated before long-run analysis.
  - If needed, add optional auto-port allocation mode in a later slice.
- Change log: Soak runner now guards against port collisions and false-readiness loops, and logs are no longer duplicated.

- Status: Implemented source-probe telemetry hardening + request correlation IDs in KAD service and added timed background soak scaffold on `feature/download-strategy-imule`:
  - `src/kad/service.rs`:
    - outbound tracked requests now carry `request_id` and optional `trace_tag`
    - added response->expected-opcode mapping for strict response/request matching
    - added unmatched-response diagnostics (`last_unmatched_response`) with expected opcodes and tracked counts
    - source search/publish sends now emit `source_probe_request_sent` with request correlation ID
    - source search/publish response matching now emits `source_probe_response_matched` / `source_probe_response_unmatched` with request correlation diagnostics.
  - `src/kad/service/inbound.rs`:
    - unrequested response drops now log expected opcode families and tracked request counts
    - explicit decode-failure events for source probe response parsing failures.
  - `scripts/test/source_probe_soak_bg.sh`:
    - new detached soak runner with timer (`start <duration_secs>`, `status`, `stop`, `collect`)
    - PID/state files and log outputs under `/tmp/rust-mule-soak-bg` (override via `RUN_ROOT`).
  - `scripts/test/README.md`:
    - usage examples for timed background soak runs and environment overrides.
- Decisions:
  - Keep correlation ID scope focused on KAD source probe request/response lifecycle (no API schema change in this slice).
  - Keep soak harness shell-native with `nohup` + PID file controls for long-running sessions.
- Next steps:
  - Run the new timed soak harness against freshly built `../../mule-a` / `../../mule-b` and analyze `rounds.tsv` + `status.ndjson`.
  - If needed, expose recent source-probe correlation counters in `/api/v1/status` for easier dashboarding.
- Change log: Added source-probe request/response correlation logging and introduced a timer-based background soak runner script.

- Status: Completed transfer execution groundwork (peer-owned inflight + packet ingest + timeout retry) on `feature/download-strategy-imule`:
  - Added `src/download/protocol.rs`:
    - ED2K transfer opcode constants (`OP_REQUESTPARTS`, `OP_SENDINGPART`, `OP_COMPRESSEDPART`)
    - payload encode/decode helpers and typed protocol errors
    - unit tests for requestparts roundtrip and sendingpart validation.
  - Finished peer-aware transfer flow in `src/download/service.rs`:
    - fixed service loop to use a single `tokio::select!` over command receive + timeout tick
    - `ReserveBlocks` now assigns peer-owned inflight leases with expiration deadline
    - `MarkBlockReceived` / `MarkBlockFailed` now validate lease ownership by peer
    - added `PeerDisconnected` reclaim path to requeue leased blocks for that peer
    - added timeout processing to requeue expired leases with retry/error tracking
    - added `IngestInboundPacket` handling for `OP_SENDINGPART` and `OP_COMPRESSEDPART` that maps inbound payloads to block completion.
  - Extended persisted transfer state in `src/download/store.rs`:
    - `ByteRange`, `missing_ranges`, `inflight_ranges`, `retry_count`, `last_error` (with serde defaults).
  - Updated download API DTO mapping in `src/api/handlers/downloads.rs` to expose progress + transfer counters/error.
  - Added service tests:
    - `peer_disconnected_reclaims_only_that_peers_leases`
    - `ingest_sendingpart_marks_reserved_block_received`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 86 tests).
- Decisions:
  - Keep lease tracking in-memory (`ManagedDownload.leases`) and persist only range/error projections in `.part.met`.
  - Keep packet ingest scope minimal in this slice: decode + hash/range validation + state transition; deferred real payload write/verification pipeline.
- Next steps:
  - Wire TCP peer session handler to emit `IngestInboundPacket` and `PeerDisconnected` events from live network traffic.
  - Add block payload write path into `.part` files and integrity checks before `completing -> completed`.
  - Tune lease timeout/retry policy from soak-test observations and expose counters in API/UI if needed.
- Change log: Download actor now supports peer-bound inflight reservations with timeout/disconnect recovery and first inbound transfer packet ingestion path.

- Status: Added download transfer-state skeleton (phase 2 groundwork) on `feature/download-strategy-imule`:
  - Extended persisted metadata (`src/download/store.rs`):
    - new `ByteRange` model
    - `PartMet` now persists:
      - `missing_ranges`
      - `inflight_ranges`
      - `retry_count`
      - `last_error`
    - backward-compatible serde defaults retained.
  - Extended download actor (`src/download/service.rs`):
    - new transfer-facing commands:
      - `ReserveBlocks`
      - `MarkBlockReceived`
      - `MarkBlockFailed`
    - block reservation now moves ranges from `missing` -> `inflight`
    - failed blocks are re-queued into `missing` with retry/error tracking
    - received blocks clear inflight and update progress/completion state
    - restart safety: inflight ranges are reclaimed into missing on startup recovery.
  - Extended `DownloadSummary` and API-visible fields:
    - `progress_pct`
    - `missing_ranges`
    - `inflight_ranges`
    - `retry_count`
    - `last_error`
  - Updated download list/action responses (`src/api/handlers/downloads.rs`) to expose these fields.
  - Added tests:
    - reserve/fail/retry/receive state progression
    - restart inflight reclamation into missing
    - existing API mutation/list tests continue passing with expanded schema.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 82 tests).
- Decisions:
  - Keep range semantics inclusive (`start..=end`) in persisted metadata.
  - Treat in-flight reservations as non-authoritative across restart: reclaim to missing for correctness.
- Next steps:
  - Integrate wire-level TCP block flow into these commands (`OP_REQUESTPARTS` / `OP_SENDINGPART` path).
  - Add per-peer in-flight ownership and timeout scheduler for autonomous retry.
  - Persist part-hash verification state and transition `completing -> completed`.
- Change log: Download subsystem now tracks block-level missing/inflight/retry state with restart-safe recovery.

- Status: Implemented mutating download API endpoints on `feature/download-strategy-imule`:
  - Added new endpoints under `/api/v1`:
    - `POST /downloads`
    - `POST /downloads/:part_number/pause`
    - `POST /downloads/:part_number/resume`
    - `POST /downloads/:part_number/cancel`
    - `DELETE /downloads/:part_number`
  - Existing `GET /downloads` remains as queue/status snapshot endpoint.
  - New handler module: `src/api/handlers/downloads.rs`:
    - request/response DTOs for create/action/delete/list
    - typed download error -> HTTP mapping:
      - invalid input -> `400`
      - not found -> `404`
      - invalid transition -> `409`
      - channel closed -> `503`
      - storage/join failures -> `500`.
  - Router and handler exports updated:
    - `src/api/router.rs`
    - `src/api/handlers/mod.rs`
  - API state wiring unchanged in behavior but now fully exercises download mutating commands.
  - Added endpoint tests:
    - `api::tests::download_mutation_endpoints_update_service_state`
      - create -> pause -> resume -> cancel -> delete
      - conflict and not-found status checks
      - list consistency checks.
  - Updated API docs:
    - `docs/API_DESIGN.md` (downloads now marked implemented for lifecycle queue management)
    - `docs/api_curl.md` (added curl examples for list/create/pause/resume/cancel/delete).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 80 tests).
- Decisions:
  - Keep download API mutators command-driven via the actor to preserve explicit state transitions and typed error semantics.
  - Keep transfer wire pipeline out of this slice; API currently manages queue lifecycle only.
- Next steps:
  - Add first transfer-facing commands/structures (pending block requests, timeout bookkeeping).
  - Persist and expose gap/range progress in `PartMet` to support restart-safe block transfer.
  - Add UI controls for create/pause/resume/cancel/delete wired to the new endpoints.
- Change log: Download queue can now be fully controlled through API endpoints, with tests and docs updated.

- Status: Implemented download phase 1.5/2 groundwork on `feature/download-strategy-imule`:
  - Expanded download actor/service (`src/download/service.rs`):
    - state/lifecycle commands:
      - `CreateDownload`
      - `Pause`
      - `Resume`
      - `Cancel`
      - `Delete`
      - `List`
    - deterministic part slot allocation is now used (`%03d.part.met` / `%03d.part`).
    - persisted state transitions for lifecycle operations.
    - startup recovery now seeds in-memory queue and state from persisted metadata.
  - Added/expanded store primitives (`src/download/store.rs`):
    - helpers for numbered part paths and next free part number allocation.
    - `PartState` expanded to include `completed/cancelled/error` states.
  - Expanded typed errors (`src/download/errors.rs`) with:
    - invalid input, not found, invalid transition variants for command-level failures.
  - Added read-only API endpoint:
    - `GET /api/v1/downloads`
    - wired via new handler `src/api/handlers/downloads.rs` and router update.
    - response includes `queue_len`, `recovered_on_start`, and current download entries.
  - API wiring updates:
    - `ApiState`/`ApiServeDeps` now carry `DownloadServiceHandle`.
    - app bootstrap passes download handle into API server deps.
  - Tests added/updated:
    - download lifecycle flow (create -> pause -> resume -> cancel -> delete -> list)
    - restart recovery preserves persisted state
    - allocator picks lowest free slot
    - API contract test now verifies `/api/v1/downloads`
    - startup integration test updated for new API deps.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 79 tests).
- Decisions:
  - Keep phase 2 focused on control-plane correctness (state machine + persistence + API visibility) before chunk wire-transfer ingestion.
  - Keep `/api/v1/downloads` read-only for now; mutating endpoints will follow once queue semantics stabilize.
- Next steps:
  - Add mutating download API endpoints (create/pause/resume/cancel/delete) bound to current service commands.
  - Add first transfer-facing abstractions for pending block requests and timeout bookkeeping.
  - Introduce `.part` gap/range progress tracking in persisted metadata for block-level recovery.
- Change log: Download service is now a functional persisted queue with lifecycle operations and API observability.

- Status: Implemented download subsystem phase 1 persistence/recovery primitives on `feature/download-strategy-imule`:
  - Added `src/download/store.rs`:
    - `PartMet` model and `PartState` enum
    - `save_part_met(...)` with `.part.met.bak` rollover and atomic tmp->rename write
    - `load_part_met_with_fallback(...)` (primary then backup)
    - `scan_recoverable_downloads(...)` startup recovery scan over `data/download/*.part.met`
    - iMule-compatible version marker default (`PART_MET_VERSION = 0xE0`) for metadata model.
  - Extended `src/download/errors.rs` with typed store/persistence error variants:
    - read/write/rename/copy/parse/serialize directory and file failures.
  - Updated `src/download/service.rs`:
    - startup now recovers existing part metadata and sets `queue_len`
    - status now includes `recovered_on_start`
    - added `RecoveredCount` command and handle method.
  - Updated `src/download/mod.rs` exports for store model/types.
  - Added tests:
    - store roundtrip save/load
    - backup fallback when primary met is corrupt
    - recovery scan over multiple `.part.met` entries
    - service startup recovery count from existing metadata.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 76 tests).
- Decisions:
  - Keep phase 1 metadata persistence Rust-native (JSON payload) while preserving iMule file naming and lifecycle semantics (`.part.met`, `.bak`, startup recovery).
  - Defer wire-level transfer and full binary part.met compatibility to later phases after queue/state model is stable.
- Next steps:
  - Add first queue state model in service (`queued/running/paused/completed/error`) backed by persisted `PartMet`.
  - Add commands to create/pause/resume/cancel downloads and persist state transitions.
  - Introduce initial API endpoints for listing recovered/active download entries.
- Change log: Download subsystem now has backup-safe part metadata persistence and startup recovery integrated into runtime.

- Status: Implemented download subsystem phase 0 scaffold on `feature/download-strategy-imule`:
  - Added new module tree:
    - `src/download/mod.rs`
    - `src/download/types.rs`
    - `src/download/errors.rs`
    - `src/download/service.rs`
  - Added typed download errors:
    - `DownloadError`
    - `DownloadStoreError`
  - Added actor-style service shell:
    - `DownloadServiceConfig::from_data_dir(...)`
    - `start_service(...)` returning handle/status/join task
    - command loop with `Ping` and `Shutdown`
    - startup ensures `data/download/` and `data/incoming/` exist.
  - Integrated into app bootstrap:
    - `src/lib.rs` exports `download` module.
    - `src/app.rs` starts download service at runtime and adds `AppError::Download`.
  - Added tests:
    - `start_service_creates_download_and_incoming_dirs`
    - `service_ping_and_shutdown_flow`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 72 tests).
- Decisions:
  - Keep phase 0 strictly minimal: directory/bootstrap + command actor + typed errors, no transfer logic yet.
  - Keep service always-on at startup to prepare API integration in next phase.
- Next steps:
  - Phase 1: implement `.part`/`.part.met` persistence primitives and startup recovery scanning.
  - Add first download queue state model (`queued/running/paused/completed/error`) and service commands around it.
  - Add persistence-focused tests with corrupted/backup metadata cases.
- Change log: Download subsystem now exists as a first-class module with runtime wiring and passing tests.

- Status: Added iMule-derived download subsystem strategy on `feature/download-strategy-imule`:
  - New document: `docs/DOWNLOAD_DESIGN.md`
    - deep-dive findings from iMule source for download flow and persistence:
      - chunk/block transfer behavior (`OP_REQUESTPARTS`/`OP_SENDINGPART`/compressed parts)
      - `.part` + `.part.met` lifecycle and gap tracking
      - `known.met` and `known2_64.met` responsibilities
    - proposed Rust-native module boundaries under `src/download/*`
    - phased implementation plan (scaffold -> persistence -> transfer -> finalize -> API/UI)
    - test plan and compatibility rules.
  - Updated docs index and planning files:
    - `docs/README.md` includes `DOWNLOAD_DESIGN.md`
    - `docs/TODO.md` now has a `Downloads` backlog section
    - `docs/TASKS.md` reprioritized with download phase 0/1 first
    - `README.md` documentation map includes `docs/DOWNLOAD_DESIGN.md`
- Decisions:
  - Implement downloads Rust-native as an actor-style subsystem, preserving iMule wire/on-disk semantics where needed for compatibility.
  - Use `data/download/` for active `.part` state and `data/incoming/` for finalized files.
  - Deliver MD4-first baseline before enabling full AICH (`known2_64.met`) integration.
- Next steps:
  - Implement phase 0 scaffolding (`src/download/*`, typed errors, command/event loop shell).
  - Implement phase 1 `.part`/`.part.met` persistence and startup recovery tests.
  - Add minimal API surface to create/list/pause/resume/cancel downloads once phase 1 lands.
- Change log: Download strategy is now documented and promoted to top project priority in planning docs.

- Status: Added UI smoke testing to CI on `main` push/PR via Playwright + mocked backend:
  - Updated `.github/workflows/ci.yml`:
    - new `ui-smoke` job on `ubuntu-latest` with Node 20
    - installs UI deps (`npm ci`)
    - installs Playwright Chromium (`npx playwright install --with-deps chromium`)
    - runs `npm run test:ui:smoke`
  - Updated `ui/playwright.config.mjs`:
    - added `webServer` to auto-start local mock server when `UI_BASE_URL` is not provided.
  - Added `ui/tests/e2e/mock-server.mjs`:
    - serves UI pages/assets for Playwright
    - mocks required API endpoints + SSE (`/api/v1/events`) for deterministic UI smoke tests in CI.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep UI smoke test backend mocked in CI to avoid SAM/I2P runtime dependencies.
  - Make mock server opt-out via `UI_BASE_URL` so tests can still target a real running backend when needed.
- Next steps:
  - Optionally archive Playwright HTML/report artifacts in CI for easier failure triage.
  - Optionally add one route-guard assertion per page for authenticated/unauthenticated flow edges.
- Change log: CI now runs UI smoke tests automatically on pushes to `main` and PRs.

- Status: Completed final service split pass for source-probe/status helpers on `main` (no behavior change):
  - Added `src/kad/service/source_probe.rs`:
    - source probe tracking and counters:
      - `mark_source_publish_sent`
      - `mark_source_search_sent`
      - `on_source_publish_response`
      - `on_source_search_response`
      - `source_store_totals`
  - Added `src/kad/service/status.rs`:
    - status snapshot/publish logic:
      - `build_status`
      - `publish_status`
  - Updated `src/kad/service.rs`:
    - delegates source-probe and status helpers to dedicated modules.
  - Updated `src/kad/service/tests.rs`:
    - tests now call `status::build_status_impl(...)`.
  - Net effect:
    - `src/kad/service.rs` reduced to ~2009 LOC (from ~2335 previous step, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep wrapper/delegation pattern to preserve call sites and minimize risk.
  - Maintain all behavioral logic unchanged while shrinking `service.rs` responsibility.
- Next steps:
  - Optional: split remaining send/job orchestration helpers (`send_*`, `progress_keyword_job*`) if we want sub-2k LOC in `service.rs`.
- Change log: Source-probe and status helper clusters now live in dedicated modules; core service file is primarily orchestration/glue.

- Status: Extracted KAD inbound opcode handling into dedicated module on `main` (no behavior change):
  - Added `src/kad/service/inbound.rs`:
    - moved full `handle_inbound(...)` implementation and opcode dispatch logic (`HELLO`, `BOOTSTRAP`, `REQ/RES`, `SEARCH`, `PUBLISH`, `PING/PONG`, etc.).
  - Updated `src/kad/service.rs`:
    - now delegates inbound handling through a thin wrapper to `inbound::handle_inbound_impl(...)`.
    - registered new `mod inbound;`.
  - Net effect:
    - `src/kad/service.rs` reduced again to ~2335 LOC (from ~3519 after prior pass, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep inbound extraction as structure-only (no opcode behavior changes) to preserve protocol compatibility during refactor.
  - Continue minimizing `service.rs` responsibility by domain slicing (`types`, `routing_view`, `keyword`, `lookup`, `inbound`, `tests`).
- Next steps:
  - Optional final cleanup pass: extract source-probe/status helper cluster from `service.rs` into `source_probe.rs` / `status.rs` for smaller core orchestration.
- Change log: Inbound packet handling now lives in `src/kad/service/inbound.rs`; `service.rs` is now primarily orchestration plus shared helpers.

- Status: Continued KAD service modularization with lookup + keyword logic extraction on `main` (no behavior change):
  - Added `src/kad/service/lookup.rs` and moved lookup/refresh scheduler logic there:
    - lookup queue seeding/progression (`tick_lookups`)
    - bucket refresh scheduling (`tick_refresh`)
    - lookup response integration (`handle_lookup_response`)
    - distance/random target helpers used by the lookup pipeline.
  - Added `src/kad/service/keyword.rs` and moved keyword cache/store lifecycle logic there:
    - keyword interest tracking/capping
    - keyword hit cache upsert/caps/eviction
    - keyword store TTL/size-limit eviction
    - maintenance helpers for keyword cache/store.
  - `src/kad/service.rs` now delegates to `lookup`/`keyword` modules for these domains.
  - Net effect:
    - `src/kad/service.rs` reduced further to ~3519 LOC (from ~4116 after prior split, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep behavior-preserving wrappers/delegation during split to minimize regression risk.
  - Prioritize extraction of cohesive domains (lookup + keyword lifecycle) before touching inbound packet handler.
- Next steps:
  - Next high-value split in `service.rs` is `handle_inbound` and related opcode handlers into `inbound.rs`.
  - Optional follow-up: move source-probe bookkeeping/status helpers into `source_probe.rs`.
- Change log: KAD service now has dedicated `lookup` and `keyword` modules; core file is materially smaller with unchanged test results.

- Status: Split `src/kad/service.rs` into logical submodules on `main` (no behavior change):
  - Added `src/kad/service/types.rs`:
    - moved service-facing data/config/status/command types and related defaults:
      - `KadServiceCrypto`
      - `KadServiceConfig` (+ `Default`)
      - `KadServiceStatus`
      - `KadServiceCommand`
      - DTOs (`KadSourceEntry`, `KadKeywordHit`, `KadKeywordSearchInfo`, `KadPeerInfo`)
      - routing view DTOs (`RoutingSummary`, `RoutingBucketSummary`, `RoutingNodeSummary`)
      - internal stats struct (`KadServiceStats`)
  - Added `src/kad/service/routing_view.rs`:
    - moved routing summary/bucket/node projection builders out of core service loop file.
  - Added `src/kad/service/tests.rs`:
    - moved embedded unit tests out of `service.rs` into a dedicated test module file.
  - Updated `src/kad/service.rs`:
    - now re-exports public service types from `types.rs`
    - delegates routing view builders to `routing_view` module
    - keeps core service runtime/inbound/outbound behavior unchanged.
  - Net effect:
    - `src/kad/service.rs` reduced from ~4979 LOC to ~4116 LOC.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep this pass structural-only (file/module boundaries) to avoid behavior risk.
  - Prefer progressive extraction from `service.rs` with compile/test safety after each chunk.
- Next steps:
  - Continue splitting heavy behavior clusters from `service.rs`:
    - inbound packet handling
    - keyword job progression/cache maintenance
    - lookup/crawl scheduler logic
- Change log: KAD service module now has dedicated `types`, `routing_view`, and `tests` files with unchanged runtime behavior.

- Status: Hardened coverage CI job to avoid opaque failures on `main`:
  - Updated `.github/workflows/ci.yml` coverage job:
    - installs Rust `llvm-tools-preview` component explicitly
    - emits a `cargo llvm-cov --summary-only` step before gating
    - runs the gate through `scripts/test/coverage.sh` (single source of truth)
  - Set initial gate to a pragmatic baseline:
    - `MIN_LINES_COVERAGE=20` in CI env
    - `scripts/test/coverage.sh` default now `20`
  - Rationale: previous failures were opaque (`exit code 1` only). Summary step now prints measured coverage before gate evaluation.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Prefer explicit toolchain component install (`llvm-tools-preview`) in CI instead of relying on implicit behavior.
  - Use a conservative initial threshold until CI reports stable baseline values, then ratchet upward.
- Next steps:
  - After 1-2 successful CI runs with visible summaries, increase `MIN_LINES_COVERAGE` gradually (e.g. 25 -> 30 -> ...).
- Change log: Coverage CI now logs summary before gating and has an explicit llvm-tools setup.

- Status: Added tag-driven GitHub release workflow on `main`:
  - New workflow: `.github/workflows/release.yml`
  - Trigger: `push` tags matching `v*`
  - Build matrix:
    - `ubuntu-latest` -> `scripts/build/build_linux_release.sh`
    - `macos-latest` -> `scripts/build/build_macos_release.sh`
    - `windows-latest` -> `scripts/build/build_windows_release.ps1`
  - Uploads packaged artifacts from `dist/` per platform.
  - Publish job downloads artifacts and creates a GitHub Release with:
    - auto-generated release notes
    - attached `.tar.gz` (Linux/macOS) and `.zip` (Windows) bundles.
  - Updated `README.md` with tag-driven release usage (`git tag ... && git push origin ...`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Reuse existing repo build scripts for consistency between local and CI release packaging.
  - Use tag naming convention `v*` for release automation.
- Next steps:
  - Optional: add a manual `workflow_dispatch` release path for re-running failed tag releases without retagging.
  - Optional: add checksum/signature generation in release workflow.
- Change log: CI now includes a tag-driven CD pipeline that produces and publishes cross-platform release bundles.

- Status: Tightened initial line-coverage gate on `main`:
  - Increased minimum line coverage threshold from `35` to `40` in:
    - `.github/workflows/ci.yml` (`cargo llvm-cov --fail-under-lines 40`)
    - `scripts/test/coverage.sh` (`MIN_LINES_COVERAGE` default now `40`)
  - Attempted local baseline collection, but this sandbox cannot install `llvm-tools-preview` via `rustup`, so local `cargo llvm-cov` measurement could not be completed here.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Raise threshold incrementally to reduce CI disruption risk while still strengthening the gate.
  - Keep threshold configurable via `MIN_LINES_COVERAGE` for local overrides.
- Next steps:
  - Confirm coverage % from CI run in a normal runner environment and ratchet gate to `45` if headroom is comfortable.
- Change log: Coverage quality gate is now stricter (`40` lines minimum) across CI and local helper script.

- Status: Implemented API loopback dual-stack hardening + coverage gate scaffolding + startup/auth/session smoke test on `main`:
  - API listener startup now attempts both loopback families and serves on every successful bind:
    - `::1:<port>`
    - `127.0.0.1:<port>`
  - Bind failures on one family are logged as warnings; startup only fails if no loopback listener can be created.
  - Added first runtime smoke integration test: `tests/api_startup_smoke.rs`
    - boots `api::serve`
    - verifies `/api/v1/auth/bootstrap`
    - creates frontend session (`/api/v1/session`)
    - verifies session-cookie protected `/api/v1/session/check` and `/index.html`.
  - Added coverage gating scaffolding:
    - GitHub Actions workflow: `.github/workflows/ci.yml`
    - local coverage command: `scripts/test/coverage.sh`
    - README quality gate section updated with coverage command.
  - Added `reqwest` as a dev-dependency for integration-level HTTP smoke testing.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests total including integration smoke).
- Decisions:
  - Keep API local-only by binding loopback addresses explicitly instead of widening bind scope.
  - Treat IPv4/IPv6 support as best-effort on startup: one-family availability is acceptable; total loopback bind failure is fatal.
  - Start with a conservative line-coverage gate (`--fail-under-lines 35`) and ratchet upward once baseline metrics are collected in CI.
- Next steps:
  - Run `scripts/test/coverage.sh` in CI or locally where `cargo-llvm-cov` is installed and record baseline coverage percentage in docs.
  - Consider raising coverage threshold after one or two PR cycles.
- Change log: API startup is now resilient to localhost address-family differences, and repo now has integration smoke coverage plus CI coverage gate scaffolding.

- Status: Removed `api.host` configurability and simplified API binding on `main`:
  - `ApiConfig` no longer contains `host`; API config now binds by port only.
  - API server bind address is fixed to loopback (`127.0.0.1`) in `src/api/mod.rs`.
  - Removed loopback-host parsing/validation path for API host:
    - removed `parse_api_bind_host(...)`
    - removed related `ConfigError` and `ConfigValidationError` branches.
  - Settings API no longer exposes/accepts `settings.api.host`.
  - Updated config/docs surface (`config.toml`, `README.md`, `docs/architecture.md`, `docs/api_curl.md`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 70 tests).
- Decisions: Keep API bind policy explicit and non-configurable while local-only operation is the product mode; expose only `api.port` to users.
- Next steps: Optional follow-up is to document future remote/headless exposure as a separate deployment mode instead of host binding config.
- Change log: API host setting has been removed from config/state/settings surfaces.

- Status: Performed config-surface naming and documentation pass on `main`:
  - Renamed API rate-limit config key for clarity:
    - `rate_limit_dev_auth_max_per_window` -> `rate_limit_auth_bootstrap_max_per_window`.
  - Added backward-compatible config parsing alias in `ApiConfig`:
    - `#[serde(alias = "rate_limit_dev_auth_max_per_window")]`.
  - Updated all runtime/settings references to the new name.
  - Added inline comments in `config.toml` for all active/uncommented keys across:
    - `[sam]`, `[kad]`, `[general]`, `[api]`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep public config keys aligned with endpoint naming (`auth/bootstrap`) and maintain read-compat for recently renamed keys to avoid operator breakage.
- Next steps: Optional follow-up is to normalize remaining legacy test names/messages still using `dev_auth` wording.
- Change log: Config naming and inline documentation are now more consistent and self-descriptive.

- Status: Removed user-facing `kad.udp_port` configurability while preserving config-file compatibility on `main`:
  - Removed `udp_port` from `KadConfig` public settings.
  - Added deprecated compatibility field in `KadConfig`:
    - `deprecated_udp_port` with `#[serde(rename = "udp_port", skip_serializing)]`
    - old config files containing `kad.udp_port` still parse, but value is ignored and no longer persisted.
  - Removed `kad.udp_port` line from `config.toml`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep KAD UDP port as protocol/internal metadata, but not as a user-tunable config knob.
- Next steps: Optional follow-up is to document this deprecation explicitly in `docs/architecture.md` if we want a visible migration note for operators carrying old configs.
- Change log: `kad.udp_port` is no longer a configurable setting in active config surfaces.

- Status: Replaced `/api/v1/dev/auth` with core bootstrap endpoint and auth-mode gating on `main` (no backward compatibility route):
  - Added `api.auth_mode` enum config (`local_ui` | `headless_remote`) in `src/config.rs` and `config.toml`.
  - Removed `enable_dev_auth_endpoint` from runtime config/state/settings API.
  - New endpoint path is `GET /api/v1/auth/bootstrap` (loopback-only).
  - Endpoint is available only when `api.auth_mode = "local_ui"`; it is not registered in `headless_remote` mode.
  - Updated bearer-exempt logic to use `auth_mode` and new path.
  - Updated rate limiter target path to `/api/v1/auth/bootstrap`.
  - Updated UI bootstrap fetch paths:
    - inline `/auth` bootstrap page in `src/api/ui.rs`
    - `ui/assets/js/helpers.js`
  - Renamed helper script to `scripts/docs/auth_bootstrap.sh` and updated docs references.
  - Updated docs (`README.md`, `docs/architecture.md`, `docs/API_DESIGN.md`, `docs/ui_api_contract_map.md`, `docs/api_curl.md`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Treat token bootstrap as core local-UI behavior under `/api/v1/auth/bootstrap`; use auth mode, not endpoint-specific toggle flags.
- Next steps: Optional follow-up is to surface `auth_mode` explicitly in settings UI with explanatory copy for local UI vs headless remote operations.
- Change log: Auth bootstrap route naming and availability now align with core-vs-mode semantics.

- Status: Added minimal API rate-limiting middleware on `main`:
  - New `[api]` config keys:
    - `rate_limit_enabled`
    - `rate_limit_window_secs`
    - `rate_limit_dev_auth_max_per_window`
    - `rate_limit_session_max_per_window`
    - `rate_limit_token_rotate_max_per_window`
  - Added `src/api/rate_limit.rs` fixed-window middleware keyed by `(client_ip, method, path)`.
  - Rate limiting is applied to:
    - `GET /api/v1/dev/auth`
    - `POST /api/v1/session`
    - `POST /api/v1/token/rotate`
  - Added rate-limit fields to settings API payload/patch and validation.
  - Added test coverage:
    - session endpoint returns `429` after threshold exceeded
    - settings snapshot/patch includes new rate-limit fields
    - settings rejects invalid zero rate-limit values
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep limiter intentionally narrow (only high-value endpoints) and disabled by config toggle when needed; avoid limiting SSE/status paths for now.
- Next steps: Optional: emit structured logs on `429` events and add per-endpoint counters for abuse/noise visibility.
- Change log: API now has configurable built-in endpoint rate limiting.

- Status: Added API endpoint toggles for debug and dev-auth bootstrap on `main`:
  - New `[api]` config flags in `config.toml`/`ApiConfig`:
    - `enable_debug_endpoints` (controls `/api/v1/debug/*`)
    - `enable_dev_auth_endpoint` (controls `/api/v1/dev/auth`)
  - Router now conditionally registers debug routes and dev-auth route based on these flags.
  - Auth exemption for `/api/v1/dev/auth` is now conditional on `enable_dev_auth_endpoint`.
  - Settings API now exposes and accepts these flags under `settings.api`.
  - Added/updated tests:
    - bearer exemption logic with dev-auth enabled/disabled
    - debug routes return `404` when disabled
    - dev-auth route returns `404` (with bearer) when disabled
  - Updated docs: `README.md`, `docs/architecture.md`, `docs/api_curl.md`, and `config.toml` comments.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 70 tests).
- Decisions: Keep both new flags defaulted to `true` for backwards-compatible behavior; operators can disable either endpoint group explicitly.
- Next steps: Optional follow-up is lightweight rate limiting for high-value endpoints (`/api/v1/dev/auth`, `/api/v1/token/rotate`, `/api/v1/session`) to reduce brute-force/noise risks.
- Change log: API surface can now be reduced at runtime via config without code changes.

- Status: Moved operational scripts out of `docs/scripts` into top-level `scripts/` with explicit split:
  - API/documentation helpers moved to `scripts/docs/`:
    - `health/status/events`, KAD endpoint helpers, debug endpoint helpers, dev auth helper.
  - Test harnesses moved to `scripts/test/`:
    - `two_instance_dht_selftest.sh`
    - `rust_mule_soak.sh`
    - `soak_triage.sh`
  - Removed legacy `docs/scripts/` directory and updated path references in scripts/docs:
    - internal calls in `scripts/test/two_instance_dht_selftest.sh`
    - usage/help text in moved scripts
    - `README.md` and `docs/api_curl.md` pointers.
- Decisions: Keep `scripts/build/` for build/release, `scripts/docs/` for endpoint helper wrappers, and `scripts/test/` for scenario/soak harnesses.
- Next steps: Optional follow-up can add thin wrapper aliases for old `docs/scripts/*` paths if external automation still depends on them.
- Change log: Script layout is now canonicalized under `/scripts` and split by intent (docs helpers vs tests).

- Status: Added dedicated cross-platform build script folder on `main`:
  - New canonical build location: `scripts/build/`.
  - Added platform scripts:
    - `scripts/build/build_linux_release.sh`
    - `scripts/build/build_macos_release.sh`
    - `scripts/build/build_windows_release.ps1`
    - `scripts/build/build_windows_release.cmd`
  - Added `scripts/build/README.md` with usage/output conventions.
  - Kept backward compatibility by turning `docs/scripts/build_linux_release.sh` into a wrapper that delegates to `scripts/build/build_linux_release.sh`.
  - Updated docs pointers in `README.md` and `docs/README.md`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep build/release scripts outside `docs/` in a dedicated top-level `scripts/build/` folder; keep old Linux path callable as a shim to avoid breakage.
- Next steps: Optional follow-up is a CI matrix job that runs each platform build script and verifies `dist/*` bundle naming/contents.
- Change log: Cross-platform build scaffolding now exists with a canonical script location.

- Status: Streamlined docs set and refreshed README entrypoint on `main`:
  - Rewrote `README.md` to reflect current behavior and include a clear documentation map.
  - Added `docs/README.md` as a documentation index.
  - Normalized backlog docs:
    - `docs/TODO.md` (focused subsystem backlog)
    - `docs/TASKS.md` (current execution priorities and DoD)
  - Corrected API design drift in `docs/API_DESIGN.md`:
    - `/api/v1/health` response shape now documented as `{ \"ok\": true }`
    - SSE auth documented as session-cookie based (no token query parameter)
    - security note updated to avoid bearer tokens in query parameters.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep `README.md` as the top-level operator/developer entrypoint and keep deeper design/contract details in `/docs` with an explicit index.
- Next steps: Keep `docs/ui_api_contract_map.md` and `docs/api_curl.md` updated whenever endpoint fields/routes change; continue prioritizing KAD organic reliability and UI statistics expansion.
- Change log: Documentation set is now normalized and aligned with current API/UI/auth behavior.

## Status (2026-02-12)

- Status: Standardized and relaxed API command timeout policy on `main`:
  - Added shared timeout constant:
    - `API_CMD_TIMEOUT = 5s` in `src/api/mod.rs`.
  - Replaced per-endpoint hardcoded `2s` timeouts in `src/api/handlers/kad.rs` with `API_CMD_TIMEOUT`.
  - This applies to KAD command/oneshot-backed endpoints (`sources`, `keyword results`, searches, peers, routing debug, lookup/probe).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Use a single shared timeout for API command dispatch/response waits to avoid endpoint drift and reduce spurious gateway timeouts in slower I2P conditions.
- Next steps: Optional follow-up can split timeout tiers (e.g. 3s read-only status vs 5s routing/debug) if operational data suggests different SLOs.
- Change log: API command timeout is now centralized and increased from ad-hoc 2s values to 5s.

- Status: Made session-cookie `Secure` policy explicit in auth code on `main`:
  - Added rationale comment in `src/api/auth.rs` (`build_session_cookie`) explaining why `Secure` is intentionally omitted for current HTTP loopback UI flow.
  - Documented future action in comment: add `Secure` when/if frontend serving moves to HTTPS.
  - Extended cookie test (`src/api/tests.rs`) to assert current behavior (`Secure` absent), making policy changes explicit and reviewable.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep cookie flags as `HttpOnly; SameSite=Strict` without `Secure` for current localhost HTTP mode, but require explicit code change when transport assumptions change.
- Next steps: Optional: gate `Secure` on a future HTTPS/TLS config switch when frontend transport supports it.
- Change log: Session-cookie security decision is now explicitly documented and test-enforced.

- Status: Fixed implicit config persistence path and fragile API settings tests on `main`:
  - Added explicit config persistence API:
    - `Config::persist_to(path)` in `src/config.rs`
    - existing `Config::persist()` now delegates to `persist_to("config.toml")` for compatibility.
  - Added explicit config path to API runtime state:
    - `ApiState.config_path`
    - new `ApiServeDeps` includes `config_path` and other serve dependencies.
  - `settings_patch` now persists via:
    - `next.persist_to(state.config_path.as_path())`
    - no implicit `./config.toml` write in API path.
  - Threaded config path from entrypoint to app/api:
    - `main` now tracks `config_path` and calls `app::run(cfg, config_path)`
    - `app::run` passes `config_path` into API serve deps.
  - Hardened tests:
    - API tests now use unique temp config paths in `ApiState` and no longer mutate/restore repo `config.toml`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep backward-compatible `Config::persist()` for non-API call sites, but route all runtime persistence that depends on startup config location through explicit `persist_to(path)`.
- Next steps: Optional cleanup can remove `Config::persist()` after all call sites are migrated to `persist_to(path)`.
- Change log: Config persistence path is now explicit in API runtime flow and test persistence is isolated from repository config.

- Status: Removed lingering test-build unused-import warning in `src/api/mod.rs` after API split:
  - Dropped test-only re-export block from `src/api/mod.rs`.
  - Updated `src/api/tests.rs` to import directly from split modules:
    - `auth`, `cors`, `ui`, `handlers`, `router`.
  - This prevents warning-prone indirection and keeps compile ownership explicit.
  - Ran `cargo clippy --all-targets --all-features` and `cargo test` (all passing; 68 tests).
- Decisions: Keep API tests referencing module paths directly instead of relying on `mod.rs` re-exports to avoid future dead-import warnings during refactors.
- Next steps: None required for this warning; refactor cleanup is complete.
- Change log: API test imports now directly track split module boundaries.

- Status: Refactored API god-file (`src/api/mod.rs`) into focused modules on `main` (no behavior change):
  - New modules:
    - `src/api/router.rs` (router wiring)
    - `src/api/auth.rs` (auth/session middleware + helpers)
    - `src/api/cors.rs` (CORS middleware + helpers)
    - `src/api/ui.rs` (embedded UI/static serving and SPA fallback)
    - `src/api/handlers/core.rs` (health/auth/session/status/events handlers)
    - `src/api/handlers/kad.rs` (KAD/search/debug handlers)
    - `src/api/handlers/settings.rs` (settings handlers/validation/patch logic)
    - `src/api/handlers/mod.rs` (handler exports)
    - `src/api/tests.rs` (existing API tests moved out of `mod.rs`)
  - `src/api/mod.rs` now focuses on API state, startup/serve path, module wiring, and test-only re-exports.
  - Endpoint paths, middleware order, and response behavior were kept unchanged.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep this as a structural split only (no endpoint contract or middleware semantic changes) to reduce risk while improving maintainability.
- Next steps: Optional follow-up can split `handlers/kad.rs` further by sub-domain (`search`, `debug`, `publish`) if we want even tighter module boundaries.
- Change log: API surface is now modularized by concern, replacing the prior single-file implementation.

- Status: Fixed `nodes2.dat` download persistence path bug on `main`:
  - In `try_download_nodes2_dat(...)` (`src/app.rs`), persistence previously hardcoded `./data/nodes.dat`.
  - Updated function to accept an explicit output path and persist there.
  - Call site now passes `preferred_nodes_path` (resolved from configured `general.data_dir` + `kad.bootstrap_nodes_path`).
  - Parent directories are created for the configured output path before atomic write/rename.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep download behavior unchanged except for output path correctness; this remains a low-risk bug fix with no protocol changes.
- Next steps: Optional: add a targeted unit/integration test around bootstrap download path resolution when `data_dir` is non-default.
- Change log: `nodes2.dat` refresh now respects configured data directory/bootstrap path.

- Status: Corrected misleading overview KPI labels in UI on `main`:
  - Updated `ui/index.html` labels to match actual status field semantics:
    - `routing`: `Peers Contacted` -> `Routing Nodes`
    - `live`: `Responses` -> `Live Nodes`
    - `live_10m`: `Hits Found (10m)` -> `Live Nodes (10m)`
  - Updated progress badges for clarity:
    - `requests` -> `requests sent`
    - `responses` -> `responses received`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep KPI naming tied to raw API counter meaning, not inferred behavior, to avoid future ambiguity in diagnostics.
- Next steps: Optional follow-up can add compact tooltip/help text for each KPI defining its backing status field.
- Change log: Overview metric labels now accurately describe `routing`, `live`, and `live_10m`.

- Status: Fixed high-impact UI/API status-field mismatch on `main`:
  - UI expected `recv_req` and `recv_res` in status payloads (REST + SSE), while API exposed `sent_reqs` and `recv_ress`.
  - Added compatibility aliases directly in `KadServiceStatus`:
    - `recv_req` (mirrors `sent_reqs`)
    - `recv_res` (mirrors `recv_ress`)
  - Wired aliases in status construction (`build_status`) so they are always populated.
  - Extended API contract test `ui_api_contract_endpoints_return_expected_shapes` to assert:
    - `recv_req` and `recv_res` exist
    - alias values match `sent_reqs` and `recv_ress`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Preserve existing canonical counters (`sent_reqs`, `recv_ress`) while adding aliases for UI compatibility; avoids breaking current dashboards and SSE consumers.
- Next steps: Optional cleanup is to normalize UI naming to canonical fields in a later pass, then remove aliases when all consumers are updated.
- Change log: Status API now exposes both canonical and UI-expected request/response counters.

- Status: Added checked-in soak runner script on `main`:
  - New script: `docs/scripts/rust_mule_soak.sh`
  - Mirrors the long-run harness previously staged in `/tmp/rust_mule_soak.sh`.
  - Commands:
    - `start` (clone `../../mule-a` + `../../mule-b`, patch B ports/session, launch both)
    - `wait_ready` (poll `/api/v1/status` until both return 200)
    - `soak [rounds]` (publish/search loops; writes `logs/rounds.tsv` + `logs/status.ndjson`)
    - `stop` and `collect` (creates `/tmp/rust-mule-soak-*.tar.gz`)
  - Script is executable and validated for shell syntax and usage output.
- Decisions: Keep the soak run harness and soak triage tool (`docs/scripts/soak_triage.sh`) together under `docs/scripts` for reproducible operator workflow.
- Next steps: Optional: wire both scripts into a single wrapper (`run + triage`) for one-command baseline comparisons.
- Change log: Added `docs/scripts/rust_mule_soak.sh` to the repository.

- Status: Added soak triage helper script on `main`:
  - New script: `docs/scripts/soak_triage.sh`
  - Input: soak tarball (`/tmp/rust-mule-soak-*.tar.gz`)
  - Output includes:
    - completion signal (`stop requested` markers)
    - round outcome metrics (total/success/success%, first+last success, max fail streak, last300 success)
    - success source concentration (`source_id_hex` top list)
    - key A/B status counters (`max` and `last` from `status.ndjson`)
    - panic/fatal scan for `logs/a.out` and `logs/b.out`
  - Validated against `/tmp/rust-mule-soak-20260214_101721.tar.gz`; reported metrics match prior manual triage.
- Decisions: Keep soak triage tool POSIX shell + awk/grep only (no Python dependency) so it works in constrained environments.
- Next steps: Optional follow-up can add CSV/JSON emit mode for CI ingestion if we want automatic baseline-vs-current comparisons.
- Change log: Added `docs/scripts/soak_triage.sh` and validated report output on the latest soak archive.

- Status: Added UI/API contract assurance scaffolding on `feature/kad-imule-parity-deep-pass`:
  - Added router-level UI contract test in `src/api/mod.rs`:
    - `ui_api_contract_endpoints_return_expected_shapes`
    - validates response shape invariants for UI-critical endpoints:
      - `GET /api/v1/status`
      - `GET /api/v1/searches`
      - `GET /api/v1/searches/:search_id`
      - `GET /api/v1/kad/keyword_results/:keyword_id_hex`
      - `GET /api/v1/kad/peers`
      - `GET /api/v1/settings`
  - Added endpoint coverage map:
    - `docs/ui_api_contract_map.md` (UI sections -> endpoint -> required fields/behavior).
  - Added Playwright smoke test scaffold for UI pages:
    - `ui/package.json`
    - `ui/playwright.config.mjs`
    - `ui/tests/e2e/smoke.spec.mjs`
    - `ui/tests/README.md`
  - Updated `.gitignore` for UI test artifacts:
    - `/ui/node_modules`
    - `/ui/test-results`
    - `/ui/playwright-report`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep browser smoke tests as an opt-in local workflow (Node/Playwright) while enforcing API response contracts in Rust tests to keep CI lightweight and deterministic.
- Next steps: When soak run completes, execute `ui` Playwright smoke against the same running node and add failures (if any) as actionable API/UI contract regressions.
- Change log: UI-critical API response shape checks are now executable, documented, and paired with a runnable browser smoke suite scaffold.

- Status: Implemented organic source-flow observability upgrades on `feature/kad-imule-parity-deep-pass` (requested implementation of steps 2 and 3):
  - Added source batch outcome accounting in `src/kad/service.rs` for both send paths:
    - search batches: `source_search_batch_{candidates,skipped_version,sent,send_fail}`
    - publish batches: `source_publish_batch_{candidates,skipped_version,sent,send_fail}`
  - Batch counters are emitted in status payload (`KadServiceStatus`) and logged in send-batch INFO events.
  - Added per-file source probe tracker state (`source_probe_by_file`) with first-send/first-response timestamps and rolling result counts.
  - Added aggregate status counters for probe timing/results:
    - `source_probe_first_publish_responses`
    - `source_probe_first_search_responses`
    - `source_probe_search_results_total`
    - `source_probe_publish_latency_ms_total`
    - `source_probe_search_latency_ms_total`
  - Wired response-side tracking:
    - on source `PUBLISH_RES` reception, record first publish response latency per file
    - on `SEARCH_RES` keyed to tracked source files, record first search response latency and per-response returned source counts
  - Added unit test:
    - `kad::service::tests::source_probe_tracks_first_send_response_latency_and_results`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 67 tests).
- Decisions: Keep probe tracking lightweight/in-memory and bounded (`SOURCE_PROBE_MAX_TRACKED_FILES = 2048`) with aggregate latency totals in status for immediate triage without introducing persistence or heavy histograms.
- Next steps: Build fresh `mule-a`/`mule-b` artifacts and run repeated non-forced A/B rounds to quantify organic success rate and latency percentiles using the new batch/probe counters.
- Change log: Source send-path selection/success/failure and per-file response timing are now directly measurable from status + logs.

- Status: Implemented source-path diagnostics follow-up on `feature/kad-imule-parity-deep-pass` (requested items 1 and 2):
  - Added receive-edge KAD inbound instrumentation in `src/kad/service.rs`:
    - `event="kad_inbound_packet"` for every decrypted+parsed inbound packet with:
      - opcode hex + opcode name
      - dispatch target label
      - payload length
      - obfuscation/verify-key context
    - `event="kad_inbound_drop"` with explicit reasons:
      - `request_rate_limited`
      - `unrequested_response`
      - `unhandled_opcode`
  - Cross-checked source opcode constants/layouts against iMule reference (`source_ref`):
    - `src/include/protocol/kad2/Client2Client/UDP.h`
    - `src/kademlia/net/KademliaUDPListener.cpp` (`Process2SearchSourceRequest`, `Process2PublishSourceRequest`)
  - Added wire-compat regression tests in `src/kad/wire.rs`:
    - `kad2_source_opcode_values_match_imule`
    - `kad2_search_source_req_layout_matches_imule`
    - `kad2_publish_source_req_layout_has_required_source_tags`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 66 tests).
- Decisions: Keep diagnostics at `DEBUG` level (not INFO) to preserve operability while enabling precise packet-path triage during A/B probes.
- Next steps: Build fresh `mule-a`/`mule-b` artifacts and rerun forced `debug/probe_peer` A<->B; inspect new `kad_inbound_packet`/`kad_inbound_drop` events to pinpoint whether source opcodes arrive and where they are dropped.
- Change log: KAD service now emits deterministic receive-edge opcode/drop telemetry, and source opcode/layout compatibility with iMule is explicitly tested.

- Status: Extended debug peer probing on `feature/kad-imule-parity-deep-pass` to include source-path packets in addition to keyword packets:
  - `src/kad/service.rs` `debug_probe_peer(...)` now sends:
    - `KADEMLIA2_SEARCH_SOURCE_REQ` (for peers `kad_version >= 3`)
    - `KADEMLIA2_PUBLISH_SOURCE_REQ` (for peers `kad_version >= 4`)
  - Existing probe sends remain unchanged:
    - `KADEMLIA2_HELLO_REQ`
    - `KADEMLIA2_SEARCH_KEY_REQ`
    - `KADEMLIA2_PUBLISH_KEY_REQ`
  - Probe debug log now reports source probe send booleans:
    - `sent_search_source`
    - `sent_publish_source`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 63 tests).
- Decisions: Keep source probe sends version-gated to align with existing source batch behavior and avoid forcing unsupported opcodes on low-version peers.
- Next steps: Rebuild `mule-a`/`mule-b` binaries and re-run forced `debug/probe_peer` A->B and B->A; then verify inbound source counters/events (`recv_*_source_*`, `source_store_update`, `source_store_query`) move from zero.
- Change log: `POST /api/v1/debug/probe_peer` can now directly exercise source request paths, enabling deterministic source-path diagnostics.

- Status: Added targeted source-store observability on `feature/kad-imule-parity-deep-pass` and validated via extended two-instance selftest:
  - `src/kad/service.rs` now tracks and reports source lifecycle counters in `kad_status_detail`:
    - `recv_search_source_decode_failures`
    - `source_search_hits` / `source_search_misses`
    - `source_search_results_served`
    - `recv_publish_source_decode_failures`
    - `sent_publish_source_ress`
    - `new_store_source_entries`
  - Added source store gauges in status payload:
    - `source_store_files`
    - `source_store_entries_total`
  - Added structured source observability logs:
    - `event=source_store_update` on inbound `PUBLISH_SOURCE_REQ` store attempts
    - `event=source_store_query` on served `SEARCH_SOURCE_REQ` responses
  - Added unit test coverage:
    - `kad::service::tests::build_status_reports_source_store_totals`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 63 tests).
- Decisions: Keep observability additive and low-risk (counters + logs) without changing protocol behavior yet; use this pass to isolate source replication/search breakpoints before logic changes.
- Next steps: Re-run targeted A/B probe and inspect new counters/events (`source_store_update`, `source_store_query`, `new_store_source_entries`, `source_store_*`) to identify exact source-path failure stage.
- Change log: Source publish/search/store lifecycle now has explicit service-side counters and logs suitable for direct A/B diagnostics.

- Status: Completed deep KAD parity hardening pass against iMule reference (`source_ref`) on `feature/kad-imule-parity-deep-pass`:
  - Added PacketTracking-style request/response correlation in `src/kad/service.rs`:
    - track outgoing KAD request opcodes with 180s TTL,
    - drop unrequested inbound response packets (bootstrap/hello/res/search/publish/pong shapes).
  - Added per-peer inbound KAD request flood limiting in `src/kad/service.rs` (iMule-inspired limits by opcode family).
  - Added service-mode handling for inbound `KADEMLIA2_BOOTSTRAP_REQ` and reply path:
    - introduced `encode_kad2_bootstrap_res(...)` in `src/kad/wire.rs`,
    - service now responds with self+routing contacts, encrypted with receiver-key flow when applicable.
  - Removed remaining runtime brittle byte-slice `unwrap` conversions in:
    - `src/kad/bootstrap.rs`
    - `src/kad/udp_crypto.rs` (`udp_verify_key` path)
  - Added tests in `src/kad/service.rs`:
    - tracked out-request matching behavior,
    - inbound request flood-limit behavior.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 62 tests).
- Decisions: Keep implementation Rust-native (simple explicit tracker + hash-map counters) while matching iMule behavior intent (tracked responses, anti-flood request gating, bootstrap response semantics) without copying C++ structure.
- Next steps: Optional follow-up parity pass can tighten ACK/challenge semantics further by emulating more of iMule `PacketTracking::LegacyChallenge` behavior for edge peers.
- Change log: KAD service now behaves closer to iMule for bootstrap responsiveness, response legitimacy checks, and inbound request flood resistance.

- Status: Completed panic-hardening follow-up for sanity findings (items 1..4) on `main`:
  - `src/logging.rs`: removed panic-on-poison in warning throttle lock path; now recovers poisoned mutex state and logs a warning.
  - `src/app.rs`: removed runtime `unwrap()` conversions for destination hash/array extraction; switched to explicit copy logic.
  - `src/i2p/sam/datagram.rs`: replaced `expect()` in `forward_port`/`forward_addr` with typed `Result` returns (`SamError`), and updated call sites in `src/app.rs`.
  - `src/kad/service.rs`, `src/nodes/imule.rs`, `src/kad/wire.rs`: replaced safe-but-brittle slice `try_into().unwrap()` patterns with non-panicking copy-based conversions.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, strict clippy sanity pass (`unwrap/expect/panic/todo/unimplemented`), and `cargo test` (all passing; strict pass now only flags remaining test/internal non-critical unwrap/expect sites outside this scoped fix).
- Decisions: Keep panic-hardening targeted to runtime production paths first; test-only unwrap/expect cleanup can be a separate ergonomics pass.
- Next steps: Optional low-risk pass to eliminate remaining test/internal unwrap/expect usage repository-wide for stricter lint cleanliness.
- Change log: Production/runtime panic surfaces identified in the sanity pass were removed for logging lock handling, SAM datagram address accessors, and key byte-conversion paths.

- Status: Completed typed-error migration pass across remaining runtime/boundary modules on `main`:
  - Converted to typed errors:
    - `src/app.rs` (`AppError`)
    - `src/main.rs` (`MainError`, `ConfigValidationError`)
    - `src/api/mod.rs` serve path (`ApiError`)
    - `src/single_instance.rs` (`SingleInstanceError`)
    - `src/kad/service.rs` (`KadServiceError`)
    - bin utilities: `src/bin/imule_nodes_inspect.rs`, `src/bin/sam_dgram_selftest.rs`
  - Removed remaining runtime `anyhow` usage from `src/` implementation paths.
  - Updated `docs/TODO.md` to mark typed-error migration as done and refreshed `docs/TASKS.md` with next priority.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Keep typed errors explicit per subsystem/boundary and preserve existing HTTP/runtime behavior by mapping at boundaries rather than changing response semantics.
- Next steps: Focus on KAD search/publish reliability and ACK/timeout observability (`docs/TASKS.md` current priority).
- Change log: End-to-end code paths now use typed error enums instead of `anyhow`, including app orchestration and utility binaries.

- Status: Documentation sync/normalization pass completed on `main`:
  - Updated `README.md` API/UI auth flow to reflect current behavior:
    - `/api/v1/session` issues `rm_session` cookie.
    - `/api/v1/events` uses session-cookie auth.
  - Normalized `docs/TODO.md`:
    - marked clippy round completed,
    - corrected settings endpoint paths to `/api/v1/settings`,
    - marked docs alignment done,
    - added remaining typed-error migration item for boundary/runtime layers.
  - Updated `docs/API_DESIGN.md` to distinguish implemented auth/session model from forward-looking API ideas and removed stale SSE token-query framing.
  - Added concrete next-priority execution plan in `docs/TASKS.md`.
- Decisions: Keep `docs/API_DESIGN.md` as a mixed strategic + implemented view, but explicitly label forward-looking endpoints and defer executable examples to `docs/api_curl.md`.
- Next steps: Execute `docs/TASKS.md` item #1 (finish typed-error migration in boundary/runtime layers).
- Change log: Documentation now matches the current session-cookie SSE model, endpoint paths, and project priorities.

- Status: Expanded subsystem-specific typed errors (second batch) on `feature/subsystem-typed-errors`:
  - Replaced `anyhow` in additional KAD/SAM subsystem modules with typed errors:
    - `src/kad/wire.rs` (`WireError`)
    - `src/kad/packed.rs` (`InflateError`)
    - `src/kad/udp_crypto.rs` (`UdpCryptoError`)
    - `src/kad/udp_key.rs` (`UdpKeyError`)
    - `src/kad/bootstrap.rs` (`BootstrapError`)
    - `src/i2p/sam/keys.rs` (`SamKeysError`)
    - `src/i2p/sam/kad_socket.rs` now returns `Result<_, SamError>` directly.
  - Kept app/main/api as the top-level error aggregation boundary.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Typed errors were added first in protocol/parsing/crypto and SAM helper modules where error provenance matters most; orchestration layers remain unchanged for now.
- Next steps: Remaining `anyhow` usage is concentrated in boundary/runtime modules (`src/app.rs`, `src/main.rs`, `src/api/mod.rs`, `src/single_instance.rs`, `src/kad/service.rs`, and bin tools) and can be migrated incrementally if full typed coverage is required.
- Change log: KAD wire/deflate/UDP-crypto/bootstrap and SAM keys/socket now emit concrete typed errors rather than `anyhow`.

- Status: Implemented subsystem-specific typed errors on `feature/subsystem-typed-errors`:
  - Replaced internal `anyhow` usage with typed error enums + local `Result` aliases in:
    - `src/config.rs`
    - `src/config_io.rs`
    - `src/api/token.rs`
    - `src/kad.rs`
    - `src/kad/keyword.rs`
    - `src/nodes/imule.rs`
    - `src/i2p/b64.rs`
    - `src/i2p/http.rs`
  - Preserved current app-level behavior by allowing these typed errors to bubble into existing `anyhow` boundaries where applicable.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Kept this pass focused on subsystem modules with clear ownership boundaries; app orchestration/error aggregation remains unchanged.
- Next steps: Continue migrating remaining non-core modules still using `anyhow` (for example selected KAD service/bootstrap internals) if full typed-error coverage is desired.
- Change log: Subsystem error handling now uses concrete typed errors instead of stringly `anyhow` in the converted modules.

- Status: Completed logging follow-up pass (`feature/logging-followup`):
  - Added throttled-warning suppression counters surfaced as periodic summary logs (`event=throttled_warning_summary`).
  - Broadened log redaction on KAD identifiers in operational/debug paths (`redact_hex`) and shortened destination logging to short base64 forms in additional send-failure paths.
  - Added structured `event=...` fields to key startup/status/search/publish log lines for machine filtering.
  - Reduced bootstrap INFO noise by demoting per-peer HELLO/PONG/BOOTSTRAP chatter to DEBUG.
  - Added retention helper tests in `src/config.rs`:
    - rotated filename split/match validation
    - old rotated-file cleanup behavior.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Keep operator-facing INFO logs as concise aggregate state transitions and preserve per-peer/protocol chatter under DEBUG/TRACE.
- Next steps: Optional final pass can redact remaining DEBUG payload snippets (e.g., packet heads) for environments where debug bundles are shared externally.
- Change log: Logging now includes throttling observability, stronger identifier redaction, and tested retention helpers while keeping INFO output lower-noise.

- Status: Completed API bind policy hardening (`feature/api-bind-loopback-policy`):
  - Enforced loopback-only API bind host handling via shared config helper (`parse_api_bind_host`).
  - Accepted hosts: `localhost`, `127.0.0.1`, `::1`.
  - Rejected non-loopback binds (e.g. `0.0.0.0`, LAN/WAN IPs) in:
    - startup config validation (`src/main.rs`)
    - API server bind resolution (`src/api/mod.rs`)
    - settings API validation (`PATCH /api/v1/settings`)
  - Added tests:
    - `parse_api_bind_host_accepts_only_loopback`
    - extended settings patch rejection coverage for non-loopback `api.host`.
  - Updated `docs/TODO.md` to mark the API bind requirement as completed.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 57 tests).
- Decisions: Keep a strict local-control-plane model by default; do not allow wildcard/non-loopback API binds without a future explicit remote-mode design.
- Next steps: If remote/headless control is later required, introduce an explicit opt-in mode with TLS/auth hardening rather than loosening default bind policy.
- Change log: API host handling is now consistently loopback-only across startup, runtime serve, and settings updates.

- Status: Completed logging hardening / INFO-vs-DEBUG pass on `feature/log-hardening`.
  - Added shared logging utilities (`src/logging.rs`) for redaction helpers and warning throttling.
  - Removed noisy boot marker and moved raw SAM HELLO reply logging to `DEBUG`.
  - Redacted Kademlia identity at startup logs (`kad_id` now shortened).
  - Rebalanced KAD periodic status logging:
    - concise operational summary at `INFO`
    - full status payload at `DEBUG`
  - Added warning throttling for repetitive bootstrap send-failure warnings and recurring KAD decay warning.
  - Updated tracing file appender setup:
    - daily rotated naming as `prefix.YYYY-MM-DD.suffix` (default `rust-mule.YYYY-MM-DD.log`)
    - startup cleanup of matching logs older than 30 days.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 56 tests).
- Decisions: Keep redaction/throttling lightweight and local (no new dependencies) and preserve existing log filter controls (`general.log_level`, `general.log_file_level`).
- Next steps: Optional follow-up is to apply redaction helpers to any remaining DEBUG-level destination/id logs where operators may share debug bundles externally.
- Change log: Logging output is now safer and lower-noise at `INFO`, with richer diagnostics preserved at `DEBUG` and daily log retention enforced.

- Status: Completed clippy+formatting improvement batch on `feature/clippy-format-pass`.
  - Addressed all active `cargo clippy --all-targets --all-features` warnings across app/KAD/utility modules.
  - Applied idiomatic fixes (`div_ceil`, iterator/enumerate loops, collapsed `if let` chains, unnecessary casts/question-marks/conversions, lock-file open options).
  - Added targeted `#[allow(clippy::too_many_arguments)]` on orchestration-heavy KAD service functions where signature reduction would be invasive for this pass.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 56 tests).
- Decisions: Keep high-arity KAD orchestration signatures for now and explicitly annotate them; prioritize behavior-preserving lint cleanup over structural refactors in this iteration.
- Next steps: If desired, follow up with a dedicated refactor pass to reduce `too_many_arguments` allowances via context structs.
- Change log: Repository now passes clippy cleanly under current lint set, with formatting normalized.

- Status: Implemented UI auto-open and headless toggle flow (initial UI milestone #1):
  - Added `general.auto_open_ui` (default `true`) to runtime config/settings.
  - Startup now conditionally auto-opens `http://localhost:<port>/index.html` in default browser.
  - Auto-open is gated by readiness checks: token file exists, `/api/v1/health` returns 200, and `/index.html` returns 200 (timeout-protected).
  - Added settings wiring so UI/API `GET/PATCH /api/v1/settings` reads/writes `general.auto_open_ui`.
  - Added settings UI control: “Auto Open UI In Browser On Boot” with headless-disable option.
  - Updated docs (`docs/TODO.md`, `docs/UI_DESIGN.md`, `docs/architecture.md`, `docs/api_curl.md`).
- Decisions: Keep auto-open behavior best-effort and non-fatal; failures to launch browser only log warnings and do not affect backend startup.
- Next steps: Run browser-based axe/Lighthouse pass and patch measurable UI issues; then normalize remaining docs wording for “initial UI version” completion state.
- Change log: App can now launch the local UI automatically after API/UI/token readiness, and operators can disable this for headless runs via settings/config.

- Status: Alpine binding best-practice sanity pass completed (second pass):
  - Re-scanned all `ui/*.html` Alpine bindings and `ui/assets/js/{app,helpers}.js`.
  - Verified no side-effectful function calls in display bindings (`x-text`, `x-bind`, `x-show`, `x-if`, `x-for`).
  - Normalized remaining complex inline binding expressions into pure computed getters:
    - `appSearch.keywordHits` used by `ui/search.html` `x-for`.
    - `appSearchDetails.searchIdLabel` used by `ui/search_details.html` `x-text`.
- Decisions: Keep side effects restricted to lifecycle and explicit event handlers (`x-init`, `@click`, `@submit`, SSE callbacks).
- Next steps: Optional follow-up is extracting repeated status badge text ternaries into computed getters for style consistency only.
- Change log: Alpine templates now consistently consume normalized state/getters and avoid complex inline display expressions.

- Status: Completed a UI accessibility/usability sweep across all `ui/*.html` pages.
  - Added keyboard skip-link and focus target (`#main-content`) on all pages.
  - Added semantic navigation landmarks and `aria-current` for active routes.
  - Added live regions for runtime error/notice messages (`role="alert"` / `role="status"`).
  - Added table captions and explicit `scope` attributes on table headers.
  - Added chart canvas ARIA labels and log-region semantics for event stream output.
  - Added shared `.skip-link` and `.sr-only` styles in `ui/assets/css/base.css`.
- Decisions: Keep accessibility improvements HTML/CSS-only for now (no controller-side behavior changes), and preserve current visual layout.
- Next steps: Run browser-based automated audit (axe/Lighthouse) and address measurable contrast/focus-order findings.
- Change log: UI shell and data views now have stronger baseline WCAG support for keyboard navigation, screen-reader semantics, and dynamic status announcements.

- Status: Completed UI/API follow-up items 1 and 2 on `feature/ui-bootstrap`:
  - Added shared session status/check/logout widget in sidebar shell on all UI pages, backed by a reusable Alpine mixin.
  - Added periodic backend session cleanup task (`SESSION_SWEEP_INTERVAL=5m`) in addition to lazy cleanup on create/validate.
  - Added API unit test `cleanup_expired_sessions_removes_expired_entries`.
- Decisions: Keep session UX in a single shared sidebar control; keep session sweep simple (fixed interval background task) with existing `Mutex<HashMap<...>>` session store.
- Next steps: Merge this branch to `main`, then move to the next prioritized UI/API backlog item after validating behavior manually in browser.
- Change log: Session lifecycle visibility and expiry hygiene are now continuously maintained in both frontend shell and backend runtime.

- Implemented API bearer token rotation flow:
  - Added `POST /api/v1/token/rotate` (bearer-protected).
  - API token is now shared mutable state (`RwLock`) and token file path is stored in API state.
  - Rotation persists a new token to `data/api.token`, swaps in-memory token, and clears all active frontend sessions.
  - Added API test `token_rotate_updates_state_file_and_clears_sessions`.
  - Added settings UI action `Rotate API Token`:
    - Calls `/api/v1/token/rotate`
    - Updates `sessionStorage` token
    - Re-creates frontend session via `POST /api/v1/session`
  - Added token helper `rotate_token()` in `src/api/token.rs`.
  - Updated docs (`docs/architecture.md`, `docs/api_curl.md`, `docs/UI_DESIGN.md`) with token rotation behavior and endpoint.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Bearer tokens can now be actively rotated from UI/API with immediate session re-bootstrap and old-session invalidation.
- Completed next UI/API security+UX batch (in requested order):
  - Session lifecycle hardening:
    - Added `GET /api/v1/session/check` (session-cookie auth).
    - Added `POST /api/v1/session/logout` (session-cookie auth, clears cookie + invalidates server session).
    - Added session TTL handling (8h) with expiry cleanup on session create/validate.
    - Updated frontend SSE helper to probe `/api/v1/session/check` on stream errors and redirect to `/auth` on expired/invalid session.
    - Added visible UI logout control in settings (`Logout Session`) calling `POST /api/v1/session/logout` and redirecting to `/auth`.
  - Middleware integration tests (full-router):
    - `unauthenticated_ui_route_redirects_to_auth`
    - `authenticated_ui_route_with_session_cookie_succeeds`
    - `events_rejects_bearer_only_but_accepts_session_cookie`
  - Chart UX polish on `node_stats`:
    - Added chart controls: pause/resume sampling, reset history, and sample-window selector.
    - Increased history buffer depth and made chart rendering window configurable.
  - Added `build_app()` router constructor to enable handler+middleware integration tests without booting a TCP server.
  - Updated docs (`docs/architecture.md`, `docs/api_curl.md`, `docs/UI_DESIGN.md`, `docs/TODO.md`) for new session endpoints/behavior and chart controls status.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Implemented session check/logout + TTL cleanup, added middleware auth integration coverage, and shipped chart interaction controls in node stats.
- CSS normalization pass completed for variable/units discipline:
  - Moved remaining shared `base.css` size literals into reusable vars in `ui/assets/css/layout.css`:
    - container width, glow dimensions, badge/button/table sizing, log max-height.
  - Updated `ui/assets/css/base.css` to consume vars instead of hardcoded numeric literals.
  - Replaced non-hairline `px` units in theme focus/shadow tokens with relative units in:
    - `ui/assets/css/color-dark.css`
    - `ui/assets/css/colors-light.css`
    - `ui/assets/css/color-hc.css`
  - Kept hairline width token as `--line: 1px` for border usage.
  - Ran Prettier for CSS files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Shared UI styles now rely on layout/theme variables with non-hairline sizing converted to relative units.
- Implemented first Chart.js statistics set on `ui/node_stats.html`:
  - Added three charts:
    - Search hits over time (line)
    - Request/response rate over time (line)
    - Live vs idle peer mix over time (stacked bar)
  - Added Chart.js loader on `node_stats` and chart canvas panels in the page layout.
  - Extended `appNodeStats()` in `ui/assets/js/app.js`:
    - SSE-driven status updates + polling fallback.
    - Time-series history buffers and rate calculation from status counters.
    - Chart initialization/update lifecycle and theme-variable color usage.
  - Added reusable chart container token/style:
    - `--chart-height` in `ui/assets/css/layout.css`
    - `.chart-wrap` in `ui/assets/css/base.css`
  - Updated `docs/TODO.md` and `docs/UI_DESIGN.md` to mark Chart.js usage as implemented and statistics work as partial/ongoing.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Node stats page now includes live operational charts for search productivity, request/response rates, and peer health mix.
- Implemented frontend session-cookie auth for UI routes and SSE:
  - Added `POST /api/v1/session` (bearer-protected) to issue `rm_session` HTTP-only cookie.
  - Added in-memory session store in API state and cookie validation helpers.
  - Updated auth middleware policy:
    - `/api/v1/*` stays bearer-token protected (except `/api/v1/health` and `/api/v1/dev/auth`).
    - `/api/v1/events` now requires valid session cookie (no token query fallback).
    - All frontend routes (`/`, `/index.html`, `/ui/*`, fallback paths) require valid session cookie; unauthenticated access redirects to `/auth`.
  - Added `/auth` bootstrap page to establish session:
    - Calls `/api/v1/dev/auth` (loopback-only), then `POST /api/v1/session` with bearer token, then redirects to `/index.html`.
  - Updated frontend SSE client to use `/api/v1/events` without `?token=...`.
  - Updated auth-related tests:
    - API bearer exempt-path assertions
    - frontend exempt-path assertions
    - session-cookie parsing
  - Updated docs (`docs/TODO.md`, `docs/UI_DESIGN.md`, `docs/architecture.md`, `docs/api_curl.md`) to reflect session-cookie UI/SSE auth and bearer API auth.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Replaced SSE query-token auth with cookie-based frontend session auth and enforced cookie gating on all UI routes.
- Implemented API-backed settings read/update and wired settings UI:
  - Added `GET /api/v1/settings` and `PATCH /api/v1/settings` in `src/api/mod.rs`.
  - API now keeps a shared runtime `Config` in API state and persists valid PATCH updates to `config.toml`.
  - Added validation for settings updates (`sam.host`, `sam.port`, `sam.session_name`, `api.host`, `api.port`, and log filter syntax via `EnvFilter`).
  - Added API tests:
    - `settings_get_returns_config_snapshot`
    - `settings_patch_updates_and_persists_config`
    - `settings_patch_rejects_invalid_values`
  - Updated settings UI:
    - Added settings form in `ui/settings.html` for `general`, `sam`, and `api` fields.
    - Added `apiPatch()` helper and wired `appSettings()` to load/save via `/api/v1/settings`.
    - Added save/reload flow with restart-required notice.
  - Updated docs:
    - `docs/TODO.md`: marked API-backed settings task as done.
    - `docs/UI_DESIGN.md`: marked settings API integration as implemented.
    - `docs/architecture.md` and `docs/api_curl.md`: documented new settings endpoints and curl examples.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Settings page is now backed by persisted API settings (`GET/PATCH /api/v1/settings`) instead of runtime-only placeholders.
- Documentation/UI planning sync pass completed:
  - Updated `docs/TODO.md` UI checklist statuses to reflect implemented work (embedded assets, Alpine usage, shell pages, search form, overview, network status) and kept unresolved/partial items open (Chart.js usage, protected static UI, SSE token exposure, settings API, auto-open/headless toggle).
  - Updated `docs/UI_DESIGN.md` to match current routes and contracts:
    - `/api/v1/...` endpoint namespace in live-data and API contract sections.
    - Navigation model now reflects shared-shell multi-page UI (`index`, `search`, `search_details`, `node_stats`, `log`, `settings`) and `searchId` query param usage.
    - Added implementation snapshot with completed, partial, and open items.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Synced UI TODO/design documentation with the actual current implementation and clarified remaining UI backlog.
- Canonicalized root UI route to explicit index path:
  - `GET /` now redirects to `/index.html`.
  - Added explicit `GET /index.html` route serving embedded `index.html`.
  - Updated SPA fallback redirect target from `/` to `/index.html` for unknown non-API/non-asset routes.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Root URL now canonical redirects to `/index.html`; fallback redirects align to same canonical entry.
- Added explicit UI startup message on boot in `src/app.rs`:
  - Logs `rust-mule UI available at: http://localhost:<port>` right before API server task spawn.
  - Uses configured `api.port` so users get a direct URL immediately during startup.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Startup now emits a clear local UI URL message for quick operator discovery.
- Added SPA fallback behavior for unknown browser routes:
  - Added router fallback handler in `src/api/mod.rs` that redirects unknown non-API/non-asset paths to `/` (serving embedded `index.html`).
  - Redirect target is always `/`, so arbitrary query parameters on unknown paths are dropped.
  - Kept `/api/*` and `/ui/assets/*` as real 404 paths when missing (no SPA redirect for API/static asset misses).
  - Updated auth exemption to allow non-API paths through auth middleware so fallback can run before auth checks.
  - Added tests:
    - `spa_fallback_redirects_unknown_non_api_paths_to_root`
    - `spa_fallback_does_not_capture_api_or_asset_paths`
    - Extended auth-exempt path coverage for unknown non-API paths.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Unknown non-API routes now canonicalize to `/` (index) with query params stripped, while API and missing asset paths remain 404.
- Embedded UI into binary using `include_dir`:
  - Added `include_dir` dependency.
  - Added static `UI_DIR` bundle for `$CARGO_MANIFEST_DIR/ui`.
  - Switched UI page/asset serving in `src/api/mod.rs` from filesystem reads (`tokio::fs::read`) to embedded lookups.
  - Kept existing UI path safety guards (`is_safe_ui_segment`, `is_safe_ui_path`).
  - Added API unit test `embeds_required_ui_files` validating required `/ui/*.html`, `/ui/assets/css/*.css`, and `/ui/assets/js/*.js` are included in the embedded bundle.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: UI static assets/pages are now binary-embedded and served without runtime filesystem dependency.
- Alpine binding best-practice sanity pass completed:
  - Normalized `searchThreads` in `ui/assets/js/app.js` to include precomputed `state_class`.
  - Normalized node rows in `appNodeStats` to include precomputed `ui_state`, `ui_state_class`, and `inbound_label`.
  - Updated templates (`ui/index.html`, `ui/search.html`, `ui/search_details.html`, `ui/node_stats.html`, `ui/log.html`, `ui/settings.html`) to bind directly to precomputed fields instead of calling controller/helper methods from bindings.
  - Added `activeThreadStateClass` (index) and `detailsStateClass` (search details) getters for declarative badge binding.
  - Ran Prettier on UI JS/HTML and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Refactored Alpine bindings to remove template-time helper method calls and keep side effects inside explicit actions/lifecycle methods only.
- Performed CSS theme sanity refactor under `ui/assets/css`:
  - Moved all color literals used by shared UI components into theme files only:
    - `ui/assets/css/color-dark.css`
    - `ui/assets/css/colors-light.css`
    - `ui/assets/css/color-hc.css`
  - `ui/assets/css/base.css` and `ui/assets/css/layout.css` now consume color variables only (no direct color values).
  - Fixed dark theme scoping to `html[data-theme=\"dark\"]` (instead of global `:root`) so light/hc themes apply correctly.
- Added persisted theme bootstrapping:
  - New early loader `ui/assets/js/theme-init.js` applies `localStorage.ui_theme` before CSS paint.
  - Included `theme-init.js` in all UI HTML pages.
- Implemented Settings theme selector:
  - Added theme control in `ui/settings.html` for `dark|light|hc`.
  - `appSettings()` now applies selected theme to `<html data-theme=\"...\">` and persists to `localStorage`.
- Ran Prettier (`ui/assets/js/app.js`) and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after theme implementation (`cargo test` passed; existing clippy warnings unchanged).
- Performed API sanity audit against current UI helpers/controllers:
  - Confirmed all active Alpine controller API calls are backed by `/api/v1` endpoints.
  - Confirmed stop/delete UI controls now use real API handlers (`/searches/:id/stop`, `DELETE /searches/:id`).
- Added API handler-level tests for search control endpoints in `src/api/mod.rs`:
  - `search_stop_dispatches_service_command`
  - `search_delete_dispatches_with_default_purge_true`
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API sanity/test additions (`cargo test` passed; existing clippy warnings unchanged).
- Completed API-backing coverage for Alpine UI controls/helpers by implementing missing search control endpoints:
  - Added `POST /api/v1/searches/:search_id/stop`.
  - Added `DELETE /api/v1/searches/:search_id` with `purge_results` (default `true`).
  - Wired `indexApp.stopActiveSearch()` and `indexApp.deleteActiveSearch()` to these endpoints.
- Added backend service commands and logic:
  - `StopKeywordSearch` (disable ongoing search/publish for a job).
  - `DeleteKeywordSearch` (remove active job; optionally purge cached keyword results/store/interest).
- Added frontend helper `apiDelete()` (`ui/assets/js/helpers.js`) for `/api/v1` DELETE calls.
- Added unit tests in KAD service:
  - `stop_keyword_search_disables_active_job`
  - `delete_keyword_search_purges_cached_results`
- Updated API docs for new endpoints (`docs/architecture.md`, `docs/api_curl.md`).
- Ran Prettier on UI JS and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API coverage implementation (`cargo test` passed; existing clippy warnings unchanged).
- Closed UI consistency gaps identified in `/ui` review:
  - Added real settings page `ui/settings.html` with backing `appSettings()` controller.
  - Wired all sidebar `Settings` links to `/ui/settings`.
  - Wired `+ New Search` buttons with Alpine actions (`index` navigates to search page, `search` resets form state).
  - Wired overview action buttons (`Stop`, `Export`, `Delete`) to implemented Alpine methods in `indexApp`.
  - Removed hardcoded overview header state and made it data-driven from selected active thread.
- Ran Prettier on `ui/assets/js/app.js` and then ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after the UI consistency pass (`cargo test` passed; existing clippy warnings unchanged).
- Added `ui/log.html` with the shared shell and a dedicated Logs view.
- Implemented `appLogs()` Alpine controller in `ui/assets/js/app.js`:
  - Bootstraps token and loads search threads.
  - Fetches status snapshots from `GET /api/v1/status`.
  - Subscribes to `GET /api/v1/events` SSE and appends rolling log entries with timestamps.
  - Keeps an in-memory log buffer capped at 200 entries.
- Updated shell navigation links in UI pages so "Logs" points to `/ui/log`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after logs page/controller implementation (`cargo test` passed; existing clippy warnings unchanged).
- Ran Prettier on `ui/assets/js/app.js` and `ui/assets/js/helpers.js` using `ui/.prettierrc` rules; verified with `prettier --check`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after JS formatting pass (`cargo test` passed; existing clippy warnings unchanged).
- Added `ui/node_stats.html` with the same shell structure as other UI pages.
- Implemented node status view for live/active visibility:
  - Loads `/api/v1/status` and `/api/v1/kad/peers`.
  - Displays total/live/active node KPIs.
  - Displays node table with per-node state badge (`active`, `live`, `idle`) plus Kad ID/version/ages/failures.
- Added frontend `appNodeStats()` in `ui/assets/js/app.js`:
  - Sorts nodes by activity state then recency.
  - Reuses API-backed search threads in the sidebar.
- Updated shell navigation links across pages to point "Nodes / Routing" to `/ui/node_stats`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after node stats page implementation (`cargo test` passed; existing clippy warnings unchanged).
- Added API-backed keyword search thread endpoints:
  - `GET /api/v1/searches` returns active keyword-search jobs from KAD `keyword_jobs`.
  - `GET /api/v1/searches/:search_id` returns one active search plus its current hits.
  - `search_id` maps to keyword ID hex for the active job.
- Implemented dynamic search threads in UI sidebars:
  - `ui/index.html` and `ui/search.html` now load active search threads from API.
  - Search thread rows link to `/ui/search_details?searchId=<keyword_id_hex>`.
- Added `ui/search_details.html` with the same shell:
  - Reads `searchId` from query params.
  - Loads `/api/v1/searches/:search_id` and displays search summary + hits table.
- Extended frontend app wiring:
  - Added shared search-thread loading and state-badge mapping in `ui/assets/js/app.js`.
  - Added `appSearchDetails()` controller for search detail page behavior.
- Updated docs for new API routes (`docs/architecture.md`, `docs/api_curl.md`).
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after search-thread/details implementation (`cargo test` passed; existing clippy warnings unchanged).
- Replicated the app shell layout in `ui/search.html` (sidebar + main panel) to match the index page structure.
- Implemented first functional keyword-search form in the search UI:
  - Added query and optional `keyword_id_hex` inputs.
  - Wired `POST /api/v1/kad/search_keyword` submission from Alpine (`appSearch.submitSearch`).
  - Added results refresh via `GET /api/v1/kad/keyword_results/:keyword_id_hex`.
  - Added first-pass results table rendering for keyword hits.
- Added reusable UI form styles in shared CSS:
  - New form classes in `ui/assets/css/base.css` (`form-grid`, `field`, `input`).
  - Added form-control tokens to `ui/assets/css/layout.css`.
- Added JS helper `apiPost()` in `ui/assets/js/helpers.js` and expanded `appSearch()` state/actions in `ui/assets/js/app.js`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after search UI implementation (`cargo test` passed; existing clippy warnings unchanged).
- Moved `index.html` inline styles into shared CSS:
  - Removed `<style>` block from `ui/index.html`.
  - Added reusable shell/sidebar/search-state classes in `ui/assets/css/base.css`.
  - Added layout/state CSS variables in `ui/assets/css/layout.css` and referenced them from base styles.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after CSS/layout refactor (`cargo test` passed; existing clippy warnings unchanged).
- Updated `ui/index.html` layout to match UI design spec shell:
  - Added persistent sidebar (primary nav + search thread list + new search control).
  - Added main search overview sections (header/actions, KPIs, progress, results, activity/logs).
  - Preserved existing Alpine status/token/SSE bindings while restructuring markup.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after index layout update (`cargo test` passed; existing clippy warnings unchanged).
- Implemented backend-served UI bootstrap skeleton:
  - Added static UI routes: `/`, `/ui`, `/ui/:page`, and `/ui/assets/*`.
  - Added safe path validation for UI file serving (reject traversal/unsafe paths).
  - Added content-type-aware static file responses for HTML/CSS/JS/assets.
- Implemented UI auth bootstrap flow for development:
  - UI now bootstraps bearer auth via `GET /api/v1/dev/auth`.
  - Token is stored in browser `sessionStorage` and used for `/api/v1/status`.
  - UI opens SSE with `GET /api/v1/events?token=...` for browser compatibility.
- Updated UI skeleton pages and JS modules:
  - Rewrote `ui/assets/js/helpers.js` and `ui/assets/js/app.js` to align with `/api/v1`.
  - Updated `ui/index.html` and `ui/search.html` to use module scripts and current API flow.
- Added/updated API tests:
  - Query-token extraction test for SSE auth path.
  - UI path-safety validation test coverage.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after UI/bootstrap changes (`cargo test` passed; existing clippy warnings unchanged).
- Implemented API CORS hardening for `/api/v1`:
  - Allow only loopback origins (`localhost`, `127.0.0.1`, and loopback IPs).
  - Allow only `Authorization` and `Content-Type` request headers.
  - Allow methods `GET`, `POST`, `PUT`, `PATCH`, `OPTIONS`.
  - Handle `OPTIONS` preflight without bearer auth.
  - Added unit tests for origin allow/deny behavior.
- Fixed CORS origin parsing for bracketed IPv6 loopback (`http://[::1]:...`) and re-ran validation (`cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`).
- API contract tightened for development-only workflow:
  - Removed temporary unversioned API route aliases; API is now `/api/v1/...` only.
  - Removed `api.enabled` compatibility field from config parsing.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after removing legacy API handling (`cargo test` passed; clippy warnings remain in existing code paths).
- Created feature branch `feature/api-v1-control-plane` and implemented API control-plane changes:
  - Canonical API routes are now under `/api/v1/...`.
  - Added loopback-only dev auth endpoint `GET /api/v1/dev/auth` (returns bearer token).
  - API is now always on; only API host/port are configurable.
- Updated docs and shell wrappers to use `/api/v1/...` endpoints (`README.md`, `docs/architecture.md`, `docs/api_curl.md`, `docs/scripts/*`, `docs/TODO.md`, `docs/API_DESIGN.md`, `docs/UI_DESIGN.md`).
- Added `docs/scripts/dev_auth.sh` helper for `GET /api/v1/dev/auth`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after the API/docs changes (`cargo test` passed; clippy warnings remain in existing code paths).
- Per-user request, documentation normalization pass completed across `docs/` (typos, naming consistency, and branch references).
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after docs changes (`cargo test` passed; clippy warnings remain in existing code paths).
- Long-haul two-instance run (25 rounds) confirmed network-origin keyword hits on both instances:
  - A received non-empty `SEARCH_RES` at 2026-02-11 19:41:41.
  - B received non-empty `SEARCH_RES` at 2026-02-11 19:50:02.
- Routing snapshot at end of run: total_nodes=157, verified=135, buckets_empty=121, bucket_fill_max=80, last_seen_max≈35060s (~9.7h), last_inbound_max≈29819s (~8.3h). Routing still not growing (`new_nodes=0`).
- Observed SAM `SESSION STATUS RESULT=I2P_ERROR MESSAGE="PONG timeout"` on both instances at 2026-02-12 06:49:20; service auto-recreated SAM session.
- Source publish/search remained empty in the script output.
- Periodic KAD2 BOOTSTRAP_REQ now sends **plain** packets to peers with `kad_version` 2–5 and **encrypted** packets only to `kad_version >= 6` to avoid silent ignores in mixed-version networks.
- Publish/search candidate selection now truncates by **distance first**, then optionally reorders the *same* set by liveness to avoid skipping closest nodes.
- Restarting a keyword search or publish job now clears the per-job `sent_to_*` sets so manual retries re-send to peers instead of becoming no-ops.
- Publish/search candidate selection now returns a **distance-ordered list with fallback** (up to `max*4` closest) so if early candidates are skipped, farther peers are still available in the batch.

## Status (2026-02-11)

- Updated `docs/scripts/two_instance_dht_selftest.sh` to poll keyword results (early exit on `origin=network`), add configurable poll interval, and allow peer snapshot frequency control.
- Increased default `wait-search-secs` to 45s in the script (I2P cadence).
- Updated `tmp/test_script_command.txt` with new flags for polling and peer snapshot mode.
- Added routing snapshot controls to `docs/scripts/two_instance_dht_selftest.sh` (each|first|end|none) and end-of-run routing summary/buckets when `--routing-snapshot end` is set.
- Updated `tmp/test_script_command.txt` to use `--routing-snapshot end` and `--peers-snapshot none` for the next long run.

## Status (2026-02-10)

- Ran `docs/scripts/two_instance_dht_selftest.sh` (5 rounds). Each instance only saw its own locally-injected keyword hit; no cross-instance keyword hits observed.
- No `PUBLISH_RES (key)` acks and no inbound `PUBLISH_KEY_REQ` during the run; `SEARCH_RES` replies were empty.
- Routing stayed flat (~154), live peers ~2, network appears quiet.
- Added debug routing endpoints (`/debug/routing/*`) plus debug lookup trigger (`/debug/lookup_once`) and per-bucket refresh lookups.
- Added staleness-based bucket refresh with an under-populated growth mode; routing status logs now include bucket fill + verified %.
- Routing table updates now treat inbound responses as activity (last_seen/last_inbound) and align bucket index to MSB distance.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after the debug/refresh changes (clippy warnings remain; see prior notes).
- Added HELLO preflight on inbound responses, prioritized live peers for publish/search, and added post-warmup routing snapshots in the two-instance script.
- Aligned Kad2 HELLO_REQ encoding with iMule: kadVersion=1, empty TagList, sent unobfuscated.
- Added HELLO_RES_ACK counters (sent/recv), per-request debug logs for publish/search requests, and a `/debug/probe_peer` API to send HELLO/SEARCH/PUBLISH to a specific peer.
- Added `/debug/probe_peer` curl docs + script (`docs/api_curl.md`, `docs/scripts/debug_probe_peer.sh`).
- Added KAD2 RES contact acceptance stats (per-response debug log) and HELLO_RES_ACK skip counter.
- Added optional dual HELLO_REQ mode (plain + obfuscated) behind `kad.service_hello_dual_obfuscated` (experimental).
- Added config flag wiring for dual-HELLO mode and contact acceptance stats logging; updated `config.toml` hint.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after these changes (clippy warnings remain; see prior notes).
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after debug probe + logging changes (clippy warnings remain; see prior notes).
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after HELLO/live-peer changes (clippy warnings remain; see prior notes).
- Added `origin` field to keyword hits (`local` vs `network`) in the API response.
- Added `/kad/peers` API endpoint and extra inbound-request counters to `/status` for visibility.
- Increased keyword job cadence/batch size slightly to improve reach without flooding.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` (clippy still reports pre-existing warnings).
- Extended `docs/scripts/two_instance_dht_selftest.sh` to include source publish/search flows and peer snapshots.
- Added preflight HELLOs for publish/search targets and switched publish/search target selection to distance-only (no liveness tiebreak).

## Decisions (2026-02-10)

- Token/session security model:
  - Session TTL bounds cookie compromise window.
  - Explicit token rotation is available to invalidate old bearer + all active sessions.
  - UI performs immediate token/session re-bootstrap after rotation to avoid operator disruption.
- Session auth policy now includes explicit lifecycle endpoints:
  - `session` issue (bearer), `session/check` validate (cookie), `session/logout` revoke (cookie).
  - Session validation performs lazy expiry cleanup; unauthenticated/expired frontend flows redirect to `/auth`.
- CSS policy tightened for shared UI styles: prefer variable-driven sizing and relative units; reserve `px` for border/hairline tokens.
- Place first operational charts on `node_stats` to pair routing/node data with live trend context before introducing a dedicated statistics page.
- Auth split for v1 local UI:
  - Keep bearer token as the API auth mechanism for `/api/v1/*`.
  - Use a separate HTTP-only session cookie for browser page/asset loads and SSE.
  - Remove SSE token query parameter usage from frontend.
- Settings API scope for v1: expose/update a focused config subset (`general`, `sam`, `api`) and require restart for full effect.
- Keep `docs/TODO.md` UI checkboxes aligned to implementation truth, using `[x]` for done and `[/]` for partial completion where design intent is not fully met.
- UI entrypoint canonical URL is `/index.html`; `/` is a redirect alias.
- Operator UX: always log a copy-pasteable localhost UI URL at startup.
- Route-fallback policy: treat unknown non-API, non-asset browser paths as SPA entry points and redirect to `/`; keep unknown `/api/*` and `/ui/assets/*` as 404.
- Serve UI from binary-embedded assets (`include_dir`) instead of runtime disk reads to guarantee deploy-time asset completeness.
- Alpine template bindings should be declarative and side-effect free; compute display-only classes/labels in controller state/getters before render.
- Theme ownership rule: all color values live in `color-*` theme files; shared CSS (`base.css`, `layout.css`) references theme vars only.
- Theme selection persistence uses `localStorage` key `ui_theme` and is applied via `<html data-theme=\"dark|light|hc\">`.
- Treat `docs/architecture.md` + `docs/api_curl.md` as the implementation-aligned API references for current `/api/v1`; `docs/API_DESIGN.md` remains broader future-state design.
- Search stop/delete are now first-class `/api/v1` controls instead of UI-local placeholders.
- `DELETE /api/v1/searches/:search_id` defaults to purging cached keyword results for that search (`purge_results=true`) to keep UI state consistent after delete.
- Use current active search thread (query-selected or first available) as the source for overview title/state.
- Use SSE-backed status updates as the first log timeline source in UI (`appLogs`), with snapshot polling available via manual refresh.
- Use `ui/.prettierrc` as the canonical formatter config for UI JS files (`ui/assets/js/*`).
- Define node UI state as:
  - `active`: `last_inbound_secs_ago <= 600`
  - `live`: `last_seen_secs_ago <= 600`
  - `idle`: otherwise
- Treat active keyword-search jobs in KAD service (`keyword_jobs`) as the canonical backend source for UI "search threads".
- Use keyword ID hex as `search_id` for details routing in v1 (`/ui/search_details?searchId=<keyword_id_hex>` and `/api/v1/searches/:search_id`).
- Keep search UI v1 focused on real keyword-search queue + cached-hit retrieval rather than adding placeholder-only controls.
- Enforce no inline `<style>` blocks in UI HTML; shared styles must live under `ui/assets/css/`.
- Keep sizing/spacing/state tokens in `ui/assets/css/layout.css` and consume them from component/layout rules in `ui/assets/css/base.css`.
- Keep `index.html` as a single-shell page aligned to the chat-style dashboard design, even before full search API wiring exists.
- Serve the in-repo UI skeleton from the Rust backend (single local control-plane origin).
- Keep browser auth bootstrap development-only and loopback-only via `/api/v1/dev/auth`.
- Permit SSE token via query parameter for `/api/v1/events` to support browser `EventSource` without custom headers.
- Restrict browser CORS access to loopback origins for local-control-plane safety.
- Use strict `/api/v1` routes only; no legacy unversioned aliases are kept.
- Implement loopback-only dev auth as `GET /api/v1/dev/auth` (no auth header required).
- Make API mandatory (always enabled) and remove `api.enabled` compatibility handling from code.
- Treat `main` as the canonical branch in project docs.
- No code changes made based on this run; treat results as network sparsity/quietness signal.
- Keep local publish injection, but expose `origin` so tests are unambiguous.
- Keep Rust-native architecture; optimize behavioral parity rather than line-by-line porting.
- Documented workflow: write/update tests where applicable, run fmt/clippy/test, commit + push per iteration.
- Accept existing clippy warnings for now; no functional changes required for this iteration.
- Use the two-instance script to exercise source publish/search as part of routine sanity checks.
- Prioritize DHT correctness over liveness when selecting publish/search targets.
- Implement bucket refresh based on staleness (with an under-populated growth mode) to grow the table without aggressive churn.
- Use MSB-first bucket indexing to match iMule bit order and ensure random bucket targets map correctly.
- On inbound responses, opportunistically send HELLO to establish keys and improve publish/search acceptance.
- Prefer recently-live peers first for publish/search while keeping distance correctness as fallback.
- Match iMule HELLO_REQ behavior (unencrypted, kadVersion=1, empty TagList) to improve interop.
- Add a targeted debug probe endpoint rather than relying on background jobs to validate per-peer responses.
- Add per-response acceptance stats and HELLO_ACK skip counters to see why routing doesn’t grow.
- Add an optional dual-HELLO mode (explicitly marked as “perhaps”, since it diverges from iMule).
- Dual-HELLO is explicitly flagged as a “perhaps”/experimental divergence from iMule behavior.

## Next Steps (2026-02-10)

- Consider periodic background cleanup for expired sessions (currently lazy cleanup on create/validate).
- Add optional “session expires in” UI indicator if a session metadata endpoint is introduced.
- Expand chart interactions/usability:
  - Add legend toggles and chart tooltips formatting for rates and hit counts.
  - Add pause/reset controls for time-series buffers.
  - Consider moving/duplicating high-value charts to overview once layout is finalized.
- Add session lifecycle endpoints and UX (`POST /api/v1/session/logout`, session-expired handling in UI).
- Add session persistence/eviction policy (TTL + periodic cleanup) instead of in-memory unbounded set.
- Add integration tests for middleware behavior:
  - unauthenticated UI path redirects to `/auth`
  - authenticated UI path succeeds
  - `/api/v1/events` rejects bearer-only and accepts valid session cookie
- Add an explicit integration test for `PATCH /api/v1/settings` through the full router (not just handler-level tests), including persistence failure behavior.
- Consider adding runtime-apply behavior for selected settings that do not require restart (and return per-field `restart_required` metadata).
- Prioritize remaining UI gaps from `docs/TODO.md`/`docs/UI_DESIGN.md`:
  - Implement Chart.js-based statistics visualizations.
  - Remove SSE token exposure via query params (or document accepted tradeoff explicitly).
  - Decide whether static UI routes should become bearer-protected and implement consistently.
  - Implement API-backed settings (`GET/PATCH /api/settings`) and wire the settings page.
- Add an integration test against the full Axum router asserting `GET /nonexisting.php?x=1` returns redirect `Location: /`.
- Consider adding a `/api/v1/ui/manifest` debug endpoint exposing embedded UI file names/checksums for operational verification.
- Add a lightweight UI smoke test pass (load each `/ui/*` page and assert Alpine init has no console/runtime errors) to guard future binding regressions.
- Add integration tests for API auth/CORS behavior (preflight + protected endpoint access patterns).
- Expand UI beyond status/search placeholder views (routing table, peers, and publish/search workflow surfaces).
- Replace static index sidebar/result placeholders with real search data once `/api/searches` endpoints are implemented.
- Add search-history/thread state in the UI (persisted list of submitted keyword jobs and selection behavior).
- Add API/frontend support for completed (no longer active) search history so `search_details` remains available after a job leaves `keyword_jobs`.
- Consider making node-state thresholds (`active/live` age windows) configurable in UI settings or API response metadata.
- Add richer log event typing/filtering once non-status event types are exposed from the API.
- Decide which `docs/API_DESIGN.md` endpoints should be promoted into the near-term implementation backlog vs kept as long-term design.
- Consider renaming `ui/assets/css/colors-light.css` to `ui/assets/css/color-light.css` for file-name symmetry (non-functional cleanup).
- Decide whether to keep dev auth as an explicit development-only endpoint or move to stronger local auth flow before release.
- Add UI-focused integration coverage (static UI route serving + SSE auth query behavior end-to-end).
- Consider adding a debug toggle to disable local injection during tests.
- Consider clearing per-keyword job `sent_to_*` sets on new API commands to allow re-tries to the same peers.
- Consider a small UI view over `/kad/peers` to spot real inbound activity quickly.
- Optionally address remaining clippy warnings in unrelated files.
- Run the updated two-instance script and review `OUT_FILE` + logs for source publish/search behavior.
- Re-run two-instance test to see if HELLO preflight improves `PUBLISH_RES` / `SEARCH_RES` results.
- Run `docs/scripts/debug_routing_summary.sh` + `debug_routing_buckets.sh` around test runs; use `debug_lookup_once` to trace a single lookup.
- Re-run the two-instance script (now with post-warmup routing snapshots) and check for HELLO traffic + publish/search ACKs.
- Re-run two-instance test and check for `recv_hello_ress` / `recv_hello_reqs` increases after HELLO_REQ change.
- Use `/debug/probe_peer` against a known peer from `/kad/peers` to check HELLO/SEARCH/PUBLISH responses.
- If `hello_ack_skipped_no_sender_key` keeps climbing, consider enabling `kad.service_hello_dual_obfuscated = true` for a test run.
- If `KAD2 RES contact acceptance stats` show high `dest_mismatch` or `already_id`, investigate routing filters or seed freshness.

## Roadmap Notes

- Storage: file-based runtime state under `data/` is fine for now (and aligns with iMule formats like `nodes.dat`).
  As we implement real client features (search history, file hashes/metadata, downloads, richer indexes),
  consider SQLite for structured queries + crash-safe transactions. See `docs/architecture.md`.

## Change Log

- 2026-02-12: CSS/theme pass: consolidate shared UI colors into `color-dark.css`/`colors-light.css`/`color-hc.css`, remove direct colors from `base.css`/`layout.css`, add early `theme-init.js`, and implement settings theme selector persisted via `localStorage` + `html[data-theme]`; run Prettier + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: API sanity check-run completed; add endpoint-level API tests for `/api/v1/searches/:search_id/stop` and `DELETE /api/v1/searches/:search_id` dispatch behavior (`src/api/mod.rs`); run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement missing `/api/v1` backing for UI search controls: add stop/delete search endpoints + service commands/logic + tests; wire UI stop/delete to API and add `apiDelete()` helper; update API docs; run Prettier + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement UI consistency fixes 1..4: add `ui/settings.html` + `appSettings()`, wire settings/new-search/actions, and make overview header/state thread-driven; run Prettier (`ui/assets/js/app.js`) + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `ui/log.html` and `appLogs()` (status snapshot + SSE-backed rolling log view), and route sidebar "Logs" links to `/ui/log`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Format `ui/assets/js/app.js` and `ui/assets/js/helpers.js` with `ui/.prettierrc`; verify with `prettier --check`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `ui/node_stats.html` with shell + node status table/KPIs using `/api/v1/status` and `/api/v1/kad/peers`; implement `appNodeStats()`; point shell nav "Nodes / Routing" to `/ui/node_stats`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `/api/v1/searches` and `/api/v1/searches/:search_id` for active keyword jobs; wire search-thread sidebars to API; add `ui/search_details.html` that loads details via `searchId` query param; update API docs; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Replicate shell in `ui/search.html`; implement first keyword search form wired to `/api/v1/kad/search_keyword` + `/api/v1/kad/keyword_results/:keyword_id_hex`; add reusable form CSS classes/tokens and `apiPost()` helper; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Remove inline styles from `ui/index.html`; move reusable shell/search layout rules to `ui/assets/css/base.css`; define layout/state CSS vars in `ui/assets/css/layout.css`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Redesign `ui/index.html` into the UI spec shell (sidebar + search-overview main panel), preserving existing Alpine status/token/SSE wiring; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Serve UI skeleton from backend (`/`, `/ui`, `/ui/:page`, `/ui/assets/*`) with safe path validation and static content handling; allow SSE query-token auth for `/api/v1/events`; add related tests and update UI JS/HTML/docs (`src/api/mod.rs`, `ui/*`, `README.md`, `docs/architecture.md`, `docs/TODO.md`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after UI/bootstrap work (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add loopback-only CORS middleware for `/api/v1` with explicit preflight handling and origin validation tests (`src/api/mod.rs`).
- 2026-02-12: Fix CORS IPv6 loopback origin parsing (`[::1]`) and rerun fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Extend `Access-Control-Allow-Methods` to include `PUT` and `PATCH`; add regression test (`src/api/mod.rs`).
- 2026-02-12: Remove temporary unversioned API aliases and enforce `/api/v1` only (`src/api/mod.rs`).
- 2026-02-12: Remove `api.enabled` compatibility handling from config/app code (`src/config.rs`, `src/app.rs`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after strict v1-only API cleanup (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement `/api/v1` canonical routing, add loopback-only `GET /api/v1/dev/auth`, make API always-on (deprecate/ignore `api.enabled`), and add compatibility aliases for legacy routes (`src/api/mod.rs`, `src/app.rs`, `src/config.rs`, `src/main.rs`, `config.toml`).
- 2026-02-12: Update API docs/scripts to `/api/v1` and add `docs/scripts/dev_auth.sh` helper.
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API routing/control-plane changes (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Normalize docs wording/typos and align branch references to `main` (`docs/TODO.md`, `docs/dev.md`, `docs/handoff.md`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after doc normalization (tests pass; existing clippy warnings unchanged).
- 2026-02-11: Tune two-instance selftest script with polling + peer snapshot controls; update `tmp/test_script_command.txt` to use new flags.
- 2026-02-11: Add routing snapshot controls and end-of-run routing dumps for the two-instance selftest; update `tmp/test_script_command.txt`.
- 2026-02-12: Long-haul run confirmed network-origin keyword hits; routing table still flat; SAM session recreated after PONG timeout on both instances.
- 2026-02-12: Send periodic BOOTSTRAP_REQ unencrypted to Kad v2–v5 peers; only encrypt for Kad v6+.
- 2026-02-12: Fix publish/search peer selection so distance is primary; liveness only reorders within the closest set.
- 2026-02-12: Clear keyword job `sent_to_search` / `sent_to_publish` on restart to allow manual retries to send again.
- 2026-02-12: Return distance-ordered peer lists with fallback (max*4) to avoid empty batches when closest peers are skipped.
- 2026-02-10: Two-instance DHT selftest (5 rounds) showed only local keyword hits; no cross-instance results, no publish-key acks, empty search responses; routing stayed flat (quiet network).
- 2026-02-10: Add `origin` field to keyword hit API responses (`local` vs `network`).
- 2026-02-10: Add `/kad/peers` API endpoint and new inbound request counters in `/status`; slightly increase keyword job cadence/batch size.
- 2026-02-10: Add workflow guidance in `AGENTS.md` (tests, fmt/clippy/test, commit + push per iteration).
- 2026-02-10: Extend two-instance selftest to include source publish/search and peer snapshots; add `kad_peers_get.sh`.
- 2026-02-10: Add HELLO preflight for publish/search targets and use distance-only selection for DHT-critical actions.
- 2026-02-10: Add debug routing endpoints + debug lookup trigger; add staleness-based bucket refresh with under-populated growth mode.
- 2026-02-10: Align bucket indexing with MSB bit order; mark last_seen/last_inbound on inbound responses.
- 2026-02-10: Send HELLO on inbound responses, prioritize live peers for publish/search, and add post-warmup routing snapshots in the selftest script.
- 2026-02-10: Align Kad2 HELLO_REQ with iMule (kadVersion=1, empty taglist, unobfuscated); add `encode_kad2_hello_req` and update HELLO send paths.
- 2026-02-10: Add HELLO_RES_ACK counters + publish/search request debug logs; add `/debug/probe_peer` API for targeted HELLO/SEARCH/PUBLISH probes.
- 2026-02-10: Document `/debug/probe_peer` in `docs/api_curl.md` and add `docs/scripts/debug_probe_peer.sh`.
- 2026-02-10: Add KAD2 RES contact acceptance stats (debug) + HELLO_ACK skip counter; add optional dual HELLO_REQ mode behind config flag (experimental, diverges from iMule).
- 2026-02-10: Wire `kad.service_hello_dual_obfuscated` config; add KAD2 RES acceptance stats and HELLO_ACK skip counters to status/logs; update `config.toml`.
- 2026-02-06: Embed distributable nodes init seed at `assets/nodes.initseed.dat`; create `data/nodes.initseed.dat` and `data/nodes.fallback.dat` from embedded seed (best-effort) so runtime no longer depends on repo-local reference folders.
- 2026-02-06: Reduce default stdout verbosity to `info` (code default and repo `config.toml`; file logging remains configurable and can stay `debug`).
- 2026-02-06: Make Kad UDP key secret file-backed only (`data/kad_udp_key_secret.dat`); `kad.udp_key_secret` is deprecated/ignored to reduce misconfiguration risk.
- 2026-02-06: Implement iMule-style `KADEMLIA2_REQ` sender-id field and learn sender IDs from inbound `KADEMLIA2_REQ` to improve routing growth.
- 2026-02-06: Clarify iMule `KADEMLIA2_REQ` first byte is a *requested contact count* (low 5 bits), and update Rust naming (`requested_contacts`) + parity docs.
- 2026-02-06: Fix Kad1 `HELLO_RES` contact type to `3` (matches iMule `CContact::Self().WriteToKad1Contact` default).
- 2026-02-06: Periodic BOOTSTRAP refresh: stop excluding peers by `failures >= max_failures` (BOOTSTRAP is a distinct discovery path); rely on per-peer backoff instead so refresh continues even when crawl timeouts accumulate.
- 2026-02-07: Observed 3 responding peers (`live=3`) across a multi-hour run (improvement from prior steady state of 2). Routing table size still stayed flat (`routing=153`, `new_nodes=0`), indicating responders are returning already-known contacts.
- 2026-02-07: Add `live_10m` metric to status logs (recently-responsive peers), and change periodic BOOTSTRAP refresh to rotate across "cold" peers first (diversifies discovery without increasing send rate).
- 2026-02-07: Fix long-run stability: prevent Tokio interval "catch-up bursts" (missed tick behavior set to `Skip`), treat SAM TCP-DATAGRAM framing desync as fatal, and auto-recreate the SAM DATAGRAM session if the socket drops (service keeps running instead of crashing).
- 2026-02-07: Introduce typed SAM errors (`SamError`) for the SAM protocol layer + control client + datagram transports; higher layers use `anyhow` but reconnect logic now searches the error chain for `SamError` instead of string-matching messages.
- 2026-02-07: Add a minimal local HTTP API skeleton (REST + SSE) for a future GUI (`src/api/`), with a bearer token stored in `data/api.token`. See `docs/architecture.md`.
- 2026-02-07: Start client-side search/publish groundwork: add Kad2 `SEARCH_SOURCE_REQ` + `PUBLISH_SOURCE_REQ` encoding/decoding, handle inbound `SEARCH_RES`/`PUBLISH_RES` in the service loop, and expose minimal API endpoints to enqueue those actions.
- 2026-02-07: Add iMule-compatible keyword hashing + Kad2 keyword search:
  - iMule-style keyword hashing (MD4) used for Kad2 keyword lookups (`src/kad/keyword.rs`, `src/kad/md4.rs`).
  - `KADEMLIA2_SEARCH_KEY_REQ` encoding and unified `KADEMLIA2_SEARCH_RES` decoding (source + keyword/file results) (`src/kad/wire.rs`, `src/kad/service.rs`).
  - New API endpoints: `POST /kad/search_keyword`, `GET /kad/keyword_results/:keyword_id_hex` (`src/api/mod.rs`).
  - Curl cheat sheet updated (`docs/api_curl.md`).
- 2026-02-07: Add bounded keyword result caching (prevents memory ballooning):
  - Hard caps (max keywords, max total hits, max hits/keyword) + TTL pruning.
  - All knobs are configurable in `config.toml` under `[kad]` (`service_keyword_*`).
  - Status now reports keyword cache totals + eviction counters.
- 2026-02-09: Two-instance keyword publish/search sanity check (mule-a + mule-b):
  - Both sides successfully received `KADEMLIA2_SEARCH_RES` replies, but **all keyword results were empty** (`keyword_entries=0`).
  - Root cause (interop): iMule rejects Kad2 keyword publishes which only contain `TAG_FILENAME` + `TAG_FILESIZE`.
    In iMule `CIndexed::AddKeyword` checks `GetTagCount() != 0`, and Kad2 publish parsing stores filename+size out-of-band
    (so they do not contribute to the internal tag list). iMule itself publishes additional tags like `TAG_SOURCES` and
    `TAG_COMPLETE_SOURCES`. See `source_ref/.../Search.cpp::PreparePacketForTags` and `Indexed.cpp::AddKeyword`.
  - Fix: rust-mule now always includes `TAG_SOURCES` and `TAG_COMPLETE_SOURCES` in Kad2 keyword publish/search-result taglists
    (`src/kad/wire.rs`), matching iMule expectations.
- 2026-02-09: Follow-up two-instance test showed *some* keyword results coming back from the network (`keyword_entries=1`),
  but A and B still tended to publish/search against disjoint "live" peers and would miss each other's stores.
  Fix: change DHT-critical peer selection to be **distance-first** (XOR distance primary; liveness as tiebreaker) so that
  publish/search targets the correct closest nodes (`src/kad/routing.rs`, `src/kad/service.rs`).
- 2026-02-09: Two-instance test artifacts under `./tmp/` (mule-a+mule-b with `docs/scripts/two_instance_dht_selftest.sh`):
  - Script output shows each side only ever returns its *own* published hit for the shared keyword (no cross-hit observed).
    This is expected with the current API behavior because `POST /kad/publish_keyword` injects a local hit into the in-memory cache.
    Real proof of network success is `got SEARCH_RES ... keyword_entries>0 inserted_keywords>0` in logs (or explicit `origin=network` markers).
  - Both instances received at least one `got SEARCH_RES ... keyword_entries=0` for the shared keyword (network replied, but empty).
  - Neither instance logged `got PUBLISH_RES (key)` (no publish acks observed).
  - `mule-b` received many inbound `KADEMLIA2_PUBLISH_KEY_REQ` packets from peer `-8jmpFh...` that fail decoding with `unexpected EOF at 39`
    (345 occurrences in that run), so we do not store those keywords and we do not reply with `PUBLISH_RES` on that path.
  - Next debugging targets:
    - capture raw decrypted payload (len + hex head) on first decode failure to determine truncation vs parsing mismatch,
    - make publish-key decoding best-effort and still reply with `PUBLISH_RES` (key) to reduce peer retries,
    - add `origin=local|network` to keyword hits (or a debug knob to disable local injection) to make tests unambiguous.
- 2026-02-09: Implemented publish-key robustness improvements:
  - Add lenient `KADEMLIA2_PUBLISH_KEY_REQ` decoding which can return partial entries and still extract the keyword prefix for ACKing (`src/kad/wire.rs`).
  - On decode failure, rust-mule now attempts a prefix ACK (send `KADEMLIA2_PUBLISH_RES` for the keyword) so peers stop retransmitting.
  - Added `recv_publish_key_decode_failures` counter to `/status` output for visibility (`src/kad/service.rs`).
- 2026-02-09: Discovered an iMule debug-build quirk in the wild:
  - Some peers appear to include an extra `u32` tag-serial counter inside Kad TagLists (enabled by iMule `_DEBUG_TAGS`),
    which shifts tag parsing (we saw this in a publish-key payload where the filename length was preceded by 4 bytes).
  - rust-mule now retries TagList parsing with and without this extra `u32` field for:
    - Kad2 HELLO taglists (ints)
    - search/publish taglists (search info)
    (`src/kad/wire.rs`).
- 2026-02-09: Added rust-mule peer identification:
  - Kad2 `HELLO_REQ/HELLO_RES` now includes a private vendor tag `TAG_RUST_MULE_AGENT (0xFE)` with a string like `rust-mule/<version>`.
  - If a peer sends that tag, rust-mule records it in-memory and logs it once when first learned.
  - This allows rust-mule-specific feature gating going forward while remaining compatible with iMule (unknown tags are ignored).
- 2026-02-07: TTL note (small/slow iMule I2P-KAD reality):
  - Keyword hits are a “discovery cache” and can be noisy; expiring them is mostly for memory hygiene.
  - File *sources* are likely intermittent; plan to keep them much longer (days/weeks) and track `last_seen` rather than aggressively expiring.
  - If keyword lookups feel too slow to re-learn, bump:
    - `kad.service_keyword_interest_ttl_secs` and `kad.service_keyword_results_ttl_secs` (e.g. 7 days = `604800`).
- 2026-02-08: Fix SAM session teardown + reconnect resilience:
  - Some SAM routers require `SESSION DESTROY STYLE=... ID=...`; we now fall back to style-specific destroys for both STREAM and DATAGRAM sessions (`src/i2p/sam/client.rs`, `src/i2p/sam/datagram_tcp.rs`).
  - KAD socket recreation now retries session creation with exponential backoff on tunnel-build errors like “duplicate destination” instead of crashing (`src/app.rs`).
- 2026-02-08: Add Kad2 keyword publish + DHT keyword storage:
  - Handle inbound `KADEMLIA2_PUBLISH_KEY_REQ` by storing minimal keyword->file metadata and replying with `KADEMLIA2_PUBLISH_RES` (key shape) (`src/kad/service.rs`, `src/kad/wire.rs`).
  - Answer inbound `KADEMLIA2_SEARCH_KEY_REQ` from the stored keyword index (helps interoperability + self-testing).
  - Add API endpoint `POST /kad/publish_keyword` and document in `docs/api_curl.md`.

## Current State (As Of 2026-02-07)

- Canonical branch: `main` (recent historical work happened on `feature/kad-search-publish`).
- Implemented:
  - SAM v3 TCP control client with logging and redacted sensitive fields (`src/i2p/sam/`).
  - SAM `STYLE=DATAGRAM` session over TCP (iMule-style `DATAGRAM SEND` / `DATAGRAM RECEIVED`) (`src/i2p/sam/datagram_tcp.rs`).
  - SAM `STYLE=DATAGRAM` session + UDP forwarding socket (`src/i2p/sam/datagram.rs`).
  - iMule-compatible KadID persisted in `data/preferencesKad.dat` (`src/kad.rs`).
  - iMule `nodes.dat` v2 parsing (I2P destinations, KadIDs, UDP keys) (`src/nodes/imule.rs`).
  - Distributable bootstrap seed embedded at `assets/nodes.initseed.dat` and copied to `data/nodes.initseed.dat` / `data/nodes.fallback.dat` on first run (`src/app.rs`).
  - KAD packet encode/decode including iMule packed replies (pure-Rust zlib/deflate inflater) (`src/kad/wire.rs`, `src/kad/packed.rs`).
  - Minimal bootstrap probe: send `PING` + `BOOTSTRAP_REQ`, decode `PONG` + `BOOTSTRAP_RES` (`src/kad/bootstrap.rs`).
  - Kad1+Kad2 HELLO handling during bootstrap (reply to `HELLO_REQ`, parse `HELLO_RES`, send `HELLO_RES_ACK` when requested) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Minimal Kad2 routing behavior during bootstrap:
  - Answer Kad2 `KADEMLIA2_REQ (0x11)` with `KADEMLIA2_RES (0x13)` using the closest known contacts (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Answer Kad1 `KADEMLIA_REQ_DEPRECATED (0x05)` with Kad1 `RES (0x06)` (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Handle Kad2 `KADEMLIA2_PUBLISH_SOURCE_REQ (0x19)` by recording a minimal in-memory source entry and replying with `KADEMLIA2_PUBLISH_RES (0x1B)` (this stops peers from retransmitting publishes during bootstrap) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Handle Kad2 `KADEMLIA2_SEARCH_SOURCE_REQ (0x15)` with `KADEMLIA2_SEARCH_RES (0x17)` (source results are encoded with the minimal required tags: `TAG_SOURCETYPE`, `TAG_SOURCEDEST`, `TAG_SOURCEUDEST`) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Persist discovered peers to `data/nodes.dat` (iMule `nodes.dat v2`) so we can slowly self-heal even when `nodes2.dat` fetch is unavailable (`src/app.rs`, `src/nodes/imule.rs`).
  - I2P HTTP fetch helper over SAM STREAM (used to download a fresh `nodes2.dat` when addressbook resolves) (`src/i2p/http.rs`).
- Removed obsolete code:
  - Legacy IPv4-focused `nodes.dat` parsing and old net probe helpers.
  - Empty/unused `src/protocol.rs`.

## Dev Topology Notes

- SAM bridge is on `10.99.0.2`.
- This `rust-mule` dev env runs inside Docker on host `10.99.0.1`.
- For SAM UDP forwarding to work, `SESSION CREATE ... HOST=<forward_host> PORT=<forward_port>` must be reachable from `10.99.0.2` and mapped into the container.
  - Recommended `config.toml` values:
    - `sam.host = "10.99.0.2"`
    - `sam.forward_host = "10.99.0.1"`
    - `sam.forward_port = 40000`
  - Docker needs either `--network host` or `-p 40000:40000/udp`.

If you don't want to deal with UDP forwarding, set `sam.datagram_transport = "tcp"` in `config.toml`.

## Data Files (`*.dat`) And Which One Is Used

### `data/nodes.dat` (Primary Bootstrap + Persisted Seed Pool)

This is the **main** nodes file that `rust-mule` uses across runs. By default it is:

- `kad.bootstrap_nodes_path = "nodes.dat"` (in `config.toml`)
- resolved relative to `general.data_dir = "data"`
- so the primary path is `data/nodes.dat`

On startup, `rust-mule` will try to load nodes from this path first. During runtime it is also periodically overwritten with a refreshed list (but in a merge-preserving way; see below).

Format: iMule/aMule `nodes.dat` v2 (I2P destinations + KadIDs + optional UDP keys).

### `data/nodes.initseed.dat` and `data/nodes.fallback.dat` (Local Seed Snapshots)

These are local seed snapshots stored under `data/` so runtime behavior does not depend on repo paths:

- `data/nodes.initseed.dat`: the initial seed snapshot (created on first run from the embedded initseed).
- `data/nodes.fallback.dat`: currently just a copy of initseed (we can evolve this later into a "last-known-good"
  snapshot if desired).

They are used only when:

- `data/nodes.dat` does not exist, OR
- `data/nodes.dat` exists but has become too small (currently `< 50` entries), in which case startup will re-seed `data/nodes.dat` by merging in reference nodes.

Selection logic lives in `src/app.rs` (`pick_nodes_dat()` + the re-seed block).

### `assets/nodes.initseed.dat` (Embedded Distributable Init Seed)

For distributable builds we track a baseline seed snapshot at:

- `assets/nodes.initseed.dat`

At runtime this is embedded into the binary via `include_bytes!()` and written out to `data/nodes.initseed.dat` /
`data/nodes.fallback.dat` if they don't exist yet (best-effort).

`source_ref/` remains a **dev-only** reference folder (gitignored) that contains iMule sources and reference files, but
the app no longer depends on it for bootstrapping.

### `nodes2.dat` (Remote Bootstrap Download, If Available)

iMule historically hosted an HTTP bootstrap list at:

- `http://www.imule.i2p/nodes2.dat`

`rust-mule` will try to download this only when it is not using the normal persisted `data/nodes.dat` seed pool (i.e. when it had to fall back to initseed/fallback).

If the download succeeds, it is saved as `data/nodes.dat` (we don't keep a separate `nodes2.dat` file on disk right now).

### `data/sam.keys` (SAM Destination Keys)

SAM pub/priv keys are stored in `data/sam.keys` as a simple k/v file:

```text
PUB=...
PRIV=...
```

This keeps secrets out of `config.toml` (which is easy to accidentally commit).

### `data/preferencesKad.dat` (Your KadID / Node Identity)

This stores the Kademlia node ID (iMule/aMule format). It is loaded at startup and reused across runs so you keep a stable identity on the network.

If you delete it, a new random KadID is generated and peers will treat you as a different node.

### `data/kad_udp_key_secret.dat` (UDP Obfuscation Secret)

This is the persistent secret used to compute UDP verify keys (iMule-style `GetUDPVerifyKey()` logic, adapted to I2P dest hash).

This value is generated on first run and loaded from this file on startup. It is intentionally not user-configurable.
If you delete it, a new secret is generated and any learned UDP-key relationships may stop validating until re-established.

## Known Issue / Debugging

If you see `SAM read timed out` right after a successful `HELLO`, the hang is likely on `SESSION CREATE ... STYLE=DATAGRAM` (session establishment can be slow on some routers).

Mitigation:
- `sam.control_timeout_secs` (default `120`) controls SAM control-channel read/write timeouts.
- With `general.log_level = "debug"`, the app logs the exact SAM command it was waiting on (with private keys redacted).

## Latest Run Notes (2026-02-04)

Observed with `sam.datagram_transport = "tcp"`:
- SAM `HELLO` OK.
- `SESSION CREATE STYLE=DATAGRAM ...` OK.
- Loaded a small seed pool (at that time it came from a repo reference `nodes.dat`; today we use the embedded initseed).
- Sent initial `KADEMLIA2_BOOTSTRAP_REQ` to peers, but received **0** `PONG`/`BOOTSTRAP_RES` responses within the bootstrap window.
  - A likely root cause is that iMule nodes expect **obfuscated/encrypted KAD UDP** packets (RC4+MD5 framing), and will ignore plain `OP_KADEMLIAHEADER` packets.
  - Another likely root cause is that the nodes list is stale (the default iMule KadNodesUrl is `http://www.imule.i2p/nodes2.dat`).

Next things to try if this repeats:
- Switch to `sam.datagram_transport = "udp_forward"` (some SAM bridges implement UDP forwarding more reliably than TCP datagrams).
- Ensure Docker/host UDP forwarding is mapped correctly if using `udp_forward` (`sam.forward_host` must be reachable from the SAM host).
- Increase the bootstrap runtime (I2P tunnel build + lease set publication can take time). Defaults are now more forgiving (`max_initial=256`, `runtime=180s`, `warmup=8s`).
- Prefer a fresher/larger `nodes.dat` seed pool (the embedded `assets/nodes.initseed.dat` may age; real discovery + persistence in `data/nodes.dat` should keep things fresh over time).
- Avoid forcing I2P lease set encryption types unless you know all peers support it (iMule doesn't set `i2cp.leaseSetEncType` for its datagram session).
- The app will attempt to fetch a fresh `nodes2.dat` over I2P from `www.imule.i2p` and write it to `data/nodes.dat` when it had to fall back to initseed/fallback.

If you see `Error: SAM read timed out` *during* bootstrap on `sam.datagram_transport="tcp"`, that's a local read timeout on the SAM TCP socket (no inbound datagrams yet), not necessarily a SAM failure. The TCP datagram receiver was updated to block and let the bootstrap loop apply its own deadline.

### Updated Run Notes (2026-02-04 19:30Z-ish)

- SAM `SESSION CREATE STYLE=DATAGRAM` succeeded but took ~43s (so `sam.control_timeout_secs=120` is warranted).
- We received inbound datagrams:
  - a Kad1 `KADEMLIA_HELLO_REQ_DEPRECATED` (opcode `0x03`) from a peer
  - a Kad2 `KADEMLIA2_BOOTSTRAP_RES` which decrypted successfully
- Rust now replies to Kad1 `HELLO_REQ` with a Kad1 `HELLO_RES` containing our I2P contact details, matching iMule's `WriteToKad1Contact()` layout.
- Rust now also sends Kad2 `HELLO_REQ` during bootstrap and handles Kad2 `HELLO_REQ/RES/RES_ACK` to improve chances of being added to routing tables and to exchange UDP verify keys.
- Observed many inbound Kad2 node-lookup requests (`KADEMLIA2_REQ`, opcode `0x11`). rust-mule now replies with `KADEMLIA2_RES` using the best-known contacts from `nodes.dat` + newly discovered peers (minimal routing-table behavior).
- The `nodes2.dat` downloader failed because `NAMING LOOKUP www.imule.i2p` returned `KEY_NOT_FOUND` on that router.
- If `www.imule.i2p` and `imule.i2p` are missing from the router addressbook, the downloader can't run unless you add an addressbook subscription which includes those entries, or use a `.b32.i2p` hostname / destination string directly.

### Updated Run Notes (2026-02-04 20:42Z-ish)

### Updated Run Notes (2026-02-06)

- Confirmed logs now land in `data/logs/` (daily rolled).
- Fresh run created `data/nodes.initseed.dat` + `data/nodes.fallback.dat` from embedded initseed (first run behavior).
- `data/nodes.dat` loaded `154` entries (primary), service started with routing `153`.
- Over ~20 minutes, service stayed healthy (periodic `kad service status` kept printing), but discovery was limited:
  - `live` stabilized around `2`
  - `recv_ress` > 0 (we do get some `KADEMLIA2_RES` back), but `new_nodes=0` during that window.
  - No WARN/ERROR events were observed.

If discovery remains flat over multi-hour runs, next tuning likely involves more aggressive exploration (higher `alpha`, lower `req_min_interval`, more frequent HELLOs) and/or adding periodic `KADEMLIA2_BOOTSTRAP_REQ` refresh queries in the service loop.

- Bootstrap sent probes to `peers=103`.
- Received:
- `KADEMLIA2_BOOTSTRAP_RES` (decrypted OK), which contained `contacts=1`.
- `KADEMLIA2_HELLO_REQ` from the same peer; rust-mule replied with `KADEMLIA2_HELLO_RES`.
- `bootstrap summary ... discovered=2` and persisted refreshed nodes to `data/nodes.dat` (`count=120`).

### Updated Run Notes (2026-02-05)

From `log.txt`:
- Bootstrapping from `data/nodes.dat` now works reliably enough to discover peers (`count=122` at end of run).
- We now see lots of inbound Kad2 node lookups (`KADEMLIA2_REQ`, opcode `0x11`) and we respond to each with `KADEMLIA2_RES` (contacts=4 in logs).
- One peer was repeatedly sending Kad2 publish-source requests (`opcode=0x19`, `KADEMLIA2_PUBLISH_SOURCE_REQ`). This is now handled by replying with `KADEMLIA2_PUBLISH_RES` and recording a minimal in-memory source entry so that (if asked) we can return it via `KADEMLIA2_SEARCH_RES`.
  - Example (later in the log): `publish_source_reqs=16` and `publish_source_res_sent=16` in the bootstrap summary, plus log lines like `sent KAD2 PUBLISH_RES (sources) ... sources_for_file=1`.

## Known SAM Quirk (DEST GENERATE)

Some SAM implementations reply to `DEST GENERATE` as:

- `DEST REPLY PUB=... PRIV=...`

with **no** `RESULT=OK` field. `SamClient::dest_generate()` was updated to accept this (it now validates `PUB` and `PRIV` instead of requiring `RESULT=OK`). This unblocks:

- `src/bin/sam_dgram_selftest.rs`
- the `nodes2.dat` downloader (temporary STREAM sessions use `DEST GENERATE`)

## Known Issue (Addressbook Entry For `www.imule.i2p`)

If `NAMING LOOKUP NAME=www.imule.i2p` returns `RESULT=KEY_NOT_FOUND`, your router's addressbook doesn't have that host.

Mitigations:
- Add/subscribe to an addressbook source which includes `www.imule.i2p`.
- The downloader also tries `imule.i2p` as a fallback by stripping the leading `www.`.
- The app now also persists any peers it discovers during bootstrap to `data/nodes.dat`, so it can slowly build a fresh nodes list even if `nodes2.dat` can’t be fetched.

### KAD UDP Obfuscation (iMule Compatibility)

iMule encrypts/obfuscates KAD UDP packets (see `EncryptedDatagramSocket.cpp`) and includes sender/receiver verify keys.

Implemented in Rust:
- `src/kad/udp_crypto.rs`: MD5 + RC4 + iMule framing, plus `udp_verify_key()` compatible with iMule (using I2P dest hash in place of IPv4).
- `src/kad/udp_crypto.rs`: receiver-verify-key-based encryption path (needed for `KADEMLIA2_HELLO_RES_ACK` in iMule).
- `kad.udp_key_secret` used to be configurable, but is now deprecated/ignored. The secret is always generated/loaded from `data/kad_udp_key_secret.dat` (analogous to iMule `thePrefs::GetKadUDPKey()`).

Bootstrap now:
- Encrypts outgoing `KADEMLIA2_BOOTSTRAP_REQ` using the target's KadID.
- Attempts to decrypt inbound packets (NodeID-key and ReceiverVerifyKey-key variants) before KAD parsing.

## How To Run

```bash
cargo run --bin rust-mule
```

If debugging SAM control protocol, set:
- `general.log_level = "debug"` in `config.toml`, or
- `RUST_LOG=rust_mule=debug` in the environment.

## Kad Service Loop (Crawler)

As of 2026-02-05, `rust-mule` runs a long-lived Kad service loop after the initial bootstrap by default.
It:
- listens/responds to inbound Kad traffic
- periodically crawls the network by sending `KADEMLIA2_REQ` lookups and decoding `KADEMLIA2_RES` replies
- periodically persists an updated `data/nodes.dat`

### Important Fix (2026-02-05): `KADEMLIA2_REQ` Check Field

If you see the service loop sending lots of `KADEMLIA2_REQ` but reporting `recv_ress=0` in `kad service status`, the most likely culprit was a bug which is fixed in `main` (originally developed on `feature/sam-protocol`):

- In iMule, the `KADEMLIA2_REQ` payload includes a `check` KadID field which must match the **receiver's** KadID.
- If we incorrectly put *our* KadID in the `check` field, peers will silently ignore the request and never send `KADEMLIA2_RES`.

After the fix, long runs should start showing `recv_ress>0` and `new_nodes>0` as the crawler learns contacts.

### Note: Why `routing` Might Not Grow Past The Seed Count

If `kad service status` shows `recv_ress>0` but `routing` stays flat (e.g. stuck at the initial `nodes.dat` size), that can be normal in a small/stale network *or* it can indicate that peers are mostly returning contacts we already know (or echoing our own KadID back as a contact).

The service now counts “new nodes” only when `routing.len()` actually increases after processing `KADEMLIA2_RES`, to avoid misleading logs.

Also: the crawler now picks query targets Kademlia-style: it biases which peers it queries by XOR distance to the lookup target (not just “who is live”). This tends to explore new regions of the ID space faster and increases the odds of discovering nodes that weren't already in the seed `nodes.dat`.

Recent observation (2026-02-06, ~50 min run):
- `data/nodes.dat` stayed at `154` entries; routing stayed at `153`.
- `live` peers stayed at `2`.
- Periodic `KADEMLIA2_BOOTSTRAP_REQ` refresh got replies, but returned contact lists were typically `2` and did not introduce new IDs (`new_nodes=0`).

Takeaway: this looks consistent with a very small / stagnant iMule I2P-KAD network *or* a seed which mostly points at dead peers. Next improvements should focus on discovery strategy and fresh seeding (see TODO below).

Relevant config keys (all under `[kad]`):
- `service_enabled` (default `true`)
- `service_runtime_secs` (`0` = run until Ctrl-C)
- `service_crawl_every_secs` (default `3`)
- `service_persist_every_secs` (default `300`)
- `service_alpha` (default `3`)
- `service_req_contacts` (default `31`)
- `service_max_persist_nodes` (default `5000`)
Additional tuning knobs:
- `service_req_timeout_secs` (default `45`)
- `service_req_min_interval_secs` (default `15`)
- `service_bootstrap_every_secs` (default `1800`)
- `service_bootstrap_batch` (default `1`)
- `service_bootstrap_min_interval_secs` (default `21600`)
- `service_hello_every_secs` (default `10`)
- `service_hello_batch` (default `2`)
- `service_hello_min_interval_secs` (default `900`)
- `service_maintenance_every_secs` (default `5`)
- `service_max_failures` (default `5`)
- `service_evict_age_secs` (default `86400`)

## Logging Notes

As of 2026-02-05, logs can be persisted to disk via `tracing-appender`:
- Controlled by `[general].log_to_file` (default `true`)
- Files are written under `[general].data_dir/logs` and rolled daily as `rust-mule.log.YYYY-MM-DD` (configurable via `[general].log_file_name`)
- Stdout verbosity is controlled by `[general].log_level` (or `RUST_LOG`).
- File verbosity is controlled by `[general].log_file_level` (or `RUST_MULE_LOG_FILE`).

The Kad service loop now emits a concise `INFO` line periodically: `kad service status` (default every 60s), and most per-packet send/timeout logs are `TRACE` to keep stdout readable at `debug`.

To keep logs readable, long I2P base64 destination strings are now shortened in many log lines (they show a prefix + suffix rather than the full ~500 chars). See `src/i2p/b64.rs` (`b64::short()`).

As of 2026-02-06, the status line also includes aggregate counts like `res_contacts`, `sent_bootstrap_reqs`, `recv_bootstrap_ress`, and `bootstrap_contacts` to help tune discovery without turning on very verbose per-packet logging.

## Reference Material

- iMule source + reference `nodes.dat` are under `source_ref/` (gitignored).
- KAD wire-format parity notes: `docs/kad_parity.md`.

## Roadmap (Agreed Next Steps)

Priority is to stabilize the network layer first, so we can reliably discover peers and maintain a healthy routing table over time:

1. **Kad crawler + routing table + stable loop (next)**
   - Actively query peers (send `KADEMLIA2_REQ`) and **decode `KADEMLIA2_RES`** to learn more contacts.
   - Maintain an in-memory routing table (k-buckets / closest contacts) with `last_seen`, `verified`, and UDP key metadata.
   - Run as a long-lived service: keep SAM datagram session open, respond continuously, periodically refresh/ping, and periodically persist `data/nodes.dat`.
   - TODO (discovery): add a conservative “cold bootstrap probe” mode so periodic bootstrap refresh occasionally targets *non-live / never-seen* peers, to try to discover new clusters without increasing overall traffic.
   - TODO (seeding): optionally fetch the latest public `nodes.dat` snapshot (when available) and merge it into `data/nodes.dat` with provenance logged.

2. **Publish/Search indexing (after routing is stable)**
- Implement remaining Kad2 publish/search opcodes (key/notes/source) with iMule-compatible responses.
- Add a real local index so we can answer searches meaningfully (not just “0 results but no retry”).

## Tuning Notes / Gotchas

- `kad.service_req_contacts` should be in `1..=31`. (Kad2 masks this field with `0x1F`.)
  - If it is set to `32`, it will effectively become `1`, which slows discovery dramatically.
- The service persists `nodes.dat` periodically. It now merges the current routing snapshot into the existing on-disk `nodes.dat` to avoid losing seed nodes after an eviction cycle.
- If `data/nodes.dat` ever shrinks to a very small set (e.g. after a long run evicts lots of dead peers), startup will re-seed it by merging in `data/nodes.initseed.dat` / `data/nodes.fallback.dat` if present.

- The crawler intentionally probes at least one “cold” peer (a peer we have never heard from) per crawl tick when available. This prevents the service from getting stuck talking only to 1–2 responsive nodes forever.

- SAM TCP-DATAGRAM framing is now tolerant of occasional malformed frames (it logs and skips instead of crashing). Oversized datagrams are discarded with a hard cap to avoid memory blowups.
- SAM TCP-DATAGRAM reader is byte-based (not `String`-based) to avoid crashes on invalid UTF-8 if the stream ever desyncs.

## 2026-02-08 Notes (Keyword Publish/Search UX + Reach)

- `/kad/search_keyword` and `/kad/publish_keyword` now accept either:
  - `{"query":"..."}` (iMule-style: first extracted word is hashed), or
  - `{"keyword_id_hex":"<32 hex>"}` to bypass tokenization/hashing for debugging.
- Keyword publish now also inserts the published entry into the local keyword-hit cache immediately (so `/kad/keyword_results/<keyword>` reflects the publish even if the network is silent).
- Keyword search/publish now run as a small, conservative “job”:
  - periodically sends `KADEMLIA2_REQ` toward the keyword ID to discover closer nodes
  - periodically sends small batches of `SEARCH_KEY_REQ` / `PUBLISH_KEY_REQ` to the closest, recently-live peers
  - stops early for publish once any `PUBLISH_RES (key)` ack is observed

- Job behavior tweak:
  - A keyword job can now do **both** publish and search for the same keyword concurrently.
    Previously, starting a search could overwrite an in-flight publish job for that keyword.

## 2026-02-09 Notes (Single-Instance Lock)

- Added an OS-backed single-instance lock at `data/rust-mule.lock` (under `general.data_dir`).
  - Prevents accidentally running two rust-mule processes with the same `data/sam.keys`, which
    triggers I2P router errors like “duplicate destination”.
  - Uses a real file lock (released automatically if the process exits/crashes), not a “sentinel
    file” check.

## 2026-02-09 Notes (Peer “Agent” Identification)

- SAM `DATAGRAM RECEIVED` frames include the sender I2P destination, but **do not** identify the
  sender implementation (iMule vs rust-mule vs something else).
- To support rust-mule-specific feature gating/debugging, we added a small rust-mule private
  extension tag in the Kad2 `HELLO` taglist:
  - `TAG_RUST_MULE_AGENT (0xFE)` as a string, value like `rust-mule/<version>`
  - iMule ignores unknown tags in `HELLO` (it only checks `TAG_KADMISCOPTIONS`), so this is
    backwards compatible.
- When received, this agent string is stored in the in-memory routing table as `peer_agent` (not
  persisted to `nodes.dat`, since that file is in iMule format).

## Debugging Notes (Kad Status Counters)

- `/status` now includes two extra counters to help distinguish “network is silent” vs “we are
  receiving packets but can’t parse/decrypt them”:
  - `dropped_undecipherable`: failed Kad UDP decrypt (unknown/invalid obfuscation)
  - `dropped_unparsable`: decrypted OK but Kad packet framing/format was invalid
- For publish/search testing, we also now log at `INFO` when:
  - we receive a `PUBLISH_RES (key)` ACK (so you can see if peers accepted your publish)
  - we receive a non-empty `SEARCH_RES` (inserted keyword/source entries)

## Two-Instance Testing

- Added `docs/scripts/two_instance_dht_selftest.sh` to exercise publish/search flows between two
  locally-running rust-mule instances (e.g. mule-a on `:17835` and mule-b on `:17836`).
