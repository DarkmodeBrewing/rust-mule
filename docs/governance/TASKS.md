Status: ACTIVE
Last Reviewed: 2026-03-10

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
6. v1 interop objective: seamless mixed-client operation with iMule over I2P.
   - protocol interoperability is release-critical (behavioral parity is secondary)

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
- add timezone configuration + UI control backlog:
  - add config key for timezone (IANA zone, e.g. `Europe/Stockholm`) with validation + fallback behavior
  - expose timezone under Settings UI/API so it can be changed without manual file edits
  - apply configured timezone to application log timestamps (instead of UTC-only output)
  - document runtime behavior when timezone is invalid or unavailable
- add managed incoming auto-share backlog:
  - completed downloads in the app-managed `incoming` directory should become auto-shared by the
    application without requiring the user to add the internal runtime data tree as a share root
  - keep the existing safety rule that user-configured share roots must not overlap the managed
    app data directory
  - if download/incoming paths become configurable later, preserve the same semantic rule:
    the managed completed-download output is auto-shared, regardless of path
  - surface managed incoming shares distinctly from user-configured shared folders in the UI/API
- add debug lookup traceability backlog:
  - implement `POST /api/v1/debug/trace_lookup` as debug-only endpoint
  - use async execution (`202 Accepted` + `trace_id`) with poll endpoint
  - return bounded hop-by-hop lookup trace for a target KAD key
  - enforce strict input/runtime bounds (`max_hops`, `parallelism`, `timeout_ms`)
  - bound active traces + trace TTL; optionally support cancellation
  - require debug second-factor secret (`api.debug_token`, `X-Debug-Token`) in addition to normal auth
  - implement debug-disabled `404` and invalid/missing debug-token `403` behavior
  - enforce token lifecycle policy: no auto-delete on debug-disabled; explicit rotation only
  - add dedicated rate limiting and counters for trace requests
  - reference design: `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`
- add debug bootstrap restart backlog:
  - implement `POST /api/v1/debug/bootstrap/restart` as async (`202 + job_id`)
  - implement `GET /api/v1/debug/bootstrap/jobs/{job_id}` for job status
  - enforce single-flight + cooldown + bounded job TTL
  - require debug second-factor secret (`api.debug_token`, `X-Debug-Token`)
  - enforce token lifecycle policy: no auto-delete on debug-disabled; explicit rotation only
  - reference design: `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`
- add logging-surface cleanup backlog:
  - audit trace/routing logs and move non-essential internals behind debug-enabled gating
  - specifically gate verbose bucket/routing-table detail logs behind debug flag
  - keep default logs operator-focused (health/progress/errors) and avoid high-cardinality noisy output
  - rebalance `info` vs `debug` so `info` still shows meaningful forward progress:
    - search dispatched / search completed / search timed out
    - download queued / started / resumed / completed / failed
    - publish/index actions started / completed / failed
  - avoid letting `info` collapse into only periodic status/bucket refresh noise
  - treat bucket refresh chatter as debug-level unless it directly signals an operator-relevant
    state transition or failure
  - rate-limit or summarize repeated `kad_inbound_drop reason="legacy_kad1_disabled"` events per
    peer/opcode window so one noisy legacy peer does not spam debug logs continuously
  - keep counters for dropped legacy KAD1 traffic, but avoid one-line-per-packet logging for
    sustained legacy request storms
- add runtime SAM transport resilience backlog:
  - model SAM transport/session lifecycle explicitly instead of treating
    `SESSION STATUS RESULT=OK` as sufficient recovery proof by itself
  - distinguish:
    - control connection health
    - session creation success
    - datagram readiness
    - verified usable transport
  - require post-create transport verification before marking KAD transport healthy again
  - surface degraded/recovering state in logs, `/api/v1/status`, and UI
  - classify recovery failures explicitly (`duplicate_id`, `duplicate_destination`,
    `control_framing_error`, `router_disconnect`, `tunnel_build_failed`, etc.)
  - reference design: `docs/10_architecture/SAM_TRANSPORT_STATE_MACHINE.md`
- add acceptance/soak validation hardening backlog:
  - fail phase0 gate when key metrics resolve to `nan`/unexpected `SKIP` unless explicitly allowlisted
  - add lightweight script sanity mode in CI for soak scripts (env parsing, trap behavior, report/summary generation)
  - add pass-with-degradation runbook guidance for suspicious-but-zero-exit runs
- add soak artifact governance backlog:
  - define canonical artifact bundle per run (`summary.txt`, `resume_report.txt`, diagnostics JSON, optional stack tarball)
  - define retention period, naming rules, and archive cadence to prevent accidental data loss/sprawl
