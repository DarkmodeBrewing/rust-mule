# Task Plan

## Current Priority

1. Download subsystem phase 0/1: scaffold + `.part`/`.part.met` lifecycle in `data/download` and finalize into `data/incoming`.
2. KAD organic reliability pass (search/publish under real peer variance).
3. UI statistics follow-up (dedicated statistics page + richer chart controls).

## Scope (Current Iteration)

- finalize iMule-compatible download architecture and execution plan
- implement typed download errors and persistence primitives first
- keep KAD reliability tracking and UI/API contract checks updated as fields evolve

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
