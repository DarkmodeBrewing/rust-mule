# Task Plan

## Current Priority

1. KAD organic reliability pass (search/publish under real peer variance).
2. UI statistics follow-up (dedicated statistics page + richer chart controls).
3. Operational hardening (memory-pressure visibility + headless runbook polish).

## Scope (Current Iteration)

- quantify non-forced success/failure over soak runs
- identify dominant timeout/drop buckets and close highest-impact gaps
- keep UI/API contract checks updated as fields evolve

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- clear status/log counters for timeout/retry/drop classes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