- add post-restart download diagnostics backlog:
  - expose explicit cancellation/queue transition reasons and timestamps for restart triage
  - include reason fields in diagnostics snapshot so completion timeouts are directly explainable
- add config evolution backlog:
  - introduce config schema versioning + migration notes for future keys (timezone/debug/CLI-related additions)
- add shared library + real upload serving backlog:
  - add configurable shared folders list in config + settings API/UI (multi-path support)
  - implement library scanner/indexer that hashes files and builds publishable source metadata
  - publish source records from indexed shared files (not only synthetic/manual publish calls)
  - track file path binding for published sources so inbound transfer requests map to real local file bytes
  - implement real uploader path for peer requests (`OP_REQUESTPARTS` -> `OP_SENDINGPART`) reading block ranges from disk
  - add safeguards for path traversal/symlink policy/permission failures in shared folders
  - reject unsafe share roots by policy (system root `/`, core OS dirs, app/runtime data dirs) with clear validation errors
  - normalize + canonicalize share paths before accept; prevent duplicate/overlapping entries by policy
  - expose scanner/index health + per-folder stats in settings/status UI for operator visibility
  - reference checklist: `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`
- add transfer rate telemetry backlog:
  - track download speed as rolling bytes/sec per download and aggregate download throughput
  - track upload speed as rolling bytes/sec per active upload and aggregate upload throughput
  - expose rate fields in API responses used by the downloads UI:
    - `/api/v1/downloads`
    - `/api/v1/uploads`
    - optionally `/api/v1/status` aggregate transfer totals
  - surface transfer rates in `/ui/downloads` for both download and upload sections
  - define smoothing/window semantics explicitly (for example 5s / 30s rolling windows) so
    UI values are stable and comparable across sessions
  - ensure zero-fill fallback uploads still report served bytes/rates truthfully and can be
    distinguished from shared-file-backed upload rates when needed
- add UI auto-open startup race backlog:
  - investigate `UI auto-open skipped: API/UI/token did not become ready before timeout`
    when `data/api.token` is created shortly after startup
  - verify ordering between API bind, UI static readiness probe, token-file creation, and
    auto-open timeout window
  - make the readiness gate resilient to near-simultaneous token-file creation instead of
    treating that startup race as a hard skip
  - add logging that distinguishes:
    - API port not ready
    - UI route not ready
    - token file missing
    - token file present but unreadable/empty
- add search-thread classification backlog:
  - stop surfacing background keyword publish/search jobs as ordinary user search threads
  - current symptom: shared-library indexing of filenames like
    `rust-mule-35a24e3-macos-x86_64.tar.gz` produces multiple visible search threads for split
    filename tokens
  - classify search-thread origins explicitly, for example:
    - user search
    - background keyword publish
    - background source lookup/publish
  - keep user-facing Search UI focused on operator/user-initiated searches
  - move background publish/discovery activity into shared-library status, operator diagnostics,
    or a separate background-jobs/debug surface
  - preserve internal tokenization/indexing behavior if it is protocol-correct; fix the UI/API
    abstraction first so internal jobs do not leak into the user search model
  - review shared-library keyword publish lifetime separately from UI thread lifetime:
    - the local `keyword_job` TTL is only the retry/progress/UI window (~2h)
    - remote peers that accepted `PUBLISH_KEY` keep entries on their own keyword-store TTL
    - if reachability is weak during the initial local window, rust-mule may stop retrying too
      early
    - decide whether shared-library keyword publishing should become a sustained refresh
      responsibility instead of a short-lived startup/background burst
- add search-page information architecture cleanup backlog:
  - remove search threads from the global sidebar; that rail does not scale once many searches are
    active
  - make `/ui/search` the compact active-search index/list page
  - move the current search workflow/detail surface to a dedicated detail route, for example:
    `/ui/search/detail?id=<search_id>`
  - keep the search index optimized for dense scanning of many concurrent searches
  - keep per-search controls, results, and deep state on the dedicated detail view instead of
    overloading the main search page or the global navigation rail
- add shared/downloads page split backlog:
  - split the current combined downloads/shared-library surface into separate navigation items:
    - `Downloads`
    - `Shared`
  - keep download transfer lifecycle, queue state, and download troubleshooting on `Downloads`
  - move shared-library inventory, publish status, shared-folder controls, and uploader-focused
    visibility to `Shared`
  - reduce page density and mixed responsibilities by avoiding one oversized combined operations
    page
