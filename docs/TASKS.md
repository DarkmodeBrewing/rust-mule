# Task Plan

## Current Priority

1. Complete current KAD long-run baseline validation and merge active tuning branch.
2. Immediately after merge: KAD hostile-input hardening pass:
   - clamp untrusted wire `count` fields before decoder allocations
   - add cap/eviction for inbound per-source limiter state (`tracked_in_requests`)
   - switch shaper jitter from deterministic LCG to OS-seeded non-crypto RNG
   - add adversarial parser tests and fuzz targets (`kad/wire`, `kad/packed`)
3. Download subsystem phase 0/1: scaffold + `.part`/`.part.met` lifecycle in `data/download` and finalize into `data/incoming`.
4. KAD organic reliability pass (search/publish under real peer variance) and complete phase 0 baseline from `docs/KAD_WIRE_REFACTOR_PLAN.md`.
5. UI statistics follow-up (dedicated statistics page + richer chart controls).
6. Defer full KAD/wire timing refactor until soak baseline remains stable; then execute phased plan (`docs/KAD_WIRE_REFACTOR_PLAN.md`) slice-by-slice.
7. Apply `docs/RUST-MULE_ROUTING_PHILOSOPHY.md` as implementation backlog:
   - add peer reliability classes and health-driven routing/eviction
   - add transport-aware latency evaluation and local path-memory prioritization
   - expose counters required to verify these policies in long-run baselines

## Scope (Current Iteration)

- finalize iMule-compatible download architecture and execution plan
- implement typed download errors and persistence primitives first
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

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- KAD/wire refactor prerequisites documented and baselined before scheduling code-heavy changes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
