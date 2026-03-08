Status: ACTIVE
Last Reviewed: 2026-03-03

# Task Plan

## Current Priority

1. Download subsystem phase 2: block scheduler + transfer execution hardening (`OP_REQUESTPARTS` / `OP_SENDINGPART` reliability).
2. KAD organic reliability pass (search/publish under real peer variance) and complete phase 0 baseline from `docs/KAD_WIRE_REFACTOR_PLAN.md`.
3. UI statistics follow-up (dedicated statistics page + richer chart controls).
4. Defer full KAD/wire timing refactor until soak baseline remains stable; then execute phased plan (`docs/KAD_WIRE_REFACTOR_PLAN.md`) slice-by-slice.
5. Apply `docs/RUST-MULE_ROUTING_PHILOSOPHY.md` as implementation backlog:
   - add peer reliability classes and health-driven routing/eviction
   - add transport-aware latency evaluation and local path-memory prioritization
   - expose counters required to verify these policies in long-run baselines

## Scope (Current Iteration)

- continue download phase 2 transfer work on top of merged phase 0/1 lifecycle + `known.met`
- complete download phase-0 acceptance runbook execution and artifact capture (`scripts/test/download_phase0_acceptance.sh`)
- next download slice: `known.met` compatibility depth + restart/resume robustness assertions
- next user-value slice: hash-first discovery/initiation path (direct MD4/file-hash driven flow)
- keep KAD reliability tracking and UI/API contract checks updated as fields evolve
- keep behavior-contract documentation authoritative for all network/protocol changes
- phase 0 baseline instrumentation is in place; gather before/after baseline artifacts for upcoming KAD shaper work
- add repo-wide naming/comment refactor task:
  - replace `Imule*` identifiers with neutral `Mule*`/protocol-neutral naming
  - normalize code comments to compatibility wording (avoid explicit iMule/aMule/eMule wording in code comments)
- convert routing philosophy into concrete, measurable milestones:
  - peer class transitions and reliability scoring with tests
  - bucket health model and eviction rationale metrics
  - transport-context latency thresholds and regression baselines
- add build/release script hardening backlog:
  - switch release scripts to explicit target triples instead of host-only `target/release` artifacts
  - define first-class targets: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`
  - document target-specific prerequisites and native-runner requirements for CI release jobs
- add CLI/runtime startup ergonomics backlog:
  - support `rust-mule --config <path>` to load config from an explicit location
  - fail fast with clear error when `--config` path does not exist or is unreadable
  - support `rust-mule --help` and `rust-mule -?` for parameter/usage output
  - keep default behavior (`config.toml` in CWD) when `--config` is omitted
  - support `rust-mule --version` for support/debug reporting
  - support `rust-mule --check-config` to validate config and exit
  - support `rust-mule --print-effective-config` for troubleshooting resolved runtime config
- add timezone configuration + UI control backlog:
  - add config key for timezone (IANA zone, e.g. `Europe/Stockholm`) with validation + fallback behavior
  - expose timezone under Settings UI/API so it can be changed without manual file edits
  - apply configured timezone to application log timestamps (instead of UTC-only output)
  - document runtime behavior when timezone is invalid or unavailable
- add debug lookup traceability backlog:
  - implement `POST /api/v1/debug/trace_lookup` as debug-only endpoint
  - use async execution (`202 Accepted` + `trace_id`) with poll endpoint
  - return bounded hop-by-hop lookup trace for a target KAD key
  - enforce strict input/runtime bounds (`max_hops`, `parallelism`, `timeout_ms`)
  - bound active traces + trace TTL; optionally support cancellation
  - require debug second-factor secret (`api.debug_token`, `X-Debug-Token`) in addition to normal auth
  - implement debug-disabled `404` and invalid/missing debug-token `403` behavior
  - enforce token lifecycle policy: no auto-delete on debug-disabled; explicit rotation only
  - add dedicated rate limiting and counters for trace requests
  - reference design: `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`
- add debug bootstrap restart backlog:
  - implement `POST /api/v1/debug/bootstrap/restart` as async (`202 + job_id`)
  - implement `GET /api/v1/debug/bootstrap/jobs/{job_id}` for job status
  - enforce single-flight + cooldown + bounded job TTL
  - require debug second-factor secret (`api.debug_token`, `X-Debug-Token`)
  - enforce token lifecycle policy: no auto-delete on debug-disabled; explicit rotation only
  - reference design: `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`
- add logging-surface cleanup backlog:
  - audit trace/routing logs and move non-essential internals behind debug-enabled gating
  - specifically gate verbose bucket/routing-table detail logs behind debug flag
  - keep default logs operator-focused (health/progress/errors) and avoid high-cardinality noisy output
- add acceptance/soak validation hardening backlog:
  - fail phase0 gate when key metrics resolve to `nan`/unexpected `SKIP` unless explicitly allowlisted
  - add lightweight script sanity mode in CI for soak scripts (env parsing, trap behavior, report/summary generation)
  - add pass-with-degradation runbook guidance for suspicious-but-zero-exit runs
- add soak artifact governance backlog:
  - define canonical artifact bundle per run (`summary.txt`, `resume_report.txt`, diagnostics JSON, optional stack tarball)
  - define retention period, naming rules, and archive cadence to prevent accidental data loss/sprawl
- add post-restart download diagnostics backlog:
  - expose explicit cancellation/queue transition reasons and timestamps for restart triage
  - include reason fields in diagnostics snapshot so completion timeouts are directly explainable
- add config evolution backlog:
  - introduce config schema versioning + migration notes for future keys (timezone/debug/CLI-related additions)
- add shared library + real upload serving backlog:
  - add configurable shared folders list in config + settings API/UI (multi-path support)
  - implement library scanner/indexer that hashes files and builds publishable source metadata
  - publish source records from indexed shared files (not only synthetic/manual publish calls)
  - track file path binding for published sources so inbound transfer requests map to real local file bytes
  - implement real uploader path for peer requests (`OP_REQUESTPARTS` -> `OP_SENDINGPART`) reading block ranges from disk
  - add safeguards for path traversal/symlink policy/permission failures in shared folders
  - reject unsafe share roots by policy (system root `/`, core OS dirs, app/runtime data dirs) with clear validation errors
  - normalize + canonicalize share paths before accept; prevent duplicate/overlapping entries by policy
  - expose scanner/index health + per-folder stats in settings/status UI for operator visibility
  - reference checklist: `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- KAD/wire refactor prerequisites documented and baselined before scheduling code-heavy changes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