- add UI stats refresh backlog:
  - audit which UI stats are currently static until full page reload versus updated via SSE/polling
  - add a timed refresh loop for non-reactive stats where SSE coverage does not exist yet
  - or extend the reactive/evented data model so those stats update without manual refresh
  - keep refresh cadence conservative enough to avoid unnecessary API churn while still making the
    UI feel alive during real operation
  - ensure page-level stats surfaces are internally consistent (avoid some counters updating live
    while neighboring counters stay stale)
  - unify liveness terminology across `/ui/` and `/ui/node_stats`
    - current bug: overview uses backend `live` / `live_10m`, while node-stats derives a broader
      frontend-only `live` state from `last_seen_secs_ago`
    - either reuse the backend counters everywhere or rename the node-stats categories so `live`
      does not mean two different things in two places
- add runtime SAM/KAD transport resilience backlog:
  - detect runtime loss of the effective SAM datagram/KAD transport session, not just startup
    failure
  - surface degraded/disconnected transport state explicitly in status/UI so a long-running client
    cannot look healthy while inert
  - automatically retry or recreate the SAM datagram session when the router reports transient
    failures like `duplicate destination` during runtime reconnect paths
  - improve diagnostics so logs and status distinguish:
    - duplicate session id
    - duplicate destination
    - router disconnect
    - tunnel-build/session-establish failure
  - include a short destination/session fingerprint in warnings so multi-instance diagnosis is
    possible without exposing full keys
- add documentation hygiene and publishing-scope backlog:
  - decide which documentation should be published to GitHub Pages versus kept as internal
    governance/handoff/backlog material
  - define explicit inclusion/exclusion rules for the VitePress/Pages build so operational and
    governance docs are not exposed accidentally
  - audit docs navigation so published docs present a coherent external-facing information
    architecture instead of mirroring internal working notes
- add governance-doc archiving backlog:
  - create an archive structure under `docs/governance/archive/`
  - keep `handoff.md` short and current:
    - current status
    - active decisions
    - next steps
    - recent change log only
  - keep `TASKS.md` focused on active backlog/priorities rather than historical narrative
  - move stale or historical entries into dated or milestone-based archive files
  - add archive references from the active docs so history remains discoverable without keeping the
    working docs bloated
- add GitHub community-standards documentation backlog:
  - align repo/community docs with GitHub community standards where applicable
  - review and maintain the expected top-level/community-facing files such as:
    - `README`
    - `LICENSE`
    - `CODE_OF_CONDUCT`
    - `CONTRIBUTING`
    - issue/PR templates as needed
    - support/security/community guidance where appropriate
  - make sure the published docs and repository root docs complement each other instead of
    duplicating or contradicting each other
- add timed-out search lifecycle backlog:
  - do not silently drop timed-out searches from the UI
  - preserve timed-out searches as explicit state with clear status labeling
  - allow per-search actions:
    - resubmit timed-out search
    - remove timed-out search
  - add bulk actions:
    - remove all timed-out searches
    - resubmit all timed-out searches
  - keep timeout history visible long enough for operator triage instead of making failed searches
    appear to vanish

## v1 Stable Interop Release Gates

- verify wire compatibility with iMule for core flows:
  - HELLO/session establishment
  - source publish/search (`PUBLISH_SOURCE`, `SEARCH_SOURCE`)
  - transfer request/serve (`OP_REQUESTPARTS`, `OP_SENDINGPART`)
- align default transfer sizing/pacing to iMule-compatible baseline (configurable override allowed)
- pass mixed-client end-to-end tests (`rust-mule <-> iMule`) for:
  - discover source
  - request data
  - restart/resume transfer
  - complete and verify resulting file/hash
- enforce no-regression checks on those interop paths before v1 tag
- enforce shaper compatibility contract:
  - shaping may change timing/order/rate policy, but must not alter wire format/semantics
  - run before/after decode-equivalence and mixed-client soak verification for shaping changes

## Interop Fallback Strategy (When Live iMule Soak Is Blocked)

- add offline/controlled interop harness:
  - replay canonical iMule-like packet sequences from fixtures/pcap-derived vectors
  - validate decode/encode behavior and service state transitions for core flows
- add wire golden tests for critical messages:
  - HELLO/session
  - source publish/search
  - transfer request/serve (`OP_REQUESTPARTS` / `OP_SENDINGPART`)
- keep live mixed-client soak as pre-release requirement:
  - not required for every daily iteration when environment/tooling is blocked
  - required before v1 release tag and final compatibility sign-off

## Definition Of Done

- measurable improvement in search/publish round-trip success over baseline
- download subsystem phase 0/1 merged with tests
- clear status/log counters for timeout/retry/drop classes
- KAD/wire refactor prerequisites documented and baselined before scheduling code-heavy changes
- `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test` pass
- documentation updated (`README.md`, `docs/TODO.md`, `docs/handoff.md`)
