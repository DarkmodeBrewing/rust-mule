# Task Plan

## Current Priority

1. Download subsystem phase 0/1: scaffold + `.part`/`.part.met` lifecycle in `data/download` and finalize into `data/incoming`.
2. KAD organic reliability pass (search/publish under real peer variance) and complete phase 0 baseline from `docs/KAD_WIRE_REFACTOR_PLAN.md`.
3. UI statistics follow-up (dedicated statistics page + richer chart controls).
4. Defer full KAD/wire timing refactor until soak baseline remains stable; then execute phased plan (`docs/KAD_WIRE_REFACTOR_PLAN.md`) slice-by-slice.

## Scope (Current Iteration)

- finalize iMule-compatible download architecture and execution plan
- implement typed download errors and persistence primitives first
- keep KAD reliability tracking and UI/API contract checks updated as fields evolve
- keep behavior-contract documentation authoritative for all network/protocol changes
- add repo-wide naming/comment refactor task:
  - replace `Imule*` identifiers with neutral `Mule*`/protocol-neutral naming
  - normalize code comments to compatibility wording (avoid explicit iMule/aMule/eMule wording in code comments)

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- KAD/wire refactor prerequisites documented and baselined before scheduling code-heavy changes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
