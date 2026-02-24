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
- [ ] Ensure CI/CD build + release flow is fully tag-driven (triggered from Git tags with artifact publish and verification).
- [ ] Naming refactor pass: replace `Imule*` type/module/function identifiers with neutral `Mule*` naming (or neutral equivalents) across codebase.
- [ ] Comment text normalization: avoid explicit iMule/aMule/eMule wording in code comments; use compatibility-focused wording instead (keep protocol-compat details in docs/tests as needed).

## KAD

- [x] Post-current longrun + merge: harden KAD wire decoders against allocation amplification by clamping untrusted `count` fields before `Vec::with_capacity(...)`.
- [x] Post-current longrun + merge: cap `tracked_in_requests` growth with explicit max entries + eviction policy (hostile source churn protection).
- [x] Post-current longrun + merge: replace deterministic LCG shaper jitter with OS-seeded non-crypto RNG jitter.
- [ ] Post-current longrun + merge: split `src/kad/service.rs` into smaller modules (`outbound`, `tracking`, `maintenance`) to reduce complexity risk.
- [x] Post-current longrun + merge: add hostile-input parser tests (oversized counts/truncated payloads) and fuzz targets for `kad/wire` + `kad/packed`.
- [ ] Improve organic search/publish success rate (non-forced peers) with measurable baseline vs improved runs.
- [ ] Continue iMule wire/parity verification for discovery, routing, and source lifecycle edge cases.
- [ ] Add clearer timeout/retry outcome buckets for request -> response conversion diagnostics.
- [ ] Implement explicit peer reliability classification (`unknown`, `verified`, `stable`, `unreliable`) and use it in routing/publish candidate selection.
- [ ] Add transport-aware latency baselines and scoring so I2P latency variance is evaluated contextually, not globally.
- [ ] Add bucket health scoring (responsiveness + timeout ratio + density quality) and make refresh/eviction decisions health-driven.
- [ ] Add local path memory for successful search/publish paths and use it as a soft priority signal (local/ephemeral only).
- [ ] Add KAD1/KAD2 noise/drop counters in `/api/v1/status` (legacy drops, malformed drops, framing-desync correlation) for long-run analysis.
- [ ] Execute `docs/KAD_WIRE_REFACTOR_PLAN.md` phase 0 (baseline + timing/ordering guardrails).
- [x] Define and expose Phase 0 timing/ordering baseline counters in `/api/v1/status`.
- [x] Add KAD Phase 0 baseline capture script (`scripts/test/kad_phase0_baseline.sh`).
- [x] Add KAD/wire PR reviewer evidence gate (template + reviewer checklist update).
- [ ] Implement centralized outbound KAD shaper (delay + jitter + randomized ordering + hard caps).
- [ ] Remove KAD outbound bypass/fast paths so all traffic uses the shaper.
- [ ] Align timeout/retry scheduling to iMule-compatible envelopes with jitter (no exact periodicity).
- [ ] Re-run soak baselines after refactor slices and record before/after deltas.

## Downloads

- [ ] Post-current longrun + merge: do not mark `OP_COMPRESSEDPART` blocks received until payload is decompressed, size-validated, and persisted successfully.
- [ ] Post-current longrun + merge: enforce explicit inbound payload/block caps in `download::protocol` (`MAX_PART_PAYLOAD`, `MAX_COMPRESSED_PAYLOAD`, `MAX_BLOCK_LEN`).
- [ ] Post-current longrun + merge: cap `reserve_blocks` fan-out per call (`MAX_RESERVE_BLOCKS_PER_CALL`) to bound worst-case CPU work.
- [ ] Post-current longrun + merge: remove production-path `unwrap()` in `download::protocol` decoders and return typed `ProtocolError` instead.
- [ ] Post-current longrun + merge: add hostile-input tests for download protocol decode/ingest (oversized payloads, malformed compressed parts, semantic mismatch cases).
- [ ] Implement download subsystem scaffold (`src/download/*`) with typed errors and actor-style command loop.
- [ ] Implement `.part` / `.part.met` persistence and startup recovery from `data/download/`.
- [ ] Implement block scheduler and transfer pipeline (`OP_REQUESTPARTS`, `OP_SENDINGPART`, compressed blocks).
- [ ] Implement completion flow into `data/incoming/` with known file persistence (`known.met`).
- [ ] Phase in AICH hashset support (`known2_64.met`) after MD4-first baseline is stable.

## API

- [ ] Post-current longrun + merge: add explicit request body size limits for JSON endpoints (global + per-route overrides where needed).
- [ ] Post-current longrun + merge: extend API rate limiting beyond auth bootstrap/session/token rotate to high-frequency read/mutation routes.
- [ ] Post-current longrun + merge: make `load_or_create_token` self-heal invalid/corrupt token files (rotate + replace with warning).
- [ ] Post-current longrun + merge: emit warning/metric when SSE status serialization falls back to `{}`.
- [ ] Post-current longrun + merge: standardize typed API error envelope for non-2xx responses (code + human message).
- [ ] Consider tiered API command timeouts (shared baseline exists; tune by endpoint class if needed).
- [ ] Evaluate optional typed API error response envelope consistency for all non-2xx responses.
- [ ] Add human-friendly messages for HTTP error status responses (consistent, user-facing text alongside status code).

## UI

- [ ] Complete dedicated statistics page (currently charts live on `node_stats.html`).
- [ ] Expand chart controls/time windows and verify usability under long-running sessions.
- [ ] Continue WCAG/accessibility hardening and keyboard-only flow checks.
- [ ] Import old part files (api/ui/service)

## SAM / Runtime

- [ ] Post-current longrun + merge: bound `src/i2p/http.rs` response reads (replace unbounded `read_to_end` with capped read loop).
- [ ] Post-current longrun + merge: add SAM control line max-length guard in `src/i2p/sam/client.rs` (align with `datagram_tcp` framing limits).
- [ ] Post-current longrun + merge: harden chunked HTTP parsing with explicit per-chunk CRLF validation and malformed-input rejection tests.
- [ ] Post-current longrun + merge: enforce outbound payload size cap in `src/i2p/sam/datagram.rs` send path.
- [ ] Post-current longrun + merge: log warning when `sam.keys` permission hardening (`chmod 600`) fails.
- [ ] Post-current longrun + merge: add hostile-input regression tests for i2p module (oversized SAM lines, oversized HTTP body, malformed chunked bodies).
- [ ] Investigate exposing a custom SAM client label instead of generic `SAM UDP Client` in router views.
- [ ] Add memory-pressure instrumentation for routing/lookups/search caches.

## Documentation

- [x] Normalize core docs and README entrypoint.
- [x] Add behavior contract, reviewer checklist, and compatibility timing policy docs.
- [x] Add a phased KAD/wire refactor plan document.
- [ ] Keep `docs/ui_api_contract_map.md` in sync whenever UI/API fields change.

## CLI / Headless

- [ ] Evaluate stronger headless operational workflow docs (session/bootstrap/token rotation runbooks).
