## Summary

- what changed
- why
- risk/compatibility notes

## Validation

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`

## KAD/Wire Gate (Only if touching `src/kad/**` or wire behavior)

- [ ] Baseline capture included (before + after) using `scripts/test/kad_phase0_baseline.sh`
- [ ] Counter deltas summarized:
  - `pending_overdue`, `pending_max_overdue_ms`
  - `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
- [ ] Any significant drift explained and aligned with:
  - `docs/BEHAVIOURAL_CONTRACT.md`
  - `docs/IMULE_COMPABILITY_TIMING.md`
  - `docs/REVIEWERS_CHECKLIST.md`

## Follow-ups

- remaining TODOs (if any)
