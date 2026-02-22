# KAD/Wire Behavioral Alignment Plan

This plan captures the required refactor work to align KAD/wire behavior with:

- `docs/BEHAVIOURAL_CONTRACT.md`
- `docs/REVIEWERS_CHECKLIST.md`
- `docs/IMULE_COMPABILITY_TIMING.md`

Policy:

- Behavior contract is authoritative.
- iMule compatibility is maintained at protocol and timing-envelope level.
- Refactor is deferred until current soak stabilization remains green.

## Phase 0: Baseline and Guardrails (Document/Measure First)

- [x] Capture baseline soak metrics for current KAD/search/publish behavior.
  - Latest confirmed 6h runs on `main` (February 22, 2026):
    - `samples=4197`, `restarts=0`
    - `restart_markers=0`
    - `sam_framing_desync_total_max=0`
    - productive traffic totals remained positive for full run window (`sent_reqs_total`, `recv_ress_total`, `timeouts_total` all increased over run)
  - Acceptance gate for future KAD changes:
    - `restart_markers == 0`
    - `sam_framing_desync_total_max == 0`
    - `sent_reqs_total` and `recv_ress_total` increase over run
- [x] Define observable timing/ordering counters to compare before/after refactor.
  - Implemented status counters:
    - `pending_overdue`, `pending_max_overdue_ms`
    - `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
  - Baseline capture script:
    - `scripts/test/kad_phase0_baseline.sh`
- [x] Add PR template/checklist reference to reviewer gates for KAD/wire changes.
  - Added `.github/pull_request_template.md` KAD/wire gate section with required baseline evidence.
  - Added reviewer checklist baseline evidence gate (`docs/REVIEWERS_CHECKLIST.md`).

## Phase 1: Central Outbound Shaper

- [ ] Introduce a single outbound scheduling layer for KAD traffic.
- [ ] Enforce base delay + jitter on all outbound messages.
- [ ] Enforce randomized dequeue/ordering (no deterministic send order).
- [ ] Enforce global and per-peer hard caps independent of host performance.

## Phase 2: Remove Bypass Paths

- [ ] Route all KAD responses through the shaper (no immediate sends).
- [ ] Route maintenance/refresh traffic through the shaper.
- [ ] Route error/failure traffic through the same scheduler path.
- [ ] Remove or gate any message-type fast paths.

## Phase 3: Retry/Timeout Envelope Alignment

- [ ] Match iMule-compatible timeout ranges with randomized per-transaction jitter.
- [ ] Preserve retry shape (including backoff family), not exact periodic spacing.
- [ ] Ensure no retry occurs on exact deterministic cadence.

## Phase 4: Failure Uniformity and Fingerprint Hardening

- [ ] Normalize observable failure behavior (delay + generic drop/failure classes).
- [ ] Verify unknown/malformed messages do not emit distinct external timing patterns.
- [ ] Audit logging/API exposure so internals do not leak on-wire behavior differences.

## Phase 5: Validation and Rollout

- [ ] Re-run A/B soak comparisons (before/after) for search/publish success and timing drift.
- [ ] Confirm no regression in interoperability envelope with iMule-like peers.
- [ ] Update `docs/handoff.md`, `docs/TODO.md`, and `docs/TASKS.md` with results.
