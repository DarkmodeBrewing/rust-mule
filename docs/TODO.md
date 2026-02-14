# TODO

Backlog by subsystem. Keep this aligned with `docs/TASKS.md` and `docs/handoff.md`.

## KAD

- [ ] Improve organic search/publish success rate (non-forced peers) with measurable baseline vs improved runs.
- [ ] Continue iMule wire/parity verification for discovery, routing, and source lifecycle edge cases.
- [ ] Add clearer timeout/retry outcome buckets for request -> response conversion diagnostics.

## API

- [ ] Consider tiered API command timeouts (shared baseline exists; tune by endpoint class if needed).
- [ ] Evaluate optional typed API error response envelope consistency for all non-2xx responses.

## UI

- [ ] Complete dedicated statistics page (currently charts live on `node_stats.html`).
- [ ] Expand chart controls/time windows and verify usability under long-running sessions.
- [ ] Continue WCAG/accessibility hardening and keyboard-only flow checks.

## SAM / Runtime

- [ ] Investigate exposing a custom SAM client label instead of generic `SAM UDP Client` in router views.
- [ ] Add memory-pressure instrumentation for routing/lookups/search caches.

## Documentation

- [x] Normalize core docs and README entrypoint.
- [ ] Keep `docs/ui_api_contract_map.md` in sync whenever UI/API fields change.

## CLI / Headless

- [ ] Evaluate stronger headless operational workflow docs (session/bootstrap/token rotation runbooks).
