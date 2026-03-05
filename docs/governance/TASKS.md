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
- add debug lookup traceability backlog:
  - implement `POST /api/v1/debug/trace_lookup` as debug-only endpoint
  - use async execution (`202 Accepted` + `trace_id`) with poll endpoint
  - return bounded hop-by-hop lookup trace for a target KAD key
  - enforce strict input/runtime bounds (`max_hops`, `parallelism`, `timeout_ms`)
  - bound active traces + trace TTL; optionally support cancellation
  - add dedicated rate limiting and counters for trace requests
  - reference design: `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- KAD/wire refactor prerequisites documented and baselined before scheduling code-heavy changes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
