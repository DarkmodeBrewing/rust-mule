# TODO

Backlog by subsystem. Keep this aligned with `docs/TASKS.md` and `docs/handoff.md`.

## General

- [ ] Slim logging more, take stuff like buckets behind debug, if needed
- [ ] Security scan of the code
- [ ] Wire security scan - harden, if needed, packet parsing to prevent overflows
- [ ] Rust - Bestpractise run
- [ ] Memory management
- [ ] Timezone selection - for logging
- [ ] Fuzzing
- [ ] Naming refactor pass: replace `Imule*` type/module/function identifiers with neutral `Mule*` naming (or neutral equivalents) across codebase.
- [ ] Comment text normalization: avoid explicit iMule/aMule/eMule wording in code comments; use compatibility-focused wording instead (keep protocol-compat details in docs/tests as needed).

## KAD

- [ ] Improve organic search/publish success rate (non-forced peers) with measurable baseline vs improved runs.
- [ ] Continue iMule wire/parity verification for discovery, routing, and source lifecycle edge cases.
- [ ] Add clearer timeout/retry outcome buckets for request -> response conversion diagnostics.
- [ ] Execute `docs/KAD_WIRE_REFACTOR_PLAN.md` phase 0 (baseline + timing/ordering guardrails).
- [ ] Implement centralized outbound KAD shaper (delay + jitter + randomized ordering + hard caps).
- [ ] Remove KAD outbound bypass/fast paths so all traffic uses the shaper.
- [ ] Align timeout/retry scheduling to iMule-compatible envelopes with jitter (no exact periodicity).
- [ ] Re-run soak baselines after refactor slices and record before/after deltas.

## Downloads

- [ ] Implement download subsystem scaffold (`src/download/*`) with typed errors and actor-style command loop.
- [ ] Implement `.part` / `.part.met` persistence and startup recovery from `data/download/`.
- [ ] Implement block scheduler and transfer pipeline (`OP_REQUESTPARTS`, `OP_SENDINGPART`, compressed blocks).
- [ ] Implement completion flow into `data/incoming/` with known file persistence (`known.met`).
- [ ] Phase in AICH hashset support (`known2_64.met`) after MD4-first baseline is stable.

## API

- [ ] Consider tiered API command timeouts (shared baseline exists; tune by endpoint class if needed).
- [ ] Evaluate optional typed API error response envelope consistency for all non-2xx responses.

## UI

- [ ] Complete dedicated statistics page (currently charts live on `node_stats.html`).
- [ ] Expand chart controls/time windows and verify usability under long-running sessions.
- [ ] Continue WCAG/accessibility hardening and keyboard-only flow checks.
- [ ] Import old part files (api/ui/service)

## SAM / Runtime

- [ ] Investigate exposing a custom SAM client label instead of generic `SAM UDP Client` in router views.
- [ ] Add memory-pressure instrumentation for routing/lookups/search caches.

## Documentation

- [x] Normalize core docs and README entrypoint.
- [x] Add behavior contract, reviewer checklist, and compatibility timing policy docs.
- [x] Add a phased KAD/wire refactor plan document.
- [ ] Keep `docs/ui_api_contract_map.md` in sync whenever UI/API fields change.

## CLI / Headless

- [ ] Evaluate stronger headless operational workflow docs (session/bootstrap/token rotation runbooks).
