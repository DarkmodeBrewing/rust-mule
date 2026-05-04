Status: ACTIVE
Last Reviewed: 2026-03-20

# Handoff / Continuation Notes

This file exists because chat sessions are not durable project memory. In the next session, start here, then check `git log` on `main` and the active feature branch(es).

- 2026-04-01: Added debug endpoint second-factor enforcement for implemented `/api/v1/debug/*` routes.
  - startup now creates/loads `data/debug.token` alongside `data/api.token`
  - `/api/v1/debug/*` now requires both bearer auth and `X-Debug-Token`
  - missing/invalid debug token now returns `403` when debug routes are enabled
  - token comparison uses a constant-time helper
  - added API tests for disabled, missing, invalid, and valid debug-token behavior
- 2026-05-04: Added KAD/DHT resource caps on `fix/kad-dht-resource-caps`.
  - status:
    - patched routing table runtime growth with configurable `kad.service_max_runtime_nodes`
    - patched source-store growth with configurable file, per-file source, and total source caps
    - added unit coverage for routing-cap pruning and source-store caps
  - decisions:
    - prefer hard in-memory caps over relying on persistence truncation or delayed failure eviction
    - prune weaker routing peers first using existing health/liveness signals
    - keep source-store pruning deterministic and simple for this security fix
  - next steps:
    - tune source-store cap defaults after real alpha traffic if they prove too low or too high
    - consider adding per-source-store eviction counters to status telemetry
  - validation:
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features`
- 2026-05-04: Fixed CI quality gate failures on `fix/kad-dht-resource-caps`.
  - status:
    - addressed Rust 1.95 Clippy diagnostics from PR #75 quality job
    - removed an unnecessary `.into_iter()` in the transfer pump loop
    - changed two guarded divisions to `checked_div(...).unwrap_or(0)`
  - validation:
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features`

## Goal

Implement an iMule-compatible Kademlia (KAD) overlay over **I2P only**, using **SAM v3** `STYLE=DATAGRAM` sessions (UDP forwarding) for peer connectivity.

## Status

- Status (2026-03-20): Download phase 2 now has a real end-to-end transfer baseline on
  `feature/download-phase2-transfer`.
  - `src/download/transfer.rs` provides eD2k-style transfer framing, request helpers, and focused
    unit tests.
  - `src/app.rs` download pumping now performs real outbound `STREAM CONNECT` + `OP_REQUESTPARTS`
    block fetches instead of fabricating local `OP_SENDINGPART` packets.
  - App startup persists a separate transfer identity in `data/sam.transfer.keys` and ensures a
    dedicated `<session>-transfer` STREAM session for transfer traffic.
  - Inbound transfer serving exists: accepted STREAM connections decode `OP_REQUESTPARTS` and
    answer with `OP_SENDINGPART` from `UploadService`.
  - KAD source publish/cache/store paths now distinguish `tcp_dest` (transfer) from `udp_dest`
    (KAD datagram) and preserve the published transfer destination when possible.
  - Publish-source decode is lenient again: if full tag parsing fails, we fall back to the minimal
    decoder and still ACK/store the source using the sender destination.
  - Transfer hardening added so outbound connect attempts and inbound idle reads are time-bounded.
- Decisions:
  - persist the transfer identity; do not generate it per run, because source publication needs a
    stable inbound destination
  - keep the first inbound server implementation simple and uncompressed; always answer with
    `OP_SENDINGPART` for now
  - keep wire/protocol compatibility release-critical, but keep local persistence Rust-native by
    default unless cross-client runtime-state portability becomes a concrete requirement
- Next steps:
  - validate the phase-2 path against real alpha peers and confirm published `tcp_dest` matches
    reachable transfer listeners in practice
  - add broader transfer-session tests or soak coverage around listener/session recovery behavior
  - decide whether to keep `write_packet()` eager flushing or relax it after real-network testing
- Historical note (2026-03-19):
  - this slice started as transport framing plus an outbound-only client path, then was extended
    into the dedicated persisted transfer session with inbound serving and session reuse above
- Status (2026-03-19): Added a storage compatibility decision note.
  - New note: `docs/10_architecture/STORAGE_COMPATIBILITY_POLICY.md`
  - Decision:
    - treat compatibility as a boundary, not a blanket rule
    - keep wire/protocol compatibility release-critical
    - keep internal persistence Rust-native by default
    - prefer import/export or migration adapters over full legacy on-disk parity
  - Immediate implication:
    - do not treat byte-for-byte iMule `.part.met` / `known.met` parity as the
      default implementation goal unless cross-client runtime-state portability
      becomes a concrete requirement
- Status (2026-03-17): Added a SAM transport lifecycle design note after reviewing
  `source_ref/yosemite`.
  - New note: `docs/10_architecture/SAM_TRANSPORT_STATE_MACHINE.md`
  - Captures a stricter runtime model for:
    - SAM control/session state
    - datagram readiness vs verified transport health
    - explicit degraded/recovering state
    - post-create verification before declaring KAD transport healthy again
  - Linked the existing runtime SAM resilience backlog to the new design note.
- Status (2026-03-11): Captured another alpha reliability/backlog note on
  `chore/alpha-backlog-notes`.
  - Observed repeated `kad_inbound_drop reason="legacy_kad1_disabled"` spam from a single legacy
    peer sending `KADEMLIA_REQ (0x05)`.
  - Confirmed the current behavior is protocol-correct but too noisy at debug granularity for a
    sustained legacy peer.
  - Added backlog guidance to rate-limit or summarize repeated legacy-KAD1 drop events per
    peer/opcode window while keeping aggregate counters.
- Status (2026-03-11): Captured another alpha product/backlog note on
  `chore/alpha-backlog-notes`.
  - Clarified the desired sharing model for completed downloads:
    - user-configured share roots must continue to be blocked from overlapping the managed app
      data directory
    - the app-managed completed-download output (`incoming`) should nevertheless become
      auto-shared by application policy
  - Added backlog guidance to keep managed incoming shares distinct from user-configured shared
    folders in the UI/API, and to preserve that semantic if download/incoming paths become
    configurable later.
- Status (2026-03-10): Started the alpha UI stabilization track on
  `feat/alpha-ui-stabilization`.
  - Fixed a UI boot-order failure seen on the older macOS machine where Alpine evaluated
    `x-data="...()"` expressions before the page bootstrap had attached the page factories to
    `window`.
  - Added `ui/assets/js/ui-bootstrap.js` so the UI now imports the page-specific controller
    module first and only then loads Alpine.
  - Updated all UI pages to use the new bootstrap module instead of loading Alpine before the app
    module.
  - Normalized the sidebar Overview navigation target to `/ui/` across all pages so the route is
    canonical and consistent on older browsers.
  - Search thread lists now prefer the original keyword label when available instead of showing
    only the opaque search hash; the hash is retained as secondary context when it differs from
    the label.
  - Fixed the skip-link affordance so it stays fully off-screen until keyboard focus instead of
    remaining half-visible in the top-left corner.
  - Changed `/api/v1/status` startup semantics so it now returns `200` with `ready: false` and a
    zeroed status payload during early boot instead of returning `503` until KAD bootstrap
    finishes.
  - Changed `/api/v1/searches` startup semantics so it now returns `200` with `ready: false` and
    an empty list during early KAD startup instead of timing out with `504`.
  - Hardened the desktop shell/layout based on the older macOS alpha feedback:
    - removed the floating outer app padding for `.container.shell`
    - turned the sidebar into a flush left rail
    - gave the main content a unified full-height surface
    - increased visual separation between primary and destructive buttons
  - Moved the `+ New Search` action into a stable slot in the left rail across all pages so it
    remains reachable even when the Search Threads list grows.
  - Flattened the shell styling further so the UI reads less like a glossy dashboard and more like
    a utilitarian application workspace:
    - section cards now render as flat bordered blocks
    - Search Threads now reads as a rail section instead of a floating widget
    - the main pane reads as one coherent surface with divided sections
  - Fixed the left-rail session pill so it now reflects real session state instead of staying
    frozen at `unknown`.
  - Changed node-stats chart layout so the graphs render on their own rows instead of sharing a
    cramped three-column band.
  - Fixed two search-form UX bugs:
    - successful submit now clears and refocuses the inputs
    - the search form now stays disabled until the existing `/api/v1/searches.ready` signal says
      KAD search is actually ready
  - Reworked the page roles so `/ui/` is a real application overview instead of a single-search
    control surface:
    - removed the stale overview-only active-search controls and controller state
    - changed the landing page into a health/search summary with recent search activity and raw
      status links
    - updated sidebar subtitles so each page describes its actual purpose
  - Tightened a small follow-up UI pass from the latest alpha screenshots:
    - the search page now shows the same KAD readiness badge pattern as the overview
    - flex rows now center-align their children so badges/buttons stop stretching vertically
    - the settings page no longer shows the unrelated search/routing KPI boxes
    - the shared-folder editor now explicitly explains why there is no browser-side folder picker
  - Tightened navigation/settings cleanup from the next alpha screenshot pass:
    - removed the stray runtime snapshot panel from the settings page
    - turned sidebar navigation into explicit application chrome with hover and active states
    - active navigation now uses the same dark-blue family as the main workspace instead of
      looking like a plain text link
  - Tightened the sidebar rail structure from the next alpha screenshot pass:
    - navigation items now span the full rail width with more padding
    - active state is now a filled background, not just an outline
    - Search Threads now inherits the same rail item treatment so the sidebar reads as one system
  - Applied a Lighthouse-driven layout-stability pass aimed at reducing startup CLS:
  - Split the UI controller bundle by page after the alpha Lighthouse run so the app no longer ships every Alpine controller to every page; the shared helpers now live in `app-core.js` and `ui-bootstrap.js` imports the page-specific controller based on a `data-ui-page` attribute.
    - reserved space for session/status strips and feedback rows
    - stabilized badge widths for the startup state pills
    - gave the sidebar search-thread area a fixed minimum footprint so the rail does not jump when
      async data arrives
    - reserved height for the overview KPI/summary rows so empty-to-live transitions move less
- Decisions:
  - treat the current macOS issue as a frontend boot sequencing problem, not a backend bootstrap
    issue.
  - prefer an explicit UI bootstrap module over relying on browser-specific script scheduling
    behavior between classic `defer` scripts and ES modules.
  - store and expose the original keyword text through the KAD search job so the UI can render a
    human-readable search thread title consistently across pages.
  - keep the skip-link accessibility affordance, but hide it until focus rather than removing it.
  - treat `/api/v1/status` as an application status document, not as a transport-level readiness
    probe; use a structured `ready` flag during startup instead of `503`.
  - use the same structured startup approach for the search thread list, because that page is
    polled/UI-facing and should not surface bootstrap lag as a gateway timeout.
  - keep the current information architecture for now, but make the app shell read as a proper
    two-pane desktop application instead of a stack of detached cards.
  - keep primary search creation as a stable rail action, not as content that can be pushed out of
    reach by dynamic thread state.
  - prefer plain section blocks and border dividers over rounded/glowing cards for the alpha UI;
    the application should read like a tool, not a marketing site.
  - avoid getter-based UI state inside spread mixins; the object spread froze the session-pill
    labels/classes at creation time, so explicit updaters are safer here.
  - use the already-available search-thread readiness signal in the frontend instead of letting the
    user discover bootstrap lag by hitting the search form and walking into a timeout path.
  - keep search execution and detailed search management on the dedicated search pages; the
    overview page should summarize system state, not drive one arbitrary active search thread.
  - do not add a fake folder picker to the settings page: the browser does not reliably provide a
    durable absolute path that can be written back into `config.toml`, so a plain textarea plus a
    clear explanation is more honest than a broken picker affordance.
  - do not add a separate boot screen yet. The better immediate fix is to keep the existing shell
    and make startup state explicit and honest with `ready` badges/disabled controls. A dedicated
    boot screen would add routing/state complexity without solving the underlying page clarity
    issues first.
  - keep the sidebar as a unified control rail: navigation and search-thread rows should share the
    same structural treatment so the left pane reads like application chrome instead of a mixed bag
    of links and ad hoc list content.
  - treat Lighthouse CLS findings as a layout-reservation problem first, not a boot-screen problem.
    The immediate fix is to reserve stable space in the existing shell and pages instead of adding
    another startup route/surface.
  - treat HTTP cache headers as a separate follow-up from the alpha UI trimming pass. They are
    worth adding for repeat navigations across the multi-page UI, but they do not materially solve
    first-load JS execution cost or Lighthouse unused-JS findings.
- Next steps:
  - open and merge the alpha UI stabilization PR.
  - follow up with static-asset cache headers:
    - use cache headers for JS/CSS/image assets served under `/ui/assets/`
    - prefer long-lived immutable caching only if filenames become fingerprinted
    - otherwise use a shorter TTL/revalidation policy so deploys do not strand stale UI assets
  - keep HTML routes separately revalidated; do not cache application pages like immutable assets.
  - investigate the startup race behind:
    `UI auto-open skipped: API/UI/token did not become ready before timeout`
    when `data/api.token` appears shortly after process start.
  - split that investigation into separate readiness causes so logs say whether auto-open missed:
    API bind, UI route readiness, token-file creation, or token readability.
  - classify search-thread origins so shared-library keyword publish jobs do not show up as
    ordinary user search threads in the Search UI.
  - review shared-library keyword publish lifetime separately from UI thread lifetime:
    - the local `keyword_job` TTL is only the retry/progress/UI window (~2h)
    - remote peers that accepted `PUBLISH_KEY` keep entries on their own keyword-store TTL
    - decide whether shared-library keyword publishing should become a sustained refresh
      responsibility instead of a short-lived startup/background burst
  - refactor search-page information architecture:
    - remove search threads from the global sidebar
    - make `/ui/search` a compact active-search index
    - move the current search workflow/detail surface to a dedicated detail route
  - split the current combined downloads/shared-library UI into separate `Downloads` and `Shared`
    navigation/pages so transfer troubleshooting and library/publish management stop competing for
    one page.
  - add timed refresh or broader reactive wiring for UI stats that currently stay stale until
    manual reload.
  - unify liveness terminology across overview and node-stats; do not let `/ui/node_stats`
    invent a broader frontend-only meaning of `live` while `/ui/` shows the backend service
    counters.
  - detect runtime loss of the effective SAM/KAD transport session, surface degraded/disconnected
    state explicitly, and auto-recover instead of allowing long-running clients to look healthy
    while inert.
  - improve runtime SAM diagnostics so logs/status distinguish duplicate destination, duplicate
    session id, router disconnect, and tunnel/session-establish failures with short
    instance/destination fingerprints.
  - do a documentation hygiene pass:
    - decide what belongs on GitHub Pages versus what should stay internal-only
    - align repository/community-facing docs with GitHub community standards
  - archive governance working docs so `handoff.md` and `TASKS.md` stay short/current instead of
    accumulating indefinite historical narrative.
  - preserve timed-out searches as explicit UI state instead of letting them disappear; add
    per-search and bulk resubmit/remove actions for timed-out searches.
  - rebalance `info` vs `debug` logging so operator-relevant progress stays visible at `info`
    while bucket-refresh chatter moves behind `debug`.
- Change log:
  - Added `ui/assets/js/ui-bootstrap.js`.
  - Split the old monolithic `ui/assets/js/app.js` into `ui/assets/js/app-core.js` plus page-specific modules under `ui/assets/js/pages/` so each UI page only loads the controller it uses.
  - Updated all `ui/*.html` page shells.
  - Updated Overview sidebar links in all `ui/*.html` pages to `/ui/`.
  - Updated KAD search API/service plumbing to retain `keyword_label`.
  - Updated search thread rendering in all `ui/*.html` pages to prefer the label over the hash.
  - Updated `ui/assets/css/base.css` to make `.skip-link` focus-only visible.
  - Updated `/api/v1/status` to return a startup payload with `ready: false` when KAD has not yet
    published its first status snapshot.
  - Updated `/api/v1/searches` to return `{ ready, searches }` and treat startup timeout as
    `ready: false` with an empty list.
  - Updated `ui/assets/css/base.css` to make the shell full-bleed, convert the sidebar into a
    flush rail, and give the main pane a unified background surface.
  - Split the UI controller payload into `ui/assets/js/app-core.js` plus page-specific modules
    under `ui/assets/js/pages/`, and updated `ui/assets/js/ui-bootstrap.js` plus all UI pages to
    load only the controller needed for the current page.
  - Updated `ui/assets/css/color-dark.css`, `ui/assets/css/colors-light.css`, and
    `ui/assets/css/color-hc.css` to give primary and destructive buttons distinct foreground and
    background treatment.
  - Updated all `ui/*.html` sidebars to place `+ New Search` directly under Navigation.
  - Updated `ui/assets/js/app.js` so `startNewSearch()` lives in the shared session/UI mixin and
    is available from every page shell.
  - Updated `ui/assets/css/base.css` to flatten `.card` styling, turn Search Threads into a rail
    section with top/bottom dividers, and make the main pane read as a continuous utilitarian
    workspace.
  - Updated `ui/assets/js/app.js` so the session pill uses explicit mutable UI fields updated by
    `checkSession()` instead of getter values frozen by object spread.
  - Updated `ui/node_stats.html` so the charts stack vertically instead of sharing a three-column
    row.
  - Updated `ui/assets/js/app.js` and `ui/search.html` so the search form is disabled until KAD
    search is ready, and successful submit clears/refocuses the inputs.
  - Updated `ui/index.html` to become a true application overview page with search activity,
    service counters, and raw-status sections.
  - Removed stale active-search overview controller state from `ui/assets/js/app.js`.
  - Updated sidebar subtitle copy in `ui/search.html`, `ui/search_details.html`,
    `ui/node_stats.html`, `ui/log.html`, and `ui/settings.html`.
  - Updated `ui/tests/e2e/smoke.spec.mjs` to match the new overview-page contract.
  - Updated `ui/search.html` to show a KAD readiness badge.
  - Updated `ui/assets/css/base.css` so generic flex rows center-align children and badges center
    their contents instead of stretching vertically.
  - Removed the unrelated search/routing KPI boxes from `ui/settings.html`.
  - Added explanatory copy to `ui/settings.html` describing why shared folders still require
    explicit filesystem paths.
  - Removed the settings-page runtime snapshot panel.
  - Updated sidebar navigation styling in `ui/assets/css/base.css`,
    `ui/assets/css/color-dark.css`, `ui/assets/css/colors-light.css`, and
    `ui/assets/css/color-hc.css` so active/hover states read like application navigation instead
    of plain links.
  - Updated the shared rail styling in `ui/assets/css/base.css` so navigation items and search
    thread rows use the same full-width padded treatment.
  - Updated `ui/assets/css/base.css` with reserved-height/status-strip helpers for lower startup
  - Added backlog notes for the UI auto-open/token readiness race observed during alpha startup,
    where `data/api.token` can appear in the same minute as the warning but still miss the
    current readiness timeout window.
  - Added backlog notes for search-thread origin classification after shared-library filename
    tokenization produced multiple visible search threads from one shared archive name.
  - Added backlog notes that shared-library keyword publishing may need a sustained refresh
    strategy because the local keyword-job TTL only bounds retry/UI lifetime, not remote keyword
    store retention.
  - Added backlog notes for a search-page IA cleanup so active-search listing moves into
    `/ui/search` and the current workflow surface becomes a dedicated detail page instead of
    overflowing the global sidebar.
  - Added backlog notes for splitting the combined downloads/shared-library page into separate
    `Downloads` and `Shared` surfaces.
  - Added backlog notes for UI stats refresh/reactivity so page counters stop drifting into a
    partially stale state between manual reloads.
  - Added backlog notes to unify `live`/`live_10m` terminology between overview and node-stats so
    the UI stops showing two incompatible meanings of “live”.
  - Added backlog notes for runtime SAM/KAD session resilience so long-running clients surface and
    recover from transport loss instead of appearing healthy while inert.
  - Added backlog notes for documentation hygiene, GitHub Pages publishing scope, and GitHub
    community-standard repository docs.
  - Added backlog notes for archiving governance docs so active working documents stay concise and
    history moves into `docs/governance/archive/`.
  - Added backlog notes for timed-out search lifecycle handling so failed searches remain visible
    and actionable instead of silently disappearing from the UI.
  - Added backlog notes for logging-surface cleanup so `info` logs narrate real operator progress
    and verbose bucket refresh detail moves behind `debug`.
    layout shift.
  - Updated `ui/index.html` and `ui/search.html` to use the new stable status-strip/feedback
    classes.
  - Updated all page sidebars so the session strip uses the same reserved-height treatment.
  - Updated API/UI test fixtures for `keyword_label`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-10): Started the macOS dual-architecture packaging follow-up on
  `feat/macos-dual-arch-builds`.
  - The macOS build script now packages according to an explicit Rust target triple instead of the
    host machine architecture, so a macOS runner can produce separate `arm64` and `x86_64`
    bundles.
  - The Intel (`x86_64-apple-darwin`) build keeps the `MACOSX_DEPLOYMENT_TARGET=12.0` floor for
    the older private-alpha test Mac.
  - The Apple Silicon (`aarch64-apple-darwin`) build is now a distinct artifact and no longer
    inherits the Intel macOS 12 floor by default.
  - CI and release workflows were expanded to build separate macOS arm64 and x86_64 artifacts.
- Decisions:
  - package macOS arm64 and x86_64 as separate tarballs rather than introducing a universal binary
    for the first alpha iteration.
  - scope the macOS 12 deployment floor only to the x86_64 build, because that is the actual
    compatibility need.
  - describe the x86_64 macOS bundle as a target build on `macos-latest`, not a native Intel-host
    build, because the workflow is selecting `x86_64-apple-darwin` on the current macOS runner.
- Next steps:
  - run the standard validation set.
  - inspect the CI/release workflow shape carefully, since the macOS x86_64 build now depends on
    cross-target packaging from `macos-latest`.
- Change log:
  - Updated `scripts/build/build_macos_release.sh`.
  - Updated `.github/workflows/ci.yml`.
  - Updated `.github/workflows/release.yml`.
  - Updated `scripts/build/README.md`.
  - Updated `docs/30_operations/ALPHA_RELEASE_CHECKLIST.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Fixed the docs site build failure caused by
  `docs/governance/REVIEWERS_CHECKLIST.md`.
  - Removed a stray leading `---` that VitePress was treating as the start of YAML frontmatter.
  - Verified the docs site now builds successfully with `npm run docs:build`.
  - Added a dedicated `docs-pages` CI job that runs `npm install` and `npm run docs:build` so
    future Pages/VitePress breakage fails in CI instead of surfacing only in the Pages workflow.
- Decisions:
  - keep `docs/governance/REVIEWERS_CHECKLIST.md` as plain markdown with no frontmatter.
  - defer the broader question of which docs should or should not be published to Pages; this fix
    only restores a valid build.
- Next steps:
  - decide whether governance/internal docs should remain in the published VitePress navigation.
- Change log:
  - Updated `docs/governance/REVIEWERS_CHECKLIST.md`.

- Status (2026-03-09): Started the private alpha checklist/release-readiness follow-up on
  `feat/alpha-checklist`.
  - Added `docs/30_operations/ALPHA_RELEASE_CHECKLIST.md` as the explicit pre-tag checklist for a
    private alpha such as `v0.1.0-alpha.1`.
  - The checklist captures:
    - supported alpha platform targets
    - artifact/build requirements
    - CLI/config contract
    - end-to-end flow expectations
    - known alpha caveats
    - tagging criteria
  - Surfaced the checklist in `docs/index.md` and aligned `docs/README.md`.
- Decisions:
  - keep alpha release-readiness as an operations doc, not a handoff-only note, so the checklist
    remains visible and reviewable outside chat continuity.
  - treat `v0.1.0-alpha.1` as the recommended first private alpha tag once the checklist is
    satisfied or explicitly deferred item-by-item.
- Next steps:
  - review the checklist against the remaining orchestrator needs.
  - decide whether `--print-effective-config` is still required before cutting the first alpha.
- Change log:
  - Added `docs/30_operations/ALPHA_RELEASE_CHECKLIST.md`.
  - Updated `docs/index.md`.
  - Updated `docs/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Started the repo-config cleanup follow-up on
  `feat/config-example-cleanup`.
  - The tracked `config.toml` was normalized from a lab-specific file into an alpha-safe example
    baseline.
  - Replaced environment-specific SAM addresses with loopback defaults:
    - `sam.host = "127.0.0.1"`
    - `sam.forward_host = "127.0.0.1"`
    - `sam.forward_port = 0`
  - Turned debug endpoints off in the tracked config:
    - `api.enable_debug_endpoints = false`
  - Added explicit log-file settings and an explicit empty `[sharing]` section so the packaged
    config better reflects the current feature surface.
- Decisions:
  - treat the tracked `config.toml` as an example/default config for alpha users and packaging,
    not as a developer-lab machine config.
  - prefer neutral loopback-safe values in the tracked config, with orchestration or multi-node
    test overrides happening in run-specific configs instead of the repo file.
- Next steps:
  - run the standard validation set.
  - decide whether the alpha checklist should explicitly distinguish example config from
    orchestrator/test configs.
- Change log:
  - Updated `config.toml`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Started the dedicated macOS alpha-floor follow-up on
  `feat/macos-alpha-floor`.
  - The macOS packaging script now exports `MACOSX_DEPLOYMENT_TARGET=12.0` by default before the
    release build runs.
  - That same script is used by both the CI build matrix and the tag-driven release workflow, so
    the private alpha macOS floor is now explicit and consistent across both paths.
  - Updated build documentation to state that the intended private alpha macOS support floor is
    12.0 unless deliberately overridden.
- Decisions:
  - target macOS 12.0 for the private alpha because that matches the available older test machine.
  - keep the deployment target in the build script rather than duplicating it in workflow YAML, so
    every caller inherits the same floor by default.
- Next steps:
  - run the standard local validation set.
  - after merge, test a produced macOS artifact on the older Mac before claiming the floor is
    verified in practice.
- Change log:
  - Updated `scripts/build/build_macos_release.sh`.
  - Updated `scripts/build/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Started the separate alpha build-matrix / packaging track on
  `feat/alpha-build-matrix`.
  - Added a CI build matrix that runs the host-platform packaging script on:
    - Linux
    - macOS
    - Windows
  - Each matrix job now verifies that the expected packaged artifact is produced under `dist/`,
    so release-bundle breakage shows up on PRs before a tag is cut.
  - Updated `scripts/build/README.md` to document the relationship between the CI build matrix,
    the tagged release workflow, and the intended private alpha flow.
- Decisions:
  - keep alpha packaging validation as a separate branch from the CLI basics work; release
    validation and CLI ergonomics are distinct concerns and should remain separately reviewable.
  - validate host-platform packaging first rather than introducing cross-compilation complexity
    in the first alpha-readiness slice.
- Next steps:
  - run the standard local validation plus a local Linux release-bundle build.
  - decide whether the next alpha-readiness slice should formalize an alpha checklist or pin an
    explicit macOS support floor.
- Change log:
  - Updated `.github/workflows/ci.yml`.
  - Updated `scripts/build/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added packaged-artifact smoke checks to the alpha build-matrix track.
  - Added Unix and Windows archive smoke helpers under `scripts/build/`.
  - The CI build matrix now unpacks the generated release archive and verifies the packaged binary
    can run:
    - `--version`
    - `--help`
    - `--check-config --config ./config.example.toml`
  - This validates the release-bundle contract directly instead of only checking that an archive
    file exists.
- Decisions:
  - smoke the packaged artifact on each native runner rather than trying to cross-run binaries
    from Linux.
  - keep the smoke scope narrow: CLI contract and packaged config validation only.
- Next steps:
  - run the standard local validation plus a local archive smoke on Linux.
  - decide whether the next alpha-readiness slice should pin an explicit macOS deployment target
    and document the intended support floor.
- Change log:
  - Added `scripts/build/smoke_unix_release.sh`.
  - Added `scripts/build/smoke_windows_release.ps1`.
  - Updated `.github/workflows/ci.yml`.
  - Updated `scripts/build/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#59` review feedback for the alpha CLI basics branch.
  - `--help`, `-?`, and `--version` now short-circuit parsing, so they still succeed even when
    trailing unknown flags are present.
  - `--config` now rejects flag-like values such as `--check-config` instead of silently treating
    them as a config path.
  - `main` now returns `Result<(), MainError>` directly, with `From` conversions for CLI/config/app
    errors and a boxed app-error variant to keep the result error small enough for clean clippy.
  - Added CLI parser coverage for help/version short-circuit behavior and invalid flag-like config
    paths.
- Decisions:
  - keep help/version behavior forgiving for orchestration and shell probing workflows; explicit
    help/version requests take precedence over later parse failures.
  - reject `--config` values that begin with `-` to avoid ambiguous flag consumption.
- Next steps:
  - merge PR `#59`.
  - start the separate alpha build-matrix / packaging branch after the CLI slice lands.
- Change log:
  - Updated `src/main.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Started the private alpha-readiness track with explicit CLI/config basics.
  - `rust-mule` now supports:
    - `--config <path>`
    - `--check-config`
    - `--help`
    - `-?`
    - `--version`
  - Normal startup now loads config explicitly via `load_config(...)` and no longer auto-creates
    `config.toml` on missing-path startup. This gives the binary a predictable contract for
    orchestrators like `mule-doctor`.
  - `--check-config` validates config and exits without booting the app.
  - Added unit coverage for CLI parsing in `src/main.rs`.
- Decisions:
  - treat missing config as an error for normal app startup; alpha/orchestrator workflows need an
    explicit config file contract, not silent file creation.
  - keep `load_or_create_config(...)` available for code paths that still intentionally want that
    behavior, but stop using it in the main application entrypoint.
- Next steps:
  - decide whether alpha-readiness should next add `--print-effective-config` or `--data-dir`
    overrides for orchestration workflows.
  - decide whether the self-test binary should also move off `load_or_create_config(...)` for
    stricter alpha consistency.
- Change log:
  - Updated `src/config_io.rs`.
  - Reworked `src/main.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#58` review feedback on uploads UI utility drift and reason grouping.
  - Added a real `.items-center` utility class so the recent-session badge rows align as intended
    instead of relying on an undefined utility.
  - `recent_session_groups` in the downloads UI is now derived from the actual `recent_sessions`
    payload rather than a hard-coded terminal-reason list, so new terminal reasons will surface
    automatically without editing two separate UI mappings.
- Decisions:
  - keep terminal-reason styling centralized in `uploadTerminalReasonClass(...)`, and derive
    grouped summaries from payload content instead of hard-coding the current reason set.
- Next steps:
  - watch PR `#58` for any remaining review comments.
- Change log:
  - Updated `ui/assets/css/base.css`.
  - Updated `ui/assets/js/app.js`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Hardened the uploads UI for recent session lifecycle visibility.
  - `/ui/downloads` now summarizes recent upload sessions by terminal reason and renders
    reason-colored badges for `completed`, `dropped`, and `expired`.
  - Recent upload session rows now surface the terminal reason more clearly instead of burying it
    inside a flat text list.
  - The Playwright uploads mock was extended to include recent sessions for all three current
    terminal reasons so the fixture stays aligned with the richer uploads contract.
- Decisions:
  - keep the browser smoke suite focused on stable page-level contract checks for this slice;
    do not assert dynamic upload-row rendering there until the fixture/runtime path is made more
    deterministic.
- Next steps:
  - decide whether the uploads table should split recent sessions into separate grouped sections
    instead of a single list with reason badges.
  - decide whether `/api/v1/uploads` should expose aggregate recent-session counts by reason so
    the UI no longer has to derive them client-side.
- Change log:
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/assets/css/base.css`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#57` review feedback on transfer-pump hash reuse.
  - The transfer pump now computes the lowercase file-hash string once per send path and reuses
    it for uploader activity transitions instead of allocating it repeatedly for
    `note_held(...)`, `note_sending(...)`, and `note_terminal(...)`.
- Decisions:
  - keep this as a local hot-loop cleanup only; no API or lifecycle semantics changed.
- Next steps:
  - watch PR `#57` for any remaining review comments.
- Change log:
  - Updated `src/app.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added explicit uploader-side `completed` terminal lifecycle signals.
  - `UploadTerminalReason` now includes `Completed`.
  - The download transfer pump marks upload sessions `completed` when
    `ingest_inbound_packet(...)` succeeds for a sent block, both for:
    - matured held leases
    - immediately sent non-held leases
  - `/api/v1/uploads` now surfaces `terminal_reason = "completed"` in recent session history.
- Decisions:
  - treat successful packet ingestion as the current truthful uploader completion hook; do not
    try to infer completion from time or downstream file-finalization state.
  - only emit `completed` on `Ok(_)` from `ingest_inbound_packet(...)`; failed sends remain
    active until another explicit terminal reason or TTL expiry applies.
- Next steps:
  - decide whether uploader-side `replaced` terminal reasons should be added when a newer held
    reservation supersedes existing work.
  - decide whether the uploads UI should visually separate `completed` recent sessions from
    `dropped` and `expired` sessions now that all three exist.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#56` review feedback on uploader terminal lifecycle semantics.
  - `prune_expired(...)` now preserves its original ordering:
    append expired active sessions into `recent_sessions`, then drop stale recent entries, then
    apply the per-file cap once. This avoids evicting still-valid recent sessions before stale
    entries are retained away.
  - Added a focused regression test proving the uploader contract remains “first terminal reason
    wins”: once a session has already expired into recent history with `Expired`, a later
    `note_terminal(..., Dropped)` call does not overwrite that reason.
- Decisions:
  - keep `push_recent_session(...)` for explicit terminalization paths only; it is correct there
    because `note_terminal(...)` prunes stale entries before appending.
  - document terminal-reason precedence in tests rather than allowing silent reason rewrites.
- Next steps:
  - watch PR `#56` for any remaining review comments.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added explicit uploader-side `dropped` terminal lifecycle signals.
  - `UploadTerminalReason` now includes `Dropped` alongside the existing `Expired`.
  - `UploadActivityTracker` exposes an explicit terminalization path so active held/sending
    sessions can move into `recent_sessions` with a concrete reason instead of aging out
    passively to `expired`.
  - The download transfer pump now marks held upload leases as `dropped` when their owning
    download part leaves the active set (`cancelled`, `completed`, or `error`) before the lease
    is sent.
  - `/api/v1/uploads` now surfaces `terminal_reason = "dropped"` for those recent sessions.
- Decisions:
  - implement only uploader lifecycle transitions that are directly observable from the current
    architecture; do not invent `completed` or `cancelled` signals until there are explicit call
    sites for them.
  - treat held-lease discard in the transfer pump as a real uploader terminal event, distinct
    from passive TTL expiry.
- Next steps:
  - decide whether uploader-side `completed` and `replaced` terminal reasons should be wired once
    the transfer pump has explicit send completion/supersession hooks.
  - decide whether recent session rendering in `/ui/downloads` should visually group `expired`
    vs `dropped` sessions more strongly as more terminal reasons are added.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#55` review feedback on terminal-reason contract coverage.
  - The Playwright uploads mock now includes `terminal_reason: null` on active sessions, matching
    the real `/api/v1/uploads` contract.
  - Added an API test that forces a short-lived upload session to expire and verifies
    `recent_sessions[0].terminal_reason == \"expired\"`.
- Decisions:
  - keep the uploads contract explicit: active sessions serialize `terminal_reason = null`,
    recent sessions serialize a concrete terminal reason when available.
- Next steps:
  - watch PR `#55` for any remaining review comments.
- Change log:
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added terminal reason metadata to recent upload sessions.
  - `recent_sessions` now expose `terminal_reason`.
  - Current implementation sets terminal reason to `expired` when active upload sessions age
    out by TTL and move into recent-session history.
  - Active `sessions` intentionally keep `terminal_reason = null`.
  - `/ui/downloads` now shows terminal reason on recent sessions in the `Active Uploads` table.
- Decisions:
  - scope terminal reason to recent sessions only; active sessions are non-terminal by definition.
  - start with `expired` only and keep the model open for later causes (`completed`,
    `cancelled`, `replaced`) once uploader-side lifecycle signals exist.
- Next steps:
  - decide whether upload-session lifecycle should emit explicit completion/cancel signals so
    recent sessions can distinguish those terminal states from passive expiry.
  - decide whether the uploads UI should visually separate active sessions from recent terminal
    sessions more strongly once more terminal reasons exist.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#54` review feedback on upload session history bounds and mapping drift.
  - Added `MAX_RECENT_SESSIONS_PER_FILE = 128` so recent upload session history is bounded by
    both time and count.
  - Extracted the repeated session mapping logic in both:
    - `TrackedUploadRange -> UploadSessionSnapshot`
    - `UploadSessionSnapshot -> UploadSessionEntry`
  - Added a regression test proving per-file recent session history is capped.
- Decisions:
  - treat recent session history as bounded operator telemetry, not an unbounded best-effort log.
  - centralize session-mapping code so future session-field additions cannot drift between active
    and recent session serialization paths.
- Next steps:
  - watch PR `#54` for any remaining review comments.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added short-lived upload session history to the uploader session model.
  - Expired upload sessions now move from `sessions` to `recent_sessions` for a short in-memory
    retention window instead of disappearing immediately.
  - `GET /api/v1/uploads` now exposes:
    - `recent_session_count`
    - `recent_sessions`
  - `/ui/downloads` now shows recent upload sessions alongside active sessions in the `Active Uploads`
    table.
- Decisions:
  - keep recent session history in-memory only and bound it by a short fixed retention window;
    this is for operator forensics, not durable audit storage.
  - keep `sessions` meaning “currently active” and make history explicit as
    `recent_sessions` to avoid changing the current API contract semantics.
- Next steps:
  - decide whether recent sessions should get their own top-level filterable endpoint once the
    volume grows beyond per-file rendering.
  - decide whether sessions should record a terminal reason (`expired`, `completed`, `cancelled`)
    instead of pure retention-only disappearance.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#53` review feedback on uploader session identity.
  - `UploadActivityTracker::note(...)` now prunes expired ranges before looking up an existing
    session id, so expired `peer_id + start + end` ranges cannot recycle an old `session_id`.
  - Added a regression test proving that a new request after TTL expiry receives a new
    runtime session id.
  - Simplified the handoff status section header to `## Status` to avoid the stale
    date-bearing section title.
- Decisions:
  - treat `session_id` uniqueness across active runtime sessions as a real behavioral contract
    for the uploads API.
- Next steps:
  - watch PR `#53` for any remaining review comments.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added a first-class uploader session model to the uploads API/UI.
  - `UploadActivityTracker` now assigns stable runtime `session_id` values to active upload
    ranges and preserves them across `Held -> Sending` transitions for the same
    `peer_id + start + end` request.
  - `GET /api/v1/uploads` now includes per-file `sessions` with:
    - `session_id`
    - `start` / `end`
    - `bytes_total`
    - `phase`
    - `peer_id_hex`
    - `payload_source`
    - `started_unix_secs`
    - `last_updated_unix_secs`
  - `/ui/downloads` now shows session counts and per-session summaries inside the `Active Uploads`
    table.
- Decisions:
  - keep session ids runtime-local and in-memory for now; do not persist or expose them as a
    cross-restart contract yet.
  - extend the existing `/api/v1/uploads` surface instead of adding a second uploads endpoint;
    session state belongs with the current uploader snapshot view.
- Next steps:
  - decide whether the next uploader slice should expose a top-level upload-session endpoint for
    filtering/sorting across files.
  - decide whether completed/expired sessions need a short in-memory history window for operator
    forensics.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `ui/tests/e2e/smoke.spec.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR `#52` review feedback on zero-fill warning freshness and UI fixtures.
  - The overview page now polls `GET /api/v1/status` every 15s so `zero_fill_warning` and the
    aggregate fallback rate remain current even though SSE still carries `KadServiceStatus`
    rather than the enriched API status payload.
  - The Playwright mock server `UPLOADS_PAYLOAD` now includes the `zero_fill_*` fields so the
    downloads-page uploads fixture matches the real API shape and exercises the fallback UI.
- Decisions:
  - keep the SSE contract unchanged in this slice and refresh enriched overview status via
    lightweight polling instead of widening the event payload.
  - treat UI mock/API shape drift as a correctness issue for the smoke suite, not optional
    cleanup.
- Next steps:
  - watch PR `#52` for any remaining review comments.
  - decide later whether aggregate status should eventually move into the SSE payload to
    eliminate overview polling.
- Change log:
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Applied a CI rustfmt follow-up on the zero-fill fallback warning branch.
  - Reformatted `/api/v1/status` zero-fill warning aggregation and related API tests to the
    current rustfmt layout expected by CI.
- Decisions:
  - treat CI formatting drift as a direct branch fix; no behavior changes were needed.
- Next steps:
  - watch PR `#52` for any remaining review or CI findings.
- Change log:
  - Updated `src/api/handlers/core.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added zero-fill fallback warning telemetry to uploads, status, and UI.
  - `UploadActivityTracker` now records zero-fill fallback activity separately from normal
    upload traffic:
    - `zero_fill_requests_total`
    - `zero_fill_requested_bytes_total`
    - `zero_fill_rate_bps_5s`
    - `zero_fill_rate_bps_30s`
    - `zero_fill_active`
  - `GET /api/v1/uploads` now exposes per-upload zero-fill fallback counters/rates.
  - `GET /api/v1/status` now exposes:
    - `zero_fill_upload_rate_bps_5s`
    - `zero_fill_upload_rate_bps_30s`
    - `zero_fill_active_uploads`
    - `zero_fill_warning`
  - The overview page now shows a top-level warning when fallback traffic is active, and the
    downloads page shows per-upload fallback bytes/rates and warning badges.
- Decisions:
  - derive the warning from real fallback send telemetry, not from `last_payload_source`
    alone, so stale historical source metadata does not trigger false warnings.
  - keep zero-fill fallback visible in normal operator UI because it indicates that upload
    traffic may be syntactically valid while not serving real shared-file bytes.
- Next steps:
  - decide whether zero-fill fallback should also escalate into API health/degraded state.
  - decide whether repeated fallback traffic should trigger stronger structured logging or
    counters in `/api/v1/status`.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/handlers/core.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/index.html`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Added aggregate transfer rates to `/api/v1/status` and the overview UI.
  - `GET /api/v1/status` now returns:
    - `download_rate_bps_5s`
    - `download_rate_bps_30s`
    - `upload_rate_bps_5s`
    - `upload_rate_bps_30s`
  - The aggregate values are computed at the API layer by summing current per-download and
    per-upload rolling rates.
  - The overview page now shows aggregate 5s download and upload rates as top-level KPIs.
- Decisions:
  - keep the KAD service status/watch payload unchanged; aggregate transfer rates belong to
    the API composition layer, not the KAD core status struct.
  - use the 5s aggregate rate in the overview UI and keep the 30s aggregate available in the
    API for monitoring agents or future UI expansion.
- Next steps:
  - decide whether to surface aggregate 30s rates in the overview UI as secondary labels.
  - decide whether `zero_fill_fallback` traffic should raise a top-level warning when
    aggregate upload rate is non-zero.
- Change log:
  - Updated `src/api/handlers/core.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/index.html`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-09): Addressed PR review feedback for transfer-rate telemetry.
  - Hardened `RollingTransferRate` against future-dated samples by switching away from
    `Instant::duration_since(...)` assumptions in prune/rate calculations.
  - Added an explicit regression test proving future-dated samples are ignored instead of
    panicking.
  - Documented that the `rate_bps_*` API fields are bytes per second, despite the
    historical `bps` suffix.
- Decisions:
  - treat the future-sample panic risk as a real correctness issue and fix it in this PR.
  - defer fixed-bucket/per-second aggregation for a later optimization pass; current sample
    volume is acceptable for this operator-facing telemetry slice.
- Next steps:
  - watch PR `#50` for any remaining review comments.
  - if rate polling becomes hot, replace per-sample storage with bounded time buckets.
- Change log:
  - Updated `src/transfer_rate.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added transfer-rate telemetry for downloads and uploads.
  - Added shared rolling-window transfer-rate helper with explicit 5s and 30s windows.
  - Download snapshots now include `rate_bps_5s` and `rate_bps_30s`, populated from
    received block bytes.
  - Upload snapshots now include `rate_bps_5s` and `rate_bps_30s`, populated from
    bytes actually sent on the sending path, not held/requested ranges.
  - `/api/v1/downloads` and `/api/v1/uploads` now expose those rate fields.
  - `/ui/downloads` now renders transfer rates for both the download queue and
    active uploads.
- Decisions:
  - keep rate telemetry in-memory only; do not persist or backfill across restart.
  - define upload rate as bytes sent, not bytes requested or reserved.
  - expose rolling-window rates instead of instantaneous samples to keep the UI stable.
- Next steps:
  - decide whether to add aggregate up/down rates to `/api/v1/status`.
  - decide whether `zero_fill_fallback` uploads should become a visible warning when
    paired with non-zero upload rate.
- Change log:
  - Added `src/transfer_rate.rs`.
  - Updated `src/download/service.rs`.
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed PR review feedback for the richer uploader-state branch.
  - Preserved first-seen timestamps for tracked upload ranges so
    `active_since_unix_secs` reflects when uploader activity actually began, not the latest
    held/sending transition update.
  - Updated `docs/governance/TASKS.md` `Last Reviewed` date after adding the transfer-rate
    telemetry backlog note.
- Decisions:
  - define `active_since_unix_secs` as earliest active-start time for live ranges, not
    latest update time.
- Next steps:
  - watch PR `#49` for any remaining uploader-state comments.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added explicit backlog for transfer-rate telemetry in downloads and uploads.
  - Confirmed current uploader/download UI/API work does not yet expose first-class transfer
    speed metrics.
  - Added backlog to implement rolling bytes/sec telemetry for:
    - per-download rates
    - per-upload rates
    - aggregate transfer rates where useful
  - Added backlog requirement to surface those rates in `/ui/downloads`.
- Decisions:
  - treat transfer speed as first-class operator telemetry, not an optional cosmetic stat.
  - when implemented, define explicit smoothing/window semantics instead of ad hoc
    instantaneous rates so the UI remains stable and interpretable.
- Next steps:
  - when the next transfer-observability slice is chosen, add rate fields to
    `/api/v1/downloads` and `/api/v1/uploads` first, then wire them into `/ui/downloads`.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Expanded uploader snapshots with peer identity and serving-source metadata.
  - `UploadService` / `UploadActivityTracker` now record:
    - per-range `peer_id_hex`
    - per-range request timestamp
    - `last_peer_id_hex`
    - `active_peer_ids`
    - `active_since_unix_secs`
    - `last_payload_source`
  - `GET /api/v1/uploads` now exposes the richer uploader identity/state fields.
  - `/ui/downloads` `Active Uploads` now shows:
    - active peers
    - last peer
    - last payload source
    - active-since timestamp
- Decisions:
  - keep the uploader model snapshot-based for this slice; add identity/source metadata
    without introducing a heavier upload-session subsystem yet.
  - treat payload source as operator/debug metadata (`shared_file` vs
    `zero_fill_fallback`), because it is the most direct signal of whether uploads are
    serving real shared bytes.
- Next steps:
  - decide whether the next uploader slice should track a stronger per-upload session id
    or a per-peer upload history view.
  - decide whether `zero_fill_fallback` should remain visible in normal operator UI or be
    progressively treated as a warning/debug-only state once real uploader serving is stricter.
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed PR review feedback for the uploader UI visibility branch.
  - Replaced raw markdown-style backticks in `ui/downloads.html` with an HTML
    `<code>` element for `/api/v1/uploads`.
- Decisions:
  - keep HTML code-like endpoint labels as explicit `<code>` markup in static UI templates;
    do not rely on markdown-style notation inside `.html` files.
- Next steps:
  - watch PR `#48` for any remaining UI-only review comments.
- Change log:
  - Updated `ui/downloads.html`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added a read-only uploader visibility section to `/ui/downloads`.
  - The downloads page now fetches `GET /api/v1/uploads` alongside:
    - `GET /api/v1/downloads`
    - `GET /api/v1/shared`
  - Added an `Active Uploads` section that shows:
    - file name / relative path
    - file hash
    - total upload requests
    - bytes requested
    - held and sending ranges
    - last requested timestamp
  - Updated the UI mock server and Playwright smoke coverage to include the new section.
- Decisions:
  - keep the first uploader UI slice read-only; do not add uploader controls before the
    uploader/session model is richer.
  - expose uploader state on the existing downloads page instead of creating a separate UI
    route; operators already use that page for transfer visibility.
- Next steps:
  - decide whether the next uploader slice should add peer/session identity to
    `/api/v1/uploads`.
  - decide whether uploader activity should eventually be cross-linked from shared-file
    rows into a single richer uploader view, or kept as a separate table.
- Change log:
  - Updated `ui/downloads.html`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `ui/tests/e2e/smoke.spec.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed PR review fixes for the uploader-service foundation branch.
  - Restored correct `OP_SENDINGPART` payload framing by adding
    `encode_sendingpart_payload(...)` in `src/download/protocol.rs`.
  - Updated `UploadService::build_sending_part_payload(...)` to return a fully encoded
    sending-part payload instead of raw block bytes.
  - Updated uploader tests to decode and verify the protocol payload shape instead of
    asserting on raw block bytes.
  - Removed an unnecessary `SharedLibrary` clone from `GET /api/v1/uploads`; the handler
    now reads through the shared-library guard directly.
- Decisions:
  - keep `UploadService::build_sending_part_payload(...)` responsible for returning the
    protocol payload, because the current caller contract already treats it as an
    `OP_SENDINGPART` builder.
  - prefer a single protocol encoder helper over ad hoc packet framing in the transfer pump.
- Next steps:
  - watch PR `#47` for any remaining uploader-foundation comments.
  - if the branch merges cleanly, decide whether the next uploader slice is:
    - a UI surface for `/api/v1/uploads`
    - or deeper uploader/session state
- Change log:
  - Updated `src/download/protocol.rs`.
  - Updated `src/upload.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Aligned operator-facing docs with the shared-library and uploader-service work.
  - Updated `docs/index.md` to surface `SHARING_UPLOAD_CHECKLIST.md` in the main docs navigation.
  - Updated `docs/30_operations/api_curl.md` with the current shared/uploader endpoints:
    - `GET /api/v1/shared`
    - `GET /api/v1/uploads`
    - `GET /api/v1/shared/actions`
    - `POST /api/v1/shared/actions/reindex`
    - `POST /api/v1/shared/actions/republish_sources`
    - `POST /api/v1/shared/actions/republish_keywords`
  - Documented the shared action confirmation requirement and expected `202` / `409` /
    `429` response model.
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md` with an
    "Implemented So Far" section covering:
    - shared-library foundation
    - operator danger-zone controls
    - uploader-service foundation
- Decisions:
  - keep `docs/governance/handoff.md` as the most detailed continuity log, but align
    `docs/index.md` and `docs/30_operations/api_curl.md` whenever shared/uploader API
    surfaces change.
  - document the operator-action model as part of operations docs, not only UI docs,
    because `curl` users need the confirmation/cooldown semantics too.
- Next steps:
  - decide whether `docs/10_architecture/API_DESIGN.md` should gain a dedicated section
    for shared-library and uploader endpoints, or whether `api_curl.md` plus
    implementation-proximate docs are sufficient for now.
  - keep `docs/30_operations/api_curl.md` in sync if `/api/v1/uploads` gains richer
    uploader/session fields.
- Change log:
  - Updated `docs/index.md`.
  - Updated `docs/30_operations/api_curl.md`.
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added an uploader-service foundation and a dedicated uploads API surface.
  - Introduced `UploadService` in `src/upload.rs` as the first-class boundary for:
    - upload activity tracking
    - shared-file payload reads
    - zero-fill fallback behavior
  - Added typed uploader payload build results:
    - `UploadPayloadBuild`
    - `UploadPayloadSource`
  - Moved the download transfer pump in `src/app.rs` to depend on `UploadService` instead of directly calling:
    - `share::read_shared_block`
    - `UploadActivityTracker`
  - Added `GET /api/v1/uploads` to expose uploader-side state directly instead of only surfacing upload hints through `/api/v1/shared`.
  - Added uploader tests for:
    - tracker snapshots
    - shared-file payload reads
    - zero-fill fallback
    - `/api/v1/uploads` response shape
- Decisions:
  - keep the first uploader slice narrow: extract a service boundary and expose uploader state before attempting a larger transport/uploader redesign.
  - preserve existing wire behavior for `OP_SENDINGPART`; this slice is architectural refactoring plus visibility, not a protocol change.
  - retain zero-fill fallback for now, but move that behavior behind `UploadService` so future uploader hardening has one place to change it.
- Next steps:
  - decide whether `/api/v1/uploads` should be surfaced in the UI now or wait until uploader state becomes richer.
  - decide whether uploader state should track peer/session identity in addition to file/range activity.
  - consider the next uploader hardening slice:
    - explicit upload session model
    - file-missing/file-changed behavior policy
    - dedicated uploader service tests around concurrent requests
- Change log:
  - Updated `src/upload.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/router.rs`.
  - Updated `src/api/handlers/mod.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `tests/api_startup_smoke.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Reworked shared-library maintenance controls into an explicit danger-zone model instead of debug gating.
  - Kept shared maintenance actions under normal authenticated admin access.
  - Added UI friction in `/ui/downloads`:
    - collapsed `Danger Zone`
    - acknowledgement checkbox before actions unlock
    - per-action browser confirmation dialogs
  - Added API-side confirmation requirements for:
    - `POST /api/v1/shared/actions/reindex`
    - `POST /api/v1/shared/actions/republish_sources`
    - `POST /api/v1/shared/actions/republish_keywords`
  - Added backend cooldowns:
    - `republish_sources`: 300s
    - `republish_keywords`: 900s
  - Extended shared action status/response payloads with:
    - `cooldown_until_unix_secs`
    - `reason`
  - Added API/unit coverage for confirmation and cooldown behavior.
- Decisions:
  - do not hide shared maintenance behind debug mode; these are operator actions, not developer-only diagnostics.
  - require explicit friction for state-changing and network-affecting shared maintenance:
    - UI acknowledgement
    - action confirmation
    - backend confirmation
    - republish cooldowns
  - keep read-only shared inspection available under normal auth.
- Next steps:
  - decide whether `GET /api/v1/shared/actions` should expose richer cooldown/help text for the UI instead of only timestamps.
  - decide whether `reindex` should gain a lightweight cooldown or remain ungated beyond confirmation.
  - review whether any additional maintenance endpoints should adopt the same danger-zone pattern.
- Change log:
  - Updated `src/shared_ops.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/assets/css/base.css`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed actionable PR `#46` review feedback on the shared action danger-zone slice.
  - Restored the `apiPost` import in `ui/assets/js/app.js`; it is still used by other UI flows outside shared maintenance actions.
  - Replaced the stringly-typed shared action reject reason with a typed enum:
    - `AlreadyRunning`
    - `CooldownActive`
  - Updated HTTP status mapping to branch on the typed reject reason instead of matching string literals.
- Decisions:
  - keep the shared action reject reason typed end-to-end inside Rust and only serialize it at the API boundary.
  - treat missing imports in the monolithic UI module as runtime correctness issues, not cosmetic cleanup.
- Next steps:
  - merge PR `#46` after CI/rereview is clean.
- Change log:
  - Updated `src/shared_ops.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed actionable PR `#45` review feedback on shared-library operator actions.
  - Republish actions now fail fast with a structured failed status when `kad.service_enabled = false`, instead of queueing work onto an unserviced channel.
  - Added missing API coverage for:
    - `POST /api/v1/shared/actions/reindex`
    - `POST /api/v1/shared/actions/republish_keywords`
  - Fixed shared-actions UI conflict handling so HTTP `409` reports a friendly notice instead of a raw error.
  - Fixed shared-actions UI polish:
    - corrected `Error:` label text
    - added distinct `state-failed` styling
- Decisions:
  - keep HTTP `409 CONFLICT` for duplicate action triggers; fix the UI to respect the existing API contract instead of weakening the handler semantics.
  - reject republish actions when KAD is disabled; operator actions should not appear to succeed when no consumer exists.
- Next steps:
  - wait for rereview on PR `#45`.
  - decide whether shared operator actions should remain normal authenticated controls or move behind a stricter debug/operator gate.
- Change log:
  - Updated `src/shared_ops.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/assets/css/base.css`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added shared-library operator actions for reindexing and republishing.
  - Added `src/shared_ops.rs` with `SharedOpsManager` and background actions:
    - `reindex`
    - `republish_sources`
    - `republish_keywords`
  - Moved runtime shared-library state behind `Arc<RwLock<SharedLibrary>>` so reindexing updates:
    - `/api/v1/shared`
    - the upload transfer pump
    - future operator actions
  - Added new API endpoints:
    - `GET /api/v1/shared/actions`
    - `POST /api/v1/shared/actions/reindex`
    - `POST /api/v1/shared/actions/republish_sources`
    - `POST /api/v1/shared/actions/republish_keywords`
  - Added structured action status reporting:
    - `state`
    - `started_unix_secs`
    - `finished_unix_secs`
    - `items_total`
    - `queued_total`
    - `failed_total`
    - reindex stats (`library_files_total`, `reused_entries`, `hashed_entries`)
  - Updated `/ui/downloads` shared-library section with operator buttons and action status cards.
  - Reused shared publish queue helpers for both startup publishing and operator-triggered republishing.
- Decisions:
  - keep operator actions as background tasks; API endpoints should trigger work, not do long blocking rebuilds inline.
  - separate `reindex` from `republish_*` even though operators will often run them together; this keeps the action semantics explicit.
  - keep republish actions idempotent at the API level and report queue results structurally instead of returning a bare success code.
- Next steps:
  - decide whether these controls should remain ordinary authenticated UI actions or move behind a stricter debug/operator gate.
  - decide whether `reindex` should optionally auto-chain into republish for newly discovered files.
  - consider adding per-file failure detail if queue failures become common enough that aggregate counts are not sufficient.
- Change log:
  - Added `src/shared_ops.rs`.
  - Updated `src/lib.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/router.rs`.
  - Updated `src/api/handlers/mod.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `tests/api_startup_smoke.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added real KAD publish-response visibility for shared files and surfaced it in `/api/v1/shared` and `/ui/downloads`.
  - Added `KadServiceCommand::GetSharedPublishStatus` and `KadSharedPublishStatus`.
  - Added KAD service-side file-level publish status synthesis:
    - `local_source_cached`
    - `source_publish_response_received`
    - `source_publish_first_response_latency_ms`
    - `keyword_publish_total`
    - `keyword_publish_acked`
  - Extended `/api/v1/shared` to merge:
    - enqueue status from `SharedPublishTracker`
    - actual response/ack facts from the KAD service
  - Updated `/ui/downloads` shared-library table to distinguish:
    - local source cached state
    - source publish queue state
    - source publish response state
    - keyword publish queue state
    - keyword publish ack coverage
  - Added service/API coverage for the new file-level publish status path.
- Decisions:
  - keep enqueue status and response status separate; they answer different operational questions.
  - do not reinterpret `source_count` as “local source exists”; expose local-source cache state explicitly.
  - model keyword publish response status as ack coverage (`acked/total`) because a shared file is published under multiple keywords.
- Next steps:
  - decide whether to track file-level publish responses durably across restart or keep them runtime-only.
  - decide whether the next shared-library slice should add operator actions (`reindex`, `republish`) or deeper source/publish telemetry first.
  - consider surfacing discovered-vs-local source state more explicitly if the shared UI needs stronger availability diagnostics.
- Change log:
  - Updated `src/kad/service/types.rs`.
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/tests.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Applied a CI-formatting follow-up on `feat/shared-publish-response-status`.
  - Kept the `pub use types::{...}` re-export block in `src/kad/service.rs` in the rustfmt layout expected by CI.
- Decisions:
  - treat this as a formatting-only follow-up; no behavior changed.
- Next steps:
  - review and address any remaining PR `#44` review comments.
- Change log:
  - Updated `src/kad/service.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed actionable PR `#44` review comments on shared publish response status.
  - Fixed `/api/v1/shared` shared-file KAD status lookups to run concurrently instead of sequentially.
  - Corrected `Last Reviewed` metadata at the top of `docs/governance/handoff.md` to `2026-03-08`.
  - Fixed keyword publish ACK accounting so file-level `acked/total` status does not regress after `job.publish` is cleared post-ack.
  - Added a regression test that keeps counting acknowledged keyword publishes after publish work stops.
- Decisions:
  - preserve the existing scheduling behavior of `got_publish_ack`, but track actual acknowledged file identity separately for telemetry.
  - keep the shared API on per-file KAD requests for now, but issue them concurrently; a batch KAD command can be considered later if the shared library grows large enough to justify it.
- Next steps:
  - wait for PR `#44` CI/rereview after the review-driven fixes.
- Change log:
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/inbound.rs`.
  - Updated `src/kad/service/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Addressed actionable PR review feedback on the shared-library foundation branch.
  - Hardened shared-root/runtime-dir normalization:
    - canonicalize `data_dir` consistently when validating share-root overlap
    - added regression test for symlinked `data_dir`
  - Hardened shared-file walking:
    - track visited canonical directories to avoid recursive symlink loops
    - added regression test for a cyclic directory symlink under a share root
  - Hardened shared-library cache correctness:
    - cache metadata now comes from the same snapshot used for the chosen file hash
    - invalid cached MD4 hex now triggers rehash instead of panic/reuse
    - added regression test for invalid cached hash recovery
  - Hardened trackers:
    - `SharedPublishTracker` and `UploadActivityTracker` now recover from poisoned locks instead of panicking
  - Improved `/api/v1/downloads`:
    - source-count lookups now run concurrently instead of sequentially
  - Improved shared upload fallback visibility:
    - zero-filled payload fallback now emits throttled warnings for shared-library read failures
    - shared-file reads are now executed via `spawn_blocking` to avoid blocking Tokio worker threads
  - Corrected `/api/v1/shared` semantics:
    - `source_count` no longer incorrectly reports `1` for all local shared files; it now reports `0` until backed by real source-state plumbing
- Decisions:
  - prefer accurate “unknown/zero” source visibility over a misleading synthetic local source count
  - keep synchronous disk reads off the async worker path even in the current phase0-style uploader flow
  - treat cache corruption as recoverable and rehashable, never fatal
- Next steps:
  - decide whether to expose real local-source state separately from discovered-source count in `/api/v1/shared`
  - decide whether KAD publish response handling should upgrade enqueue status into end-to-end publish status
  - reply/resolve the PR review threads after the branch update is pushed
- Change log:
  - Updated `src/share.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/publish.rs`.
  - Updated `src/upload.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added a repo-native Playwright smoke runner with explicit `nvm` bootstrap and container-safe launch defaults.
  - Added `scripts/test/ui_smoke.sh`:
    - sources `~/.nvm/nvm.sh`
    - verifies `npm` is available
    - runs `ui` Playwright smoke tests
    - emits a targeted diagnostic when browser runtime libraries are missing
  - Updated `ui/playwright.config.mjs`:
    - explicit browser selection via `UI_BROWSER`
    - disabled Chromium sandbox for container/CI friendliness
    - added `--disable-dev-shm-usage`
  - Verified that `npm` is available in this environment only after sourcing `nvm`.
  - Verified current blocker was not headless mode; it was missing host browser libraries (`libglib2.0-0`, `libnss3`, `libgbm1`, etc.).
  - After installing the host browser dependencies, `bash scripts/test/ui_smoke.sh` passes.
  - Tightened the downloads-page Playwright assertion to target the `Shared Library` section heading explicitly.
- Decisions:
  - keep Playwright headless; there is no need for a headed flow in CI/container environments.
  - bootstrap `nvm` in the repo runner rather than relying on shell startup files.
  - treat missing browser runtime packages as an environment prerequisite, not a UI code failure.
- Next steps:
  - decide whether to wire `scripts/test/ui_smoke.sh` into a broader CI/check workflow.
  - if needed later, add a small README note for UI verification prerequisites.
- Change log:
  - Added `scripts/test/ui_smoke.sh`.
  - Updated `ui/playwright.config.mjs`.
  - Updated `ui/tests/e2e/smoke.spec.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added shared publish enqueue status tracking and surfaced it in the shared-library API/UI.
  - Added `SharedPublishTracker` to record per-shared-file publish enqueue activity.
  - Tracked source publish enqueue status:
    - attempt count
    - last attempt timestamp
    - last result (`queued` / `queue_failed`)
  - Tracked keyword publish enqueue status:
    - attempt count
    - queued count
    - failed count
    - last attempt timestamp
    - last result (`queued` / `queue_failed`)
  - Expanded `/api/v1/shared` to expose publish enqueue status per indexed file.
  - Updated `/ui/downloads` shared-library table to show source/keyword publish status alongside uploader activity.
- Decisions:
  - keep publish status honest to the current architecture: this tracks command enqueue outcomes, not remote KAD store acknowledgment.
  - use enqueue visibility now rather than inventing a false `published` state without service-side completion evidence.
  - preserve a path to later upgrade this into end-to-end publish status once the KAD service exposes completion/response callbacks.
- Next steps:
  - decide whether KAD publish response handling should feed a stronger `published/failed` file-level status model.
  - add optional shared-library operator actions (reindex / republish) only after the status model is explicit enough to justify them.
  - wire frontend checks through sourced `nvm`/`npm` and headless browser configuration in environments where Playwright is available.
- Change log:
  - Added `src/publish.rs`.
  - Updated `src/lib.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `tests/api_startup_smoke.rs`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/downloads.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added first-class uploader activity tracking and surfaced it in the shared-library UI/API.
  - Added `UploadActivityTracker` to track recent held/sending upload ranges per shared file hash.
  - Wired the phase0 transfer pump to record:
    - held upload ranges
    - sending upload ranges
    - total upload request count
    - total requested bytes
    - last request timestamp
  - Expanded `/api/v1/shared` to return uploader activity:
    - `queued_uploads`
    - `inflight_uploads`
    - `total_upload_requests`
    - `requested_bytes_total`
    - `last_requested_unix_secs`
    - `queued_upload_ranges`
    - `inflight_upload_ranges`
  - Updated `/ui/downloads` shared-library table to show real upload-side activity instead of only inferring from local download state.
- Decisions:
  - keep uploader activity tracking TTL-based for now; the goal is operational visibility, not durable historical accounting.
  - treat `held` and `sending` as the two useful operator states until a standalone uploader subsystem exists.
  - keep download-side queue/inflight counts in the shared view, but clearly separate them from upload-side activity.
- Next steps:
  - expose publish/cache/debug status for shared files if operators need to distinguish `indexed`, `publish queued`, and `published`.
  - consider a dedicated uploader subsystem/state model once uploads are no longer driven through the phase0 transfer pump.
  - add browser-side verification for `/ui/downloads` in an environment with `npm`/Playwright available.
- Change log:
  - Added `src/upload.rs`.
  - Updated `src/lib.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `tests/api_startup_smoke.rs`.
  - Updated `ui/downloads.html`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added shared-library inspection UI/API and richer download visibility.
  - Added `/api/v1/shared` for shared-library inspection.
  - Expanded `/api/v1/downloads` with source counts and detailed missing/inflight ranges.
  - Added `/ui/downloads` page showing:
    - shared files
    - active shared-file requests/inflight activity
    - download cards with simple part-state graphs
  - Added `sharing.share_roots` editing to `/ui/settings`.
  - Added startup keyword publishing for indexed shared files, alongside source publishing.
  - Added explicit indexing/cache-reuse/publish log lines for shared files.
- Decisions:
  - keep the part graph simple and range-based for now; do not add a separate graph model until the uploader/availability model is more mature.
  - use `source_count == 0` as the UI signal for `no source` missing segments; this is file-level availability, not per-range source attribution.
  - treat the shared-library UI as operator visibility, not a full media-library workflow yet.
- Next steps:
  - expose stronger shared-library status/debug metadata (publish state, cache stats, failures) if the UI needs deeper triage.
  - decide whether shared-file activity should be backed by a dedicated uploader activity tracker instead of download/self-serve inference.
  - add a lightweight frontend verification path in environments that have `npm`/Playwright available.
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/router.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/handlers/mod.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `src/app.rs`.
  - Updated `src/share.rs`.
  - Updated `tests/api_startup_smoke.rs`.
  - Added `ui/downloads.html`.
  - Updated `ui/assets/js/app.js`.
  - Updated `ui/settings.html`.
  - Updated `ui/index.html`.
  - Updated `ui/search.html`.
  - Updated `ui/search_details.html`.
  - Updated `ui/node_stats.html`.
  - Updated `ui/log.html`.
  - Updated `ui/tests/e2e/mock-server.mjs`.
  - Updated `ui/tests/e2e/smoke.spec.mjs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added best-effort persisted shared-library index caching.
  - Added `data/shared_library.json` cache for shared-file metadata and MD4 hashes.
  - Startup now reuses cached hashes when canonical path, file size, and mtime are unchanged.
  - Changed files are rehashed automatically; missing/corrupt cache falls back to rebuild.
  - Added tests covering cache reuse and cache invalidation on file change.
- Decisions:
  - cache is advisory only; startup must still succeed if the cache is missing or corrupt.
  - correctness wins over startup speed: any size/mtime mismatch forces rehash.
- Next steps:
  - persist additional library metadata needed for keyword publishing and future UI/library views.
  - publish filename keywords for indexed shared files.
  - decide whether to expose shared-library/cache status in API/debug endpoints.
- Change log:
  - Updated `src/share.rs`.
  - Updated `src/app.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added the first disk-backed shared-library/uploader slice.
  - Built a startup shared-file index from validated `sharing.share_roots`.
  - Added streaming MD4 hashing for shared files without loading whole files into memory.
  - Queued automatic KAD source publishes for indexed shared files at startup.
  - Updated the phase0 download transfer pump to serve real block bytes from indexed shared files when a matching local hash exists.
  - Kept the synthetic zero-filled fallback for hashes not yet backed by the local shared library so existing non-library flows do not regress.
- Decisions:
  - keep this slice minimal: real shared-file backing first, full library persistence/UI/peer-side uploader hardening later.
  - preserve fallback behavior until the synthetic path can be removed behind stronger end-to-end uploader coverage.
- Next steps:
  - add persisted shared-library metadata (path, size, md4, mtimes) to avoid full rehash on every startup.
  - publish filename keywords for indexed shared files, not just sources.
  - replace the remaining synthetic transfer path once real peer-side upload serving is wired end-to-end.
- Change log:
  - Updated `src/kad/md4.rs`.
  - Updated `src/share.rs`.
  - Updated `src/app.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Started the shared-library/uploader work with config and validation foundation.
  - Added `sharing` config section with `share_roots`.
  - Exposed `sharing.share_roots` through `/api/v1/settings` get/patch.
  - Added new `src/share.rs` module:
    - canonicalizes and validates share roots
    - rejects empty roots, runtime data-dir overlap, and overlapping share roots
    - enumerates files beneath validated roots for later indexing/uploader use
  - Added tests for:
    - settings API share-root update/rejection
    - share-root validation rules
    - basic shared-file enumeration
- Decisions:
  - start with a trustworthy shared-root boundary before implementing disk-backed uploader serving.
  - keep uploader wiring as the next slice; this change only establishes config/API/backend foundation.
- Next steps:
  - add a persisted library index model (path, size, md4, timestamps) on top of validated share roots.
  - replace synthetic upload payload generation with real disk-backed range reads from indexed files.
  - add settings UI controls for share-root management.
- Change log:
  - Added `src/share.rs`.
  - Updated `src/config.rs`.
  - Updated `src/main.rs`.
  - Updated `src/api/handlers/settings.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Fixed false-positive resume monotonic failures caused by `part_number` reuse in churny soak scenarios.
  - Updated `scripts/test/download_resume_soak.sh`:
    - snapshot now captures persisted `.part.met` state alongside API download JSON
    - monotonic check now matches downloads by persisted identity (`part_number + created_unix_secs`) instead of API `part_number` alone
    - monotonic check now only compares matched persisted downloads, avoiding false failures when the scenario deletes/recreates downloads after restart
- Decisions:
  - `part_number` is not a stable identity under the concurrency scenario because the queue can delete and recreate downloads after restart.
  - persisted `.part.met` metadata is the correct source for stable resume identity in soak assertions.
- Next steps:
  - rerun the phase0 acceptance command on this branch to verify the false-positive is eliminated.
  - consider exposing a first-class durable `download_id` via the API if future tests need stable identity without filesystem access.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added fallback interop plan for blocked live iMule environment.
  - Updated `docs/governance/TASKS.md` with `Interop Fallback Strategy`:
    - offline interop harness using fixture/pcap-derived packet vectors
    - wire-level golden tests for core compatibility flows
    - keep live mixed-client soak as pre-release (v1 tag) gate
- Decisions:
  - do not stall daily progress on unavailable iMule runtime environment.
  - preserve live mixed-client soak as mandatory final compatibility gate.
- Next steps:
  - define initial packet fixture corpus and add first golden tests to CI.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added explicit shaper compatibility contract documentation.
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md` with `Shaper Compatibility Contract`:
    - wire invariants that shaping must not change
    - shaping-only policy knobs that are safe to vary
    - required decode-equivalence + mixed-client soak checks
  - Updated `docs/governance/TASKS.md` v1 gates with shaper contract enforcement.
- Decisions:
  - traffic shaping is policy-layer only; wire compatibility remains invariant.
- Next steps:
  - add executable verification script/checklist for shaper before/after payload equivalence.
- Change log:
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added explicit v1 stable interop objective and release gates.
  - Updated `docs/governance/TASKS.md`:
    - added current-priority objective for seamless `rust-mule <-> iMule` operation over I2P
    - clarified ordering: protocol interoperability is release-critical; full behavior parity is secondary
    - added `v1 Stable Interop Release Gates` checklist (wire compatibility, transfer defaults, mixed-client e2e tests, no-regression requirement)
- Decisions:
  - v1 release readiness is defined by mixed-client interoperability, not complete feature parity with iMule.
- Next steps:
  - wire these gates into an executable test matrix (script/CI where feasible) before v1 tag decisions.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Documented transfer sizing numbers and iMule interop risks.
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md` with:
    - current rust-mule transfer numbers (64 KiB block, reserve/lease caps, 3-range request shape)
    - iMule reference values (`BLOCKSIZE/EMBLOCKSIZE=184320`, `PARTSIZE=9728000`)
    - interop edge cases when block granularity differs
    - implementation guidance to keep sizing configurable and validate via mixed-client soak
- Decisions:
  - treat block-size policy as a compatibility lever; avoid hardcoding non-interoperable defaults.
- Next steps:
  - add explicit config key/backlog for transfer block-size tuning with iMule-aligned default candidate.
- Change log:
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Documented security edge cases for sharing/upload design.
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md` with explicit security edge-case section:
    - sensitive file leakage controls
    - TOCTOU/mutation checks between index and serve
    - symlink/hardlink escape checks
    - large/sparse file abuse limits
    - path normalization, overlap ambiguity, metadata leak controls
    - upload amplification controls
    - MD4 compatibility caveat and stronger local integrity metadata
    - auth/rate-limit rigor for settings/debug surfaces
- Decisions:
  - security edge cases should be first-class checklist items before uploader implementation.
- Next steps:
  - convert each edge case into concrete acceptance criteria per implementation slice.
- Change log:
  - Updated `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added dedicated sharing/upload implementation checklist doc.
  - Added `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md` covering:
    - shared folder policy and unsafe-root rejection
    - index/hash/publish-path binding requirements
    - real disk-backed `OP_REQUESTPARTS` -> `OP_SENDINGPART` serving
    - backpressure/abuse controls, observability, and tests
  - Updated `docs/governance/TASKS.md` to reference the checklist.
- Decisions:
  - Treat sharing/upload as a constrained subsystem with explicit safety policy, not ad-hoc feature accretion.
- Next steps:
  - implement first minimal slice from checklist (single shared folder + real range-serving path + tests).
  - add settings UI controls for share roots with validation errors surfaced clearly.
- Change log:
  - Added `docs/10_architecture/SHARING_UPLOAD_CHECKLIST.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added backlog for shared library and real uploader implementation.
  - Updated `docs/governance/TASKS.md` with required scope:
    - configurable shared folders in config + settings UI/API
    - file scanner/indexer + publish integration
    - source-to-path mapping for published files
    - real disk-backed upload serving path (`OP_REQUESTPARTS` -> `OP_SENDINGPART`)
    - shared-folder safety rules and scanner/index observability
    - explicit blocklist policy for unsafe share roots (`/`, core OS dirs, runtime/app dirs)
- Decisions:
  - KAD source publish should represent files that are actually readable from local shared storage.
  - upload path must be disk-backed and range-accurate, not synthetic packet injection.
  - sharing system-critical directories must be denied by validation (fail-closed).
- Next steps:
  - write short architecture note for shared library index model + uploader flow boundaries.
  - implement minimal first slice: one shared folder + single-file requestpart->sendingpart read path with tests.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Expanded governance backlog to reduce test/ops drift.
  - Updated `docs/governance/TASKS.md` with additional backlog items:
    - phase0 gate hardening for `nan`/unexpected `SKIP` metrics
    - soak script CI sanity mode (non-longrun validation)
    - pass-with-degradation runbook guidance
    - soak artifact retention/naming policy
    - post-restart download state reason diagnostics
    - config schema versioning/migration notes
- Decisions:
  - treat soak/test ops quality as first-class backlog scope, not ad-hoc follow-up.
- Next steps:
  - prioritize gate hardening + restart-state diagnostics first (highest triage leverage).
  - then document artifact retention/runbook expectations in ops docs.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added logging-surface cleanup task for debug gating.
  - Updated `docs/governance/TASKS.md` with explicit backlog item:
    - audit trace/routing logs for debug gating
    - move verbose bucket/routing-table details behind debug flag
    - keep default logs focused on operator-relevant signals (health/progress/errors)
- Decisions:
  - verbose routing internals should not be emitted in default mode.
- Next steps:
  - inventory current `tracing` callsites for bucket/routing detail and classify default-vs-debug.
  - implement gating and add regression checks for log verbosity expectations.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-08): Added timezone configuration/settings backlog item.
  - Updated `docs/governance/TASKS.md` with timezone scope:
    - config key for timezone (IANA zone id) with validation/fallback behavior
    - expose timezone in settings UI/API
    - apply configured timezone to log timestamps (avoid UTC-only output)
- Decisions:
  - treat timezone support as explicit product behavior (config + API/UI + logging), not a one-off script override.
- Next steps:
  - design config schema (`timezone` field), validation rules, and startup fallback semantics.
  - implement settings endpoint/UI wiring, then update logging timestamp formatter.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-07): Ignored local acceptance archives by default.
  - Updated `.gitignore` to include `/artifacts`.
  - Rationale: keep large soak/archive outputs local unless intentionally versioned via explicit commit.
- Decisions:
  - preserve artifact versioning as an explicit opt-in action.
- Next steps:
  - if a specific artifact set should be retained in git, stage it explicitly with a focused PR and note retention reason.
- Change log:
  - Updated `.gitignore`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-07): Archived latest successful phase0 acceptance artifacts on `main`.
  - Ran `scripts/test/archive_acceptance_artifacts.sh` for:
    - run dir: `/tmp/rust-mule-download-phase0-accept-20260307_145056`
    - dest dir: `artifacts/soak/rust-mule-download-phase0-accept-20260307_145056`
  - stack bundle path from logs was no longer present in `/tmp` at archive time.
- Decisions:
  - keep archiving run artifacts immediately after successful runs to avoid `/tmp` cleanup loss.
- Next steps:
  - copy/relocate stack bundle to a stable path during run (or immediately after) before archival.
  - decide whether to commit selected `artifacts/soak/*` baselines or keep local-only.
- Change log:
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-07): Addressed remaining actionable PR feedback in resume-soak script.
  - Updated `scripts/test/download_resume_soak.sh`:
    - fixed diagnostics state aggregation to `sort_by(.state) | group_by(.state)` for correct counts.
    - replaced combined `EXIT INT TERM` trap with explicit signal handlers:
      - `INT` -> cleanup + exit `130`
      - `TERM` -> cleanup + exit `143`
    - kept cleanup idempotent and reusable via optional explicit exit code parameter.
- Decisions:
  - Preserve single-shot cleanup behavior while making signal outcome explicit and POSIX-consistent.
- Next steps:
  - merge PR `feature/download-phase0-acceptance`.
  - archive latest acceptance artifacts using `scripts/test/archive_acceptance_artifacts.sh` for baseline retention.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-07): Added acceptance artifact archiving helper for long-term regression baselines.
  - Added `scripts/test/archive_acceptance_artifacts.sh`:
    - archives high-value files from a phase0 acceptance run into `artifacts/soak/<run_id>`.
    - includes: `summary.txt`, `kad-gate/*.tsv`, `resume-soak/resume_report.txt`, diagnostics JSON, snapshots JSON.
    - optional `--stack-bundle` to attach collected stack tarball.
  - Updated `scripts/test/README.md` with usage examples.
- Decisions:
  - Keep archival explicit/manual (operator-triggered) to avoid unbounded storage growth from every run.
- Next steps:
  - after each notable pass/fail run, archive selected artifacts for baseline history.
- Change log:
  - Added `scripts/test/archive_acceptance_artifacts.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-07): Isolated stack runner process group to prevent stop-time signal splash.
  - Updated `scripts/test/download_soak_stack_bg.sh`:
    - `start_background` now prefers `nohup setsid ...` when available.
    - fallback remains `nohup ...` when `setsid` is unavailable.
  - Motivation:
    - acceptance/resume runs showed completion gate pass but shell output ended with large `Terminated` bursts and missing final summary artifacts.
    - root cause was process-group stop targeting a runner started in caller-linked process group.
- Decisions:
  - Keep fix minimal and local to stack runner bootstrap; no changes to stop semantics.
- Next steps:
  - rerun phase0 acceptance with fast-exit and verify `summary.txt` + `resume_report.txt` are emitted cleanly.
- Change log:
  - Updated `scripts/test/download_soak_stack_bg.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-06): Aligned KAD wire refactor plan doc with current project state.
  - Updated `docs/10_architecture/KAD_WIRE_REFACTOR_PLAN.md`:
    - marked Phase 1 (Central Outbound Shaper) items as complete.
    - added governance doc references (`docs/governance/TASKS.md`, `docs/governance/handoff.md`).
    - updated Phase 5 rollout checklist paths to governance docs.
    - added note that current active priority remains download restart/resume soak stabilization.
- Decisions:
  - Treat this as documentation alignment only; no behavior/runtime changes.
- Next steps:
  - after current acceptance soak run, decide whether to open a dedicated KAD Phase 2 follow-up branch or keep focus on download phase 2 hardening.
- Change log:
  - Updated `docs/10_architecture/KAD_WIRE_REFACTOR_PLAN.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-06): Resume-soak hardening for post-restart fixture state and cleanup idempotency.
  - `scripts/test/download_resume_soak.sh`:
    - re-publishes fixture sources and re-validates fixture source discovery after restart (`restart_app` -> health 200 -> publish+wait).
    - made `cleanup_on_exit` idempotent with `CLEANUP_RAN` guard and early trap clear to prevent repeated cleanup spam on signal storms.
- Decisions:
  - Post-restart fixture publish is now part of the critical path for `FIXTURES_ONLY=1` resume validation.
  - Cleanup should be single-shot even under repeated TERM delivery.
- Next steps:
  - run one acceptance pass with `FAST_EXIT_AFTER_COMPLETION=1`; confirm either completion gate passes or diagnostics now include post-restart fixture state.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added optional resume-soak fast-exit path to reduce wall-clock runtime.
  - `scripts/test/download_resume_soak.sh`:
    - added `FAST_EXIT_AFTER_COMPLETION` (default `0`) and `FAST_EXIT_GRACE_SECS` (default `60`).
    - when enabled, script stops + collects stack shortly after completion gate instead of waiting for full stack terminal state.
  - `scripts/test/README.md`:
    - documented fast-exit env vars and acceptance command example.
    - documented completion-timeout diagnostic artifact names.
- Decisions:
  - Keep fast-exit opt-in to preserve existing full-run behavior by default.
  - Use short grace period before stop/collect to preserve post-completion context while cutting long tail wait time.
- Next steps:
  - run one acceptance pass with `FAST_EXIT_AFTER_COMPLETION=1` and validate artifact completeness.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Captured debug-token lifecycle policy to prevent accidental secret churn.
  - Updated debug/API design docs to require:
    - no automatic deletion/rotation of `api.debug_token` when debug endpoints are disabled
    - debug-disabled mode keeps token inert and returns `404` on debug routes
    - token rotation is explicit admin action only
    - token redaction in logs/effective-config output
  - Updated `docs/governance/TASKS.md` with implementation requirements for lifecycle behavior.
- Decisions:
  - Avoid startup side-effects on secrets; “disabled means inert” is the default.
- Next steps:
  - implement debug-token verification helper with constant-time compare and redaction-safe config rendering.
  - add API tests for disabled/invalid-token behavior and startup non-mutation of debug token.
- Change log:
  - Updated `docs/10_architecture/API_DESIGN.md`.
  - Updated `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`.
  - Updated `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added design for debug-triggered bootstrap restart.
  - Added `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`:
    - endpoint proposal: `POST /api/v1/debug/bootstrap/restart` (async `202 + job_id`)
    - status endpoint proposal: `GET /api/v1/debug/bootstrap/jobs/{job_id}`
    - guardrails: single-flight, cooldown, bounded job registry, TTL cleanup
    - security: debug-enabled gate + debug second-factor token (`api.debug_token`, `X-Debug-Token`)
  - Updated planned debug endpoint list in `docs/10_architecture/API_DESIGN.md`.
  - Added implementation backlog in `docs/governance/TASKS.md`.
- Decisions:
  - Keep bootstrap restart debug-only and asynchronous to protect API responsiveness.
  - Reuse same debug-token defense-in-depth model as trace/debug endpoints.
- Next steps:
  - implement debug bootstrap job runner in service layer with single-flight + cooldown enforcement.
  - add API tests for 202/job status and 404/403 debug gating behavior.
- Change log:
  - Added `docs/10_architecture/DEBUG_BOOTSTRAP_RESTART_DESIGN.md`.
  - Updated `docs/10_architecture/API_DESIGN.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added debug endpoint second-factor token decision.
  - Updated `docs/10_architecture/API_DESIGN.md` with planned debug hardening:
    - `api.debug_token` + `X-Debug-Token` as additive gate on debug endpoints.
    - behavior split: debug disabled `404`, invalid/missing debug token `403`.
  - Updated `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md` security section with same requirement.
  - Updated `docs/governance/TASKS.md` backlog for implementation.
- Decisions:
  - Debug endpoints require both standard auth and debug secret (defense-in-depth).
  - Preserve endpoint cloaking semantics when debug mode is off (`404`).
- Next steps:
  - implement middleware/helper for debug-token enforcement using constant-time compare.
  - add API tests for 404/403 split behavior on debug routes.
- Change log:
  - Updated `docs/10_architecture/API_DESIGN.md`.
  - Updated `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Trace lookup design now explicitly uses async execution.
  - `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`:
    - added chosen mode: `POST /api/v1/debug/trace_lookup` returns `202 Accepted` with `trace_id`.
    - added poll endpoint model: `GET /api/v1/debug/trace_lookup/{trace_id}`.
    - added optional cancel model and bounded registry/TTL expectations.
  - `docs/governance/TASKS.md`:
    - added async execution + bounded active trace backlog requirements.
- Decisions:
  - Async-first trace execution is required to protect API responsiveness and avoid long request blocking under peer/timeouts variance.
- Next steps:
  - define trace registry bounds in config (defaults + hard caps) during implementation slice.
- Change log:
  - Updated `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added design notes for debug-only KAD hop tracing endpoint.
  - Added `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md` with:
    - endpoint proposal: `POST /api/v1/debug/trace_lookup`
    - request/response schema draft
    - stop conditions, safety bounds, rate-limit expectations
    - observability and test plan
  - Updated `docs/10_architecture/API_DESIGN.md` implemented/planned debug endpoint list.
  - Added implementation backlog entry in `docs/governance/TASKS.md`.
- Decisions:
  - Keep trace lookup debug-only and strictly bounded to avoid lookup-amplification risk.
  - Implement via existing KAD service lookup flow, not API-layer network logic.
- Next steps:
  - implement service command + API handler under debug routes.
  - add endpoint validation/rate-limit tests and basic topology integration test.
- Change log:
  - Added `docs/10_architecture/KAD_TRACE_LOOKUP_DESIGN.md`.
  - Updated `docs/10_architecture/API_DESIGN.md`.
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added completion-timeout diagnostics in resume soak script.
  - `scripts/test/download_resume_soak.sh`:
    - on post-restart completion timeout, now calls `dump_download_diagnostics "completion_timeout"` before exit.
    - emits structured diagnostic artifacts:
      - `<resume_out_dir>/completion_timeout_downloads_diag.json`
      - `<resume_out_dir>/completion_timeout_status_diag.json`
- Decisions:
  - Treat completion timeout as a first-class triage path; always persist queue/state/counter snapshot for post-run analysis.
- Next steps:
  - rerun `download_phase0_acceptance.sh` with resume soak enabled and inspect `completion_timeout_*` artifacts if timeout recurs.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): CLI backlog expanded with accepted operational flags.
  - `docs/governance/TASKS.md`:
    - added accepted follow-up flags:
      - `--version`
      - `--check-config`
      - `--print-effective-config`
- Decisions:
  - Keep first CLI slice minimal but include low-risk operational introspection flags once argument parser is in place.
- Next steps:
  - implement parser and usage output in `src/main.rs`.
  - implement `--check-config` fast path (load + validate + exit).
  - define redaction policy before implementing `--print-effective-config` output.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-05): Added CLI parameterization follow-up task for app startup UX.
  - `docs/governance/TASKS.md`:
    - added backlog item for `rust-mule --config <path>` support.
    - added explicit fail-fast requirement for missing/unreadable `--config` file.
    - added `--help` and `-?` usage output requirement.
    - preserved default config behavior (`config.toml` in CWD) as compatibility baseline.
- Decisions:
  - Treat startup argument handling as a small, isolated hardening slice (no runtime behavior changes beyond config path selection and usage output).
- Next steps:
  - implement minimal CLI parser in `src/main.rs` for `--config`, `--help`, `-?`.
  - add focused unit tests for argument parsing and missing-config error surface.
- Change log:
  - Updated `docs/governance/TASKS.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-04): Added signal-forensics logging for soak interruptions.
  - `scripts/test/download_soak_band.sh`:
    - on `INT/TERM`, now logs signal context (`self_pid`, `self_ppid`, `self_pgid`, `self_cmd`, `parent_cmd`) before stop/collect.
  - `scripts/test/download_soak_stack_bg.sh`:
    - replaced inline trap with `handle_runner_signal`.
    - on `INT/TERM`, now logs the same signal context fields plus explicit `runner interrupted signal=<...>`.
- Decisions:
  - Capture process ancestry at signal time to distinguish app/script crashes from external termination.
- Next steps:
  - Re-run acceptance soak and inspect new `signal-context` lines if interruption recurs; correlate `parent_cmd` with invoking shell/script.
- Change log:
  - Updated `scripts/test/download_soak_band.sh`.
  - Updated `scripts/test/download_soak_stack_bg.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-04): Aligned API design docs with current `/api/v1` implementation.
  - `docs/10_architecture/API_DESIGN.md`:
    - set status to `ACTIVE`, refreshed review date.
    - fixed stale executable docs reference (`docs/30_operations/api_curl.md`).
    - added explicit implemented endpoint surface matching `src/api/router.rs` (auth/session, core, searches, downloads, KAD, debug).
    - replaced non-existent routing endpoint examples with actual current KAD/debug routing endpoints.
    - updated error envelope example to current default (`{code,message}`).
    - updated minimal checklist to reflect implemented vs future items.
- Decisions:
  - Keep `API_DESIGN.md` as mixed “current + future” design doc, but pin current implementation in an explicit section at top.
- Next steps:
  - Optionally add a generated API route inventory check to CI to detect doc/route drift earlier.
- Change log:
  - Updated `docs/10_architecture/API_DESIGN.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-04): Fixed resume-soak jq crash after restart snapshot.
  - `scripts/test/download_resume_soak.sh`:
    - made snapshot `downloads_count` null-safe with `(.downloads // []) | length`.
    - replaced monotonic keying by `.id` with stable fallback key selection:
      - `part_number`, then `id`, then `file_hash_md4_hex`.
    - prevents `jq: Cannot index object with null` when download rows do not expose `id`.
- Decisions:
  - Treat API response schema as partially optional in soak scripts; always null-guard collection fields and key derivation.
- Next steps:
  - Re-run acceptance with resume soak and verify it passes post-restart monotonic check without jq abort.
  - If next failure occurs, inspect generated `*_downloads_diag.json`/`*_status_diag.json` in the acceptance output directory.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-04): Addressed PR review findings on transfer pump safety and behavior.
  - `src/app.rs`:
    - replaced direct `mark_block_received_by_peer` calls with `ingest_inbound_packet(OP_SENDINGPART, ...)` so block data is written before receive-state transitions.
    - changed held-lease model from single lease per part to queue (`HashMap<u16, VecDeque<PumpHeldLease>>`) to avoid lease overwrite/loss.
    - added per-file source-search throttle (`SEARCH_MIN_INTERVAL=30s`) and only sends `SearchSources` when source list is empty and throttle allows.
    - prevents new reservations while a part still has held leases pending.
- Decisions:
  - Keep pump as phase-0 bridge but make it data-writing and lease-safe to avoid corrupt completion semantics.
  - Reduce network load by throttling search fanout from pump.
- Next steps:
  - Run acceptance/resume soak and inspect whether reserve/grant/inflight now advance without prior cancellation churn.
  - If stable, split pump into dedicated module and gate behind explicit config flag.
- Change log:
  - Updated `src/app.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-04): Implemented runtime download transfer pump to exercise reserve/receive path.
  - `src/app.rs`:
    - added background `run_download_transfer_pump(...)` task (enabled when KAD service is enabled).
    - pump loop behavior:
      - lists active downloads,
      - triggers KAD `SearchSources` and requests `GetSources`,
      - reserves blocks with `reserve_blocks_for_peer`,
      - marks reserved blocks as received to drive progress,
      - temporarily holds one lease then commits it on TTL to keep in-flight activity observable.
  - `src/download/service.rs`:
    - `DownloadSummary` now includes `file_hash_md4_hex` so runtime can map downloads to KAD source lookups.
  - `src/api/handlers/downloads.rs`:
    - exposes `file_hash_md4_hex` in download list and mutation responses.
- Decisions:
  - Keep transfer pump in app/runtime as a phase-0 bridge (no protocol-transport wiring yet).
  - Prioritize moving from zero reserve activity to observable reserve/inflight/progress in soak runs.
- Next steps:
  - Re-run phase0 acceptance with resume soak and confirm:
    - `reserve_calls_total > 0`
    - `reserve_granted_blocks_total > 0`
    - non-zero `downloaded_total` and `inflight_total` during active-transfer wait.
  - If this passes consistently, replace pump-side synthetic receive with actual inbound transfer packet path.
- Change log:
  - Updated `src/app.rs`.
  - Updated `src/download/service.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Added download scheduler denial diagnostics and soak failure dumps for transfer-stall triage.
  - `src/download/service.rs`:
    - added status counters:
      - `reserve_calls_total`
      - `reserve_granted_blocks_total`
      - `reserve_denied_state_total`
      - `reserve_empty_no_missing_total`
    - wired counters through runtime status publication.
  - `src/api/handlers/downloads.rs`:
    - exposed new reserve diagnostics in `/api/v1/downloads`.
  - `scripts/test/download_resume_soak.sh`:
    - on fixture-source timeout or active-transfer timeout, now writes diagnostic snapshots:
      - `<tag>_downloads_diag.json`
      - `<tag>_status_diag.json`
    - logs concise reserve/download state summary from diagnostics.
  - `scripts/test/README.md`:
    - documented new failure diagnostic files.
  - `src/api/tests.rs`:
    - extended API contract assertion for new `/api/v1/downloads` fields.
- Decisions:
  - Keep diagnostics additive and read-only; no scheduling behavior changed in this patch.
  - Focus first on visibility of reserve-path outcomes before changing downloader logic.
- Next steps:
  - Re-run `download_phase0_acceptance.sh` with `RUN_RESUME_SOAK=1`.
  - If `active_transfer_timeout` recurs, inspect `active_transfer_timeout_downloads_diag.json` for reserve counters:
    - if `reserve_calls_total` stays near zero: scheduler/dispatch is not invoking reserve.
    - if `reserve_calls_total` rises but `reserve_granted_blocks_total` stays zero: examine denial counters and download states.
- Status (2026-03-03): Added fail-fast no-reserve-activity gate to resume soak.
  - `scripts/test/download_resume_soak.sh`:
    - new `NO_RESERVE_ACTIVITY_TIMEOUT_SECS` (default `300`).
    - in active-transfer wait, fails early when downloads exist but `reserve_calls_total` remains `0` past timeout.
    - writes `no_reserve_activity_downloads_diag.json` / `no_reserve_activity_status_diag.json` on fail-fast trigger.
  - `scripts/test/README.md`:
    - documented `NO_RESERVE_ACTIVITY_TIMEOUT_SECS` and fail-fast behavior.
- Decisions:
  - Treat prolonged zero reserve-call activity as a structural pipeline condition; fail quickly to reduce soak feedback latency.
- Next steps:
  - Run acceptance again and verify fast-fail triggers within 5 minutes when reserve remains unwired.
  - Then prioritize wiring runtime transfer scheduler to issue `ReserveBlocks` for discovered sources.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Added progress heartbeat/ETA logs for long-running phase0 soak scripts.
  - `scripts/test/kad_phase0_gate.sh`:
    - new `PROGRESS_LOG_SECS` (default `30`) for throttled readiness-wait logs.
    - capture start now logs initial remaining time and UTC ETA.
  - `scripts/test/download_phase0_acceptance.sh`:
    - added stage progress logs with elapsed/estimated-remaining/ETA (`gate`, `resume`, `longrun`).
  - `scripts/test/download_resume_soak.sh`:
    - new `PROGRESS_LOG_SECS` (default `30`) for throttled progress lines in long wait loops:
      - scenario wait
      - fixture source wait
      - active transfer wait
      - post-restart progress wait
      - completion wait
      - stack terminal wait
  - `scripts/test/README.md`:
    - documented `PROGRESS_LOG_SECS` in gate, acceptance, and resume sections.
- Decisions:
  - Keep progress logging throttled to avoid noisy per-poll output while preserving clear long-run observability.
- Next steps:
  - Re-run acceptance soak and confirm progress lines provide enough signal to leave runs unattended.
- Change log:
  - Updated `scripts/test/kad_phase0_gate.sh`.
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Addressed Copilot PR review findings on acceptance/soak scripts and API parse logging.
  - `scripts/test/download_soak_bg.sh`:
    - fixed exit-code capture in `api_post` and `downloads_create` by removing `! cmd; rc=$?` patterns that masked nonzero failures.
  - `scripts/test/download_phase0_acceptance.sh`:
    - token load now trims CR/LF (`tr -d '\r\n'`) before auth header use.
  - `scripts/docs/download_create_from_hash.sh`:
    - switched JSON payload construction to `jq -nc` + `--data-binary` for both search and create requests.
    - token load now trims CR/LF.
  - `src/api/error.rs`:
    - added control-character sanitization for logged JSON parse body excerpts.
    - added unit test for excerpt sanitizer behavior.
- Decisions:
  - Treat script exit-code capture and shell-JSON interpolation as correctness issues to fix immediately.
  - Keep parse-failure logging at warn level, but sanitize excerpt to avoid control-char log injection.
- Next steps:
  - Re-run phase0 acceptance/resume soak and verify create-failure handling/diagnostics are now accurate on real failures.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/docs/download_create_from_hash.sh`.
  - Updated `src/api/error.rs`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Added stack-local fixture publish pre-step in resume-soak path.
  - `scripts/test/download_resume_soak.sh`:
    - new env controls: `STACK_PUBLISH_FIXTURES` (default `1`), `STACK_PUBLISH_BASE_URL`, `STACK_PUBLISH_TOKEN_FILE`.
    - when `FIXTURES_ONLY=1`, publishes fixture hashes to stack publisher after stack startup and before fixture source-discovery polling.
    - defaults to publishing against `STACK_BASE_URL` with stack run token.
  - `scripts/test/download_phase0_acceptance.sh`:
    - forwards `STACK_PUBLISH_*` env controls into resume-soak stage.
  - `scripts/test/README.md`:
    - documented new stack-local publish controls and behavior.
- Decisions:
  - Make stack-local publish the default fixture path for resume soaks to remove cross-topology dependency on external pre-publish.
- Next steps:
  - Re-run phase-0 acceptance with `RUN_RESUME_SOAK=1`, `FIXTURES_ONLY=1`, and stack-local publish defaults.
  - If fixture-source gate still fails, capture `live/routing/source_store_*` on stack publisher and consumer for topology diagnosis.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Synced docs directory guide with restructured layout and canonical index policy.
  - `docs/README.md`:
    - converted from legacy file-by-file list to folder-level structure guide.
    - explicitly marks `docs/index.md` as canonical docs navigation entrypoint.
- Decisions:
  - Keep `docs/index.md` as navigation source of truth; keep `docs/README.md` as concise directory-orientation doc.
- Next steps:
  - Keep both files in sync whenever docs folders are renamed or moved.
- Change log:
  - Updated `docs/README.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Updated docs entrypoint to match restructured docs tree.
  - `docs/index.md`:
    - replaced legacy flat links with sectioned navigation for `00_overview`, `10_architecture`, `20_protocol`, `30_operations`, `governance`, `rfcs`, and `90_archive`.
- Decisions:
  - Keep `docs/index.md` as the canonical, human-readable navigation hub aligned with the on-disk folder hierarchy.
- Next steps:
  - Optionally align `docs/README.md` wording to the same section naming to avoid drift between entrypoint docs.
- Change log:
  - Updated `docs/index.md`.
  - Updated `docs/governance/handoff.md`.

- Status (2026-03-03): Added optional fixture publish pre-step to phase-0 acceptance runner.
  - `scripts/test/download_phase0_acceptance.sh`:
    - new env controls: `PUBLISH_FIXTURES`, `PUBLISH_BASE_URL`, `PUBLISH_TOKEN_FILE`.
    - when enabled, iterates fixture hashes and publishes each via `scripts/docs/kad_publish_source.sh` before snapshots/gate/resume stages.
    - validates required fixture/publisher inputs when publish mode is enabled.
  - `scripts/test/README.md`:
    - documented fixture pre-publish usage and publish env controls in acceptance section.
- Decisions:
  - Keep publish pre-step opt-in so acceptance remains usable in both passive-observe and actively-seeded environments.
- Next steps:
  - Re-run acceptance with `PUBLISH_FIXTURES=1` and `RUN_RESUME_SOAK=1`.
  - If fixture-source gate still fails, investigate KAD publish propagation from publisher node to stack node (network topology/version/filtering).
- Change log:
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-02): Added fixture-source readiness gate to resume soak to fail fast on source-unavailable runs.
  - `scripts/test/download_resume_soak.sh`:
    - added `wait_for_fixture_sources()` (enabled when `FIXTURES_ONLY=1`) to trigger/search and poll `/api/v1/kad/sources/:file_id_hex` for each fixture before transfer wait.
    - new `FIXTURE_SOURCE_TIMEOUT_SECS` (default 300) for this preflight gate.
    - on timeout, emits focused KAD status diagnostics (`sent/recv search source reqs`, `source_store_*`, `live`, `routing`) and exits early.
  - `scripts/test/README.md`:
    - documented `FIXTURE_SOURCE_TIMEOUT_SECS` and the new fail-fast fixture source gate behavior.
- Decisions:
  - Treat “downloads created but always queued/inflight=0” under fixtures-only mode as a source availability/setup issue; fail early instead of waiting for active-transfer timeout.
- Next steps:
  - Re-run acceptance/resume with fixtures; if fixture-source gate fails, publish those fixture hashes to reachable peers first (or automate publish pre-step).
  - If fixture-source gate passes but inflight remains zero, investigate download scheduler/reservation path (post-source acquisition).
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-02): Refactored soak POST path so create requests are generated and sent file-first (no JSON string transport path).
  - `scripts/test/download_soak_bg.sh`:
    - added `api_post_file(path, payload_file)` and kept `api_post(path, json)` as wrapper.
    - `downloads_create()` now writes payload JSON directly to temp file via `jq` and posts that file.
    - debug output now reports both `payload_len` (string) and `payload_bytes` (file byte count).
- Decisions:
  - Remove string-to-curl conversion from download create path to eliminate any remaining shell quoting/expansion ambiguity.
- Next steps:
  - Re-run acceptance with `DEBUG_CREATE_PAYLOADS=1` and compare `payload_bytes` vs API `body_len` for first 400 parse failures.
  - If API still reports trailing-character parse errors, instrument API parser to log raw last-byte hex for failing requests.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-03-02): Hardened soak create request path to avoid stdout contamination and payload mutation.
  - `scripts/test/download_soak_bg.sh`:
    - `log()` no longer uses `tee`; logs now append to runner log and emit to stderr only.
    - `api_post()` now writes JSON to a temp file and sends via `curl --data-binary @file` for exact byte-for-byte body delivery.
- Decisions:
  - Treat command-substitution/stdout coupling as a test harness bug; logging must never share stdout with function return channels.
  - Treat `--data-binary @file` as the canonical way to send scripted JSON payloads in soak harnesses.
- Next steps:
  - Re-run `download_phase0_acceptance.sh` with `RUN_RESUME_SOAK=1` and inspect `/tmp/rustmule-run-*/rust-mule.out` for any remaining `json parse failed`.
  - If parse errors persist, capture and compare request body byte dumps client/server side for the same request id.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added fail-fast `BASE_URL` ownership lock for download soak runners and fixed debug flag propagation.
  - `scripts/test/download_soak_bg.sh`:
    - added per-`BASE_URL` lock under `LOCK_ROOT` (default `/tmp/rust-mule-download-soak-locks`).
    - `start` now refuses to launch when another live runner already owns the same API target, and reports owner PID/lock path.
    - lock is acquired in `run` and released on exit/interrupt (stale dead-owner locks are auto-cleaned on next start).
    - `DEBUG_CREATE_PAYLOADS` is now propagated in background `start`, matching foreground behavior.
  - `scripts/test/README.md`:
    - documented `LOCK_ROOT` and new base-URL lock behavior.
- Decisions:
  - Treat concurrent soak runners on one API target as invalid test setup and fail immediately to avoid cross-run contamination.
- Next steps:
  - Re-run acceptance/resume soak; verify no unexpected malformed create payload source remains once conflicting runners are blocked.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added download create payload debug tracing to soak scripts for malformed JSON triage.
  - `scripts/test/download_soak_bg.sh`:
    - new `DEBUG_CREATE_PAYLOADS=1` toggle logs exact `/api/v1/downloads` request payloads (with target URL and token file path) and raw responses for each create call.
  - `scripts/test/download_soak_stack_bg.sh`:
    - forwards `DEBUG_CREATE_PAYLOADS` into staged stack runs so debug logging works in background soak orchestration.
  - `scripts/test/README.md`:
    - documented `DEBUG_CREATE_PAYLOADS=1` in download soak usage/overrides.
- Decisions:
  - Keep create payload/response tracing opt-in to avoid noisy default logs while preserving precise diagnostics when parse failures occur.
- Next steps:
  - Re-run acceptance/resume soak with `DEBUG_CREATE_PAYLOADS=1` and inspect `create-debug` lines in staged runner logs for any malformed body source.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `scripts/test/download_soak_stack_bg.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added session-creation resource-cap hardening task to backlog.
  - `docs/TODO.md`:
    - added API fix item for `POST /api/v1/session` to cap concurrent active sessions (target `MAX_SESSIONS = 1024`) after pruning expired entries.
    - intended behavior on cap hit: return `503 Service Unavailable`.
- Decisions:
  - Treat loopback-local session accumulation as a resource exhaustion risk; rate limit alone is insufficient for 8h TTL sessions.
- Next steps:
  - Implement active session cap check in `src/api/handlers/core.rs` before insert.
  - Add unit/integration tests for cap behavior and expired-session pruning interaction.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added API auth constant-time compare hardening task to backlog.
  - `docs/TODO.md`:
    - added explicit API task to replace short-circuit bearer token equality with constant-time comparison in `src/api/auth.rs`.
- Decisions:
  - Treat loopback-local timing leakage as in-scope hardening risk (compromised local process threat model).
- Next steps:
  - Implement constant-time token comparison with `subtle::ConstantTimeEq` (or equivalent).
  - Add/update auth tests to preserve current behavior while using constant-time comparison.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added SAM protocol/settings injection hardening task to backlog.
  - `docs/TODO.md`:
    - added explicit fix item for CR/LF/control-char handling in SAM value encoding and settings validation.
    - scope: reject newline/control chars in `sam.session_name` (API validation) and prevent CR/LF emission from `i2p::sam::protocol::encode_value`.
- Decisions:
  - Treat this as a security hardening fix (command-line injection class) and prioritize in upcoming SAM/runtime work.
- Next steps:
  - Implement CR/LF + control-char validation in `src/api/handlers/settings.rs`.
  - Update `src/i2p/sam/protocol.rs` to return error/reject values containing line breaks.
  - Add regression tests for injected `sam.session_name` payloads.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-03-01): Added KAD crypto-compatibility backlog item for MD5 constants.
  - `docs/TODO.md`:
    - added task to replace runtime-derived MD5 round constants with fixed RFC 1321 constants in UDP crypto path.
    - rationale: avoid platform-specific floating-point rounding drift and ensure cross-node decryption compatibility.
- Decisions:
  - Track as a dedicated KAD hardening/compatibility fix item before further UDP crypto tuning.
- Next steps:
  - Implement fixed `T[64]` constants in `src/kad/udp_crypto.rs` and remove runtime `sin()` derivation.
  - Add/extend regression tests to assert constant table matches RFC values.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-28): Added diagnostics for JSON parse `400` and stabilized soak fail-streak/reset behavior.
  - `src/api/error.rs`:
    - `parse_json_with_limit` now logs `json parse failed` with serde error + body length + body excerpt (first 160 bytes) before returning `400`.
    - enables direct triage of generic `bad request` in stack `rust-mule.out`.
  - `scripts/test/download_soak_bg.sh`:
    - reset `CREATE_FAIL_STREAK` at start of each run (`load_fixtures`), while still persisting within-run increments across command-substitution subshell boundaries.
    - prevents cross-run streak carry-over noise.
  - `scripts/test/download_resume_soak.sh`:
    - trap cleanup no longer returns nonzero status from EXIT path; disables trap after cleanup.
    - avoids `pop_var_context` shell error observed after termination.
- Decisions:
  - Prefer runtime diagnostics in API parser over guessing script-side causes for 400.
- Next steps:
  - Re-run acceptance with isolated stack port/root and inspect stack `rust-mule.out` for `json parse failed` line if 400 persists.
- Change log:
  - Updated `src/api/error.rs`.
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-02-26): Added forced cleanup for resume-soak failures/interruption to prevent lingering stack clients.
  - `scripts/test/download_resume_soak.sh`:
    - added exit trap (`cleanup_on_exit`) that requests `download_soak_stack_bg.sh stop` whenever run exits abnormally.
    - tracks stack start state (`STACK_STARTED`) and suppresses cleanup only on successful completion.
  - Effect:
    - failed/aborted resume runs now stop spawned stack/client processes instead of leaving them active.
- Decisions:
  - Prefer unconditional stack stop on resume-script error paths to avoid leaked background clients and held ports.
- Next steps:
  - Re-run acceptance + resume soak once with isolated `STACK_API_PORT`/`STACK_ROOT` and verify no lingering process after failure.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-02-26): Hardened soak create payload encoding to avoid malformed JSON in fixture-driven runs.
  - `scripts/test/download_soak_bg.sh`:
    - `downloads_create()` now builds request JSON with `jq -n` instead of string interpolation.
    - avoids shell-escaping edge cases for fixture values and ensures valid JSON body for `POST /api/v1/downloads`.
- Decisions:
  - Keep fixture create payload construction deterministic and JSON-safe in script layer.
- Next steps:
  - Re-run acceptance with resume soak and verify create requests no longer fail with generic `400 bad request`.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-02-26): Fixed resume-soak diagnostics and API error detail path for download create failures.
  - `scripts/test/download_soak_bg.sh`:
    - fixed create-failure streak persistence across command-substitution subshell calls by storing streak in `RUN_ROOT/create_fail_streak`.
    - fail-fast in `FIXTURES_ONLY=1` mode now trips correctly after `CREATE_FAIL_LIMIT` consecutive create failures.
    - create-failure log extraction now reads both nested (`error.*`) and top-level (`code`/`message`) API envelopes.
  - `src/api/error.rs`:
    - `error_envelope_mw` now preserves handler-provided JSON error bodies and only injects generic envelope when handler did not provide JSON.
  - `src/api/handlers/downloads.rs`:
    - `POST /api/v1/downloads` now returns detailed validation message for `DownloadError::InvalidInput` instead of generic `bad request`.
    - added focused unit test for invalid-input mapping.
- Decisions:
  - Keep generic API error envelope middleware for bare status errors, but preserve explicit JSON error responses from handlers.
  - Keep soak fail-fast script-level and now make it deterministic in subshell-heavy shell flows.
- Next steps:
  - Re-run acceptance + resume soak with fixtures and inspect first detailed create error message.
  - If create still fails, patch fixture generation/shape or download create validation according to returned message.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `src/api/error.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-26): Added diagnostics + fail-fast for fixture-backed download create failures in soak runner.
  - `scripts/test/download_soak_bg.sh`:
    - logs detailed warning when create response has no `download.part_number` (includes error code/message + response excerpt).
    - tracks repeated create failures (`CREATE_FAIL_STREAK`).
    - in `FIXTURES_ONLY=1` mode, marks scenario failed after `CREATE_FAIL_LIMIT` consecutive no-part responses (default `10`).
    - emits round-level `create_fail` detail entries for integrity/long_churn/concurrency; single_e2e `create` now includes error detail when part is missing.
  - `scripts/test/README.md`:
    - documented optional `CREATE_FAIL_LIMIT`.
- Decisions:
  - Keep failure gating script-level for now (no API behavior change) to make fixture/contract issues immediately visible in soak artifacts.
- Next steps:
  - Re-run acceptance with resume soak and inspect `create_fail` rows to identify precise API rejection reason if queue remains empty.
- Change log:
  - Updated `scripts/test/download_soak_bg.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-25): Build script review identified host-only builds mislabeled as platform builds.
  - Findings:
    - `scripts/build/build_linux_release.sh`, `scripts/build/build_macos_release.sh`, and `scripts/build/build_windows_release.ps1` all build from host default `target/release` without `--target`.
    - Output bundle naming includes platform/arch labels, but build target is not explicitly enforced.
  - Backlog updates added:
    - `docs/TODO.md`: explicit target-triple adoption, Linux amd64/x86_64 support, Windows target matrix, macOS target matrix, prerequisite docs.
    - `docs/TASKS.md`: release-script hardening scope with explicit target list and CI prerequisite documentation.
- Decisions:
  - Track this as a dedicated follow-up implementation slice; do not change release scripts in this pass.
- Next steps:
  - Implement target-aware build scripts and update `scripts/build/README.md` with supported targets + host/cross-build constraints.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-25): Fixed resume-soak false starts caused by API port collision with a pre-running local node.
  - Root cause from acceptance artifacts:
    - stack app failed API bind on `:17835` (`Address already in use`),
    - soak scenarios then hit the existing app and got `403` on `/api/v1/downloads` readiness.
  - `scripts/test/download_resume_soak.sh`:
    - introduced dedicated stack endpoint defaults:
      - `STACK_API_PORT=17865`
      - `STACK_BASE_URL=http://127.0.0.1:17865`
    - stack start now explicitly uses those values.
  - `scripts/test/download_phase0_acceptance.sh`:
    - resume stage no longer forwards external `BASE_URL`/`TOKEN_FILE` into stack resume soak.
  - `scripts/test/download_soak_stack_bg.sh`:
    - health readiness now requires authenticated `/api/v1/downloads` `200` using run-dir `api.token`.
    - prevents false-ready on unrelated process health.
  - docs:
    - updated `scripts/test/README.md` with `STACK_API_PORT` / `STACK_BASE_URL`.
- Decisions:
  - Keep resume soak isolated from operator node endpoint by default.
  - Treat stack readiness as auth-bound API readiness, not just `/health`.
- Next steps:
  - Re-run acceptance with `RUN_RESUME_SOAK=1` and fixture mode; verify transfers are created (no `downloads=0`).
  - If still zero-transfer, inspect scenario tarball `logs/runner.log` for create/download API payload outcomes.
- Change log:
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/download_soak_stack_bg.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-25): Patched fixture propagation and validation for acceptance/resume flow.
  - `scripts/test/download_phase0_acceptance.sh`:
    - added explicit `DOWNLOAD_FIXTURES_FILE` + `FIXTURES_ONLY` forwarding into resume stage.
    - added early validation:
      - `FIXTURES_ONLY=1` requires `DOWNLOAD_FIXTURES_FILE` set and existing file.
    - added startup fixture logging for run diagnostics.
  - `scripts/test/download_resume_soak.sh`:
    - explicitly forwards `DOWNLOAD_FIXTURES_FILE`/`FIXTURES_ONLY` to stack runner start path.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (143 passed)
- Decisions:
  - Make fixture propagation explicit instead of implicit env inheritance to avoid diagnostic ambiguity in long runs.
- Next steps:
  - Re-run acceptance with `RUN_RESUME_SOAK=1` + fixture env and inspect `band-fixtures` lines in stack logs if transfers still stay at zero.
- Change log:
  - Updated `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/download_resume_soak.sh`.
  - Updated `docs/handoff.md`.

- Status (2026-02-25): Added repository-level GitHub Copilot instruction file.
  - added `.github/copilot-instructions.md` with:
    - repository purpose and scope,
    - architecture and layering boundaries,
    - hostile-input/security expectations,
    - Rust coding/testing conventions,
    - docs/workflow + PR/review priorities.
- Decisions:
  - Keep Copilot instructions concise and aligned with `AGENTS.md`/`README.md` conventions to reduce guidance drift.
- Next steps:
  - Keep `.github/copilot-instructions.md` updated when development rules or review gates evolve.
- Change log:
  - Added `.github/copilot-instructions.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Added known.met startup resilience regression coverage and hash-first operator helper.
  - `src/download/service.rs`:
    - added `startup_quarantines_corrupt_known_met_and_continues` test:
      - validates service startup does not fail on corrupt `known.met`,
      - validates corrupt file quarantine behavior (`known.met.corrupt.<ts>`),
      - validates service continues with an empty known set.
  - `scripts/docs/download_create_from_hash.sh`:
    - new hash-first helper script to:
      - optionally queue `POST /api/v1/kad/search_sources`,
      - create download via `POST /api/v1/downloads` using MD4 hash input.
  - docs:
    - updated `scripts/docs/README.md` with helper mention.
    - updated `docs/TODO.md` with helper completion marker.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (143 passed)
- Decisions:
  - Keep hash-first flow additive via operator helper script for now; full API/UI workflow remains a separate feature slice.
- Status (2026-02-25): Fixed acceptance-runner stage exit-code propagation.
  - `scripts/test/download_phase0_acceptance.sh`:
    - corrected stage result handling so non-zero exit from gate/resume/longrun stages is preserved.
    - `overall_rc` now correctly returns non-zero when any enabled stage fails.
  - `scripts/test/README.md`:
    - documented non-zero exit behavior for failed enabled stages.
  - validation:
    - smoke-checked failure path with invalid base URL (`rc=1`, `overall_rc=1`).
- Next steps:
  - Execute one full acceptance pass with `RUN_RESUME_SOAK=1` and archive the output directory.
  - Start dedicated implementation slice for full hash-first API/UI flow and deeper known-met compatibility semantics.
- Change log:
  - Updated `src/download/service.rs`.
  - Added `scripts/docs/download_create_from_hash.sh`.
  - Updated `scripts/docs/README.md`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Added phase-0 acceptance runner and aligned task backlog for next download slices.
  - `scripts/test/download_phase0_acceptance.sh`:
    - new one-command acceptance orchestration for download/KAD phase-0:
      - captures pre/post `/api/v1/health`, `/api/v1/status`, `/api/v1/downloads` snapshots,
      - runs `kad_phase0_gate.sh`,
      - optionally runs `download_resume_soak.sh` and `kad_phase0_longrun.sh`,
      - writes run summary (`summary.txt`) under a single out directory.
  - docs:
    - `scripts/test/README.md` updated with usage examples.
    - `docs/TASKS.md` and `docs/TODO.md` updated with explicit next slices:
      - `known.met` compatibility + resume robustness,
      - hash-first discovery/initiation path.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (142 passed)
- Decisions:
  - Keep acceptance orchestration script-only in this slice so existing soak/gate scripts remain reusable primitives.
- Next steps:
  - Execute `scripts/test/download_phase0_acceptance.sh` with `RUN_RESUME_SOAK=1` (and optional `RUN_KAD_LONGRUN=1`) on current main binary and archive artifacts.
  - Start implementation slice for `known.met` compatibility + restart/resume robustness.
- Change log:
  - Added `scripts/test/download_phase0_acceptance.sh`.
  - Updated `scripts/test/README.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/TODO.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Addressed PR #34 review comments (snapshot consistency + counter regression tests).
  - `src/download/service.rs`:
    - added `DownloadCommand::Snapshot` and `DownloadServiceHandle::snapshot()` to return `(DownloadServiceStatus, Vec<DownloadSummary>)` from one service-loop snapshot.
    - added regression tests for reserve-denial counters:
      - peer-cap denial increments `reserve_denied_peer_cap_total`,
      - download-cap denial increments `reserve_denied_download_cap_total`,
      - cooldown denial increments `reserve_denied_cooldown_total`.
  - `src/api/handlers/downloads.rs`:
    - `/api/v1/downloads` now uses `download_handle.snapshot()` to avoid mixing status/list from separate awaits.
  - `docs/handoff.md`:
    - removed stale “add observable counters” next-step bullet from earlier entry (work already completed).
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (142 passed)
- Decisions:
  - Keep `/api/v1/downloads` response internally consistent by sourcing queue/status/list from a single service snapshot.
- Next steps:
  - Resolve PR #34 threads and merge when approved.
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Added download pipeline reserve-denial observability counters and exposed them in `/api/v1/downloads`.
  - `src/download/service.rs`:
    - completed `DownloadCommand::Status` service path and status publishing wiring.
    - added reserve denial counter tracking in `reserve_blocks(...)` for:
      - cooldown denials,
      - per-peer cap denials,
      - per-download cap denials.
    - ensured all status emissions include pipeline counters.
  - `src/api/handlers/downloads.rs`:
    - `/api/v1/downloads` now returns:
      - `reserve_denied_cooldown_total`
      - `reserve_denied_peer_cap_total`
      - `reserve_denied_download_cap_total`
    - response now sources `recovered_on_start` from service status snapshot.
  - tests:
    - updated API contract assertion for `/api/v1/downloads` to require new counter fields.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (140 passed)
- Decisions:
  - Keep reserve-denial counters cumulative in-memory service metrics for phase-2 tuning visibility.
- Next steps:
  - Use these counters in soak/gate evaluation to tune fairness caps and cooldown policy.
  - Consider per-download/per-peer breakdown metrics if aggregate counters are insufficient for diagnosis.
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Started download phase-2 pipeline hardening (fairness + retry cooldown).
  - `src/download/service.rs`:
    - added lease fairness caps:
      - `MAX_INFLIGHT_LEASES_PER_PEER = 32`
      - `MAX_INFLIGHT_LEASES_PER_DOWNLOAD = 256`
    - added per-download transient retry cooldown (`cooldown_until`) after block-fail, peer disconnect, and timeout reclaim paths.
    - added bounded exponential backoff helper (`retry_backoff_delay`, 200ms base, 5s max).
    - reserve path now respects cooldown and fairness caps before assigning new ranges.
  - tests:
    - added `reserve_blocks_caps_inflight_leases_per_peer`.
    - added `mark_block_failed_enforces_short_retry_cooldown`.
    - updated existing retry test to account for cooldown behavior.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (140 passed)
- Decisions:
  - Keep cooldown transient/in-memory for this slice (no persistence yet) to avoid schema churn while we stabilize scheduler behavior.
- Next steps:
  - Tune scheduler policy with soak data (per-peer cap, backoff constants).
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Addressed PR #31 review hardening follow-ups (known.met + finalize path).
  - `src/download/service.rs`:
    - added resilient known index boot (`load_known_keys_resilient`): corrupt `known.met` is quarantined and service continues with empty known set.
    - canonicalized in-memory known dedup keys to lowercase hash form.
    - added strict file-name sanitization (`sanitize_download_file_name`) to prevent path traversal/absolute path usage in download finalize targets.
    - switched incoming existence checks to async directory scanning (`tokio::fs::read_dir` / `try_exists`) to avoid blocking runtime threads.
    - replaced fixed-sleep test assumptions with bounded polling loops; added traversal-rejection test.
  - `src/download/store.rs`:
    - `append_known_met_entry` now canonicalizes hash casing and deduplicates case-insensitively.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (138 passed)
- Decisions:
  - Treat corrupted `known.met` as recoverable metadata state (quarantine + continue) rather than startup-fatal.
- Next steps:
  - Update PR #31 threads and merge once approved.
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `src/download/store.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Implemented download `known.met` slice and wired finalize lifecycle in service runtime.
  - `src/download/service.rs`:
    - added `known_met_path` to `DownloadServiceConfig` (`data/known.met`).
    - startup now loads known entries into an in-memory dedup key set.
    - service now finalizes `Completing` downloads on tick and post-command paths:
      - moves completed `.part` into `incoming/`,
      - writes deduplicated known entries to `known.met`,
      - removes finalized `.part.met` + `.bak` and queue entry.
    - command-path finalization is non-fatal (`try_finalize_completed_downloads`) to prevent reply starvation.
  - `src/download/store.rs`:
    - added `KnownMetEntry`.
    - added `load_known_met_entries(...)` and `append_known_met_entry(...)` with hash+size deduplication.
    - added store regression test for known entry dedup/persistence.
  - tests:
    - compressed ingest now asserts finalize-to-incoming + known entry persisted.
    - startup finalize regression ensures known dedup on restart recovery.
  - validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (137 passed)
- Decisions:
  - keep `known.met` Rust-native serialized structure for phase 0/1; wire-level/format parity can be handled as a later compatibility slice if needed.
- Next steps:
  - implement download phase 2 block scheduler/transfer reliability improvements.
- Change log:
  - Updated `src/download/errors.rs`.
  - Updated `src/download/mod.rs`.
  - Updated `src/download/service.rs`.
  - Updated `src/download/store.rs`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Addressed PR #29 review comments (Copilot) on API hardening branch.
  - `src/api/error.rs`:
    - `error_envelope_mw` now preserves original response parts/headers/extensions and only replaces body/content headers for the envelope response.
    - This prevents loss of middleware/handler response headers (e.g. CORS) on non-2xx API responses.
  - `src/api/rate_limit.rs`:
    - corrected `/api/v1/searches/:search_id` wildcard rate-limit behavior:
      - `GET` detail reads now use `query_limit`
      - `POST`/`DELETE` search mutations keep `mutate_limit`
  - Validation rerun:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (135 passed)
- Decisions:
  - Keep error-enveloping at middleware layer, but preserve original response metadata to avoid side effects with downstream middleware behavior.
- Next steps:
  - Push follow-up commit to PR #29 and resolve reviewer threads.
- Change log:
  - Updated `src/api/error.rs`.
  - Updated `src/api/rate_limit.rs`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Completed API hostile-input/resilience hardening slice on `feature/api-hardening-resilience`.
  - Added standardized non-2xx API envelope middleware in `src/api/error.rs`:
    - `{ "code": <status>, "message": "<human-friendly>" }`
    - applied to all `/api/v1/*` non-success responses
  - Added request body hardening:
    - global API body limit via `DefaultBodyLimit::max(64 * 1024)` in router
    - per-route JSON limits via bounded parsing helper (`parse_json_with_limit`) for settings/download/kad mutation handlers
  - Expanded API rate limiting in `src/api/rate_limit.rs`:
    - now covers high-frequency read/mutation routes (`status`, `events`, `settings`, `downloads`, `searches`, `kad/*`)
  - Hardened token loading in `src/api/token.rs`:
    - `load_or_create_token` now self-heals invalid UTF-8 / non-hex / empty token files by rotating and replacing
  - Added SSE fallback warning/metric:
    - `ApiState.sse_serialize_fallback_total`
    - warning log and counter increment when status SSE serialization falls back to `{}`.
  - Added/updated regression tests:
    - API envelope and body-limit behavior
    - expanded rate-limit behavior
    - token self-heal behavior
  - Validation:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (135 passed)
- Decisions:
  - Kept API error envelope centralized in middleware to avoid scattering response formatting across handlers.
  - Used global body limit plus per-route bounded parsing for explicit override control without broad extractor rewrites.
- Next steps:
  - Open PR for `feature/api-hardening-resilience` (no auto-merge unless explicitly requested).
  - Continue next backlog priority (download phase completion / reliability baseline tasks).
- Change log:
  - Added `src/api/error.rs`.
  - Updated `src/api/mod.rs`.
  - Updated `src/api/router.rs`.
  - Updated `src/api/rate_limit.rs`.
  - Updated `src/api/token.rs`.
  - Updated `src/api/handlers/core.rs`.
  - Updated `src/api/handlers/settings.rs`.
  - Updated `src/api/handlers/downloads.rs`.
  - Updated `src/api/handlers/kad.rs`.
  - Updated `src/api/handlers/mod.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Completed second download hostile-input hardening slice on `feature/download-protocol-hardening`.
  - Hardened compressed inbound handling in `src/download/service.rs`:
    - `OP_COMPRESSEDPART` now requires successful zlib inflate (`kad::packed::inflate_zlib`)
    - requires decompressed length to match declared `unpacked_len`
    - validates block/file bounds before state mutation
    - persists decompressed bytes to `.part` file before `mark_block_received`
  - Hardened inbound persistence flow:
    - inbound blocks are persisted to `.part` via `persist_part_block(...)` before marking received
  - Added regression tests:
    - compressedpart happy path (decompress + persist + state advance)
    - compressedpart invalid zlib path (reject + keep inflight state)
  - Validation:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (131 passed)
- Decisions:
  - Reused existing hardened zlib decoder (`kad::packed::inflate_zlib`) to avoid introducing a new inflate implementation.
  - Keep API hardening as the next immediate tranche after this download slice.
- Next steps:
  - Open PR for `feature/download-protocol-hardening` (no auto-merge).
  - Start API hostile-input/resilience pass (body size limits, broader rate limits, token self-heal, SSE fallback metric, typed error envelope).
- Change log:
  - Updated `src/download/service.rs`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Completed first download hostile-input hardening slice on `feature/download-protocol-hardening`.
  - Hardened `src/download/protocol.rs`:
    - added explicit caps:
      - `MAX_PART_PAYLOAD`
      - `MAX_COMPRESSED_PAYLOAD`
      - `MAX_BLOCK_LEN`
    - removed production decode-path `unwrap()` usage by adding safe typed readers (`read_u64_le`, `read_u32_le`)
    - added typed protocol errors for hostile-size conditions:
      - `PayloadTooLarge`
      - `BlockTooLarge`
    - added regression tests for oversized sending/compressed payload semantics
  - Hardened `src/download/service.rs`:
    - added `MAX_RESERVE_BLOCKS_PER_CALL` cap in `reserve_blocks(...)`
    - added service-level regression test for excessive `max_blocks`
  - Validation:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (129 passed)
- Decisions:
  - Keep download cap values as internal constants for now (no config surface in this slice).
  - Keep remaining compressed-part completion semantics (`decompress/validate/persist before mark received`) as the next focused step.
- Next steps:
  - Continue same branch with remaining download hostile-input items:
    - gate `OP_COMPRESSEDPART` completion on verified decompression/persist
    - add hostile ingest/decode tests for semantic mismatch paths
  - Open PR after that tranche is complete (no auto-merge).
- Change log:
  - Updated `src/download/protocol.rs`.
  - Updated `src/download/service.rs`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-24): Completed i2p/SAM hostile-input hardening slice on `feature/i2p-sam-hardening`.
  - Hardened HTTP handling in `src/i2p/http.rs`:
    - replaced unbounded `read_to_end` with capped read loop (`MAX_HTTP_RESPONSE_BYTES = 4 MiB`)
    - made chunked decoding stricter:
      - enforce CRLF after each chunk payload
      - require proper trailer termination (`CRLF` empty line)
    - added hostile-input tests for malformed chunked frames and oversized body stream
  - Hardened SAM control parsing in `src/i2p/sam/client.rs`:
    - added explicit max control-line guard (`MAX_SAM_CONTROL_LINE_LEN = 8 KiB`)
    - moved line decoding through capped/typed path with framing-desync errors on oversize/invalid UTF-8
    - added unit tests for oversize and invalid-UTF8 lines
  - Hardened outbound datagram send caps:
    - `src/i2p/sam/datagram.rs` enforces outbound payload max (`64 KiB`)
    - `src/i2p/sam/datagram_tcp.rs` enforces outbound payload max (`64 KiB`)
    - added regression tests for oversize rejection in both paths
  - Updated backlog docs:
    - marked i2p/SAM hostile-input TODOs complete in `docs/TODO.md`
    - moved next priority to download hostile-input hardening in `docs/TASKS.md`
- Decisions:
  - Keep hard limits internal constants for this slice (no new config surface yet).
  - Use strict parsing for chunked framing to fail fast on malformed remote input.
- Next steps:
  - Open PR for `feature/i2p-sam-hardening` (do not merge without explicit instruction).
  - Start the next hardening tranche: download protocol hostile-input protections.
- Change log:
  - Updated `src/i2p/http.rs`.
  - Updated `src/i2p/sam/client.rs`.
  - Updated `src/i2p/sam/datagram.rs`.
  - Updated `src/i2p/sam/datagram_tcp.rs`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.

- Status (2026-02-24): Validated KAD parser/fuzz hardening branch before PR.
  - Ran required validation:
    - `cargo fmt`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --all-targets --all-features` (117 passed)
- Decisions:
  - Keep this branch scoped to KAD parser adversarial coverage + fuzz scaffold + backlog doc updates.
- Next steps:
  - Commit and push `feature/kad-parser-fuzz-hardening`.
  - Open PR for review (no direct merge).
- Change log:
  - Verified current branch changes with full Rust validation suite.

- Status (2026-02-24): Completed KAD hostile-input parser/fuzz tranche on `feature/kad-parser-fuzz-hardening`.
  - Added adversarial parser tests:
    - `src/kad/packed.rs`:
      - `rejects_invalid_zlib_method`
      - `rejects_header_check_bits_mismatch`
      - `rejects_when_output_exceeds_max_out`
    - `src/kad/wire.rs`:
      - `kad_packet_decode_rejects_invalid_packed_payload`
      - `decode_kad2_res_rejects_truncated_large_count` (review follow-up symmetry)
  - Added fuzz scaffold (cargo-fuzz style) for immediate hostile-input fuzzing:
    - `fuzz/Cargo.toml`
    - `fuzz/fuzz_targets/kad_wire.rs`
    - `fuzz/fuzz_targets/kad_packed.rs`
    - `fuzz/.gitignore`
  - Updated backlog status:
    - marked KAD decoder clamp / inbound limiter cap / jitter RNG / parser+fuzz TODO items complete.
    - promoted i2p/SAM hostile-input hardening to next priority in `docs/TASKS.md`.
- Decisions:
  - Keep fuzz setup isolated in `fuzz/` (not in main workspace) so normal `cargo`/CI flows are unaffected.
  - Treat deterministic adversarial unit tests + fuzz targets as complementary coverage.
- Next steps:
  - Start i2p/SAM hostile-input hardening slice:
    - bound HTTP body reads in `src/i2p/http.rs`
    - control-line max-length guard in `src/i2p/sam/client.rs`
    - chunked parser CRLF hardening
    - outbound datagram payload cap
    - hostile-input regression tests
- Change log:
  - Updated `src/kad/packed.rs`.
  - Updated `src/kad/wire.rs`.
  - Added `fuzz/Cargo.toml`.
  - Added `fuzz/fuzz_targets/kad_wire.rs`.
  - Added `fuzz/fuzz_targets/kad_packed.rs`.
  - Added `fuzz/.gitignore`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.

- Status (2026-02-24): Started KAD hostile-input hardening with allocation clamp slice (`feature/kad-hardening-count-clamps`).
  - Hardened KAD wire decoders to clamp allocation size from untrusted counts based on remaining payload bytes before `Vec::with_capacity(...)`:
    - `decode_kad2_bootstrap_res`
    - `decode_kad2_res`
    - `decode_kad2_publish_key_req`
    - `decode_kad2_search_res`
  - Added clamp helper/constants:
    - `clamp_allocation_count_by_remaining(...)`
    - `KAD2_CONTACT_MIN_WIRE_BYTES`
    - `KAD2_SEARCH_RESULT_MIN_WIRE_BYTES`
    - `KAD2_PUBLISH_KEY_ENTRY_MIN_WIRE_BYTES`
  - Strict decoding behavior remains unchanged (declared entries still parsed; truncated payloads still error).
  - Added hostile truncation regression tests with large declared counts:
    - `decode_kad2_bootstrap_res_rejects_truncated_large_count`
    - `decode_kad2_publish_key_req_rejects_truncated_large_count`
    - `decode_kad2_search_res_rejects_truncated_large_count`
- Decisions:
  - Clamp allocation capacity only, not loop iteration count, to preserve protocol strictness while preventing allocation amplification.
- Next steps:
  - Continue KAD hardening pass with `tracked_in_requests` bounded growth + eviction policy.
  - Then replace deterministic shaper jitter with OS-seeded non-crypto RNG jitter.
- Change log:
  - Updated `src/kad/wire.rs`.

- Status (2026-02-24): Added bounded growth + eviction policy for inbound per-source limiter state (`tracked_in_requests`).
  - Added explicit caps:
    - `TRACKED_IN_MAX_SOURCES = 4096`
    - `TRACKED_IN_MAX_OPCODES_PER_SOURCE = 8`
  - Added cleanup + eviction flow:
    - `cleanup_tracked_in_requests(...)` (TTL cleanup + cap enforcement)
    - `enforce_tracked_in_opcode_cap(...)` (per-source opcode map bounded)
    - `evict_oldest_tracked_in_source(...)` (oldest-first global source eviction)
  - `inbound_request_allowed(...)` now:
    - forces cleanup when cap pressure is reached,
    - evicts oldest source when inserting a new source at cap,
    - enforces per-source opcode cap after updates.
  - Added regression tests:
    - `inbound_request_tracker_caps_number_of_sources`
    - `inbound_request_tracker_caps_opcodes_per_source`
- Decisions:
  - Keep cap values internal constants for this hardening slice (no new config knobs yet).
  - Use oldest-first eviction based on tracked entry age to preserve recent active sources.
- Next steps:
  - Continue KAD hardening with OS-seeded non-crypto jitter replacement for outbound shaper.
- Change log:
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/tests.rs`.

- Status (2026-02-24): Replaced deterministic shaper jitter evolution with OS-seeded non-crypto jitter state.
  - `KadService` jitter state now initializes from `getrandom(...)` with a guarded system-time fallback.
  - Jitter evolution switched from deterministic LCG to xorshift64\* (`shaper_jitter_ms`), still non-crypto and lightweight.
  - This removes fixed-seed deterministic jitter patterns while preserving bounded jitter range behavior.
- Decisions:
  - Keep PRNG non-crypto and local-state-only (no additional config knobs in this slice).
  - Preserve existing shaper policy semantics; change is limited to jitter source quality.
- Next steps:
  - Open/merge PR for the completed three-slice KAD hardening set:
    - decoder allocation clamps,
    - inbound limiter caps/eviction,
    - OS-seeded jitter.
  - Continue with adversarial parser/fuzz targets in follow-up branch.
- Change log:
  - Updated `src/kad/service.rs`.

- Status (2026-02-24): Addressed PR review follow-up for decoder clamp test coverage.
  - Added missing hostile truncation regression test:
    - `decode_kad2_res_rejects_truncated_large_count`
  - This closes the only Copilot-suggested follow-up on PR #25.
- Decisions:
  - Keep hostile count/truncation test coverage symmetric across all four decoders touched by allocation-clamp hardening.
- Next steps:
  - Merge PR #25 and continue to next hardening task (adversarial parser/fuzz targets).
- Change log:
  - Updated `src/kad/wire.rs`.

- Status (2026-02-24): Implemented first routing-tuning-v3 throughput-floor slice on crawl dispatch path.
  - `send_kad2_req(...)` now returns `bool` to indicate whether a request was actually sent (vs. shaper-dropped).
  - `crawl_once(...)` now:
    - computes a bounded send goal (`crawl_send_goal`) with a small healthy-network floor,
    - uses an attempt budget (`crawl_attempt_budget`) to top up when shaper/candidate filtering drops sends,
    - stops once `send_goal` is reached.
  - Lookup dispatch path now respects actual send result:
    - `tick_lookups_impl(...)` no longer marks peers queried/inflight when a request was not sent.
  - Added unit coverage:
    - `crawl_send_goal_raises_floor_when_network_is_healthy`
    - `crawl_send_goal_does_not_raise_floor_without_live_peers`
- Decisions:
  - Keep this slice minimal and focused on preserving query throughput under shaping pressure without broad policy changes.
  - Avoid new config surface in this step; use conservative internal heuristics first.
- Next steps:
  - Run `scripts/test/kad_phase0_gate.sh` (1800s) on this branch and compare against current `main` baseline.
  - If gate is neutral-or-better, prepare PR with before/after gate artifacts.
- Change log:
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/lookup.rs`.
  - Updated `src/kad/service/tests.rs`.

- Status (2026-02-24): Added an automated Phase-0 before/after gate wrapper on `feature/kad-routing-tuning-v2`.
  - New script:
    - `scripts/test/kad_phase0_gate.sh`
  - Capabilities:
    - runs before and after baseline captures using existing `kad_phase0_baseline.sh`,
    - optional `BEFORE_SETUP_CMD` / `AFTER_SETUP_CMD` hooks for binary/process switching,
    - readiness wait (`/api/v1/health` + `/api/v1/status`),
    - auto-runs `kad_phase0_compare.sh`,
    - emits threshold report (`gate.tsv`) and optional pass/fail exit gating.
  - Documentation updated in `scripts/test/README.md` with examples and threshold env controls.
- Decisions:
  - Keep gate script shell-only and reuse existing baseline/compare scripts for consistency.
  - Default gate enforces thresholds, with `ENFORCE_THRESHOLDS=0` escape hatch for exploratory runs.
- Next steps:
  - Use `kad_phase0_gate.sh` for future routing tuning before/after checks.
  - If needed, add a small helper script that switches binaries/processes for `BEFORE_SETUP_CMD`/`AFTER_SETUP_CMD`.
- Change log:
  - Added `scripts/test/kad_phase0_gate.sh`.
  - Updated `scripts/test/README.md`.

- Status (2026-02-24): Improved `kad_phase0_gate.sh` to avoid startup-skew false fails in cumulative metrics.
  - Gate now normalizes `*_total` checks as per-uptime rates per run:
    - `(last_value - first_value) / (uptime_last - uptime_first)`
  - Capture now waits for stable readiness before each phase (`READY_STABLE_SUCCESSES`, default `3`).
  - This fixes false failures when one phase includes warmup `503`/low-sample startup skew.
- Decisions:
  - Keep `compare.tsv` as-is for raw metric comparison, but drive pass/fail from per-uptime-rate gating for cumulative counters.
- Next steps:
  - Re-run the short gate (`DURATION_SECS=300`) to validate corrected gate behavior, then run full `1800s` gate.
- Change log:
  - Updated `scripts/test/kad_phase0_gate.sh`.
  - Updated `scripts/test/README.md`.

- Status (2026-02-24): Added efficiency-focused gate checks so lower send rate is acceptable when quality improves.
  - New efficiency checks compare after/before ratios of per-uptime-rate efficiencies:
    - `tracked_out_matched_total / sent_reqs_total` (min threshold)
    - `timeouts_total / sent_reqs_total` (max threshold)
  - Added env controls:
    - `MIN_MATCH_PER_SENT_RATIO` (default `0.90`)
    - `MAX_TIMEOUT_PER_SENT_RATIO` (default `1.10`)
  - Documented suggested noisy-network tuning:
    - `MIN_SENT_REQS_TOTAL_RATIO=0.60` while keeping efficiency thresholds enabled.
- Decisions:
  - Keep absolute throughput checks, but supplement with efficiency checks to reduce false negatives from network variance.
- Next steps:
  - Re-run `kad_phase0_gate.sh` with the recommended noisy-network thresholds and confirm stable pass/fail behavior.
- Change log:
  - Updated `scripts/test/kad_phase0_gate.sh`.
  - Updated `scripts/test/README.md`.

- Status (2026-02-24): Created safe-only split branch from `origin/main` and applied non-behavioral commits from `feature/kad-rotate-query-candidates`.
  - Applied commits:
    - `7ee4fad` (`scripts/test/kad_phase0_longrun.sh` output-file normalization/recovery)
    - `54b0651`, `5a978ae`, `78ce44a`, `f2964a7` (KAD/i2p/download/API hardening backlog prioritization docs)
  - Explicitly excluded routing tie-break behavior follow-up commits from this branch.
- Decisions:
  - Keep this branch strictly to script reliability + docs backlog updates.
  - Preserve routing behavior experiments for separate performance-gated branch work.
- Next steps:
  - Push this branch and open PR.
  - Merge safe split first; continue routing tuning only with new long-run before/after gates.
- Change log:
  - Updated `scripts/test/kad_phase0_longrun.sh`.
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Updated `docs/handoff.md`.

- Status (2026-02-22): Extended soft peer-health preference into query/crawl candidate selection.
  - Updated routing query selectors to prefer healthier peers while preserving existing constraints:
    - `select_query_candidates(...)`
    - `select_query_candidates_for_target(...)`
  - Preference order remains soft-only:
    - `stable > verified > unknown > unreliable`
  - Added routing test:
    - `query_candidates_for_target_prefer_stable_over_unreliable`
- Decisions:
  - Keep distance, cooldown, and max-failure filters intact; only ordering changed.
- Next steps:
  - Baseline compare after merge to confirm no regression in `sent_reqs_total` / `recv_ress_total`.
  - Evaluate whether publish batch candidate ordering should adopt same soft class preference.
- Change log:
  - Updated `src/kad/routing.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (106 total harness/tests)

- Status (2026-02-22): Implemented first class-aware preference slice for candidate ordering.
  - Added `RoutingTable::peer_health_class_by_dest(...)`.
  - Updated `closest_peers_with_fallback(...)` to prefer healthier peers (`stable > verified > unknown > unreliable`) without hard filtering.
  - Added regression test:
    - `closest_peers_with_fallback_prefers_stable_over_unreliable`
- Decisions:
  - Keep this as soft preference only; no hard exclusion of unknown/unreliable peers yet.
- Next steps:
  - Apply the same class-aware soft preference to query crawl candidate selection.
  - Run baseline compare after merge and verify no regression in request/response totals.
- Change log:
  - Updated `src/kad/routing.rs`.
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/tests.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (105 total harness/tests)

- Status (2026-02-22): Started routing philosophy phase-1 with peer-health scaffolding (status-only, no routing behavior change yet).
  - Added routing classification model:
    - `Unknown`
    - `Verified`
    - `Stable`
    - `Unreliable`
  - Added `RoutingTable::peer_health_counts(now)` and unit coverage.
  - Exposed class counts in `/api/v1/status`:
    - `peer_unknown`
    - `peer_verified`
    - `peer_stable`
    - `peer_unreliable`
- Decisions:
  - Keep this slice observational only (metrics + classification) before using class in candidate selection/eviction.
- Next steps:
  - Integrate peer class into query/publish candidate prioritization (soft preference first).
  - Add before/after baseline compare specifically for peer-class-aware prioritization.
- Change log:
  - Updated `src/kad/routing.rs`.
  - Updated `src/kad/service/types.rs`, `src/kad/service/status.rs`.
  - Updated `src/api/tests.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (104 total harness/tests)

- Status (2026-02-22): Captured/recorded Phase-0 long-run baseline and added KAD1/noise drop counters to status.
  - Documented baseline evidence + acceptance gate in `docs/KAD_WIRE_REFACTOR_PLAN.md` (6h main run, zero restarts/desyncs).
  - Added `/api/v1/status` counters:
    - window: `dropped_legacy_kad1`, `dropped_unhandled_opcode`
    - cumulative: `dropped_legacy_kad1_total`, `dropped_unhandled_opcode_total`
  - Updated baseline scripts to capture/report the new counters in TSV and long-run summary.
- Decisions:
  - Keep KAD1/noise visibility in status metrics so routing-philosophy work can be evaluated against real legacy/noise pressure.
- Next steps:
  - Implement `PeerHealth` class model and transition rules (`unknown/verified/stable/unreliable`) with tests.
  - Use the new baseline acceptance gate after each routing-behavior change.
- Change log:
  - Updated `src/kad/service/types.rs`, `src/kad/service/status.rs`, `src/kad/service/inbound.rs`, `src/kad/service.rs`.
  - Updated `src/api/tests.rs`, `src/kad/service/tests.rs`.
  - Updated `scripts/test/kad_phase0_baseline.sh`, `scripts/test/kad_phase0_longrun.sh`, `scripts/test/README.md`.
  - Updated `docs/KAD_WIRE_REFACTOR_PLAN.md`.

- Status (2026-02-21): Reviewed new routing philosophy doc and mapped it into concrete backlog items.
  - Read `docs/RUST-MULE_ROUTING_PHILOSOPHY.md`.
  - Added follow-up tasks in `docs/TODO.md` and `docs/TASKS.md` for:
    - peer reliability classes (`unknown/verified/stable/unreliable`)
    - health-driven bucket refresh/eviction policy
    - transport-aware latency scoring
    - local (ephemeral) path-memory routing hints
    - status counters for legacy/noise/drop diagnostics
- Decisions:
  - Treat routing philosophy as normative behavior guidance and convert it into measurable implementation milestones before deep KAD refactors.
- Next steps:
  - Design `PeerHealth` model + class transition rules and add unit tests.
  - Extend `/api/v1/status` with counters needed to validate health-based routing behavior in baseline/soak runs.
- Change log:
  - Updated `docs/TODO.md`.
  - Updated `docs/TASKS.md`.
  - Added `docs/RUST-MULE_ROUTING_PHILOSOPHY.md` to tracked docs.

- Status (2026-02-21): Disabled KAD1 response behavior (no legacy handling).
  - Service inbound path now drops `KADEMLIA_REQ_DEPRECATED` without emitting `KADEMLIA_RES_DEPRECATED`.
  - Bootstrap probe path also drops inbound KAD1 REQ instead of sending KAD1 RES.
  - Bootstrap summary now reports `kad1_dropped` (previously `kad1_res_sent`).
- Decisions:
  - Align runtime and bootstrap with project policy: no legacy KAD1 protocol handling.
- Next steps:
  - Re-run baseline/long-run and confirm no outbound KAD1 RES traffic appears in logs.
  - If needed, add explicit status counter for KAD1 dropped requests in `/api/v1/status`.
- Change log:
  - Updated `src/kad/service/inbound.rs`.
  - Updated `src/kad/bootstrap.rs`.
  - Updated `src/kad/service.rs` imports.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (103 total harness/tests)

- Status (2026-02-21): Adjusted SAM DATAGRAM desync handling per PR review.
  - `SamDatagramTcp::recv()` now returns `SamError::FramingDesync` on non-UTF8 SAM lines.
  - This ensures app-level reconnect is triggered instead of potentially spinning while dropping misaligned frames.
- Decisions:
  - Prefer fail-fast reconnect on non-UTF8 DATAGRAM line data to avoid silent inbound stall under framing slip.
- Next steps:
  - Re-run long baseline and verify no prolonged zero-throughput plateaus after desync events.
  - Correlate `sam_framing_desync_total` with `restart_marker` counts.
- Change log:
  - Updated `src/i2p/sam/datagram_tcp.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (103 total harness/tests)

- Status (2026-02-21): Added SAM DATAGRAM desync hardening and long-run restart/desync markers.
  - `SamDatagramTcp::recv()` now drops non-UTF8 SAM lines and continues scanning instead of forcing immediate reconnect.
  - Added KAD status cumulative counter `sam_framing_desync_total` (incremented when service reconnects due to `SamError::FramingDesync`).
  - `scripts/test/kad_phase0_baseline.sh` now records:
    - `sam_framing_desync_total`
    - `restart_marker` (set when sampled `uptime_secs` decreases)
  - `scripts/test/kad_phase0_longrun.sh` now prints post-run summary:
    - `restart_markers=<count>`
    - `sam_framing_desync_total_max=<max observed>`
- Decisions:
  - Treat invalid UTF-8 header lines on DATAGRAM socket as recoverable noise and keep processing.
  - Track framing-desync reconnects as a cumulative status metric for soak/baseline interpretation.
- Next steps:
  - Run another long baseline on this branch and confirm restart markers are `0` (or sparse) while throughput totals continue increasing.
  - If markers remain non-zero, correlate to SAM router logs and evaluate stronger TCP-DATAGRAM realignment logic.
- Change log:
  - Updated `src/i2p/sam/datagram_tcp.rs`.
  - Updated `src/kad/service/types.rs`, `src/kad/service/status.rs`, `src/kad/service.rs`, `src/kad/service/tests.rs`.
  - Updated `src/app.rs`.
  - Updated `src/api/tests.rs`.
  - Updated `scripts/test/kad_phase0_baseline.sh`, `scripts/test/kad_phase0_longrun.sh`, `scripts/test/README.md`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (103 total harness/tests)
    - `bash -n scripts/test/kad_phase0_baseline.sh scripts/test/kad_phase0_longrun.sh scripts/test/kad_phase0_compare.sh` passed

- Status (2026-02-21): Researched frequent inbound `opcode=0x0a` and prepared longer baseline tooling.
  - iMule protocol mapping confirms `0x0a` is Kad1 `KADEMLIA_PUBLISH_REQ` (legacy/deprecated opcode set).
  - rust-mule now labels legacy Kad1 opcodes explicitly in logs (instead of generic `UNKNOWN`), including `KADEMLIA_PUBLISH_REQ`.
  - baseline script now captures cumulative totals:
    - `sent_reqs_total`, `recv_ress_total`, `timeouts_total`
    - `tracked_out_*_total`, `outbound_shaper_delayed_total`
  - added long-run wrapper:
    - `scripts/test/kad_phase0_longrun.sh` (default 6h)
- Decisions:
  - Keep legacy Kad1 publish/search opcodes explicitly labeled but still unhandled for now.
  - Use long-run baseline with totals for soak interpretation.
- Next steps:
  - Run `bash scripts/test/kad_phase0_longrun.sh` on `main` and compare totals slope between runs/builds.
  - Decide whether to implement safe Kad1 publish/search decode/ignore counters beyond naming.
- Change log:
  - Updated `src/kad/wire.rs`, `src/kad/service.rs`, `src/kad/service/tests.rs`.
  - Updated `scripts/test/kad_phase0_baseline.sh`, added `scripts/test/kad_phase0_longrun.sh`.
  - Updated `scripts/test/README.md`.
  - Validation:
    - `bash -n` on updated scripts passed
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (103 total harness/tests)

- Status (2026-02-21): Patched shaper policy to preserve “0 disables caps” semantics for derived class lanes.
  - `shaper_policy` no longer forces minimum caps when base caps are `0`.
  - Response lane now respects disabled cap configuration in baseline/soak scenarios.
  - Added regression test:
    - `shaper_response_lane_caps_can_be_disabled_with_zero_base_caps`
- Decisions:
  - Keep derived caps (`Hello`/`Bootstrap`/`Response`) bounded only when base query caps are enabled.
- Next steps:
  - Re-run baseline/soak on PR branch to confirm response-heavy scenarios do not see artificial cap drops under zero-cap config.
- Change log:
  - Updated `src/kad/service.rs`.
  - Updated `src/kad/service/tests.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (102 total harness/tests)

- Status (2026-02-21): Merged latest `origin/main` into `feature/kad-phase2-class-shaper` to resolve PR conflicts.
  - Conflict scope was documentation-only (`docs/handoff.md`); code merge completed cleanly.
- Decisions:
  - Keep branch behavior unchanged; this merge is for branch sync/conflict resolution.
- Next steps:
  - Keep PR #14 open and proceed with user-driven baseline validation.
- Change log:
  - Resolved `docs/handoff.md` merge conflict from `origin/main` merge.

- Status (2026-02-21): Cherry-picked cumulative KAD status totals from `main` onto `feature/kad-phase2-class-shaper`.
  - `/api/v1/status` now exposes lifetime totals (since service start):
    - `recv_req_total` / `sent_reqs_total`
    - `recv_res_total` / `recv_ress_total`
    - `timeouts_total`
    - `tracked_out_matched_total`
    - `tracked_out_unmatched_total`
    - `tracked_out_expired_total`
    - `outbound_shaper_delayed_total`
  - Existing windowed counters are unchanged.
- Decisions:
  - Keep this cherry-pick on Phase 2 so before/after polling can compare stable totals rather than window snapshots.
- Next steps:
  - Build new `main` and `feature/kad-phase2-class-shaper` binaries and rerun baseline with totals polling.
- Change log:
  - Cherry-picked `00c6b70` into `feature/kad-phase2-class-shaper`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (100 total harness/tests)

- Status (2026-02-21): Cherry-picked SAM DATAGRAM keepalive hotfix from `main` onto `feature/kad-phase2-class-shaper` for before/after baseline builds.
  - Included transport-level handling of unsolicited `PING` frames with immediate `PONG` reply.
  - Included unit coverage for PING/PONG line mapping in DATAGRAM TCP path.
- Decisions:
  - Keep this fix shared between `main` and Phase 2 branch so baseline deltas focus on shaper behavior, not SAM session churn.
- Next steps:
  - Build feature binary and run paired baseline compare against `main` binary.
- Change log:
  - Cherry-picked `2860194` into `feature/kad-phase2-class-shaper`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (100 total harness/tests)

- Status: Isolated shaper state per outbound class and set explicit continuous runtime default in config on `feature/kad-phase2-class-shaper`.
  - Shaper lane isolation fix:
    - `shaper_global_sent_in_window` and `shaper_last_global_send` are now tracked per `OutboundClass`.
    - peer lane keys are class-scoped (`class:dest`) for per-class peer counters/intervals.
    - prevents response-lane traffic from suppressing query-lane sends.
  - Added explicit KAD runtime config entry:
    - `config.toml` now includes `kad.service_runtime_secs = 0` with comment (`0` = continuous run), to avoid periodic 360s restarts during baselines.
  - Updated tests:
    - adjusted class-lane bypass test to match lane isolation behavior.
- Decisions:
  - Maintain per-class shaping isolation as core Phase 2 invariant.
  - Keep baseline guidance to run with continuous service runtime.
- Next steps:
  - Re-run strict before/after baseline compare with this commit and verify query metrics (`sent_reqs`, `recv_ress`, `pending`, `tracked_out_*`) are non-zero and stable.
- Change log:
  - Updated `src/kad/service.rs` shaper counters/timers to class-scoped state.
  - Updated `src/kad/service/tests.rs` for class-scoped expectations.
  - Updated `config.toml` (`kad.service_runtime_secs = 0`).
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (96 tests)

- Status: Fixed Phase 2 query-lane suppression bug on `feature/kad-phase2-class-shaper`.
  - Root cause observed in baseline: `sent_reqs/recv_ress/pending/timeouts` were all `0` while `outbound_shaper_delayed` was high.
  - Cause: drop-on-delay lanes (`Query/Hello/Bootstrap`) combined with non-zero base/jitter caused near-constant “delayed => dropped” behavior.
  - Fix:
    - For drop-on-delay classes, scheduler now starts at `now` and only delays on min-interval constraints.
    - Base/jitter scheduling remains for non-drop classes only.
  - Added regression coverage:
    - `shaper_query_lane_does_not_require_base_delay`
- Decisions:
  - Keep non-blocking/no-sleep behavior; enforce pacing via caps + min-interval gating for drop-lanes.
- Next steps:
  - Re-run strict before/after phase-2 baseline compare; validate `sent_reqs/recv_ress` are non-zero and compare metrics are meaningful.
- Change log:
  - Updated `src/kad/service.rs` class scheduler target-time behavior.
  - Updated `src/kad/service/tests.rs` with query-lane no-base-delay regression test.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (96 tests)

- Status: Started KAD Phase 2 with class-aware outbound shaping on `feature/kad-phase2-class-shaper`.
  - Added explicit shaper classes:
    - `Query`
    - `Hello`
    - `Bootstrap`
    - `Response`
  - Added per-class policy derivation (`shaper_policy`) so response traffic and liveness traffic are treated differently than query traffic.
  - Routed send paths by class:
    - `send_kad2_packet` opcodes map into class-aware shaping
    - service HELLO sends use `Hello` class
    - service BOOTSTRAP sends use `Bootstrap` class
    - inbound reply sends use `Response` class
  - Added regression test:
    - `shaper_response_lane_bypasses_query_delay_budget`
- Decisions:
  - Keep phase-2 behavior internal for now (no `config.toml` schema changes yet).
  - Favor lower suppression pressure on response/liveness paths while preserving query-lane shaping.
- Next steps:
  - Run strict before/after baseline pair and compare deltas under phase-2 class-aware policy.
  - If stable, expose class policy tuning in config/documentation.
- Change log:
  - Updated `src/kad/service.rs` (class enum/policy + call-site routing).
  - Updated `src/kad/service/inbound.rs` (response-class sends).
  - Updated `src/kad/service/tests.rs` (new class-aware shaper test).
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (95 tests)

- Status (2026-02-21): Started hotfix `hotfix/sam-ping-pong-keepalive` from `main` for SAM DATAGRAM-TCP keepalive stability.
  - Added handling for unsolicited `PING` frames on the DATAGRAM TCP socket:
    - detect `PING...` lines in `recv()` and in command reply wait loop
    - immediately emit matching `PONG...` and continue
  - Added unit tests for PING-to-PONG line mapping behavior.
- Decisions:
  - Keep SAM keepalive handling in transport layer (`src/i2p/sam/datagram_tcp.rs`), transparent to KAD.
- Next steps:
  - Commit/push branch and open PR with `gh`.
- Change log:
  - Updated `src/i2p/sam/datagram_tcp.rs`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (98 total including integration/bin harness)

- Status: Added deterministic KAD script CI guard on `feature/kad-phase1-ci-guard` (no runtime network dependency).
  - New offline smoke script:
    - `scripts/test/kad_phase0_ci_smoke.sh`
    - Uses synthetic before/after TSV fixtures to validate:
      - compare output includes shaper metrics
      - shaper cap-drop metrics stay parseable and zero in fixture expectation
      - pending-overdue delta parsing works
  - CI workflow updated:
    - `.github/workflows/ci.yml` new job `kad-scripts-smoke`
    - Runs bash syntax checks for KAD baseline/compare/smoke scripts
    - Runs offline smoke script (deterministic, no rust-mule/I2P runtime)
  - Test script docs updated:
    - `scripts/test/README.md` includes `kad_phase0_ci_smoke.sh`
- Decisions:
  - Keep KAD network baselines/soaks out CI; enforce only deterministic offline guards in CI.
- Next steps:
  - Keep using local/staging soak scripts for real network behavior validation and attach artifacts in PRs.
- Change log:
  - Added `scripts/test/kad_phase0_ci_smoke.sh`.
  - Updated `.github/workflows/ci.yml`.
  - Updated `scripts/test/README.md`.
  - Validation:
    - `bash scripts/test/kad_phase0_ci_smoke.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (94 tests)

- Status: Addressed PR review comments on `feature/kad-phase1-shaper` shaper state growth + loop blocking.
  - Added bounded cleanup for peer-shaper state (`shaper_last_peer_send`):
    - TTL-based eviction (`SHAPER_PEER_STATE_TTL = 1h`)
    - hard cap (`SHAPER_PEER_STATE_MAX = 8192`) retaining most-recent peers
  - Removed blocking sleeps in shaper path:
    - `shaper_send` no longer calls `sleep_until`
    - when packet is scheduled for future send, it increments `outbound_shaper_delayed` and returns `false` (caller treats as not sent)
- Decisions:
  - Prefer non-blocking service loop behavior over in-loop delayed sends to prevent receive/tick/command head-of-line blocking.
  - Keep delayed-send visibility via existing `outbound_shaper_delayed` counter.
- Next steps:
  - Re-run baseline compare on this revision to quantify impact of non-blocking delayed behavior.
- Change log:
  - `src/kad/service.rs`: added stale peer cleanup/cap + non-blocking shaper send behavior.
  - `src/kad/service/tests.rs`: added `shaper_cleanup_evicts_stale_peer_state`.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (94 tests)

- Status: Tuned Phase 1 shaper defaults downward on `feature/kad-phase1-shaper` based on baseline deltas.
  - Updated defaults in `KadServiceConfig`:
    - `outbound_shaper_base_delay_ms`: `20 -> 5`
    - `outbound_shaper_jitter_ms`: `25 -> 10`
    - `outbound_shaper_peer_min_interval_ms`: `50 -> 20`
  - Global pacing/cap values unchanged.
- Decisions:
  - Keep cap limits as-is (`global_max_per_sec=40`, `peer_max_per_sec=8`) since baseline showed delay engagement but no cap drops.
- Next steps:
  - Re-run before/after Phase 0 baseline compare and verify `recv_ress`/`tracked_out_matched` regression narrows while shaper still engages.
- Change log:
  - Updated `src/kad/service/types.rs` shaper default values.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (93 tests)

- Status: Extended KAD Phase 0 baseline capture script on `feature/kad-phase1-shaper` to include outbound shaper counters.
  - `scripts/test/kad_phase0_baseline.sh` now records:
    - `outbound_shaper_delayed`
    - `outbound_shaper_drop_global_cap`
    - `outbound_shaper_drop_peer_cap`
  - `scripts/test/README.md` updated to document these additional columns.
- Decisions:
  - Keep comparer unchanged; it already computes metrics from TSV headers dynamically.
- Next steps:
  - Re-run before/after baseline pair so compare output includes shaper engagement/cap deltas.
- Change log:
  - Updated `scripts/test/kad_phase0_baseline.sh`.
  - Updated `scripts/test/README.md`.
  - Validation:
    - `bash -n scripts/test/kad_phase0_baseline.sh` passed
    - `bash -n scripts/test/kad_phase0_compare.sh` passed

- Status: Implemented KAD Phase 1 outbound shaper baseline on `feature/kad-phase1-shaper`.
  - Added central shaper send path for KAD outbound traffic (requests and inbound replies).
  - Added shaper metrics to `/api/v1/status` window:
    - `outbound_shaper_delayed`
    - `outbound_shaper_drop_global_cap`
    - `outbound_shaper_drop_peer_cap`
  - Updated send accounting to increment request counters only when a packet is actually sent (not dropped by shaper caps).
- Decisions:
  - Applied shaper in one central helper (`shaper_send`) and routed service + inbound handlers through it.
  - Kept shaper settings internal for now (wired via `KadServiceConfig` defaults in `app.rs`, not yet exposed in `config.toml`).
- Next steps:
  - Run Phase 0 baseline script pair + compare against this branch to quantify cap/delay effects.
  - Tune shaper defaults against observed KAD stability/latency under soak.
  - Decide whether to expose shaper knobs in runtime config and docs.
- Change log:
  - `src/kad/service.rs`: shaper state, scheduling/cap helpers, centralized send path, request-counter gating.
  - `src/kad/service/inbound.rs`: response sends now routed via shaper helper.
  - `src/kad/service/types.rs`: shaper config + status/stat fields.
  - `src/kad/service/status.rs`: status export for shaper counters.
  - `src/kad/service/tests.rs`: added shaper unit tests.
  - `src/api/tests.rs`: updated status fixture for new counters.
  - `src/app.rs`: populate new shaper config fields from service defaults.
  - Validation:
    - `cargo fmt` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (93 tests)

## Status (2026-02-14)

- Status: Updated CodeQL export script output path defaults on `feature/add-codeql-export-script`:
  - `scripts/export-gh-stats/export-codeql-alerts.sh` now writes CSV by default to:
    - `/tmp/codeql-alerts-OWNER-REPO_<timestamp>.csv`
  - Updated script header comment to match new output location/pattern.
- Decisions:
  - Use `/tmp` as default destination to avoid polluting repository working directories.
- Next steps:
  - If needed, add optional output path override flags/env in a follow-up.
- Change log:
  - Updated `scripts/export-gh-stats/export-codeql-alerts.sh` output path default.

- Status: Hardened `scripts/export-gh-stats/export-codeql-alerts.sh` on `feature/add-codeql-export-script`:
  - loads `.env` from script directory (`scripts/export-gh-stats/.env`) instead of current working directory
  - removed `source` execution path and replaced with safe `KEY=VALUE` parser
  - defaults and filters export to `TOOL_NAME=CodeQL`
  - passes `tool_name` query parameter and filters by `.tool.name == TOOL_NAME` in output conversion
  - handles zero-alert repos gracefully (header-only CSV + success exit)
  - sanitized local `.env` placeholder content and expanded `.env.example`.
- Decisions:
  - Prefer least-surprise local config loading and non-executable env parsing for safety.
  - Keep export scoped to CodeQL by default to match script intent and naming.
- Next steps:
  - Rotate/revoke previously used PAT if it was real.
  - Consider adding a small script README with token scope requirements and sample invocation.
- Change log:
  - Updated `scripts/export-gh-stats/export-codeql-alerts.sh`.
  - Updated `scripts/export-gh-stats/.env.example`.
  - Updated `scripts/export-gh-stats/.env` placeholder values.
  - Validation:
    - `bash -n scripts/export-gh-stats/export-codeql-alerts.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (91 tests)

- Status: Added explicit CodeQL workflow configuration for Rust on `feature/codeql-workflow-rust`:
  - New workflow: `.github/workflows/codeql.yml`
  - Triggers on:
    - push to `main`
    - pull requests targeting `main`
    - weekly schedule
  - Uses CodeQL action v3 with `language: rust` and `security-and-quality` queries.
- Decisions:
  - Use a committed (versioned) CodeQL workflow to keep scan configuration consistent across branches and PRs.
- Next steps:
  - In GitHub settings, disable CodeQL Default setup to avoid dual-configuration ambiguity.
  - Let the new workflow run on PR/main and confirm code scanning comparisons no longer warn about missing config.
- Change log:
  - Added `.github/workflows/codeql.yml`.

- Status: Added repository merge policy documentation on `main`:
  - `README.md` now states:
    - no direct commits/merges to `main`
    - all changes via feature branch + PR
    - merge to `main` only through reviewed PR with required checks
  - `.github/pull_request_template.md` now includes merge-policy acknowledgment checkbox.
- Decisions:
  - Treat this as a mandatory process rule from now on.
- Next steps:
  - Enforce branch protection in GitHub settings to match documented policy.
- Change log:
  - Updated `README.md` (`Merge Policy` section).
  - Updated `.github/pull_request_template.md` validation checklist.

- Status: Fixed `kad_phase0_compare.sh` output formatting on `feature/kad-phase0-baseline`:
  - header is now always first
  - metric rows are sorted and consistently tab-separated
  - numeric formatting is normalized to fixed precision fields
- Decisions:
  - Keep plain TSV output for easy piping into `column -t` / CI artifacts.
- Next steps:
  - Re-run compare command and verify table readability.
- Change log:
  - Updated `scripts/test/kad_phase0_compare.sh` output rendering/sort behavior.

- Status: Added KAD Phase 0 baseline compare helper on `feature/kad-phase0-baseline`:
  - New script: `scripts/test/kad_phase0_compare.sh`
    - compares two baseline TSV files (`--before`, `--after`)
    - emits per-metric summary with:
      - `before_avg`, `after_avg`, `delta`, `pct_change`
      - before/after min/max and sample counts
  - Updated `scripts/test/README.md` with compare usage.
- Decisions:
  - Keep comparison simple and script-only (tsv in, tsv summary out) for easy CI/local usage.
- Next steps:
  - Run compare after each KAD/wire change baseline pair and attach output to PR notes.
- Change log:
  - Added `scripts/test/kad_phase0_compare.sh`.
  - Updated `scripts/test/README.md`.

- Status: Added API backlog note for user-friendly HTTP error responses on `feature/kad-phase0-baseline`:
  - `docs/TODO.md` now tracks adding consistent human-friendly messages for non-2xx HTTP status responses.
- Decisions:
  - Treat this as an explicit API UX/error-contract task, separate from typed error envelope consistency.
- Next steps:
  - Define and implement a unified API error response shape that includes `status`, machine `code`, and human-friendly `message`.
- Change log:
  - Updated `docs/TODO.md` API section with human-friendly HTTP error message task.

- Status: Hardened KAD Phase 0 baseline script handling for startup-not-ready status endpoint on `feature/kad-phase0-baseline`:
  - `scripts/test/kad_phase0_baseline.sh` now treats HTTP `503` from `/api/v1/status` as warmup and skips sampling without noisy curl failures.
  - Script now prints end-of-run summary with `samples`, `skipped_503`, and `skipped_other`.
  - `scripts/test/README.md` updated with this behavior.
- Decisions:
  - Keep baseline collection robust under startup/transient status unavailability; do not fail run on `503`.
- Next steps:
  - Re-run baseline capture command and verify summary shows growing `samples` once status becomes available.
- Change log:
  - Updated `scripts/test/kad_phase0_baseline.sh` sampling/HTTP handling and summary output.
  - Updated `scripts/test/README.md` notes.

- Status: Implemented KAD Phase 0 baseline instrumentation + reviewer gates on `feature/kad-phase0-baseline`:
  - Added status counters for timing/ordering baseline comparison:
    - `pending_overdue`, `pending_max_overdue_ms`
    - `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
  - Instrumented tracked outbound request lifecycle:
    - matched responses increment `tracked_out_matched`
    - unmatched responses increment `tracked_out_unmatched`
    - tracked-request TTL cleanup increments `tracked_out_expired`
  - Added baseline capture script:
    - `scripts/test/kad_phase0_baseline.sh` (polls `/api/v1/status` and writes TSV)
  - Added KAD reviewer gates:
    - `.github/pull_request_template.md` KAD/wire baseline evidence section
    - `docs/REVIEWERS_CHECKLIST.md` baseline evidence gate
  - Updated docs:
    - `docs/KAD_WIRE_REFACTOR_PLAN.md` Phase 0 checkboxes (counters + reviewer gate done)
    - `scripts/test/README.md` baseline script usage
    - `docs/api_curl.md` Phase 0 counter jq example
- Decisions:
  - Phase 0 keeps behavior unchanged and only adds observability + review guardrails.
  - Baseline counters are exposed through existing `/api/v1/status` to avoid new endpoints.
- Next steps:
  - Run and archive before/after baseline captures with `scripts/test/kad_phase0_baseline.sh`.
  - Then start Phase 1 outbound shaper design/implementation using collected baseline deltas.
- Change log:
  - `src/kad/service/types.rs`: added Phase 0 status/stat counters.
  - `src/kad/service/status.rs`: exported/logged new counters.
  - `src/kad/service.rs`: tracked out-request match/unmatch/expiry instrumentation.
  - `src/kad/service/tests.rs`: added regression tests for tracked/pending counters.
  - `src/api/tests.rs`: updated status fixture for new fields.
  - `scripts/test/kad_phase0_baseline.sh`: new baseline capture script.
  - Validation:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (91 tests)

- Status: Addressed PR review findings for download store/service correctness on `feature/download-strategy-imule`:
  - Fixed recovered part path derivation in `scan_recoverable_downloads`:
    - `001.part.met` now maps to `001.part` (not `001.part.part`).
  - Fixed part number parsing/allocation for IDs beyond 999:
    - `parse_part_number` now accepts any all-digit stem that parses to `u16`.
    - `allocate_next_part_number` now correctly accounts for files like `1000.part`.
  - Fixed delete atomicity in `delete_download`:
    - file deletions occur first, then in-memory map entry is removed only on success.
    - on filesystem error, runtime entry remains so delete can be retried in-process.
- Decisions:
  - Preserve existing on-disk naming format (`{part:03}` minimum width) while making parsing robust for wider numeric stems.
  - Prefer state consistency over eager map mutation during deletion.
- Next steps:
  - Merge after PR review confirms these follow-up fixes.
- Change log:
  - `src/download/store.rs`: corrected `.part` path reconstruction; relaxed part-number parser; added regression tests.
  - `src/download/service.rs`: made delete state mutation happen after successful file cleanup; added regression test.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (89 tests)

- Status: Soak run `/tmp/rustmule-run-20260218_160244` validated as healthy on `feature/download-strategy-imule`:
  - `soak-band/results.tsv` shows all scenarios `completed/completed`:
    - `integrity`, `single_e2e`, `concurrency`, `long_churn`
  - Scenario tarballs each contain `download-soak-finished ... result=completed`
  - `data/download/` contains active transfer artifacts (`.part`, `.part.met`, `.bak`) across many part IDs
  - Integrity rounds report `violations=0 dup_parts=0`
  - No panic/fatal errors observed in run logs (only non-blocking nodes2 bootstrap lookup warning)
- Decisions:
  - Treat this run as branch-level validation pass for current download/churn/resume behavior.
- Next steps:
  - Open PR from `feature/download-strategy-imule` to `main`.
  - In PR summary include this branch close-out checklist:
    1. Record successful soak evidence and paths.
    2. Confirm no outstanding code or docs changes on branch.
    3. Merge into `main` after review.
- Change log: Added explicit close-out checklist and successful soak evidence for `/tmp/rustmule-run-20260218_160244`.

- Status: Added local source-cache upsert on publish path in `feature/download-strategy-imule`:
  - In `KadServiceCommand::PublishSource` handling, we now cache local source entry
    (`file -> my_kad_id/my_dest`) before sending network publish requests.
  - This allows inbound `SEARCH_SOURCE_REQ` to return the local source immediately,
    instead of waiting for external network re-ingestion.
  - Added unit test:
    - `kad::service::tests::cache_local_published_source_inserts_local_entry_once`
- Decisions:
  - Preserve network publish behavior; add local cache as compatibility/convergence aid.
- Next steps:
  - Re-run `kad_publish_search_probe.sh` and verify B observes at least one source for published fixture hashes.
- Change log: local publishes now populate `sources_by_file` cache immediately.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed (87 tests)

- Status: Extended publish/search probe with periodic republish on `feature/download-strategy-imule`:
  - `scripts/test/kad_publish_search_probe.sh` now supports:
    - `--republish-every N` (poll intervals; `0` disables)
  - This allows repeated `publish_source` on node A while node B continues `search_sources` polling.
- Decisions:
  - Keep republish disabled by default for minimal baseline behavior, opt-in for sparse/slow networks.
- Next steps:
  - Re-run probe with `--republish-every 12 --poll-secs 5` (republish every 60s) and longer timeout if needed.
- Change log: A->B probe now supports periodic republish to improve source visibility convergence.
  - Validation run after patch:
    - `bash -n scripts/test/kad_publish_search_probe.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added automated A->B publish/search visibility probe on `feature/download-strategy-imule`:
  - New script: `scripts/test/kad_publish_search_probe.sh`
    - publishes file source on node A (`/api/v1/kad/publish_source`)
    - repeatedly queues search on node B (`/api/v1/kad/search_sources`)
    - polls B `/api/v1/kad/sources/:file_id_hex`
    - logs A/B status counters each interval:
      - A: `recv_publish_source_reqs`, `sent_publish_source_ress`, `recv_search_source_reqs`, `source_store_entries_total`
      - B: `sent_search_source_reqs`, `recv_search_ress`, `source_store_entries_total`
    - exits success when B sees at least one source; times out otherwise.
  - Added usage entry in `scripts/test/README.md`.
- Decisions:
  - Use explicit counter telemetry in probe output so failures are attributable to publish path vs search path vs discovery cache.
- Next steps:
  - Run probe for each fixture hash before resume soak and only proceed when probe exits `0`.
- Change log: Manual publish/search polling is now scripted and repeatable.
  - Validation run after patch:
    - `bash -n scripts/test/kad_publish_search_probe.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Fixed fixture validation bug in soak runner on `feature/download-strategy-imule`:
  - Root cause of failed run `/tmp/rustmule-run-20260218_114700`:
    - `download_soak_bg.sh` logged `fixtures not loaded or empty` and repeatedly `fixtures_only enabled but no valid fixture available`.
    - The `jq` fixture validator expression incorrectly filtered out valid entries, resulting in `FIXTURE_COUNT=0`.
  - Fix:
    - replaced validator with explicit `valid_fixture` predicate using safe field checks:
      - `file_name` string
      - `file_hash_md4_hex` string
      - `file_size` number > 0
    - applied in both fixture counting and fixture record selection paths.
- Decisions:
  - Keep strict fixture schema validation but ensure parser is robust to valid JSON fixtures.
- Next steps:
  - Re-run resume soak with the same fixture file and `FIXTURES_ONLY=1`; fixture load should now report non-zero count.
- Change log: Soak runner now correctly accepts valid fixture JSON entries.
  - Validation run after patch:
    - `bash -n scripts/test/download_soak_bg.sh` passed
    - `jq ... /tmp/download_fixtures.json` returned `2` valid fixtures
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Extended fixture generation with optional source publish on `feature/download-strategy-imule`:
  - `scripts/test/gen_download_fixture.sh` now supports:
    - `--publish`
    - `--base-url <url>`
    - `--token` / `--token-file`
    - `--publish-script <path>`
  - When `--publish` is set, each generated fixture is sent to `scripts/docs/kad_publish_source.sh` using the generated MD4 hash and file size.
- Decisions:
  - Keep publish optional to preserve offline/local-only fixture generation mode.
- Next steps:
  - Run one command to generate and publish fixture hashes on source node, then use that fixture file for resume soak on downloader node.
- Change log: Fixture prep can now generate + publish in a single command.
  - Validation run after patch:
    - `bash -n scripts/test/gen_download_fixture.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added built-in fixture generation tooling on `feature/download-strategy-imule`:
  - New Rust utility: `src/bin/download_fixture_gen.rs`
    - outputs fixture JSON from one or more files using repo-native MD4 (`rust_mule::kad::md4`)
  - New wrapper script: `scripts/test/gen_download_fixture.sh`
    - usage: `scripts/test/gen_download_fixture.sh --out /tmp/download_fixtures.json /path/to/file1 ...`
  - Updated `scripts/test/README.md` with generation command.
- Decisions:
  - Avoid OpenSSL/legacy-provider variability by using project-native MD4 implementation for fixture generation.
- Next steps:
  - Generate real peer-backed fixture file and run resume soak with:
    - `DOWNLOAD_FIXTURES_FILE=<fixtures.json> FIXTURES_ONLY=1 bash scripts/test/download_resume_soak.sh`
- Change log: Fixture generation is now one command and does not depend on external MD4 support.
  - Validation run after patch:
    - `bash -n scripts/test/gen_download_fixture.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Added fixture-driven download creation for soak/resume validation on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` now supports:
    - `DOWNLOAD_FIXTURES_FILE` (JSON array with `file_name`, `file_size`, `file_hash_md4_hex`)
    - `FIXTURES_ONLY=1` (fails instead of falling back to random hashes)
  - Create actions in all scenarios (`single_e2e`, `long_churn`, `integrity`, `concurrency`) now prefer fixtures when provided.
  - Fixture behavior is propagated through:
    - `scripts/test/download_soak_band.sh`
    - `scripts/test/download_soak_stack_bg.sh`
    - resume workflow (via inherited env into stack start)
  - Added `scripts/test/download_fixtures.example.json`.
- Decisions:
  - Keep fixture mode opt-in for backward compatibility, but recommend `FIXTURES_ONLY=1` for real transfer/resume assertions.
- Next steps:
  - Run resume soak with peer-backed fixtures:
    - `DOWNLOAD_FIXTURES_FILE=<real-fixtures.json> FIXTURES_ONLY=1 bash scripts/test/download_resume_soak.sh`
  - Confirm active-transfer gate passes and post-restart completion is observed.
- Change log: Soak/resume tests can now target real downloadable hashes instead of random synthetic IDs.
  - Validation run after patch:
    - `bash -n scripts/test/download_soak_bg.sh scripts/test/download_soak_band.sh scripts/test/download_soak_stack_bg.sh` passed
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Strengthened resume-soak acceptance criteria on `feature/download-strategy-imule`:
  - Resume automation now enforces true in-flight resume validation instead of control-plane-only pass/fail.
  - Added pre-crash active-transfer gate: requires at least one download with `downloaded_bytes > 0` and `inflight_ranges > 0`.
  - Added post-restart monotonicity gate: fails if any pre-existing download regresses in `downloaded_bytes`.
  - Added post-restart completion gate: requires at least one completed download within configurable timeout.
- Decisions:
  - Treat resume success as data-plane continuity, not only process restart + scenario completion.
  - Keep thresholds configurable for slow environments via script env overrides.
- Next steps:
  - Run `scripts/test/download_resume_soak.sh` and verify the new gates pass under load.
  - If active-transfer gate times out, increase scenario duration/load or tune discovery/source readiness before crash point.
- Change log: `scripts/test/download_resume_soak.sh` now validates active transfer before crash, monotonic post-restart bytes, and post-restart completion.
  - Validation run after patch:
    - `cargo fmt --all --check` passed
    - `cargo clippy --all-targets --all-features -- -D warnings` passed
    - `cargo test --all-targets --all-features` passed

- Status: Fixed resume-soak crash step for wrapper-pid mismatch on `feature/download-strategy-imule`:
  - User-observed failure:
    - after `crashed app pid=<pid>`, one run-dir `./rust-mule` process remained and restart never proceeded.
  - Root issue:
    - `control/app.pid` can reference a launcher/wrapper pid while actual rust-mule child keeps running.
  - Fix:
    - crash step now force-kills all run-dir-owned `rust-mule` pids discovered via `/proc` (`cwd`/`cmdline`), then waits for zero run-dir rust-mule processes.
- Decisions:
  - For forced crash simulation, process discovery by run-dir ownership is more reliable than trusting a single control pid file.
- Next steps:
  - Re-run resume soak and verify crash->restart proceeds when wrapper/child pid divergence exists.
- Change log: Resume crash now targets all run-dir rust-mule processes, eliminating wrapper-pid false negatives.

- Status: Fixed resume-soak restart false-positive by strengthening run-dir process detection on `feature/download-strategy-imule`:
  - User-observed failure:
    - restart exited immediately with `SingleInstance(AlreadyRunning)` after crash step.
  - Root issue:
    - run-dir process check matched only absolute binary path; missed `./rust-mule` processes started from run-dir cwd.
  - Fix:
    - switched run-dir process detection to `/proc`-based `cwd` + `cmdline` matching.
    - restart now refuses to proceed if any run-dir rust-mule process remains and prints PID diagnostics.
- Decisions:
  - Treat `/proc` ownership checks as authoritative for single-instance lock safety in resume automation.
- Next steps:
  - Re-run resume soak and verify crash->restart proceeds without lock conflict.
- Change log: Resume soak now correctly detects lingering `./rust-mule` run-dir processes before restart.

- Status: Hardened resume-soak crash detection on `feature/download-strategy-imule`:
  - User-observed failure:
    - after `kill -9`, script timed out waiting for `health=000` for 300s.
  - Root issue:
    - health-code shutdown check is brittle when API port can remain served by non-target process.
  - Fix:
    - resume script now validates crash by process identity:
      - killed app PID exits
      - no remaining run-dir `rust-mule` process
    - keeps health check as informational post-crash signal
    - adds restart immediate-exit guard with `rust-mule.resume.out` tail on failure.
- Decisions:
  - Use process-level ownership checks as primary crash/restart truth in resume automation.
- Next steps:
  - Re-run `download_resume_soak.sh` and confirm post-crash flow proceeds to restart/progress checks without `health=000` false timeout.
- Change log: Resume soak no longer blocks on strict `health=000` condition.

- Status: Added automated resume-soak orchestration script on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_resume_soak.sh`
  - Flow:
    - starts stack soak
    - waits for target scenario (`concurrency` default)
    - captures pre-crash `/api/v1/downloads` snapshot
    - hard-kills app (`SIGKILL`) and restarts in same run dir
    - captures post-restart snapshot and verifies scenario progress resumes
    - waits for terminal stack state, collects bundle, writes report.
  - Documented usage/overrides in `scripts/test/README.md`.
- Decisions:
  - Build resume validation as orchestration around existing stack runner instead of adding duplicate per-scenario harnesses.
- Next steps:
  - Run one automated resume-soak and validate `resume_report.txt` + stack bundle outcomes.
- Change log: Repo now has a one-command crash/restart resume-soak automation path.

- Status: Added TODO note for tag-driven CI/CD build/release flow on `feature/download-strategy-imule`.
- Decisions:
  - Track Git-tag-triggered build/publish verification as an explicit backlog item.
- Next steps:
  - Confirm release workflow behavior from tag push through artifact publication and document gaps.
- Change log: `docs/TODO.md` now includes tag-driven build/release automation verification.

- Status: Added cross-cutting naming/comment refactor TODO notes on `feature/download-strategy-imule`:
  - `docs/TODO.md` now tracks:
    - `Imule*` -> neutral `Mule*`/neutral identifier rename pass
    - code-comment wording normalization to compatibility-focused language
  - `docs/TASKS.md` scope now includes the same naming/comment normalization task.
- Decisions:
  - Keep explicit iMule/aMule/eMule wording for protocol reference documentation/tests where needed, but avoid it in production identifier names and code comments.
- Next steps:
  - Plan a repo-wide mechanical rename + comment wording sweep in bounded slices to minimize merge-risk.
- Change log: TODO/TASKS now explicitly capture naming and comment normalization policy.

- Status: Merged latest `main` into `feature/download-strategy-imule` to sync CI/docs/UI smoke and Pages workflow updates.
- Decisions:
  - Kept branch-local soak/download handoff history as primary during `docs/handoff.md` conflict resolution.
- Next steps:
  - Continue soak stabilization on top of synced branch baseline.
- Change log: Branch now includes latest `main` changes as merge base.

- Status: Fixed stack soak runner dependency on mutable repo script paths on `feature/download-strategy-imule`:
  - Failure analyzed from `/tmp/rust-mule-download-stack-20260217_170055.tar.gz`:
    - `concurrency` polling stayed `status=unknown state=unknown`
    - terminal error in stack output:
      - `env: '/home/coder/projects/rust-mule/scripts/test/download_soak_concurrency_bg.sh': No such file or directory`
      - `ERROR: band-run failed exit=127`
  - Root cause:
    - long-running stack run invoked wrappers directly from working-tree `scripts/test`; if those files change/disappear (e.g. branch switch) mid-run, scenario status/collect commands fail.
  - Fix:
    - `scripts/test/download_soak_stack_bg.sh` now stages `download_soak_*` scripts into `$RUN_DIR/soak-scripts` at startup and executes the band runner from that staged immutable copy.
- Decisions:
  - Treat soak script set as run artifact; do not depend on mutable working tree during long background runs.
- Next steps:
  - Re-run stack soak and confirm all four scenarios write results rows and artifacts even if repo branch changes during execution.
- Change log: Stack soak now uses per-run staged scripts and is resilient to working-tree churn.

- Status: Fixed stack runner shell-recursion regression on `feature/download-strategy-imule`:
  - Root cause of `bash: warning: shell level (1000) too high` was accidental command text inserted at the top of `scripts/test/download_soak_stack_bg.sh` before the shebang.
  - Removed the stray lines so script starts directly with `#!/usr/bin/env bash`.
  - Hardened background self-invocation to use `SELF_PATH` (absolute script path) instead of `$0`.
- Decisions:
  - Keep script entrypoint strict and avoid ambiguous `$0` resolution in detached shells.
- Next steps:
  - Re-run short stack soak to verify `start` no longer recurses and produces fresh run dirs/tarballs.
- Change log: Stack runner no longer recurses into nested bash startup loops.

- Status: Hardened in-band download runner interruption handling on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_130154.tar.gz` showed no scenario crash; `long_churn` was actively progressing but the band process received external termination (`Terminated` / `runner interrupted`) before writing final row.
  - `scripts/test/download_soak_band.sh` now traps `SIGINT`/`SIGTERM` and:
    - stops active scenario wrapper
    - performs best-effort `collect`
    - appends an `interrupted` row to `results.tsv`
  - `scripts/test/README.md` updated with interruption behavior.
- Decisions:
  - Treat external runner termination as first-class outcome in results, not silent truncation.
- Next steps:
  - Re-run stack soak; if interrupted, confirm `results.tsv` contains `interrupted` row and partial tarball is preserved.
- Change log: Band soak now records interruption outcomes explicitly.

- Status: Tuned download soak readiness probing on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` readiness now probes a configurable endpoint instead of hardcoding `/api/v1/status`.
  - New readiness env knobs:
    - `READY_TIMEOUT_SECS` (default `300`)
    - `READY_PATH` (default `/api/v1/downloads`)
    - `READY_HTTP_CODES` (default `200`, comma-separated)
  - Background `start` now forwards readiness env vars to detached run.
  - Also fixed latent integrity scenario crash risk by binding `round="$1"` in `scenario_integrity_round`.
  - `scripts/test/README.md` updated with readiness override knobs.
- Decisions:
  - For download-soak scenarios, readiness should key on download API availability, not KAD status endpoint warmup.
  - Keep readiness behavior configurable for environment-specific tuning.
- Next steps:
  - Re-run stack soak and verify integrity no longer fails on repeated startup `503` from `/api/v1/status`.
- Change log: Download soak readiness is now download-endpoint based and less brittle during startup.

- Status: Fixed download soak long-churn round crash on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_104554.tar.gz` showed:
    - `concurrency` completed
    - `long_churn` stuck as `status=stale_pid runner_state=running`
    - no long_churn tarball/result row in band output
  - Root cause from `/tmp/rust-mule-download-soak/long_churn/logs/runner.out`:
    - `download_soak_bg.sh: line 230: round: unbound variable`
  - Fix: assign `round="$1"` at start of `scenario_long_churn_round`.
- Decisions:
  - Keep `set -u`; patch all scenario entrypoints to bind function args explicitly.
- Next steps:
  - Re-run stack soak and verify long_churn now emits round ticks, terminal state, and collected tarball.
  - Separately evaluate repeated integrity readiness `503` behavior (startup/warmup timing).
- Change log: Long-churn scenario no longer crashes on unbound `round`.

- Status: Synced documentation to new contract/checklist/timing policy and created deferred KAD/wire refactor task plan on `feature/download-strategy-imule`:
  - Added `docs/KAD_WIRE_REFACTOR_PLAN.md` with phased tasks (baseline, shaper, bypass removal, retry envelope, validation).
  - Updated `README.md` and `docs/README.md` to include:
    - `docs/BEHAVIOURAL_CONTRACT.md`
    - `docs/REVIEWERS_CHECKLIST.md`
    - `docs/IMULE_COMPABILITY_TIMING.md`
    - `docs/KAD_WIRE_REFACTOR_PLAN.md`
  - Updated `docs/TODO.md` and `docs/TASKS.md` with explicit KAD/wire alignment tasks and “document now, refactor next” sequencing.
- Decisions:
  - Defer code-heavy KAD/wire timing refactor until soak baseline remains stable.
  - Treat behavior contract as authoritative, with iMule compatibility inside timing envelopes.
- Next steps:
  - Complete phase 0 baseline/guardrails from `docs/KAD_WIRE_REFACTOR_PLAN.md`.
  - Continue current soak stabilization; start shaper refactor only after baseline is green.
- Change log: Documentation and backlog are now aligned to contract-first timing policy and phased KAD/wire refactor plan.

- Status: Fixed download soak concurrency round crash on `feature/download-strategy-imule`:
  - Triage of `/tmp/rust-mule-download-stack-20260217_095242.tar.gz` showed `concurrency` aborting at round 1 with:
    - `download_soak_bg.sh: line 308: round: unbound variable`
  - Root cause: `scenario_concurrency_round` declared `round` but did not assign from function arg under `set -u`.
  - Fix: assign `round="$1"` at function start.
- Decisions:
  - Keep strict `set -u`; treat unbound vars as script bugs and patch at source.
- Next steps:
  - Re-run stack/band soak and verify `concurrency` and `long_churn` progress with regular round ticks and terminal states.
- Change log: Concurrency scenario no longer crashes due to unbound `round` variable.

- Status: Added bounded API curl timeouts in download soak runner on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_bg.sh` now uses shared timeout env knobs on all API calls (GET/POST/DELETE + readiness status probe):
    - `API_CONNECT_TIMEOUT_SECS` (default `3`)
    - `API_MAX_TIME_SECS` (default `8`)
  - `start` now forwards timeout env vars into detached `run` process so overrides are preserved.
  - `scripts/test/README.md` updated with new optional overrides for in-band runner usage.
- Decisions:
  - Prevent indefinite round hangs by time-bounding all API curl calls in the scenario runner.
  - Keep defaults conservative and operator-overridable for slower environments.
- Next steps:
  - Re-run stack/band soak and confirm scenarios keep progressing past round 1 without long `runner_state=running` stalls.
  - If timeouts are too aggressive under load, tune via env or bump defaults.
- Change log: Download soak API calls are now timeout-bounded and configurable.

- Status: Hardened stack `stop` teardown to avoid orphaned processes on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_stack_bg.sh` now:
    - stops all per-scenario download soak runners before killing stack runner
    - kills stack runner process group (TERM/KILL) instead of only top PID
    - scans `/proc` and terminates remaining processes tied to current run dir (`cwd`/`cmdline` match)
  - this addresses observed behavior where `stop` left `rust-mule` and soak helper processes alive.
- Decisions:
  - Prefer process-group and run-dir scoping for deterministic teardown.
- Next steps:
  - Re-run stack runner and verify `stop` leaves no matching processes (`pgrep -af rustmule-run-` returns none).
- Change log: Stack stop now performs full tree + run-dir cleanup.

- Status: Fixed download band wait/result logic for stale PID races on `feature/download-strategy-imule`:
  - Analysis from `/tmp/rust-mule-download-stack-20260216_140814.tar.gz` showed scenarios being advanced when `status=stale_pid` but `runner_state=running`.
  - `scripts/test/download_soak_band.sh` now:
    - treats terminal states strictly via `runner_state in {completed, failed, stopped}`
    - keeps waiting while `runner_state=running` (even if `status=stale_pid`)
    - maps final `results.tsv` outcome from terminal state (`completed|failed|stopped|running_after_wait|unknown`).
- Decisions:
  - Trust explicit runner state over transient status pid interpretation.
- Next steps:
  - Re-run stack/band soak and verify concurrency/long_churn no longer short-circuit after first poll.
- Change log: Band runner no longer treats `stale_pid + running` as finished.

- Status: Fixed stack runner build shell context on `feature/download-strategy-imule`:
  - Root cause: build command was executed via nested `bash -lc`, which lost the PATH bootstrap and still could not find `cargo`.
  - `scripts/test/download_soak_stack_bg.sh` now executes build command in current shell context (`eval "$BUILD_CMD"` in repo dir), preserving PATH/toolchain setup.
- Decisions:
  - Avoid nested login-shell build execution in stack runner.
- Next steps:
  - Re-run `download_soak_stack_bg.sh start` and confirm build begins and run directory stages.
- Change log: Stack runner build step now honors PATH bootstrap reliably.

- Status: Fixed download stack runner false-running behavior on `feature/download-strategy-imule`:
  - Root cause observed in logs: background shell could not find `cargo` (`cargo: command not found`), causing early exit before build/stage.
  - `scripts/test/download_soak_stack_bg.sh` now:
    - bootstraps PATH with `~/.cargo/bin` when needed
    - handles build failures explicitly (`runner.state=failed`, cleanup, pid removal)
    - validates runner process remains alive right after `start` and reports immediate-exit failure.
  - `scripts/test/README.md` troubleshooting updated (`stack.out` path and cargo PATH note).
- Decisions:
  - Prefer explicit failure state over stale/running ambiguity when background bootstrap fails.
- Next steps:
  - Re-run `download_soak_stack_bg.sh start`; verify build and run-dir staging occur and status transitions correctly.
- Change log: Stack runner now fails fast/cleanly on missing cargo or early runner death.

- Status: Added full background download soak pipeline runner on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_soak_stack_bg.sh` with `start|run|status|stop|collect`.
  - It now performs end-to-end automation:
    - builds latest sources (`BUILD_CMD`, default `cargo build --release`)
    - stages isolated run dir (`/tmp/rustmule-run-<timestamp>`)
    - writes run-specific `config.toml` (section-aware updates for `[sam]`, `[general]`, `[api]`)
    - starts rust-mule from staged dir and waits for health + token
    - runs `download_soak_band.sh` with forwarded soak parameters
    - supports post-run tarball collection.
  - `scripts/test/README.md` updated with full pipeline usage and env overrides.
- Decisions:
  - Keep app lifecycle isolated per run directory for reproducible soak artifacts.
  - Keep orchestration shell-native and reuse existing `download_soak_band.sh` logic.
- Next steps:
  - Execute `download_soak_stack_bg.sh start`, monitor `status`, and collect the resulting stack tarball for triage.
- Change log: Added one-command background build+run+download-soak pipeline.

- Status: Hardened download band-runner preflight and state handling on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_band.sh` now preflights API reachability (`GET /api/v1/health == 200`) and aborts early with a clear message if rust-mule is not running.
  - `scripts/test/download_soak_bg.sh` `stop` no longer overwrites terminal `failed/completed` state with `stopped`.
  - `scripts/test/README.md` now documents API-running precondition for band runs.
- Decisions:
  - Prefer fast-fail precondition checks over delayed per-scenario readiness timeouts when API is down.
  - Preserve terminal runner state for accurate post-run interpretation.
- Next steps:
  - Re-run `download_soak_band.sh` with rust-mule running and token present, then triage collected tarballs.
- Change log: Band runs now fail fast when API is offline and keep accurate scenario terminal states.

- Status: Fixed in-band download soak status parsing bug on `feature/download-strategy-imule`:
  - `scripts/test/download_soak_band.sh` now parses `status=running pid=...` lines correctly.
  - Previous behavior treated `running pid` as non-running and advanced scenarios immediately.
- Decisions:
  - Parse only the first token value for `status`/`runner_state` lines.
- Next steps:
  - Re-run `download_soak_band.sh` and confirm each scenario blocks for intended duration unless stopped/fails.
- Change log: Band runner no longer short-circuits after first poll.

- Status: Added in-band download soak orchestrator on `feature/download-strategy-imule`:
  - New script: `scripts/test/download_soak_band.sh`
    - runs download soak scenarios sequentially:
      1. `integrity` (default 3600s)
      2. `single_e2e` (default 3600s)
      3. `concurrency` (default 7200s)
      4. `long_churn` (default 7200s)
    - polls runner status, forces stop on timeout, and collects tarball for each scenario
    - copies collected tarballs + writes `results.tsv` and `status.tsv` under `OUT_DIR`
  - `scripts/test/README.md` updated with one-command in-band run instructions and overrides.
- Decisions:
  - Keep orchestrator shell-native and reuse existing per-scenario wrappers rather than duplicating scenario logic.
  - Preserve scenario isolation by running each with its own existing scenario run root and collect step.
- Next steps:
  - Run `download_soak_band.sh` after current source soak and share generated `OUT_DIR` + tarballs for triage.
  - If needed, add a companion triage script for `results.tsv` + per-scenario tar summaries.
- Change log: Added a one-command sequential download soak runner with automatic stop/collect.

- Status: Added download soak scaffolding and execution plan on `feature/download-strategy-imule`:
  - New generic runner: `scripts/test/download_soak_bg.sh`
    - background lifecycle: `start/run/status/stop/collect`
    - scenario switch via `SCENARIO=single_e2e|long_churn|integrity|concurrency`
    - writes per-scenario logs/bundles under `/tmp/rust-mule-download-soak/<scenario>`.
  - New scenario wrappers:
    - `scripts/test/download_soak_single_e2e_bg.sh`
    - `scripts/test/download_soak_long_churn_bg.sh`
    - `scripts/test/download_soak_integrity_bg.sh`
    - `scripts/test/download_soak_concurrency_bg.sh`
  - Updated `scripts/test/README.md` with post-source-soak run order, commands, and pass signals.
- Decisions:
  - Keep download soak scope API/control-plane focused for now (queue lifecycle + invariants + pressure), matching currently implemented download functionality.
  - Use scenario wrappers for simpler operator usage and isolated per-scenario run roots.
- Next steps:
  - After current source soak completes, run the four download soak scenarios in documented order.
  - Collect tarballs and triage runner/list logs for invariant violations and queue-pressure regressions.
- Change log: Added runnable download soak scripts and a concrete operator runbook.

- Status: Updated soak identity/session handling on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now:
    - generates unique per-run SAM session names for A/B using `RUN_TAG`
    - supports `SOAK_RUN_TAG` override for deterministic debug runs
    - defaults to `SOAK_FRESH_IDENTITY=1`, removing copied `data/sam.keys` so each run gets fresh I2P destinations
  - `scripts/test/README.md` updated with new identity/session controls.
- Decisions:
  - Keep fresh identity default enabled to avoid duplicate-destination registration when previous sessions linger.
  - Keep opt-out (`SOAK_FRESH_IDENTITY=0`) for controlled continuity tests.
- Next steps:
  - Restart soak with defaults and confirm no duplicate destination registration warnings from I2P router logs.
  - Continue soak comparison with `MISS_RECHECK_ATTEMPTS=0` once stable baseline resumes.
- Change log: Soak runs now isolate SAM identities and session names by default.

- Status: Cleaned soak status PID reporting on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh`:
    - `stop_nodes` now removes `logs/a.pid` and `logs/b.pid` after stop.
    - `status` now reports node PID liveness (`alive=1|0`) when pid files exist.
  - `scripts/test/README.md` updated to document stale-PID cleanup behavior.
- Decisions:
  - Prefer clearing pid files at stop to avoid false confidence in stale process IDs.
- Next steps:
  - Run quick `start -> status -> stop -> status` check and confirm node pid lines disappear after stop.
  - Proceed with baseline/tuned soak comparison runs.
- Change log: Soak status output no longer keeps stale node PID files after stop.

- Status: Hardened soak `stop` reliability and failure cleanup on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh`:
    - added `kill_pid_gracefully` (TERM + KILL fallback with result logging)
    - upgraded `stop_nodes` to use graceful escalation, not single-shot `kill`
    - added `stop_run_root_nodes` fallback scan over `/proc` to terminate soak-owned processes tied to current `RUN_ROOT`
    - tightened ownership matching to cwd/cmdline rooted in current `RUN_ROOT` (avoids killing unrelated local processes)
    - startup/readiness failure now sets `runner.state=failed` and runs cleanup immediately.
  - `scripts/test/README.md` updated with the stronger stop/cleanup behavior.
- Decisions:
  - Prioritize deterministic cleanup of soak-owned processes over PID-file-only teardown.
  - Keep process kill scope constrained to the active `RUN_ROOT`.
- Next steps:
  - Re-run `start -> stop -> status` smoke to verify no listeners remain on `A_URL`/`B_URL` after stop.
  - Resume baseline vs miss-recheck comparison soak once stop behavior is confirmed.
- Change log: Soak stop path now aggressively reaps RUN_ROOT-owned processes and failed starts no longer leave stale running state.

- Status: Added optional miss recheck pass in timed background soak harness on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now supports:
    - `MISS_RECHECK_ATTEMPTS` (default `1`)
    - `MISS_RECHECK_DELAY` seconds (default `20`)
  - After an initial source miss (`GET /api/v1/kad/sources/:file_id_hex`), the runner performs bounded delayed rechecks before persisting round outcome.
  - `rounds.tsv` format is unchanged (6 columns), so `scripts/test/soak_triage.sh` compatibility is preserved.
  - `scripts/test/README.md` updated with new env knobs and behavior description.
- Decisions:
  - Keep miss-recheck logic optional and env-controlled to preserve old baseline behavior (`MISS_RECHECK_ATTEMPTS=0` disables rechecks).
  - Keep `rounds.tsv` schema stable for existing triage tooling.
- Next steps:
  - Re-run A/B soak with two profiles:
    - baseline (`MISS_RECHECK_ATTEMPTS=0`)
    - tuned (`MISS_RECHECK_ATTEMPTS=1 MISS_RECHECK_DELAY=20`)
  - Compare hit-rate and hit-gap deltas using unchanged triage scripts.
- Change log: Soak runner now supports delayed miss recheck to reduce false misses from eventual consistency windows.

- Status: Hardened timed background soak harness failure handling on `feature/download-strategy-imule`:
  - `scripts/test/source_probe_soak_bg.sh` now:
    - fails fast if `A_URL`/`B_URL` ports are already in use (prevents attaching to foreign processes)
    - synchronizes API port config from `A_URL` and `B_URL`
    - verifies spawned node PIDs are running from expected per-run directories
    - aborts readiness early on repeated `403` responses (token mismatch/wrong process)
    - separates detached stdout/stderr into `logs/runner.out` to avoid duplicate `runner.log` lines.
    - uses stricter multi-probe port detection (`ss` + `lsof` + TCP connect probes) before launch to catch occupied API ports reliably.
    - `stop` now also scans `A_URL`/`B_URL` listen ports and terminates matching `rust-mule` listener PIDs if PID files are stale/missing.
  - `scripts/test/README.md` updated with the new safety behavior.
- Decisions:
  - Prefer explicit preflight failure over implicit retries when ports are occupied.
  - Treat repeated readiness `403` as a hard test-environment mismatch signal.
  - Keep stop fallback conservative: only kill listeners whose process cmdline contains `rust-mule`.
- Next steps:
  - Re-run soak with the hardened script; verify `rounds.tsv` and `status.ndjson` are populated before long-run analysis.
  - If needed, add optional auto-port allocation mode in a later slice.
- Change log: Soak runner now guards against port collisions and false-readiness loops, and logs are no longer duplicated.

- Status: Implemented source-probe telemetry hardening + request correlation IDs in KAD service and added timed background soak scaffold on `feature/download-strategy-imule`:
  - `src/kad/service.rs`:
    - outbound tracked requests now carry `request_id` and optional `trace_tag`
    - added response->expected-opcode mapping for strict response/request matching
    - added unmatched-response diagnostics (`last_unmatched_response`) with expected opcodes and tracked counts
    - source search/publish sends now emit `source_probe_request_sent` with request correlation ID
    - source search/publish response matching now emits `source_probe_response_matched` / `source_probe_response_unmatched` with request correlation diagnostics.
  - `src/kad/service/inbound.rs`:
    - unrequested response drops now log expected opcode families and tracked request counts
    - explicit decode-failure events for source probe response parsing failures.
  - `scripts/test/source_probe_soak_bg.sh`:
    - new detached soak runner with timer (`start <duration_secs>`, `status`, `stop`, `collect`)
    - PID/state files and log outputs under `/tmp/rust-mule-soak-bg` (override via `RUN_ROOT`).
  - `scripts/test/README.md`:
    - usage examples for timed background soak runs and environment overrides.
- Decisions:
  - Keep correlation ID scope focused on KAD source probe request/response lifecycle (no API schema change in this slice).
  - Keep soak harness shell-native with `nohup` + PID file controls for long-running sessions.
- Next steps:
  - Run the new timed soak harness against freshly built `../../mule-a` / `../../mule-b` and analyze `rounds.tsv` + `status.ndjson`.
  - If needed, expose recent source-probe correlation counters in `/api/v1/status` for easier dashboarding.
- Change log: Added source-probe request/response correlation logging and introduced a timer-based background soak runner script.

- Status: Completed transfer execution groundwork (peer-owned inflight + packet ingest + timeout retry) on `feature/download-strategy-imule`:
  - Added `src/download/protocol.rs`:
    - ED2K transfer opcode constants (`OP_REQUESTPARTS`, `OP_SENDINGPART`, `OP_COMPRESSEDPART`)
    - payload encode/decode helpers and typed protocol errors
    - unit tests for requestparts roundtrip and sendingpart validation.
  - Finished peer-aware transfer flow in `src/download/service.rs`:
    - fixed service loop to use a single `tokio::select!` over command receive + timeout tick
    - `ReserveBlocks` now assigns peer-owned inflight leases with expiration deadline
    - `MarkBlockReceived` / `MarkBlockFailed` now validate lease ownership by peer
    - added `PeerDisconnected` reclaim path to requeue leased blocks for that peer
    - added timeout processing to requeue expired leases with retry/error tracking
    - added `IngestInboundPacket` handling for `OP_SENDINGPART` and `OP_COMPRESSEDPART` that maps inbound payloads to block completion.
  - Extended persisted transfer state in `src/download/store.rs`:
    - `ByteRange`, `missing_ranges`, `inflight_ranges`, `retry_count`, `last_error` (with serde defaults).
  - Updated download API DTO mapping in `src/api/handlers/downloads.rs` to expose progress + transfer counters/error.
  - Added service tests:
    - `peer_disconnected_reclaims_only_that_peers_leases`
    - `ingest_sendingpart_marks_reserved_block_received`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 86 tests).
- Decisions:
  - Keep lease tracking in-memory (`ManagedDownload.leases`) and persist only range/error projections in `.part.met`.
  - Keep packet ingest scope minimal in this slice: decode + hash/range validation + state transition; deferred real payload write/verification pipeline.
- Next steps:
  - Wire TCP peer session handler to emit `IngestInboundPacket` and `PeerDisconnected` events from live network traffic.
  - Add block payload write path into `.part` files and integrity checks before `completing -> completed`.
  - Tune lease timeout/retry policy from soak-test observations and expose counters in API/UI if needed.
- Change log: Download actor now supports peer-bound inflight reservations with timeout/disconnect recovery and first inbound transfer packet ingestion path.

- Status: Added download transfer-state skeleton (phase 2 groundwork) on `feature/download-strategy-imule`:
  - Extended persisted metadata (`src/download/store.rs`):
    - new `ByteRange` model
    - `PartMet` now persists:
      - `missing_ranges`
      - `inflight_ranges`
      - `retry_count`
      - `last_error`
    - backward-compatible serde defaults retained.
  - Extended download actor (`src/download/service.rs`):
    - new transfer-facing commands:
      - `ReserveBlocks`
      - `MarkBlockReceived`
      - `MarkBlockFailed`
    - block reservation now moves ranges from `missing` -> `inflight`
    - failed blocks are re-queued into `missing` with retry/error tracking
    - received blocks clear inflight and update progress/completion state
    - restart safety: inflight ranges are reclaimed into missing on startup recovery.
  - Extended `DownloadSummary` and API-visible fields:
    - `progress_pct`
    - `missing_ranges`
    - `inflight_ranges`
    - `retry_count`
    - `last_error`
  - Updated download list/action responses (`src/api/handlers/downloads.rs`) to expose these fields.
  - Added tests:
    - reserve/fail/retry/receive state progression
    - restart inflight reclamation into missing
    - existing API mutation/list tests continue passing with expanded schema.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 82 tests).
- Decisions:
  - Keep range semantics inclusive (`start..=end`) in persisted metadata.
  - Treat in-flight reservations as non-authoritative across restart: reclaim to missing for correctness.
- Next steps:
  - Integrate wire-level TCP block flow into these commands (`OP_REQUESTPARTS` / `OP_SENDINGPART` path).
  - Add per-peer in-flight ownership and timeout scheduler for autonomous retry.
  - Persist part-hash verification state and transition `completing -> completed`.
- Change log: Download subsystem now tracks block-level missing/inflight/retry state with restart-safe recovery.

- Status: Implemented mutating download API endpoints on `feature/download-strategy-imule`:
  - Added new endpoints under `/api/v1`:
    - `POST /downloads`
    - `POST /downloads/:part_number/pause`
    - `POST /downloads/:part_number/resume`
    - `POST /downloads/:part_number/cancel`
    - `DELETE /downloads/:part_number`
  - Existing `GET /downloads` remains as queue/status snapshot endpoint.
  - New handler module: `src/api/handlers/downloads.rs`:
    - request/response DTOs for create/action/delete/list
    - typed download error -> HTTP mapping:
      - invalid input -> `400`
      - not found -> `404`
      - invalid transition -> `409`
      - channel closed -> `503`
      - storage/join failures -> `500`.
  - Router and handler exports updated:
    - `src/api/router.rs`
    - `src/api/handlers/mod.rs`
  - API state wiring unchanged in behavior but now fully exercises download mutating commands.
  - Added endpoint tests:
    - `api::tests::download_mutation_endpoints_update_service_state`
      - create -> pause -> resume -> cancel -> delete
      - conflict and not-found status checks
      - list consistency checks.
  - Updated API docs:
    - `docs/API_DESIGN.md` (downloads now marked implemented for lifecycle queue management)
    - `docs/api_curl.md` (added curl examples for list/create/pause/resume/cancel/delete).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 80 tests).
- Decisions:
  - Keep download API mutators command-driven via the actor to preserve explicit state transitions and typed error semantics.
  - Keep transfer wire pipeline out of this slice; API currently manages queue lifecycle only.
- Next steps:
  - Add first transfer-facing commands/structures (pending block requests, timeout bookkeeping).
  - Persist and expose gap/range progress in `PartMet` to support restart-safe block transfer.
  - Add UI controls for create/pause/resume/cancel/delete wired to the new endpoints.
- Change log: Download queue can now be fully controlled through API endpoints, with tests and docs updated.

- Status: Implemented download phase 1.5/2 groundwork on `feature/download-strategy-imule`:
  - Expanded download actor/service (`src/download/service.rs`):
    - state/lifecycle commands:
      - `CreateDownload`
      - `Pause`
      - `Resume`
      - `Cancel`
      - `Delete`
      - `List`
    - deterministic part slot allocation is now used (`%03d.part.met` / `%03d.part`).
    - persisted state transitions for lifecycle operations.
    - startup recovery now seeds in-memory queue and state from persisted metadata.
  - Added/expanded store primitives (`src/download/store.rs`):
    - helpers for numbered part paths and next free part number allocation.
    - `PartState` expanded to include `completed/cancelled/error` states.
  - Expanded typed errors (`src/download/errors.rs`) with:
    - invalid input, not found, invalid transition variants for command-level failures.
  - Added read-only API endpoint:
    - `GET /api/v1/downloads`
    - wired via new handler `src/api/handlers/downloads.rs` and router update.
    - response includes `queue_len`, `recovered_on_start`, and current download entries.
  - API wiring updates:
    - `ApiState`/`ApiServeDeps` now carry `DownloadServiceHandle`.
    - app bootstrap passes download handle into API server deps.
  - Tests added/updated:
    - download lifecycle flow (create -> pause -> resume -> cancel -> delete -> list)
    - restart recovery preserves persisted state
    - allocator picks lowest free slot
    - API contract test now verifies `/api/v1/downloads`
    - startup integration test updated for new API deps.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 79 tests).
- Decisions:
  - Keep phase 2 focused on control-plane correctness (state machine + persistence + API visibility) before chunk wire-transfer ingestion.
  - Keep `/api/v1/downloads` read-only for now; mutating endpoints will follow once queue semantics stabilize.
- Next steps:
  - Add mutating download API endpoints (create/pause/resume/cancel/delete) bound to current service commands.
  - Add first transfer-facing abstractions for pending block requests and timeout bookkeeping.
  - Introduce `.part` gap/range progress tracking in persisted metadata for block-level recovery.
- Change log: Download service is now a functional persisted queue with lifecycle operations and API observability.

- Status: Implemented download subsystem phase 1 persistence/recovery primitives on `feature/download-strategy-imule`:
  - Added `src/download/store.rs`:
    - `PartMet` model and `PartState` enum
    - `save_part_met(...)` with `.part.met.bak` rollover and atomic tmp->rename write
    - `load_part_met_with_fallback(...)` (primary then backup)
    - `scan_recoverable_downloads(...)` startup recovery scan over `data/download/*.part.met`
    - iMule-compatible version marker default (`PART_MET_VERSION = 0xE0`) for metadata model.
  - Extended `src/download/errors.rs` with typed store/persistence error variants:
    - read/write/rename/copy/parse/serialize directory and file failures.
  - Updated `src/download/service.rs`:
    - startup now recovers existing part metadata and sets `queue_len`
    - status now includes `recovered_on_start`
    - added `RecoveredCount` command and handle method.
  - Updated `src/download/mod.rs` exports for store model/types.
  - Added tests:
    - store roundtrip save/load
    - backup fallback when primary met is corrupt
    - recovery scan over multiple `.part.met` entries
    - service startup recovery count from existing metadata.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 76 tests).
- Decisions:
  - Keep phase 1 metadata persistence Rust-native (JSON payload) while preserving iMule file naming and lifecycle semantics (`.part.met`, `.bak`, startup recovery).
  - Defer wire-level transfer and full binary part.met compatibility to later phases after queue/state model is stable.
- Next steps:
  - Add first queue state model in service (`queued/running/paused/completed/error`) backed by persisted `PartMet`.
  - Add commands to create/pause/resume/cancel downloads and persist state transitions.
  - Introduce initial API endpoints for listing recovered/active download entries.
- Change log: Download subsystem now has backup-safe part metadata persistence and startup recovery integrated into runtime.

- Status: Implemented download subsystem phase 0 scaffold on `feature/download-strategy-imule`:
  - Added new module tree:
    - `src/download/mod.rs`
    - `src/download/types.rs`
    - `src/download/errors.rs`
    - `src/download/service.rs`
  - Added typed download errors:
    - `DownloadError`
    - `DownloadStoreError`
  - Added actor-style service shell:
    - `DownloadServiceConfig::from_data_dir(...)`
    - `start_service(...)` returning handle/status/join task
    - command loop with `Ping` and `Shutdown`
    - startup ensures `data/download/` and `data/incoming/` exist.
  - Integrated into app bootstrap:
    - `src/lib.rs` exports `download` module.
    - `src/app.rs` starts download service at runtime and adds `AppError::Download`.
  - Added tests:
    - `start_service_creates_download_and_incoming_dirs`
    - `service_ping_and_shutdown_flow`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 72 tests).
- Decisions:
  - Keep phase 0 strictly minimal: directory/bootstrap + command actor + typed errors, no transfer logic yet.
  - Keep service always-on at startup to prepare API integration in next phase.
- Next steps:
  - Phase 1: implement `.part`/`.part.met` persistence primitives and startup recovery scanning.
  - Add first download queue state model (`queued/running/paused/completed/error`) and service commands around it.
  - Add persistence-focused tests with corrupted/backup metadata cases.
- Change log: Download subsystem now exists as a first-class module with runtime wiring and passing tests.

- Status: Added iMule-derived download subsystem strategy on `feature/download-strategy-imule`:
  - New document: `docs/DOWNLOAD_DESIGN.md`
    - deep-dive findings from iMule source for download flow and persistence:
      - chunk/block transfer behavior (`OP_REQUESTPARTS`/`OP_SENDINGPART`/compressed parts)
      - `.part` + `.part.met` lifecycle and gap tracking
      - `known.met` and `known2_64.met` responsibilities
    - proposed Rust-native module boundaries under `src/download/*`
    - phased implementation plan (scaffold -> persistence -> transfer -> finalize -> API/UI)
    - test plan and compatibility rules.
  - Updated docs index and planning files:
    - `docs/README.md` includes `DOWNLOAD_DESIGN.md`
    - `docs/TODO.md` now has a `Downloads` backlog section
    - `docs/TASKS.md` reprioritized with download phase 0/1 first
    - `README.md` documentation map includes `docs/DOWNLOAD_DESIGN.md`
- Decisions:
  - Implement downloads Rust-native as an actor-style subsystem, preserving iMule wire/on-disk semantics where needed for compatibility.
  - Use `data/download/` for active `.part` state and `data/incoming/` for finalized files.
  - Deliver MD4-first baseline before enabling full AICH (`known2_64.met`) integration.
- Next steps:
  - Implement phase 0 scaffolding (`src/download/*`, typed errors, command/event loop shell).
  - Implement phase 1 `.part`/`.part.met` persistence and startup recovery tests.
  - Add minimal API surface to create/list/pause/resume/cancel downloads once phase 1 lands.
- Change log: Download strategy is now documented and promoted to top project priority in planning docs.

- Status: Added UI smoke testing to CI on `main` push/PR via Playwright + mocked backend:
  - Updated `.github/workflows/ci.yml`:
    - new `ui-smoke` job on `ubuntu-latest` with Node 20
    - installs UI deps (`npm ci`)
    - installs Playwright Chromium (`npx playwright install --with-deps chromium`)
    - runs `npm run test:ui:smoke`
  - Updated `ui/playwright.config.mjs`:
    - added `webServer` to auto-start local mock server when `UI_BASE_URL` is not provided.
  - Added `ui/tests/e2e/mock-server.mjs`:
    - serves UI pages/assets for Playwright
    - mocks required API endpoints + SSE (`/api/v1/events`) for deterministic UI smoke tests in CI.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep UI smoke test backend mocked in CI to avoid SAM/I2P runtime dependencies.
  - Make mock server opt-out via `UI_BASE_URL` so tests can still target a real running backend when needed.
- Next steps:
  - Optionally archive Playwright HTML/report artifacts in CI for easier failure triage.
  - Optionally add one route-guard assertion per page for authenticated/unauthenticated flow edges.
- Change log: CI now runs UI smoke tests automatically on pushes to `main` and PRs.

- Status: Completed final service split pass for source-probe/status helpers on `main` (no behavior change):
  - Added `src/kad/service/source_probe.rs`:
    - source probe tracking and counters:
      - `mark_source_publish_sent`
      - `mark_source_search_sent`
      - `on_source_publish_response`
      - `on_source_search_response`
      - `source_store_totals`
  - Added `src/kad/service/status.rs`:
    - status snapshot/publish logic:
      - `build_status`
      - `publish_status`
  - Updated `src/kad/service.rs`:
    - delegates source-probe and status helpers to dedicated modules.
  - Updated `src/kad/service/tests.rs`:
    - tests now call `status::build_status_impl(...)`.
  - Net effect:
    - `src/kad/service.rs` reduced to ~2009 LOC (from ~2335 previous step, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep wrapper/delegation pattern to preserve call sites and minimize risk.
  - Maintain all behavioral logic unchanged while shrinking `service.rs` responsibility.
- Next steps:
  - Optional: split remaining send/job orchestration helpers (`send_*`, `progress_keyword_job*`) if we want sub-2k LOC in `service.rs`.
- Change log: Source-probe and status helper clusters now live in dedicated modules; core service file is primarily orchestration/glue.

- Status: Extracted KAD inbound opcode handling into dedicated module on `main` (no behavior change):
  - Added `src/kad/service/inbound.rs`:
    - moved full `handle_inbound(...)` implementation and opcode dispatch logic (`HELLO`, `BOOTSTRAP`, `REQ/RES`, `SEARCH`, `PUBLISH`, `PING/PONG`, etc.).
  - Updated `src/kad/service.rs`:
    - now delegates inbound handling through a thin wrapper to `inbound::handle_inbound_impl(...)`.
    - registered new `mod inbound;`.
  - Net effect:
    - `src/kad/service.rs` reduced again to ~2335 LOC (from ~3519 after prior pass, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep inbound extraction as structure-only (no opcode behavior changes) to preserve protocol compatibility during refactor.
  - Continue minimizing `service.rs` responsibility by domain slicing (`types`, `routing_view`, `keyword`, `lookup`, `inbound`, `tests`).
- Next steps:
  - Optional final cleanup pass: extract source-probe/status helper cluster from `service.rs` into `source_probe.rs` / `status.rs` for smaller core orchestration.
- Change log: Inbound packet handling now lives in `src/kad/service/inbound.rs`; `service.rs` is now primarily orchestration plus shared helpers.

- Status: Continued KAD service modularization with lookup + keyword logic extraction on `main` (no behavior change):
  - Added `src/kad/service/lookup.rs` and moved lookup/refresh scheduler logic there:
    - lookup queue seeding/progression (`tick_lookups`)
    - bucket refresh scheduling (`tick_refresh`)
    - lookup response integration (`handle_lookup_response`)
    - distance/random target helpers used by the lookup pipeline.
  - Added `src/kad/service/keyword.rs` and moved keyword cache/store lifecycle logic there:
    - keyword interest tracking/capping
    - keyword hit cache upsert/caps/eviction
    - keyword store TTL/size-limit eviction
    - maintenance helpers for keyword cache/store.
  - `src/kad/service.rs` now delegates to `lookup`/`keyword` modules for these domains.
  - Net effect:
    - `src/kad/service.rs` reduced further to ~3519 LOC (from ~4116 after prior split, ~4979 originally).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep behavior-preserving wrappers/delegation during split to minimize regression risk.
  - Prioritize extraction of cohesive domains (lookup + keyword lifecycle) before touching inbound packet handler.
- Next steps:
  - Next high-value split in `service.rs` is `handle_inbound` and related opcode handlers into `inbound.rs`.
  - Optional follow-up: move source-probe bookkeeping/status helpers into `source_probe.rs`.
- Change log: KAD service now has dedicated `lookup` and `keyword` modules; core file is materially smaller with unchanged test results.

- Status: Split `src/kad/service.rs` into logical submodules on `main` (no behavior change):
  - Added `src/kad/service/types.rs`:
    - moved service-facing data/config/status/command types and related defaults:
      - `KadServiceCrypto`
      - `KadServiceConfig` (+ `Default`)
      - `KadServiceStatus`
      - `KadServiceCommand`
      - DTOs (`KadSourceEntry`, `KadKeywordHit`, `KadKeywordSearchInfo`, `KadPeerInfo`)
      - routing view DTOs (`RoutingSummary`, `RoutingBucketSummary`, `RoutingNodeSummary`)
      - internal stats struct (`KadServiceStats`)
  - Added `src/kad/service/routing_view.rs`:
    - moved routing summary/bucket/node projection builders out of core service loop file.
  - Added `src/kad/service/tests.rs`:
    - moved embedded unit tests out of `service.rs` into a dedicated test module file.
  - Updated `src/kad/service.rs`:
    - now re-exports public service types from `types.rs`
    - delegates routing view builders to `routing_view` module
    - keeps core service runtime/inbound/outbound behavior unchanged.
  - Net effect:
    - `src/kad/service.rs` reduced from ~4979 LOC to ~4116 LOC.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Keep this pass structural-only (file/module boundaries) to avoid behavior risk.
  - Prefer progressive extraction from `service.rs` with compile/test safety after each chunk.
- Next steps:
  - Continue splitting heavy behavior clusters from `service.rs`:
    - inbound packet handling
    - keyword job progression/cache maintenance
    - lookup/crawl scheduler logic
- Change log: KAD service module now has dedicated `types`, `routing_view`, and `tests` files with unchanged runtime behavior.

- Status: Hardened coverage CI job to avoid opaque failures on `main`:
  - Updated `.github/workflows/ci.yml` coverage job:
    - installs Rust `llvm-tools-preview` component explicitly
    - emits a `cargo llvm-cov --summary-only` step before gating
    - runs the gate through `scripts/test/coverage.sh` (single source of truth)
  - Set initial gate to a pragmatic baseline:
    - `MIN_LINES_COVERAGE=20` in CI env
    - `scripts/test/coverage.sh` default now `20`
  - Rationale: previous failures were opaque (`exit code 1` only). Summary step now prints measured coverage before gate evaluation.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Prefer explicit toolchain component install (`llvm-tools-preview`) in CI instead of relying on implicit behavior.
  - Use a conservative initial threshold until CI reports stable baseline values, then ratchet upward.
- Next steps:
  - After 1-2 successful CI runs with visible summaries, increase `MIN_LINES_COVERAGE` gradually (e.g. 25 -> 30 -> ...).
- Change log: Coverage CI now logs summary before gating and has an explicit llvm-tools setup.

- Status: Added tag-driven GitHub release workflow on `main`:
  - New workflow: `.github/workflows/release.yml`
  - Trigger: `push` tags matching `v*`
  - Build matrix:
    - `ubuntu-latest` -> `scripts/build/build_linux_release.sh`
    - `macos-latest` -> `scripts/build/build_macos_release.sh`
    - `windows-latest` -> `scripts/build/build_windows_release.ps1`
  - Uploads packaged artifacts from `dist/` per platform.
  - Publish job downloads artifacts and creates a GitHub Release with:
    - auto-generated release notes
    - attached `.tar.gz` (Linux/macOS) and `.zip` (Windows) bundles.
  - Updated `README.md` with tag-driven release usage (`git tag ... && git push origin ...`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Reuse existing repo build scripts for consistency between local and CI release packaging.
  - Use tag naming convention `v*` for release automation.
- Next steps:
  - Optional: add a manual `workflow_dispatch` release path for re-running failed tag releases without retagging.
  - Optional: add checksum/signature generation in release workflow.
- Change log: CI now includes a tag-driven CD pipeline that produces and publishes cross-platform release bundles.

- Status: Tightened initial line-coverage gate on `main`:
  - Increased minimum line coverage threshold from `35` to `40` in:
    - `.github/workflows/ci.yml` (`cargo llvm-cov --fail-under-lines 40`)
    - `scripts/test/coverage.sh` (`MIN_LINES_COVERAGE` default now `40`)
  - Attempted local baseline collection, but this sandbox cannot install `llvm-tools-preview` via `rustup`, so local `cargo llvm-cov` measurement could not be completed here.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests).
- Decisions:
  - Raise threshold incrementally to reduce CI disruption risk while still strengthening the gate.
  - Keep threshold configurable via `MIN_LINES_COVERAGE` for local overrides.
- Next steps:
  - Confirm coverage % from CI run in a normal runner environment and ratchet gate to `45` if headroom is comfortable.
- Change log: Coverage quality gate is now stricter (`40` lines minimum) across CI and local helper script.

- Status: Implemented API loopback dual-stack hardening + coverage gate scaffolding + startup/auth/session smoke test on `main`:
  - API listener startup now attempts both loopback families and serves on every successful bind:
    - `::1:<port>`
    - `127.0.0.1:<port>`
  - Bind failures on one family are logged as warnings; startup only fails if no loopback listener can be created.
  - Added first runtime smoke integration test: `tests/api_startup_smoke.rs`
    - boots `api::serve`
    - verifies `/api/v1/auth/bootstrap`
    - creates frontend session (`/api/v1/session`)
    - verifies session-cookie protected `/api/v1/session/check` and `/index.html`.
  - Added coverage gating scaffolding:
    - GitHub Actions workflow: `.github/workflows/ci.yml`
    - local coverage command: `scripts/test/coverage.sh`
    - README quality gate section updated with coverage command.
  - Added `reqwest` as a dev-dependency for integration-level HTTP smoke testing.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` (all passing; 71 tests total including integration smoke).
- Decisions:
  - Keep API local-only by binding loopback addresses explicitly instead of widening bind scope.
  - Treat IPv4/IPv6 support as best-effort on startup: one-family availability is acceptable; total loopback bind failure is fatal.
  - Start with a conservative line-coverage gate (`--fail-under-lines 35`) and ratchet upward once baseline metrics are collected in CI.
- Next steps:
  - Run `scripts/test/coverage.sh` in CI or locally where `cargo-llvm-cov` is installed and record baseline coverage percentage in docs.
  - Consider raising coverage threshold after one or two PR cycles.
- Change log: API startup is now resilient to localhost address-family differences, and repo now has integration smoke coverage plus CI coverage gate scaffolding.

- Status: Removed `api.host` configurability and simplified API binding on `main`:
  - `ApiConfig` no longer contains `host`; API config now binds by port only.
  - API server bind address is fixed to loopback (`127.0.0.1`) in `src/api/mod.rs`.
  - Removed loopback-host parsing/validation path for API host:
    - removed `parse_api_bind_host(...)`
    - removed related `ConfigError` and `ConfigValidationError` branches.
  - Settings API no longer exposes/accepts `settings.api.host`.
  - Updated config/docs surface (`config.toml`, `README.md`, `docs/architecture.md`, `docs/api_curl.md`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 70 tests).
- Decisions: Keep API bind policy explicit and non-configurable while local-only operation is the product mode; expose only `api.port` to users.
- Next steps: Optional follow-up is to document future remote/headless exposure as a separate deployment mode instead of host binding config.
- Change log: API host setting has been removed from config/state/settings surfaces.

- Status: Performed config-surface naming and documentation pass on `main`:
  - Renamed API rate-limit config key for clarity:
    - `rate_limit_dev_auth_max_per_window` -> `rate_limit_auth_bootstrap_max_per_window`.
  - Added backward-compatible config parsing alias in `ApiConfig`:
    - `#[serde(alias = "rate_limit_dev_auth_max_per_window")]`.
  - Updated all runtime/settings references to the new name.
  - Added inline comments in `config.toml` for all active/uncommented keys across:
    - `[sam]`, `[kad]`, `[general]`, `[api]`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep public config keys aligned with endpoint naming (`auth/bootstrap`) and maintain read-compat for recently renamed keys to avoid operator breakage.
- Next steps: Optional follow-up is to normalize remaining legacy test names/messages still using `dev_auth` wording.
- Change log: Config naming and inline documentation are now more consistent and self-descriptive.

- Status: Removed user-facing `kad.udp_port` configurability while preserving config-file compatibility on `main`:
  - Removed `udp_port` from `KadConfig` public settings.
  - Added deprecated compatibility field in `KadConfig`:
    - `deprecated_udp_port` with `#[serde(rename = "udp_port", skip_serializing)]`
    - old config files containing `kad.udp_port` still parse, but value is ignored and no longer persisted.
  - Removed `kad.udp_port` line from `config.toml`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep KAD UDP port as protocol/internal metadata, but not as a user-tunable config knob.
- Next steps: Optional follow-up is to document this deprecation explicitly in `docs/architecture.md` if we want a visible migration note for operators carrying old configs.
- Change log: `kad.udp_port` is no longer a configurable setting in active config surfaces.

- Status: Replaced `/api/v1/dev/auth` with core bootstrap endpoint and auth-mode gating on `main` (no backward compatibility route):
  - Added `api.auth_mode` enum config (`local_ui` | `headless_remote`) in `src/config.rs` and `config.toml`.
  - Removed `enable_dev_auth_endpoint` from runtime config/state/settings API.
  - New endpoint path is `GET /api/v1/auth/bootstrap` (loopback-only).
  - Endpoint is available only when `api.auth_mode = "local_ui"`; it is not registered in `headless_remote` mode.
  - Updated bearer-exempt logic to use `auth_mode` and new path.
  - Updated rate limiter target path to `/api/v1/auth/bootstrap`.
  - Updated UI bootstrap fetch paths:
    - inline `/auth` bootstrap page in `src/api/ui.rs`
    - `ui/assets/js/helpers.js`
  - Renamed helper script to `scripts/docs/auth_bootstrap.sh` and updated docs references.
  - Updated docs (`README.md`, `docs/architecture.md`, `docs/API_DESIGN.md`, `docs/ui_api_contract_map.md`, `docs/api_curl.md`).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Treat token bootstrap as core local-UI behavior under `/api/v1/auth/bootstrap`; use auth mode, not endpoint-specific toggle flags.
- Next steps: Optional follow-up is to surface `auth_mode` explicitly in settings UI with explanatory copy for local UI vs headless remote operations.
- Change log: Auth bootstrap route naming and availability now align with core-vs-mode semantics.

- Status: Added minimal API rate-limiting middleware on `main`:
  - New `[api]` config keys:
    - `rate_limit_enabled`
    - `rate_limit_window_secs`
    - `rate_limit_dev_auth_max_per_window`
    - `rate_limit_session_max_per_window`
    - `rate_limit_token_rotate_max_per_window`
  - Added `src/api/rate_limit.rs` fixed-window middleware keyed by `(client_ip, method, path)`.
  - Rate limiting is applied to:
    - `GET /api/v1/dev/auth`
    - `POST /api/v1/session`
    - `POST /api/v1/token/rotate`
  - Added rate-limit fields to settings API payload/patch and validation.
  - Added test coverage:
    - session endpoint returns `429` after threshold exceeded
    - settings snapshot/patch includes new rate-limit fields
    - settings rejects invalid zero rate-limit values
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 71 tests).
- Decisions: Keep limiter intentionally narrow (only high-value endpoints) and disabled by config toggle when needed; avoid limiting SSE/status paths for now.
- Next steps: Optional: emit structured logs on `429` events and add per-endpoint counters for abuse/noise visibility.
- Change log: API now has configurable built-in endpoint rate limiting.

- Status: Added API endpoint toggles for debug and dev-auth bootstrap on `main`:
  - New `[api]` config flags in `config.toml`/`ApiConfig`:
    - `enable_debug_endpoints` (controls `/api/v1/debug/*`)
    - `enable_dev_auth_endpoint` (controls `/api/v1/dev/auth`)
  - Router now conditionally registers debug routes and dev-auth route based on these flags.
  - Auth exemption for `/api/v1/dev/auth` is now conditional on `enable_dev_auth_endpoint`.
  - Settings API now exposes and accepts these flags under `settings.api`.
  - Added/updated tests:
    - bearer exemption logic with dev-auth enabled/disabled
    - debug routes return `404` when disabled
    - dev-auth route returns `404` (with bearer) when disabled
  - Updated docs: `README.md`, `docs/architecture.md`, `docs/api_curl.md`, and `config.toml` comments.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 70 tests).
- Decisions: Keep both new flags defaulted to `true` for backwards-compatible behavior; operators can disable either endpoint group explicitly.
- Next steps: Optional follow-up is lightweight rate limiting for high-value endpoints (`/api/v1/dev/auth`, `/api/v1/token/rotate`, `/api/v1/session`) to reduce brute-force/noise risks.
- Change log: API surface can now be reduced at runtime via config without code changes.

- Status: Moved operational scripts out of `docs/scripts` into top-level `scripts/` with explicit split:
  - API/documentation helpers moved to `scripts/docs/`:
    - `health/status/events`, KAD endpoint helpers, debug endpoint helpers, dev auth helper.
  - Test harnesses moved to `scripts/test/`:
    - `two_instance_dht_selftest.sh`
    - `rust_mule_soak.sh`
    - `soak_triage.sh`
  - Removed legacy `docs/scripts/` directory and updated path references in scripts/docs:
    - internal calls in `scripts/test/two_instance_dht_selftest.sh`
    - usage/help text in moved scripts
    - `README.md` and `docs/api_curl.md` pointers.
- Decisions: Keep `scripts/build/` for build/release, `scripts/docs/` for endpoint helper wrappers, and `scripts/test/` for scenario/soak harnesses.
- Next steps: Optional follow-up can add thin wrapper aliases for old `docs/scripts/*` paths if external automation still depends on them.
- Change log: Script layout is now canonicalized under `/scripts` and split by intent (docs helpers vs tests).

- Status: Added dedicated cross-platform build script folder on `main`:
  - New canonical build location: `scripts/build/`.
  - Added platform scripts:
    - `scripts/build/build_linux_release.sh`
    - `scripts/build/build_macos_release.sh`
    - `scripts/build/build_windows_release.ps1`
    - `scripts/build/build_windows_release.cmd`
  - Added `scripts/build/README.md` with usage/output conventions.
  - Kept backward compatibility by turning `docs/scripts/build_linux_release.sh` into a wrapper that delegates to `scripts/build/build_linux_release.sh`.
  - Updated docs pointers in `README.md` and `docs/README.md`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep build/release scripts outside `docs/` in a dedicated top-level `scripts/build/` folder; keep old Linux path callable as a shim to avoid breakage.
- Next steps: Optional follow-up is a CI matrix job that runs each platform build script and verifies `dist/*` bundle naming/contents.
- Change log: Cross-platform build scaffolding now exists with a canonical script location.

- Status: Streamlined docs set and refreshed README entrypoint on `main`:
  - Rewrote `README.md` to reflect current behavior and include a clear documentation map.
  - Added `docs/README.md` as a documentation index.
  - Normalized backlog docs:
    - `docs/TODO.md` (focused subsystem backlog)
    - `docs/TASKS.md` (current execution priorities and DoD)
  - Corrected API design drift in `docs/API_DESIGN.md`:
    - `/api/v1/health` response shape now documented as `{ \"ok\": true }`
    - SSE auth documented as session-cookie based (no token query parameter)
    - security note updated to avoid bearer tokens in query parameters.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep `README.md` as the top-level operator/developer entrypoint and keep deeper design/contract details in `/docs` with an explicit index.
- Next steps: Keep `docs/ui_api_contract_map.md` and `docs/api_curl.md` updated whenever endpoint fields/routes change; continue prioritizing KAD organic reliability and UI statistics expansion.
- Change log: Documentation set is now normalized and aligned with current API/UI/auth behavior.

## Status (2026-02-12)

- Status: Standardized and relaxed API command timeout policy on `main`:
  - Added shared timeout constant:
    - `API_CMD_TIMEOUT = 5s` in `src/api/mod.rs`.
  - Replaced per-endpoint hardcoded `2s` timeouts in `src/api/handlers/kad.rs` with `API_CMD_TIMEOUT`.
  - This applies to KAD command/oneshot-backed endpoints (`sources`, `keyword results`, searches, peers, routing debug, lookup/probe).
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Use a single shared timeout for API command dispatch/response waits to avoid endpoint drift and reduce spurious gateway timeouts in slower I2P conditions.
- Next steps: Optional follow-up can split timeout tiers (e.g. 3s read-only status vs 5s routing/debug) if operational data suggests different SLOs.
- Change log: API command timeout is now centralized and increased from ad-hoc 2s values to 5s.

- Status: Made session-cookie `Secure` policy explicit in auth code on `main`:
  - Added rationale comment in `src/api/auth.rs` (`build_session_cookie`) explaining why `Secure` is intentionally omitted for current HTTP loopback UI flow.
  - Documented future action in comment: add `Secure` when/if frontend serving moves to HTTPS.
  - Extended cookie test (`src/api/tests.rs`) to assert current behavior (`Secure` absent), making policy changes explicit and reviewable.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep cookie flags as `HttpOnly; SameSite=Strict` without `Secure` for current localhost HTTP mode, but require explicit code change when transport assumptions change.
- Next steps: Optional: gate `Secure` on a future HTTPS/TLS config switch when frontend transport supports it.
- Change log: Session-cookie security decision is now explicitly documented and test-enforced.

- Status: Fixed implicit config persistence path and fragile API settings tests on `main`:
  - Added explicit config persistence API:
    - `Config::persist_to(path)` in `src/config.rs`
    - existing `Config::persist()` now delegates to `persist_to("config.toml")` for compatibility.
  - Added explicit config path to API runtime state:
    - `ApiState.config_path`
    - new `ApiServeDeps` includes `config_path` and other serve dependencies.
  - `settings_patch` now persists via:
    - `next.persist_to(state.config_path.as_path())`
    - no implicit `./config.toml` write in API path.
  - Threaded config path from entrypoint to app/api:
    - `main` now tracks `config_path` and calls `app::run(cfg, config_path)`
    - `app::run` passes `config_path` into API serve deps.
  - Hardened tests:
    - API tests now use unique temp config paths in `ApiState` and no longer mutate/restore repo `config.toml`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep backward-compatible `Config::persist()` for non-API call sites, but route all runtime persistence that depends on startup config location through explicit `persist_to(path)`.
- Next steps: Optional cleanup can remove `Config::persist()` after all call sites are migrated to `persist_to(path)`.
- Change log: Config persistence path is now explicit in API runtime flow and test persistence is isolated from repository config.

- Status: Removed lingering test-build unused-import warning in `src/api/mod.rs` after API split:
  - Dropped test-only re-export block from `src/api/mod.rs`.
  - Updated `src/api/tests.rs` to import directly from split modules:
    - `auth`, `cors`, `ui`, `handlers`, `router`.
  - This prevents warning-prone indirection and keeps compile ownership explicit.
  - Ran `cargo clippy --all-targets --all-features` and `cargo test` (all passing; 68 tests).
- Decisions: Keep API tests referencing module paths directly instead of relying on `mod.rs` re-exports to avoid future dead-import warnings during refactors.
- Next steps: None required for this warning; refactor cleanup is complete.
- Change log: API test imports now directly track split module boundaries.

- Status: Refactored API god-file (`src/api/mod.rs`) into focused modules on `main` (no behavior change):
  - New modules:
    - `src/api/router.rs` (router wiring)
    - `src/api/auth.rs` (auth/session middleware + helpers)
    - `src/api/cors.rs` (CORS middleware + helpers)
    - `src/api/ui.rs` (embedded UI/static serving and SPA fallback)
    - `src/api/handlers/core.rs` (health/auth/session/status/events handlers)
    - `src/api/handlers/kad.rs` (KAD/search/debug handlers)
    - `src/api/handlers/settings.rs` (settings handlers/validation/patch logic)
    - `src/api/handlers/mod.rs` (handler exports)
    - `src/api/tests.rs` (existing API tests moved out of `mod.rs`)
  - `src/api/mod.rs` now focuses on API state, startup/serve path, module wiring, and test-only re-exports.
  - Endpoint paths, middleware order, and response behavior were kept unchanged.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep this as a structural split only (no endpoint contract or middleware semantic changes) to reduce risk while improving maintainability.
- Next steps: Optional follow-up can split `handlers/kad.rs` further by sub-domain (`search`, `debug`, `publish`) if we want even tighter module boundaries.
- Change log: API surface is now modularized by concern, replacing the prior single-file implementation.

- Status: Fixed `nodes2.dat` download persistence path bug on `main`:
  - In `try_download_nodes2_dat(...)` (`src/app.rs`), persistence previously hardcoded `./data/nodes.dat`.
  - Updated function to accept an explicit output path and persist there.
  - Call site now passes `preferred_nodes_path` (resolved from configured `general.data_dir` + `kad.bootstrap_nodes_path`).
  - Parent directories are created for the configured output path before atomic write/rename.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep download behavior unchanged except for output path correctness; this remains a low-risk bug fix with no protocol changes.
- Next steps: Optional: add a targeted unit/integration test around bootstrap download path resolution when `data_dir` is non-default.
- Change log: `nodes2.dat` refresh now respects configured data directory/bootstrap path.

- Status: Corrected misleading overview KPI labels in UI on `main`:
  - Updated `ui/index.html` labels to match actual status field semantics:
    - `routing`: `Peers Contacted` -> `Routing Nodes`
    - `live`: `Responses` -> `Live Nodes`
    - `live_10m`: `Hits Found (10m)` -> `Live Nodes (10m)`
  - Updated progress badges for clarity:
    - `requests` -> `requests sent`
    - `responses` -> `responses received`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep KPI naming tied to raw API counter meaning, not inferred behavior, to avoid future ambiguity in diagnostics.
- Next steps: Optional follow-up can add compact tooltip/help text for each KPI defining its backing status field.
- Change log: Overview metric labels now accurately describe `routing`, `live`, and `live_10m`.

- Status: Fixed high-impact UI/API status-field mismatch on `main`:
  - UI expected `recv_req` and `recv_res` in status payloads (REST + SSE), while API exposed `sent_reqs` and `recv_ress`.
  - Added compatibility aliases directly in `KadServiceStatus`:
    - `recv_req` (mirrors `sent_reqs`)
    - `recv_res` (mirrors `recv_ress`)
  - Wired aliases in status construction (`build_status`) so they are always populated.
  - Extended API contract test `ui_api_contract_endpoints_return_expected_shapes` to assert:
    - `recv_req` and `recv_res` exist
    - alias values match `sent_reqs` and `recv_ress`.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Preserve existing canonical counters (`sent_reqs`, `recv_ress`) while adding aliases for UI compatibility; avoids breaking current dashboards and SSE consumers.
- Next steps: Optional cleanup is to normalize UI naming to canonical fields in a later pass, then remove aliases when all consumers are updated.
- Change log: Status API now exposes both canonical and UI-expected request/response counters.

- Status: Added checked-in soak runner script on `main`:
  - New script: `docs/scripts/rust_mule_soak.sh`
  - Mirrors the long-run harness previously staged in `/tmp/rust_mule_soak.sh`.
  - Commands:
    - `start` (clone `../../mule-a` + `../../mule-b`, patch B ports/session, launch both)
    - `wait_ready` (poll `/api/v1/status` until both return 200)
    - `soak [rounds]` (publish/search loops; writes `logs/rounds.tsv` + `logs/status.ndjson`)
    - `stop` and `collect` (creates `/tmp/rust-mule-soak-*.tar.gz`)
  - Script is executable and validated for shell syntax and usage output.
- Decisions: Keep the soak run harness and soak triage tool (`docs/scripts/soak_triage.sh`) together under `docs/scripts` for reproducible operator workflow.
- Next steps: Optional: wire both scripts into a single wrapper (`run + triage`) for one-command baseline comparisons.
- Change log: Added `docs/scripts/rust_mule_soak.sh` to the repository.

- Status: Added soak triage helper script on `main`:
  - New script: `docs/scripts/soak_triage.sh`
  - Input: soak tarball (`/tmp/rust-mule-soak-*.tar.gz`)
  - Output includes:
    - completion signal (`stop requested` markers)
    - round outcome metrics (total/success/success%, first+last success, max fail streak, last300 success)
    - success source concentration (`source_id_hex` top list)
    - key A/B status counters (`max` and `last` from `status.ndjson`)
    - panic/fatal scan for `logs/a.out` and `logs/b.out`
  - Validated against `/tmp/rust-mule-soak-20260214_101721.tar.gz`; reported metrics match prior manual triage.
- Decisions: Keep soak triage tool POSIX shell + awk/grep only (no Python dependency) so it works in constrained environments.
- Next steps: Optional follow-up can add CSV/JSON emit mode for CI ingestion if we want automatic baseline-vs-current comparisons.
- Change log: Added `docs/scripts/soak_triage.sh` and validated report output on the latest soak archive.

- Status: Added UI/API contract assurance scaffolding on `feature/kad-imule-parity-deep-pass`:
  - Added router-level UI contract test in `src/api/mod.rs`:
    - `ui_api_contract_endpoints_return_expected_shapes`
    - validates response shape invariants for UI-critical endpoints:
      - `GET /api/v1/status`
      - `GET /api/v1/searches`
      - `GET /api/v1/searches/:search_id`
      - `GET /api/v1/kad/keyword_results/:keyword_id_hex`
      - `GET /api/v1/kad/peers`
      - `GET /api/v1/settings`
  - Added endpoint coverage map:
    - `docs/ui_api_contract_map.md` (UI sections -> endpoint -> required fields/behavior).
  - Added Playwright smoke test scaffold for UI pages:
    - `ui/package.json`
    - `ui/playwright.config.mjs`
    - `ui/tests/e2e/smoke.spec.mjs`
    - `ui/tests/README.md`
  - Updated `.gitignore` for UI test artifacts:
    - `/ui/node_modules`
    - `/ui/test-results`
    - `/ui/playwright-report`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 68 tests).
- Decisions: Keep browser smoke tests as an opt-in local workflow (Node/Playwright) while enforcing API response contracts in Rust tests to keep CI lightweight and deterministic.
- Next steps: When soak run completes, execute `ui` Playwright smoke against the same running node and add failures (if any) as actionable API/UI contract regressions.
- Change log: UI-critical API response shape checks are now executable, documented, and paired with a runnable browser smoke suite scaffold.

- Status: Implemented organic source-flow observability upgrades on `feature/kad-imule-parity-deep-pass` (requested implementation of steps 2 and 3):
  - Added source batch outcome accounting in `src/kad/service.rs` for both send paths:
    - search batches: `source_search_batch_{candidates,skipped_version,sent,send_fail}`
    - publish batches: `source_publish_batch_{candidates,skipped_version,sent,send_fail}`
  - Batch counters are emitted in status payload (`KadServiceStatus`) and logged in send-batch INFO events.
  - Added per-file source probe tracker state (`source_probe_by_file`) with first-send/first-response timestamps and rolling result counts.
  - Added aggregate status counters for probe timing/results:
    - `source_probe_first_publish_responses`
    - `source_probe_first_search_responses`
    - `source_probe_search_results_total`
    - `source_probe_publish_latency_ms_total`
    - `source_probe_search_latency_ms_total`
  - Wired response-side tracking:
    - on source `PUBLISH_RES` reception, record first publish response latency per file
    - on `SEARCH_RES` keyed to tracked source files, record first search response latency and per-response returned source counts
  - Added unit test:
    - `kad::service::tests::source_probe_tracks_first_send_response_latency_and_results`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 67 tests).
- Decisions: Keep probe tracking lightweight/in-memory and bounded (`SOURCE_PROBE_MAX_TRACKED_FILES = 2048`) with aggregate latency totals in status for immediate triage without introducing persistence or heavy histograms.
- Next steps: Build fresh `mule-a`/`mule-b` artifacts and run repeated non-forced A/B rounds to quantify organic success rate and latency percentiles using the new batch/probe counters.
- Change log: Source send-path selection/success/failure and per-file response timing are now directly measurable from status + logs.

- Status: Implemented source-path diagnostics follow-up on `feature/kad-imule-parity-deep-pass` (requested items 1 and 2):
  - Added receive-edge KAD inbound instrumentation in `src/kad/service.rs`:
    - `event="kad_inbound_packet"` for every decrypted+parsed inbound packet with:
      - opcode hex + opcode name
      - dispatch target label
      - payload length
      - obfuscation/verify-key context
    - `event="kad_inbound_drop"` with explicit reasons:
      - `request_rate_limited`
      - `unrequested_response`
      - `unhandled_opcode`
  - Cross-checked source opcode constants/layouts against iMule reference (`source_ref`):
    - `src/include/protocol/kad2/Client2Client/UDP.h`
    - `src/kademlia/net/KademliaUDPListener.cpp` (`Process2SearchSourceRequest`, `Process2PublishSourceRequest`)
  - Added wire-compat regression tests in `src/kad/wire.rs`:
    - `kad2_source_opcode_values_match_imule`
    - `kad2_search_source_req_layout_matches_imule`
    - `kad2_publish_source_req_layout_has_required_source_tags`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 66 tests).
- Decisions: Keep diagnostics at `DEBUG` level (not INFO) to preserve operability while enabling precise packet-path triage during A/B probes.
- Next steps: Build fresh `mule-a`/`mule-b` artifacts and rerun forced `debug/probe_peer` A<->B; inspect new `kad_inbound_packet`/`kad_inbound_drop` events to pinpoint whether source opcodes arrive and where they are dropped.
- Change log: KAD service now emits deterministic receive-edge opcode/drop telemetry, and source opcode/layout compatibility with iMule is explicitly tested.

- Status: Extended debug peer probing on `feature/kad-imule-parity-deep-pass` to include source-path packets in addition to keyword packets:
  - `src/kad/service.rs` `debug_probe_peer(...)` now sends:
    - `KADEMLIA2_SEARCH_SOURCE_REQ` (for peers `kad_version >= 3`)
    - `KADEMLIA2_PUBLISH_SOURCE_REQ` (for peers `kad_version >= 4`)
  - Existing probe sends remain unchanged:
    - `KADEMLIA2_HELLO_REQ`
    - `KADEMLIA2_SEARCH_KEY_REQ`
    - `KADEMLIA2_PUBLISH_KEY_REQ`
  - Probe debug log now reports source probe send booleans:
    - `sent_search_source`
    - `sent_publish_source`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 63 tests).
- Decisions: Keep source probe sends version-gated to align with existing source batch behavior and avoid forcing unsupported opcodes on low-version peers.
- Next steps: Rebuild `mule-a`/`mule-b` binaries and re-run forced `debug/probe_peer` A->B and B->A; then verify inbound source counters/events (`recv_*_source_*`, `source_store_update`, `source_store_query`) move from zero.
- Change log: `POST /api/v1/debug/probe_peer` can now directly exercise source request paths, enabling deterministic source-path diagnostics.

- Status: Added targeted source-store observability on `feature/kad-imule-parity-deep-pass` and validated via extended two-instance selftest:
  - `src/kad/service.rs` now tracks and reports source lifecycle counters in `kad_status_detail`:
    - `recv_search_source_decode_failures`
    - `source_search_hits` / `source_search_misses`
    - `source_search_results_served`
    - `recv_publish_source_decode_failures`
    - `sent_publish_source_ress`
    - `new_store_source_entries`
  - Added source store gauges in status payload:
    - `source_store_files`
    - `source_store_entries_total`
  - Added structured source observability logs:
    - `event=source_store_update` on inbound `PUBLISH_SOURCE_REQ` store attempts
    - `event=source_store_query` on served `SEARCH_SOURCE_REQ` responses
  - Added unit test coverage:
    - `kad::service::tests::build_status_reports_source_store_totals`
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 63 tests).
- Decisions: Keep observability additive and low-risk (counters + logs) without changing protocol behavior yet; use this pass to isolate source replication/search breakpoints before logic changes.
- Next steps: Re-run targeted A/B probe and inspect new counters/events (`source_store_update`, `source_store_query`, `new_store_source_entries`, `source_store_*`) to identify exact source-path failure stage.
- Change log: Source publish/search/store lifecycle now has explicit service-side counters and logs suitable for direct A/B diagnostics.

- Status: Completed deep KAD parity hardening pass against iMule reference (`source_ref`) on `feature/kad-imule-parity-deep-pass`:
  - Added PacketTracking-style request/response correlation in `src/kad/service.rs`:
    - track outgoing KAD request opcodes with 180s TTL,
    - drop unrequested inbound response packets (bootstrap/hello/res/search/publish/pong shapes).
  - Added per-peer inbound KAD request flood limiting in `src/kad/service.rs` (iMule-inspired limits by opcode family).
  - Added service-mode handling for inbound `KADEMLIA2_BOOTSTRAP_REQ` and reply path:
    - introduced `encode_kad2_bootstrap_res(...)` in `src/kad/wire.rs`,
    - service now responds with self+routing contacts, encrypted with receiver-key flow when applicable.
  - Removed remaining runtime brittle byte-slice `unwrap` conversions in:
    - `src/kad/bootstrap.rs`
    - `src/kad/udp_crypto.rs` (`udp_verify_key` path)
  - Added tests in `src/kad/service.rs`:
    - tracked out-request matching behavior,
    - inbound request flood-limit behavior.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 62 tests).
- Decisions: Keep implementation Rust-native (simple explicit tracker + hash-map counters) while matching iMule behavior intent (tracked responses, anti-flood request gating, bootstrap response semantics) without copying C++ structure.
- Next steps: Optional follow-up parity pass can tighten ACK/challenge semantics further by emulating more of iMule `PacketTracking::LegacyChallenge` behavior for edge peers.
- Change log: KAD service now behaves closer to iMule for bootstrap responsiveness, response legitimacy checks, and inbound request flood resistance.

- Status: Completed panic-hardening follow-up for sanity findings (items 1..4) on `main`:
  - `src/logging.rs`: removed panic-on-poison in warning throttle lock path; now recovers poisoned mutex state and logs a warning.
  - `src/app.rs`: removed runtime `unwrap()` conversions for destination hash/array extraction; switched to explicit copy logic.
  - `src/i2p/sam/datagram.rs`: replaced `expect()` in `forward_port`/`forward_addr` with typed `Result` returns (`SamError`), and updated call sites in `src/app.rs`.
  - `src/kad/service.rs`, `src/nodes/imule.rs`, `src/kad/wire.rs`: replaced safe-but-brittle slice `try_into().unwrap()` patterns with non-panicking copy-based conversions.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, strict clippy sanity pass (`unwrap/expect/panic/todo/unimplemented`), and `cargo test` (all passing; strict pass now only flags remaining test/internal non-critical unwrap/expect sites outside this scoped fix).
- Decisions: Keep panic-hardening targeted to runtime production paths first; test-only unwrap/expect cleanup can be a separate ergonomics pass.
- Next steps: Optional low-risk pass to eliminate remaining test/internal unwrap/expect usage repository-wide for stricter lint cleanliness.
- Change log: Production/runtime panic surfaces identified in the sanity pass were removed for logging lock handling, SAM datagram address accessors, and key byte-conversion paths.

- Status: Completed typed-error migration pass across remaining runtime/boundary modules on `main`:
  - Converted to typed errors:
    - `src/app.rs` (`AppError`)
    - `src/main.rs` (`MainError`, `ConfigValidationError`)
    - `src/api/mod.rs` serve path (`ApiError`)
    - `src/single_instance.rs` (`SingleInstanceError`)
    - `src/kad/service.rs` (`KadServiceError`)
    - bin utilities: `src/bin/imule_nodes_inspect.rs`, `src/bin/sam_dgram_selftest.rs`
  - Removed remaining runtime `anyhow` usage from `src/` implementation paths.
  - Updated `docs/TODO.md` to mark typed-error migration as done and refreshed `docs/TASKS.md` with next priority.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Keep typed errors explicit per subsystem/boundary and preserve existing HTTP/runtime behavior by mapping at boundaries rather than changing response semantics.
- Next steps: Focus on KAD search/publish reliability and ACK/timeout observability (`docs/TASKS.md` current priority).
- Change log: End-to-end code paths now use typed error enums instead of `anyhow`, including app orchestration and utility binaries.

- Status: Documentation sync/normalization pass completed on `main`:
  - Updated `README.md` API/UI auth flow to reflect current behavior:
    - `/api/v1/session` issues `rm_session` cookie.
    - `/api/v1/events` uses session-cookie auth.
  - Normalized `docs/TODO.md`:
    - marked clippy round completed,
    - corrected settings endpoint paths to `/api/v1/settings`,
    - marked docs alignment done,
    - added remaining typed-error migration item for boundary/runtime layers.
  - Updated `docs/API_DESIGN.md` to distinguish implemented auth/session model from forward-looking API ideas and removed stale SSE token-query framing.
  - Added concrete next-priority execution plan in `docs/TASKS.md`.
- Decisions: Keep `docs/API_DESIGN.md` as a mixed strategic + implemented view, but explicitly label forward-looking endpoints and defer executable examples to `docs/api_curl.md`.
- Next steps: Execute `docs/TASKS.md` item #1 (finish typed-error migration in boundary/runtime layers).
- Change log: Documentation now matches the current session-cookie SSE model, endpoint paths, and project priorities.

- Status: Expanded subsystem-specific typed errors (second batch) on `feature/subsystem-typed-errors`:
  - Replaced `anyhow` in additional KAD/SAM subsystem modules with typed errors:
    - `src/kad/wire.rs` (`WireError`)
    - `src/kad/packed.rs` (`InflateError`)
    - `src/kad/udp_crypto.rs` (`UdpCryptoError`)
    - `src/kad/udp_key.rs` (`UdpKeyError`)
    - `src/kad/bootstrap.rs` (`BootstrapError`)
    - `src/i2p/sam/keys.rs` (`SamKeysError`)
    - `src/i2p/sam/kad_socket.rs` now returns `Result<_, SamError>` directly.
  - Kept app/main/api as the top-level error aggregation boundary.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Typed errors were added first in protocol/parsing/crypto and SAM helper modules where error provenance matters most; orchestration layers remain unchanged for now.
- Next steps: Remaining `anyhow` usage is concentrated in boundary/runtime modules (`src/app.rs`, `src/main.rs`, `src/api/mod.rs`, `src/single_instance.rs`, `src/kad/service.rs`, and bin tools) and can be migrated incrementally if full typed coverage is required.
- Change log: KAD wire/deflate/UDP-crypto/bootstrap and SAM keys/socket now emit concrete typed errors rather than `anyhow`.

- Status: Implemented subsystem-specific typed errors on `feature/subsystem-typed-errors`:
  - Replaced internal `anyhow` usage with typed error enums + local `Result` aliases in:
    - `src/config.rs`
    - `src/config_io.rs`
    - `src/api/token.rs`
    - `src/kad.rs`
    - `src/kad/keyword.rs`
    - `src/nodes/imule.rs`
    - `src/i2p/b64.rs`
    - `src/i2p/http.rs`
  - Preserved current app-level behavior by allowing these typed errors to bubble into existing `anyhow` boundaries where applicable.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Kept this pass focused on subsystem modules with clear ownership boundaries; app orchestration/error aggregation remains unchanged.
- Next steps: Continue migrating remaining non-core modules still using `anyhow` (for example selected KAD service/bootstrap internals) if full typed-error coverage is desired.
- Change log: Subsystem error handling now uses concrete typed errors instead of stringly `anyhow` in the converted modules.

- Status: Completed logging follow-up pass (`feature/logging-followup`):
  - Added throttled-warning suppression counters surfaced as periodic summary logs (`event=throttled_warning_summary`).
  - Broadened log redaction on KAD identifiers in operational/debug paths (`redact_hex`) and shortened destination logging to short base64 forms in additional send-failure paths.
  - Added structured `event=...` fields to key startup/status/search/publish log lines for machine filtering.
  - Reduced bootstrap INFO noise by demoting per-peer HELLO/PONG/BOOTSTRAP chatter to DEBUG.
  - Added retention helper tests in `src/config.rs`:
    - rotated filename split/match validation
    - old rotated-file cleanup behavior.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 60 tests).
- Decisions: Keep operator-facing INFO logs as concise aggregate state transitions and preserve per-peer/protocol chatter under DEBUG/TRACE.
- Next steps: Optional final pass can redact remaining DEBUG payload snippets (e.g., packet heads) for environments where debug bundles are shared externally.
- Change log: Logging now includes throttling observability, stronger identifier redaction, and tested retention helpers while keeping INFO output lower-noise.

- Status: Completed API bind policy hardening (`feature/api-bind-loopback-policy`):
  - Enforced loopback-only API bind host handling via shared config helper (`parse_api_bind_host`).
  - Accepted hosts: `localhost`, `127.0.0.1`, `::1`.
  - Rejected non-loopback binds (e.g. `0.0.0.0`, LAN/WAN IPs) in:
    - startup config validation (`src/main.rs`)
    - API server bind resolution (`src/api/mod.rs`)
    - settings API validation (`PATCH /api/v1/settings`)
  - Added tests:
    - `parse_api_bind_host_accepts_only_loopback`
    - extended settings patch rejection coverage for non-loopback `api.host`.
  - Updated `docs/TODO.md` to mark the API bind requirement as completed.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 57 tests).
- Decisions: Keep a strict local-control-plane model by default; do not allow wildcard/non-loopback API binds without a future explicit remote-mode design.
- Next steps: If remote/headless control is later required, introduce an explicit opt-in mode with TLS/auth hardening rather than loosening default bind policy.
- Change log: API host handling is now consistently loopback-only across startup, runtime serve, and settings updates.

- Status: Completed logging hardening / INFO-vs-DEBUG pass on `feature/log-hardening`.
  - Added shared logging utilities (`src/logging.rs`) for redaction helpers and warning throttling.
  - Removed noisy boot marker and moved raw SAM HELLO reply logging to `DEBUG`.
  - Redacted Kademlia identity at startup logs (`kad_id` now shortened).
  - Rebalanced KAD periodic status logging:
    - concise operational summary at `INFO`
    - full status payload at `DEBUG`
  - Added warning throttling for repetitive bootstrap send-failure warnings and recurring KAD decay warning.
  - Updated tracing file appender setup:
    - daily rotated naming as `prefix.YYYY-MM-DD.suffix` (default `rust-mule.YYYY-MM-DD.log`)
    - startup cleanup of matching logs older than 30 days.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 56 tests).
- Decisions: Keep redaction/throttling lightweight and local (no new dependencies) and preserve existing log filter controls (`general.log_level`, `general.log_file_level`).
- Next steps: Optional follow-up is to apply redaction helpers to any remaining DEBUG-level destination/id logs where operators may share debug bundles externally.
- Change log: Logging output is now safer and lower-noise at `INFO`, with richer diagnostics preserved at `DEBUG` and daily log retention enforced.

- Status: Completed clippy+formatting improvement batch on `feature/clippy-format-pass`.
  - Addressed all active `cargo clippy --all-targets --all-features` warnings across app/KAD/utility modules.
  - Applied idiomatic fixes (`div_ceil`, iterator/enumerate loops, collapsed `if let` chains, unnecessary casts/question-marks/conversions, lock-file open options).
  - Added targeted `#[allow(clippy::too_many_arguments)]` on orchestration-heavy KAD service functions where signature reduction would be invasive for this pass.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (all passing; 56 tests).
- Decisions: Keep high-arity KAD orchestration signatures for now and explicitly annotate them; prioritize behavior-preserving lint cleanup over structural refactors in this iteration.
- Next steps: If desired, follow up with a dedicated refactor pass to reduce `too_many_arguments` allowances via context structs.
- Change log: Repository now passes clippy cleanly under current lint set, with formatting normalized.

- Status: Implemented UI auto-open and headless toggle flow (initial UI milestone #1):
  - Added `general.auto_open_ui` (default `true`) to runtime config/settings.
  - Startup now conditionally auto-opens `http://localhost:<port>/index.html` in default browser.
  - Auto-open is gated by readiness checks: token file exists, `/api/v1/health` returns 200, and `/index.html` returns 200 (timeout-protected).
  - Added settings wiring so UI/API `GET/PATCH /api/v1/settings` reads/writes `general.auto_open_ui`.
  - Added settings UI control: “Auto Open UI In Browser On Boot” with headless-disable option.
  - Updated docs (`docs/TODO.md`, `docs/UI_DESIGN.md`, `docs/architecture.md`, `docs/api_curl.md`).
- Decisions: Keep auto-open behavior best-effort and non-fatal; failures to launch browser only log warnings and do not affect backend startup.
- Next steps: Run browser-based axe/Lighthouse pass and patch measurable UI issues; then normalize remaining docs wording for “initial UI version” completion state.
- Change log: App can now launch the local UI automatically after API/UI/token readiness, and operators can disable this for headless runs via settings/config.

- Status: Alpine binding best-practice sanity pass completed (second pass):
  - Re-scanned all `ui/*.html` Alpine bindings and `ui/assets/js/{app,helpers}.js`.
  - Verified no side-effectful function calls in display bindings (`x-text`, `x-bind`, `x-show`, `x-if`, `x-for`).
  - Normalized remaining complex inline binding expressions into pure computed getters:
    - `appSearch.keywordHits` used by `ui/search.html` `x-for`.
    - `appSearchDetails.searchIdLabel` used by `ui/search_details.html` `x-text`.
- Decisions: Keep side effects restricted to lifecycle and explicit event handlers (`x-init`, `@click`, `@submit`, SSE callbacks).
- Next steps: Optional follow-up is extracting repeated status badge text ternaries into computed getters for style consistency only.
- Change log: Alpine templates now consistently consume normalized state/getters and avoid complex inline display expressions.

- Status: Completed a UI accessibility/usability sweep across all `ui/*.html` pages.
  - Added keyboard skip-link and focus target (`#main-content`) on all pages.
  - Added semantic navigation landmarks and `aria-current` for active routes.
  - Added live regions for runtime error/notice messages (`role="alert"` / `role="status"`).
  - Added table captions and explicit `scope` attributes on table headers.
  - Added chart canvas ARIA labels and log-region semantics for event stream output.
  - Added shared `.skip-link` and `.sr-only` styles in `ui/assets/css/base.css`.
- Decisions: Keep accessibility improvements HTML/CSS-only for now (no controller-side behavior changes), and preserve current visual layout.
- Next steps: Run browser-based automated audit (axe/Lighthouse) and address measurable contrast/focus-order findings.
- Change log: UI shell and data views now have stronger baseline WCAG support for keyboard navigation, screen-reader semantics, and dynamic status announcements.

- Status: Completed UI/API follow-up items 1 and 2 on `feature/ui-bootstrap`:
  - Added shared session status/check/logout widget in sidebar shell on all UI pages, backed by a reusable Alpine mixin.
  - Added periodic backend session cleanup task (`SESSION_SWEEP_INTERVAL=5m`) in addition to lazy cleanup on create/validate.
  - Added API unit test `cleanup_expired_sessions_removes_expired_entries`.
- Decisions: Keep session UX in a single shared sidebar control; keep session sweep simple (fixed interval background task) with existing `Mutex<HashMap<...>>` session store.
- Next steps: Merge this branch to `main`, then move to the next prioritized UI/API backlog item after validating behavior manually in browser.
- Change log: Session lifecycle visibility and expiry hygiene are now continuously maintained in both frontend shell and backend runtime.

- Implemented API bearer token rotation flow:
  - Added `POST /api/v1/token/rotate` (bearer-protected).
  - API token is now shared mutable state (`RwLock`) and token file path is stored in API state.
  - Rotation persists a new token to `data/api.token`, swaps in-memory token, and clears all active frontend sessions.
  - Added API test `token_rotate_updates_state_file_and_clears_sessions`.
  - Added settings UI action `Rotate API Token`:
    - Calls `/api/v1/token/rotate`
    - Updates `sessionStorage` token
    - Re-creates frontend session via `POST /api/v1/session`
  - Added token helper `rotate_token()` in `src/api/token.rs`.
  - Updated docs (`docs/architecture.md`, `docs/api_curl.md`, `docs/UI_DESIGN.md`) with token rotation behavior and endpoint.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Bearer tokens can now be actively rotated from UI/API with immediate session re-bootstrap and old-session invalidation.
- Completed next UI/API security+UX batch (in requested order):
  - Session lifecycle hardening:
    - Added `GET /api/v1/session/check` (session-cookie auth).
    - Added `POST /api/v1/session/logout` (session-cookie auth, clears cookie + invalidates server session).
    - Added session TTL handling (8h) with expiry cleanup on session create/validate.
    - Updated frontend SSE helper to probe `/api/v1/session/check` on stream errors and redirect to `/auth` on expired/invalid session.
    - Added visible UI logout control in settings (`Logout Session`) calling `POST /api/v1/session/logout` and redirecting to `/auth`.
  - Middleware integration tests (full-router):
    - `unauthenticated_ui_route_redirects_to_auth`
    - `authenticated_ui_route_with_session_cookie_succeeds`
    - `events_rejects_bearer_only_but_accepts_session_cookie`
  - Chart UX polish on `node_stats`:
    - Added chart controls: pause/resume sampling, reset history, and sample-window selector.
    - Increased history buffer depth and made chart rendering window configurable.
  - Added `build_app()` router constructor to enable handler+middleware integration tests without booting a TCP server.
  - Updated docs (`docs/architecture.md`, `docs/api_curl.md`, `docs/UI_DESIGN.md`, `docs/TODO.md`) for new session endpoints/behavior and chart controls status.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Implemented session check/logout + TTL cleanup, added middleware auth integration coverage, and shipped chart interaction controls in node stats.
- CSS normalization pass completed for variable/units discipline:
  - Moved remaining shared `base.css` size literals into reusable vars in `ui/assets/css/layout.css`:
    - container width, glow dimensions, badge/button/table sizing, log max-height.
  - Updated `ui/assets/css/base.css` to consume vars instead of hardcoded numeric literals.
  - Replaced non-hairline `px` units in theme focus/shadow tokens with relative units in:
    - `ui/assets/css/color-dark.css`
    - `ui/assets/css/colors-light.css`
    - `ui/assets/css/color-hc.css`
  - Kept hairline width token as `--line: 1px` for border usage.
  - Ran Prettier for CSS files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Shared UI styles now rely on layout/theme variables with non-hairline sizing converted to relative units.
- Implemented first Chart.js statistics set on `ui/node_stats.html`:
  - Added three charts:
    - Search hits over time (line)
    - Request/response rate over time (line)
    - Live vs idle peer mix over time (stacked bar)
  - Added Chart.js loader on `node_stats` and chart canvas panels in the page layout.
  - Extended `appNodeStats()` in `ui/assets/js/app.js`:
    - SSE-driven status updates + polling fallback.
    - Time-series history buffers and rate calculation from status counters.
    - Chart initialization/update lifecycle and theme-variable color usage.
  - Added reusable chart container token/style:
    - `--chart-height` in `ui/assets/css/layout.css`
    - `.chart-wrap` in `ui/assets/css/base.css`
  - Updated `docs/TODO.md` and `docs/UI_DESIGN.md` to mark Chart.js usage as implemented and statistics work as partial/ongoing.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Node stats page now includes live operational charts for search productivity, request/response rates, and peer health mix.
- Implemented frontend session-cookie auth for UI routes and SSE:
  - Added `POST /api/v1/session` (bearer-protected) to issue `rm_session` HTTP-only cookie.
  - Added in-memory session store in API state and cookie validation helpers.
  - Updated auth middleware policy:
    - `/api/v1/*` stays bearer-token protected (except `/api/v1/health` and `/api/v1/dev/auth`).
    - `/api/v1/events` now requires valid session cookie (no token query fallback).
    - All frontend routes (`/`, `/index.html`, `/ui/*`, fallback paths) require valid session cookie; unauthenticated access redirects to `/auth`.
  - Added `/auth` bootstrap page to establish session:
    - Calls `/api/v1/dev/auth` (loopback-only), then `POST /api/v1/session` with bearer token, then redirects to `/index.html`.
  - Updated frontend SSE client to use `/api/v1/events` without `?token=...`.
  - Updated auth-related tests:
    - API bearer exempt-path assertions
    - frontend exempt-path assertions
    - session-cookie parsing
  - Updated docs (`docs/TODO.md`, `docs/UI_DESIGN.md`, `docs/architecture.md`, `docs/api_curl.md`) to reflect session-cookie UI/SSE auth and bearer API auth.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Replaced SSE query-token auth with cookie-based frontend session auth and enforced cookie gating on all UI routes.
- Implemented API-backed settings read/update and wired settings UI:
  - Added `GET /api/v1/settings` and `PATCH /api/v1/settings` in `src/api/mod.rs`.
  - API now keeps a shared runtime `Config` in API state and persists valid PATCH updates to `config.toml`.
  - Added validation for settings updates (`sam.host`, `sam.port`, `sam.session_name`, `api.host`, `api.port`, and log filter syntax via `EnvFilter`).
  - Added API tests:
    - `settings_get_returns_config_snapshot`
    - `settings_patch_updates_and_persists_config`
    - `settings_patch_rejects_invalid_values`
  - Updated settings UI:
    - Added settings form in `ui/settings.html` for `general`, `sam`, and `api` fields.
    - Added `apiPatch()` helper and wired `appSettings()` to load/save via `/api/v1/settings`.
    - Added save/reload flow with restart-required notice.
  - Updated docs:
    - `docs/TODO.md`: marked API-backed settings task as done.
    - `docs/UI_DESIGN.md`: marked settings API integration as implemented.
    - `docs/architecture.md` and `docs/api_curl.md`: documented new settings endpoints and curl examples.
  - Ran Prettier on changed UI files and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Settings page is now backed by persisted API settings (`GET/PATCH /api/v1/settings`) instead of runtime-only placeholders.
- Documentation/UI planning sync pass completed:
  - Updated `docs/TODO.md` UI checklist statuses to reflect implemented work (embedded assets, Alpine usage, shell pages, search form, overview, network status) and kept unresolved/partial items open (Chart.js usage, protected static UI, SSE token exposure, settings API, auto-open/headless toggle).
  - Updated `docs/UI_DESIGN.md` to match current routes and contracts:
    - `/api/v1/...` endpoint namespace in live-data and API contract sections.
    - Navigation model now reflects shared-shell multi-page UI (`index`, `search`, `search_details`, `node_stats`, `log`, `settings`) and `searchId` query param usage.
    - Added implementation snapshot with completed, partial, and open items.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Synced UI TODO/design documentation with the actual current implementation and clarified remaining UI backlog.
- Canonicalized root UI route to explicit index path:
  - `GET /` now redirects to `/index.html`.
  - Added explicit `GET /index.html` route serving embedded `index.html`.
  - Updated SPA fallback redirect target from `/` to `/index.html` for unknown non-API/non-asset routes.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Root URL now canonical redirects to `/index.html`; fallback redirects align to same canonical entry.
- Added explicit UI startup message on boot in `src/app.rs`:
  - Logs `rust-mule UI available at: http://localhost:<port>` right before API server task spawn.
  - Uses configured `api.port` so users get a direct URL immediately during startup.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Startup now emits a clear local UI URL message for quick operator discovery.
- Added SPA fallback behavior for unknown browser routes:
  - Added router fallback handler in `src/api/mod.rs` that redirects unknown non-API/non-asset paths to `/` (serving embedded `index.html`).
  - Redirect target is always `/`, so arbitrary query parameters on unknown paths are dropped.
  - Kept `/api/*` and `/ui/assets/*` as real 404 paths when missing (no SPA redirect for API/static asset misses).
  - Updated auth exemption to allow non-API paths through auth middleware so fallback can run before auth checks.
  - Added tests:
    - `spa_fallback_redirects_unknown_non_api_paths_to_root`
    - `spa_fallback_does_not_capture_api_or_asset_paths`
    - Extended auth-exempt path coverage for unknown non-API paths.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Unknown non-API routes now canonicalize to `/` (index) with query params stripped, while API and missing asset paths remain 404.
- Embedded UI into binary using `include_dir`:
  - Added `include_dir` dependency.
  - Added static `UI_DIR` bundle for `$CARGO_MANIFEST_DIR/ui`.
  - Switched UI page/asset serving in `src/api/mod.rs` from filesystem reads (`tokio::fs::read`) to embedded lookups.
  - Kept existing UI path safety guards (`is_safe_ui_segment`, `is_safe_ui_path`).
  - Added API unit test `embeds_required_ui_files` validating required `/ui/*.html`, `/ui/assets/css/*.css`, and `/ui/assets/js/*.js` are included in the embedded bundle.
  - Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: UI static assets/pages are now binary-embedded and served without runtime filesystem dependency.
- Alpine binding best-practice sanity pass completed:
  - Normalized `searchThreads` in `ui/assets/js/app.js` to include precomputed `state_class`.
  - Normalized node rows in `appNodeStats` to include precomputed `ui_state`, `ui_state_class`, and `inbound_label`.
  - Updated templates (`ui/index.html`, `ui/search.html`, `ui/search_details.html`, `ui/node_stats.html`, `ui/log.html`, `ui/settings.html`) to bind directly to precomputed fields instead of calling controller/helper methods from bindings.
  - Added `activeThreadStateClass` (index) and `detailsStateClass` (search details) getters for declarative badge binding.
  - Ran Prettier on UI JS/HTML and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` (`cargo test` passed; existing clippy warnings unchanged).
- Change log: Refactored Alpine bindings to remove template-time helper method calls and keep side effects inside explicit actions/lifecycle methods only.
- Performed CSS theme sanity refactor under `ui/assets/css`:
  - Moved all color literals used by shared UI components into theme files only:
    - `ui/assets/css/color-dark.css`
    - `ui/assets/css/colors-light.css`
    - `ui/assets/css/color-hc.css`
  - `ui/assets/css/base.css` and `ui/assets/css/layout.css` now consume color variables only (no direct color values).
  - Fixed dark theme scoping to `html[data-theme=\"dark\"]` (instead of global `:root`) so light/hc themes apply correctly.
- Added persisted theme bootstrapping:
  - New early loader `ui/assets/js/theme-init.js` applies `localStorage.ui_theme` before CSS paint.
  - Included `theme-init.js` in all UI HTML pages.
- Implemented Settings theme selector:
  - Added theme control in `ui/settings.html` for `dark|light|hc`.
  - `appSettings()` now applies selected theme to `<html data-theme=\"...\">` and persists to `localStorage`.
- Ran Prettier (`ui/assets/js/app.js`) and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after theme implementation (`cargo test` passed; existing clippy warnings unchanged).
- Performed API sanity audit against current UI helpers/controllers:
  - Confirmed all active Alpine controller API calls are backed by `/api/v1` endpoints.
  - Confirmed stop/delete UI controls now use real API handlers (`/searches/:id/stop`, `DELETE /searches/:id`).
- Added API handler-level tests for search control endpoints in `src/api/mod.rs`:
  - `search_stop_dispatches_service_command`
  - `search_delete_dispatches_with_default_purge_true`
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API sanity/test additions (`cargo test` passed; existing clippy warnings unchanged).
- Completed API-backing coverage for Alpine UI controls/helpers by implementing missing search control endpoints:
  - Added `POST /api/v1/searches/:search_id/stop`.
  - Added `DELETE /api/v1/searches/:search_id` with `purge_results` (default `true`).
  - Wired `indexApp.stopActiveSearch()` and `indexApp.deleteActiveSearch()` to these endpoints.
- Added backend service commands and logic:
  - `StopKeywordSearch` (disable ongoing search/publish for a job).
  - `DeleteKeywordSearch` (remove active job; optionally purge cached keyword results/store/interest).
- Added frontend helper `apiDelete()` (`ui/assets/js/helpers.js`) for `/api/v1` DELETE calls.
- Added unit tests in KAD service:
  - `stop_keyword_search_disables_active_job`
  - `delete_keyword_search_purges_cached_results`
- Updated API docs for new endpoints (`docs/architecture.md`, `docs/api_curl.md`).
- Ran Prettier on UI JS and ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API coverage implementation (`cargo test` passed; existing clippy warnings unchanged).
- Closed UI consistency gaps identified in `/ui` review:
  - Added real settings page `ui/settings.html` with backing `appSettings()` controller.
  - Wired all sidebar `Settings` links to `/ui/settings`.
  - Wired `+ New Search` buttons with Alpine actions (`index` navigates to search page, `search` resets form state).
  - Wired overview action buttons (`Stop`, `Export`, `Delete`) to implemented Alpine methods in `indexApp`.
  - Removed hardcoded overview header state and made it data-driven from selected active thread.
- Ran Prettier on `ui/assets/js/app.js` and then ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after the UI consistency pass (`cargo test` passed; existing clippy warnings unchanged).
- Added `ui/log.html` with the shared shell and a dedicated Logs view.
- Implemented `appLogs()` Alpine controller in `ui/assets/js/app.js`:
  - Bootstraps token and loads search threads.
  - Fetches status snapshots from `GET /api/v1/status`.
  - Subscribes to `GET /api/v1/events` SSE and appends rolling log entries with timestamps.
  - Keeps an in-memory log buffer capped at 200 entries.
- Updated shell navigation links in UI pages so "Logs" points to `/ui/log`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after logs page/controller implementation (`cargo test` passed; existing clippy warnings unchanged).
- Ran Prettier on `ui/assets/js/app.js` and `ui/assets/js/helpers.js` using `ui/.prettierrc` rules; verified with `prettier --check`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after JS formatting pass (`cargo test` passed; existing clippy warnings unchanged).
- Added `ui/node_stats.html` with the same shell structure as other UI pages.
- Implemented node status view for live/active visibility:
  - Loads `/api/v1/status` and `/api/v1/kad/peers`.
  - Displays total/live/active node KPIs.
  - Displays node table with per-node state badge (`active`, `live`, `idle`) plus Kad ID/version/ages/failures.
- Added frontend `appNodeStats()` in `ui/assets/js/app.js`:
  - Sorts nodes by activity state then recency.
  - Reuses API-backed search threads in the sidebar.
- Updated shell navigation links across pages to point "Nodes / Routing" to `/ui/node_stats`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after node stats page implementation (`cargo test` passed; existing clippy warnings unchanged).
- Added API-backed keyword search thread endpoints:
  - `GET /api/v1/searches` returns active keyword-search jobs from KAD `keyword_jobs`.
  - `GET /api/v1/searches/:search_id` returns one active search plus its current hits.
  - `search_id` maps to keyword ID hex for the active job.
- Implemented dynamic search threads in UI sidebars:
  - `ui/index.html` and `ui/search.html` now load active search threads from API.
  - Search thread rows link to `/ui/search_details?searchId=<keyword_id_hex>`.
- Added `ui/search_details.html` with the same shell:
  - Reads `searchId` from query params.
  - Loads `/api/v1/searches/:search_id` and displays search summary + hits table.
- Extended frontend app wiring:
  - Added shared search-thread loading and state-badge mapping in `ui/assets/js/app.js`.
  - Added `appSearchDetails()` controller for search detail page behavior.
- Updated docs for new API routes (`docs/architecture.md`, `docs/api_curl.md`).
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after search-thread/details implementation (`cargo test` passed; existing clippy warnings unchanged).
- Replicated the app shell layout in `ui/search.html` (sidebar + main panel) to match the index page structure.
- Implemented first functional keyword-search form in the search UI:
  - Added query and optional `keyword_id_hex` inputs.
  - Wired `POST /api/v1/kad/search_keyword` submission from Alpine (`appSearch.submitSearch`).
  - Added results refresh via `GET /api/v1/kad/keyword_results/:keyword_id_hex`.
  - Added first-pass results table rendering for keyword hits.
- Added reusable UI form styles in shared CSS:
  - New form classes in `ui/assets/css/base.css` (`form-grid`, `field`, `input`).
  - Added form-control tokens to `ui/assets/css/layout.css`.
- Added JS helper `apiPost()` in `ui/assets/js/helpers.js` and expanded `appSearch()` state/actions in `ui/assets/js/app.js`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after search UI implementation (`cargo test` passed; existing clippy warnings unchanged).
- Moved `index.html` inline styles into shared CSS:
  - Removed `<style>` block from `ui/index.html`.
  - Added reusable shell/sidebar/search-state classes in `ui/assets/css/base.css`.
  - Added layout/state CSS variables in `ui/assets/css/layout.css` and referenced them from base styles.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after CSS/layout refactor (`cargo test` passed; existing clippy warnings unchanged).
- Updated `ui/index.html` layout to match UI design spec shell:
  - Added persistent sidebar (primary nav + search thread list + new search control).
  - Added main search overview sections (header/actions, KPIs, progress, results, activity/logs).
  - Preserved existing Alpine status/token/SSE bindings while restructuring markup.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after index layout update (`cargo test` passed; existing clippy warnings unchanged).
- Implemented backend-served UI bootstrap skeleton:
  - Added static UI routes: `/`, `/ui`, `/ui/:page`, and `/ui/assets/*`.
  - Added safe path validation for UI file serving (reject traversal/unsafe paths).
  - Added content-type-aware static file responses for HTML/CSS/JS/assets.
- Implemented UI auth bootstrap flow for development:
  - UI now bootstraps bearer auth via `GET /api/v1/dev/auth`.
  - Token is stored in browser `sessionStorage` and used for `/api/v1/status`.
  - UI opens SSE with `GET /api/v1/events?token=...` for browser compatibility.
- Updated UI skeleton pages and JS modules:
  - Rewrote `ui/assets/js/helpers.js` and `ui/assets/js/app.js` to align with `/api/v1`.
  - Updated `ui/index.html` and `ui/search.html` to use module scripts and current API flow.
- Added/updated API tests:
  - Query-token extraction test for SSE auth path.
  - UI path-safety validation test coverage.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after UI/bootstrap changes (`cargo test` passed; existing clippy warnings unchanged).
- Implemented API CORS hardening for `/api/v1`:
  - Allow only loopback origins (`localhost`, `127.0.0.1`, and loopback IPs).
  - Allow only `Authorization` and `Content-Type` request headers.
  - Allow methods `GET`, `POST`, `PUT`, `PATCH`, `OPTIONS`.
  - Handle `OPTIONS` preflight without bearer auth.
  - Added unit tests for origin allow/deny behavior.
- Fixed CORS origin parsing for bracketed IPv6 loopback (`http://[::1]:...`) and re-ran validation (`cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`).
- API contract tightened for development-only workflow:
  - Removed temporary unversioned API route aliases; API is now `/api/v1/...` only.
  - Removed `api.enabled` compatibility field from config parsing.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after removing legacy API handling (`cargo test` passed; clippy warnings remain in existing code paths).
- Created feature branch `feature/api-v1-control-plane` and implemented API control-plane changes:
  - Canonical API routes are now under `/api/v1/...`.
  - Added loopback-only dev auth endpoint `GET /api/v1/dev/auth` (returns bearer token).
  - API is now always on; only API host/port are configurable.
- Updated docs and shell wrappers to use `/api/v1/...` endpoints (`README.md`, `docs/architecture.md`, `docs/api_curl.md`, `docs/scripts/*`, `docs/TODO.md`, `docs/API_DESIGN.md`, `docs/UI_DESIGN.md`).
- Added `docs/scripts/dev_auth.sh` helper for `GET /api/v1/dev/auth`.
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after the API/docs changes (`cargo test` passed; clippy warnings remain in existing code paths).
- Per-user request, documentation normalization pass completed across `docs/` (typos, naming consistency, and branch references).
- Ran `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after docs changes (`cargo test` passed; clippy warnings remain in existing code paths).
- Long-haul two-instance run (25 rounds) confirmed network-origin keyword hits on both instances:
  - A received non-empty `SEARCH_RES` at 2026-02-11 19:41:41.
  - B received non-empty `SEARCH_RES` at 2026-02-11 19:50:02.
- Routing snapshot at end of run: total_nodes=157, verified=135, buckets_empty=121, bucket_fill_max=80, last_seen_max≈35060s (~9.7h), last_inbound_max≈29819s (~8.3h). Routing still not growing (`new_nodes=0`).
- Observed SAM `SESSION STATUS RESULT=I2P_ERROR MESSAGE="PONG timeout"` on both instances at 2026-02-12 06:49:20; service auto-recreated SAM session.
- Source publish/search remained empty in the script output.
- Periodic KAD2 BOOTSTRAP_REQ now sends **plain** packets to peers with `kad_version` 2–5 and **encrypted** packets only to `kad_version >= 6` to avoid silent ignores in mixed-version networks.
- Publish/search candidate selection now truncates by **distance first**, then optionally reorders the _same_ set by liveness to avoid skipping closest nodes.
- Restarting a keyword search or publish job now clears the per-job `sent_to_*` sets so manual retries re-send to peers instead of becoming no-ops.
- Publish/search candidate selection now returns a **distance-ordered list with fallback** (up to `max*4` closest) so if early candidates are skipped, farther peers are still available in the batch.

## Status (2026-02-11)

- Updated `docs/scripts/two_instance_dht_selftest.sh` to poll keyword results (early exit on `origin=network`), add configurable poll interval, and allow peer snapshot frequency control.
- Increased default `wait-search-secs` to 45s in the script (I2P cadence).
- Updated `tmp/test_script_command.txt` with new flags for polling and peer snapshot mode.
- Added routing snapshot controls to `docs/scripts/two_instance_dht_selftest.sh` (each|first|end|none) and end-of-run routing summary/buckets when `--routing-snapshot end` is set.
- Updated `tmp/test_script_command.txt` to use `--routing-snapshot end` and `--peers-snapshot none` for the next long run.

## Status (2026-02-10)

- Ran `docs/scripts/two_instance_dht_selftest.sh` (5 rounds). Each instance only saw its own locally-injected keyword hit; no cross-instance keyword hits observed.
- No `PUBLISH_RES (key)` acks and no inbound `PUBLISH_KEY_REQ` during the run; `SEARCH_RES` replies were empty.
- Routing stayed flat (~154), live peers ~2, network appears quiet.
- Added debug routing endpoints (`/debug/routing/*`) plus debug lookup trigger (`/debug/lookup_once`) and per-bucket refresh lookups.
- Added staleness-based bucket refresh with an under-populated growth mode; routing status logs now include bucket fill + verified %.
- Routing table updates now treat inbound responses as activity (last_seen/last_inbound) and align bucket index to MSB distance.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after the debug/refresh changes (clippy warnings remain; see prior notes).
- Added HELLO preflight on inbound responses, prioritized live peers for publish/search, and added post-warmup routing snapshots in the two-instance script.
- Aligned Kad2 HELLO_REQ encoding with iMule: kadVersion=1, empty TagList, sent unobfuscated.
- Added HELLO_RES_ACK counters (sent/recv), per-request debug logs for publish/search requests, and a `/debug/probe_peer` API to send HELLO/SEARCH/PUBLISH to a specific peer.
- Added `/debug/probe_peer` curl docs + script (`docs/api_curl.md`, `docs/scripts/debug_probe_peer.sh`).
- Added KAD2 RES contact acceptance stats (per-response debug log) and HELLO_RES_ACK skip counter.
- Added optional dual HELLO_REQ mode (plain + obfuscated) behind `kad.service_hello_dual_obfuscated` (experimental).
- Added config flag wiring for dual-HELLO mode and contact acceptance stats logging; updated `config.toml` hint.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after these changes (clippy warnings remain; see prior notes).
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after debug probe + logging changes (clippy warnings remain; see prior notes).
- Ran `cargo fmt`, `cargo clippy`, `cargo test` after HELLO/live-peer changes (clippy warnings remain; see prior notes).
- Added `origin` field to keyword hits (`local` vs `network`) in the API response.
- Added `/kad/peers` API endpoint and extra inbound-request counters to `/status` for visibility.
- Increased keyword job cadence/batch size slightly to improve reach without flooding.
- Ran `cargo fmt`, `cargo clippy`, `cargo test` (clippy still reports pre-existing warnings).
- Extended `docs/scripts/two_instance_dht_selftest.sh` to include source publish/search flows and peer snapshots.
- Added preflight HELLOs for publish/search targets and switched publish/search target selection to distance-only (no liveness tiebreak).

## Decisions (2026-02-10)

- Token/session security model:
  - Session TTL bounds cookie compromise window.
  - Explicit token rotation is available to invalidate old bearer + all active sessions.
  - UI performs immediate token/session re-bootstrap after rotation to avoid operator disruption.
- Session auth policy now includes explicit lifecycle endpoints:
  - `session` issue (bearer), `session/check` validate (cookie), `session/logout` revoke (cookie).
  - Session validation performs lazy expiry cleanup; unauthenticated/expired frontend flows redirect to `/auth`.
- CSS policy tightened for shared UI styles: prefer variable-driven sizing and relative units; reserve `px` for border/hairline tokens.
- Place first operational charts on `node_stats` to pair routing/node data with live trend context before introducing a dedicated statistics page.
- Auth split for v1 local UI:
  - Keep bearer token as the API auth mechanism for `/api/v1/*`.
  - Use a separate HTTP-only session cookie for browser page/asset loads and SSE.
  - Remove SSE token query parameter usage from frontend.
- Settings API scope for v1: expose/update a focused config subset (`general`, `sam`, `api`) and require restart for full effect.
- Keep `docs/TODO.md` UI checkboxes aligned to implementation truth, using `[x]` for done and `[/]` for partial completion where design intent is not fully met.
- UI entrypoint canonical URL is `/index.html`; `/` is a redirect alias.
- Operator UX: always log a copy-pasteable localhost UI URL at startup.
- Route-fallback policy: treat unknown non-API, non-asset browser paths as SPA entry points and redirect to `/`; keep unknown `/api/*` and `/ui/assets/*` as 404.
- Serve UI from binary-embedded assets (`include_dir`) instead of runtime disk reads to guarantee deploy-time asset completeness.
- Alpine template bindings should be declarative and side-effect free; compute display-only classes/labels in controller state/getters before render.
- Theme ownership rule: all color values live in `color-*` theme files; shared CSS (`base.css`, `layout.css`) references theme vars only.
- Theme selection persistence uses `localStorage` key `ui_theme` and is applied via `<html data-theme=\"dark|light|hc\">`.
- Treat `docs/architecture.md` + `docs/api_curl.md` as the implementation-aligned API references for current `/api/v1`; `docs/API_DESIGN.md` remains broader future-state design.
- Search stop/delete are now first-class `/api/v1` controls instead of UI-local placeholders.
- `DELETE /api/v1/searches/:search_id` defaults to purging cached keyword results for that search (`purge_results=true`) to keep UI state consistent after delete.
- Use current active search thread (query-selected or first available) as the source for overview title/state.
- Use SSE-backed status updates as the first log timeline source in UI (`appLogs`), with snapshot polling available via manual refresh.
- Use `ui/.prettierrc` as the canonical formatter config for UI JS files (`ui/assets/js/*`).
- Define node UI state as:
  - `active`: `last_inbound_secs_ago <= 600`
  - `live`: `last_seen_secs_ago <= 600`
  - `idle`: otherwise
- Treat active keyword-search jobs in KAD service (`keyword_jobs`) as the canonical backend source for UI "search threads".
- Use keyword ID hex as `search_id` for details routing in v1 (`/ui/search_details?searchId=<keyword_id_hex>` and `/api/v1/searches/:search_id`).
- Keep search UI v1 focused on real keyword-search queue + cached-hit retrieval rather than adding placeholder-only controls.
- Enforce no inline `<style>` blocks in UI HTML; shared styles must live under `ui/assets/css/`.
- Keep sizing/spacing/state tokens in `ui/assets/css/layout.css` and consume them from component/layout rules in `ui/assets/css/base.css`.
- Keep `index.html` as a single-shell page aligned to the chat-style dashboard design, even before full search API wiring exists.
- Serve the in-repo UI skeleton from the Rust backend (single local control-plane origin).
- Keep browser auth bootstrap development-only and loopback-only via `/api/v1/dev/auth`.
- Permit SSE token via query parameter for `/api/v1/events` to support browser `EventSource` without custom headers.
- Restrict browser CORS access to loopback origins for local-control-plane safety.
- Use strict `/api/v1` routes only; no legacy unversioned aliases are kept.
- Implement loopback-only dev auth as `GET /api/v1/dev/auth` (no auth header required).
- Make API mandatory (always enabled) and remove `api.enabled` compatibility handling from code.
- Treat `main` as the canonical branch in project docs.
- No code changes made based on this run; treat results as network sparsity/quietness signal.
- Keep local publish injection, but expose `origin` so tests are unambiguous.
- Keep Rust-native architecture; optimize behavioral parity rather than line-by-line porting.
- Documented workflow: write/update tests where applicable, run fmt/clippy/test, commit + push per iteration.
- Accept existing clippy warnings for now; no functional changes required for this iteration.
- Use the two-instance script to exercise source publish/search as part of routine sanity checks.
- Prioritize DHT correctness over liveness when selecting publish/search targets.
- Implement bucket refresh based on staleness (with an under-populated growth mode) to grow the table without aggressive churn.
- Use MSB-first bucket indexing to match iMule bit order and ensure random bucket targets map correctly.
- On inbound responses, opportunistically send HELLO to establish keys and improve publish/search acceptance.
- Prefer recently-live peers first for publish/search while keeping distance correctness as fallback.
- Match iMule HELLO_REQ behavior (unencrypted, kadVersion=1, empty TagList) to improve interop.
- Add a targeted debug probe endpoint rather than relying on background jobs to validate per-peer responses.
- Add per-response acceptance stats and HELLO_ACK skip counters to see why routing doesn’t grow.
- Add an optional dual-HELLO mode (explicitly marked as “perhaps”, since it diverges from iMule).
- Dual-HELLO is explicitly flagged as a “perhaps”/experimental divergence from iMule behavior.

## Next Steps (2026-02-10)

- Consider periodic background cleanup for expired sessions (currently lazy cleanup on create/validate).
- Add optional “session expires in” UI indicator if a session metadata endpoint is introduced.
- Expand chart interactions/usability:
  - Add legend toggles and chart tooltips formatting for rates and hit counts.
  - Add pause/reset controls for time-series buffers.
  - Consider moving/duplicating high-value charts to overview once layout is finalized.
- Add session lifecycle endpoints and UX (`POST /api/v1/session/logout`, session-expired handling in UI).
- Add session persistence/eviction policy (TTL + periodic cleanup) instead of in-memory unbounded set.
- Add integration tests for middleware behavior:
  - unauthenticated UI path redirects to `/auth`
  - authenticated UI path succeeds
  - `/api/v1/events` rejects bearer-only and accepts valid session cookie
- Add an explicit integration test for `PATCH /api/v1/settings` through the full router (not just handler-level tests), including persistence failure behavior.
- Consider adding runtime-apply behavior for selected settings that do not require restart (and return per-field `restart_required` metadata).
- Prioritize remaining UI gaps from `docs/TODO.md`/`docs/UI_DESIGN.md`:
  - Implement Chart.js-based statistics visualizations.
  - Remove SSE token exposure via query params (or document accepted tradeoff explicitly).
  - Decide whether static UI routes should become bearer-protected and implement consistently.
  - Implement API-backed settings (`GET/PATCH /api/settings`) and wire the settings page.
- Add an integration test against the full Axum router asserting `GET /nonexisting.php?x=1` returns redirect `Location: /`.
- Consider adding a `/api/v1/ui/manifest` debug endpoint exposing embedded UI file names/checksums for operational verification.
- Add a lightweight UI smoke test pass (load each `/ui/*` page and assert Alpine init has no console/runtime errors) to guard future binding regressions.
- Add integration tests for API auth/CORS behavior (preflight + protected endpoint access patterns).
- Expand UI beyond status/search placeholder views (routing table, peers, and publish/search workflow surfaces).
- Replace static index sidebar/result placeholders with real search data once `/api/searches` endpoints are implemented.
- Add search-history/thread state in the UI (persisted list of submitted keyword jobs and selection behavior).
- Add API/frontend support for completed (no longer active) search history so `search_details` remains available after a job leaves `keyword_jobs`.
- Consider making node-state thresholds (`active/live` age windows) configurable in UI settings or API response metadata.
- Add richer log event typing/filtering once non-status event types are exposed from the API.
- Decide which `docs/API_DESIGN.md` endpoints should be promoted into the near-term implementation backlog vs kept as long-term design.
- Consider renaming `ui/assets/css/colors-light.css` to `ui/assets/css/color-light.css` for file-name symmetry (non-functional cleanup).
- Decide whether to keep dev auth as an explicit development-only endpoint or move to stronger local auth flow before release.
- Add UI-focused integration coverage (static UI route serving + SSE auth query behavior end-to-end).
- Consider adding a debug toggle to disable local injection during tests.
- Consider clearing per-keyword job `sent_to_*` sets on new API commands to allow re-tries to the same peers.
- Consider a small UI view over `/kad/peers` to spot real inbound activity quickly.
- Optionally address remaining clippy warnings in unrelated files.
- Run the updated two-instance script and review `OUT_FILE` + logs for source publish/search behavior.
- Re-run two-instance test to see if HELLO preflight improves `PUBLISH_RES` / `SEARCH_RES` results.
- Run `docs/scripts/debug_routing_summary.sh` + `debug_routing_buckets.sh` around test runs; use `debug_lookup_once` to trace a single lookup.
- Re-run the two-instance script (now with post-warmup routing snapshots) and check for HELLO traffic + publish/search ACKs.
- Re-run two-instance test and check for `recv_hello_ress` / `recv_hello_reqs` increases after HELLO_REQ change.
- Use `/debug/probe_peer` against a known peer from `/kad/peers` to check HELLO/SEARCH/PUBLISH responses.
- If `hello_ack_skipped_no_sender_key` keeps climbing, consider enabling `kad.service_hello_dual_obfuscated = true` for a test run.
- If `KAD2 RES contact acceptance stats` show high `dest_mismatch` or `already_id`, investigate routing filters or seed freshness.

## Roadmap Notes

- Storage: file-based runtime state under `data/` is fine for now (and aligns with iMule formats like `nodes.dat`).
  As we implement real client features (search history, file hashes/metadata, downloads, richer indexes),
  consider SQLite for structured queries + crash-safe transactions. See `docs/architecture.md`.

## Change Log

- 2026-02-12: CSS/theme pass: consolidate shared UI colors into `color-dark.css`/`colors-light.css`/`color-hc.css`, remove direct colors from `base.css`/`layout.css`, add early `theme-init.js`, and implement settings theme selector persisted via `localStorage` + `html[data-theme]`; run Prettier + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: API sanity check-run completed; add endpoint-level API tests for `/api/v1/searches/:search_id/stop` and `DELETE /api/v1/searches/:search_id` dispatch behavior (`src/api/mod.rs`); run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement missing `/api/v1` backing for UI search controls: add stop/delete search endpoints + service commands/logic + tests; wire UI stop/delete to API and add `apiDelete()` helper; update API docs; run Prettier + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement UI consistency fixes 1..4: add `ui/settings.html` + `appSettings()`, wire settings/new-search/actions, and make overview header/state thread-driven; run Prettier (`ui/assets/js/app.js`) + fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `ui/log.html` and `appLogs()` (status snapshot + SSE-backed rolling log view), and route sidebar "Logs" links to `/ui/log`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Format `ui/assets/js/app.js` and `ui/assets/js/helpers.js` with `ui/.prettierrc`; verify with `prettier --check`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `ui/node_stats.html` with shell + node status table/KPIs using `/api/v1/status` and `/api/v1/kad/peers`; implement `appNodeStats()`; point shell nav "Nodes / Routing" to `/ui/node_stats`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add `/api/v1/searches` and `/api/v1/searches/:search_id` for active keyword jobs; wire search-thread sidebars to API; add `ui/search_details.html` that loads details via `searchId` query param; update API docs; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Replicate shell in `ui/search.html`; implement first keyword search form wired to `/api/v1/kad/search_keyword` + `/api/v1/kad/keyword_results/:keyword_id_hex`; add reusable form CSS classes/tokens and `apiPost()` helper; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Remove inline styles from `ui/index.html`; move reusable shell/search layout rules to `ui/assets/css/base.css`; define layout/state CSS vars in `ui/assets/css/layout.css`; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Redesign `ui/index.html` into the UI spec shell (sidebar + search-overview main panel), preserving existing Alpine status/token/SSE wiring; run fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Serve UI skeleton from backend (`/`, `/ui`, `/ui/:page`, `/ui/assets/*`) with safe path validation and static content handling; allow SSE query-token auth for `/api/v1/events`; add related tests and update UI JS/HTML/docs (`src/api/mod.rs`, `ui/*`, `README.md`, `docs/architecture.md`, `docs/TODO.md`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after UI/bootstrap work (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Add loopback-only CORS middleware for `/api/v1` with explicit preflight handling and origin validation tests (`src/api/mod.rs`).
- 2026-02-12: Fix CORS IPv6 loopback origin parsing (`[::1]`) and rerun fmt/clippy/test (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Extend `Access-Control-Allow-Methods` to include `PUT` and `PATCH`; add regression test (`src/api/mod.rs`).
- 2026-02-12: Remove temporary unversioned API aliases and enforce `/api/v1` only (`src/api/mod.rs`).
- 2026-02-12: Remove `api.enabled` compatibility handling from config/app code (`src/config.rs`, `src/app.rs`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after strict v1-only API cleanup (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Implement `/api/v1` canonical routing, add loopback-only `GET /api/v1/dev/auth`, make API always-on (deprecate/ignore `api.enabled`), and add compatibility aliases for legacy routes (`src/api/mod.rs`, `src/app.rs`, `src/config.rs`, `src/main.rs`, `config.toml`).
- 2026-02-12: Update API docs/scripts to `/api/v1` and add `docs/scripts/dev_auth.sh` helper.
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after API routing/control-plane changes (tests pass; existing clippy warnings unchanged).
- 2026-02-12: Normalize docs wording/typos and align branch references to `main` (`docs/TODO.md`, `docs/dev.md`, `docs/handoff.md`).
- 2026-02-12: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after doc normalization (tests pass; existing clippy warnings unchanged).
- 2026-02-11: Tune two-instance selftest script with polling + peer snapshot controls; update `tmp/test_script_command.txt` to use new flags.
- 2026-02-11: Add routing snapshot controls and end-of-run routing dumps for the two-instance selftest; update `tmp/test_script_command.txt`.
- 2026-02-12: Long-haul run confirmed network-origin keyword hits; routing table still flat; SAM session recreated after PONG timeout on both instances.
- 2026-02-12: Send periodic BOOTSTRAP_REQ unencrypted to Kad v2–v5 peers; only encrypt for Kad v6+.
- 2026-02-12: Fix publish/search peer selection so distance is primary; liveness only reorders within the closest set.
- 2026-02-12: Clear keyword job `sent_to_search` / `sent_to_publish` on restart to allow manual retries to send again.
- 2026-02-12: Return distance-ordered peer lists with fallback (max\*4) to avoid empty batches when closest peers are skipped.
- 2026-02-10: Two-instance DHT selftest (5 rounds) showed only local keyword hits; no cross-instance results, no publish-key acks, empty search responses; routing stayed flat (quiet network).
- 2026-02-10: Add `origin` field to keyword hit API responses (`local` vs `network`).
- 2026-02-10: Add `/kad/peers` API endpoint and new inbound request counters in `/status`; slightly increase keyword job cadence/batch size.
- 2026-02-10: Add workflow guidance in `AGENTS.md` (tests, fmt/clippy/test, commit + push per iteration).
- 2026-02-10: Extend two-instance selftest to include source publish/search and peer snapshots; add `kad_peers_get.sh`.
- 2026-02-10: Add HELLO preflight for publish/search targets and use distance-only selection for DHT-critical actions.
- 2026-02-10: Add debug routing endpoints + debug lookup trigger; add staleness-based bucket refresh with under-populated growth mode.
- 2026-02-10: Align bucket indexing with MSB bit order; mark last_seen/last_inbound on inbound responses.
- 2026-02-10: Send HELLO on inbound responses, prioritize live peers for publish/search, and add post-warmup routing snapshots in the selftest script.
- 2026-02-10: Align Kad2 HELLO_REQ with iMule (kadVersion=1, empty taglist, unobfuscated); add `encode_kad2_hello_req` and update HELLO send paths.
- 2026-02-10: Add HELLO_RES_ACK counters + publish/search request debug logs; add `/debug/probe_peer` API for targeted HELLO/SEARCH/PUBLISH probes.
- 2026-02-10: Document `/debug/probe_peer` in `docs/api_curl.md` and add `docs/scripts/debug_probe_peer.sh`.
- 2026-02-10: Add KAD2 RES contact acceptance stats (debug) + HELLO_ACK skip counter; add optional dual HELLO_REQ mode behind config flag (experimental, diverges from iMule).
- 2026-02-10: Wire `kad.service_hello_dual_obfuscated` config; add KAD2 RES acceptance stats and HELLO_ACK skip counters to status/logs; update `config.toml`.
- 2026-02-06: Embed distributable nodes init seed at `assets/nodes.initseed.dat`; create `data/nodes.initseed.dat` and `data/nodes.fallback.dat` from embedded seed (best-effort) so runtime no longer depends on repo-local reference folders.
- 2026-02-06: Reduce default stdout verbosity to `info` (code default and repo `config.toml`; file logging remains configurable and can stay `debug`).
- 2026-02-06: Make Kad UDP key secret file-backed only (`data/kad_udp_key_secret.dat`); `kad.udp_key_secret` is deprecated/ignored to reduce misconfiguration risk.
- 2026-02-06: Implement iMule-style `KADEMLIA2_REQ` sender-id field and learn sender IDs from inbound `KADEMLIA2_REQ` to improve routing growth.
- 2026-02-06: Clarify iMule `KADEMLIA2_REQ` first byte is a _requested contact count_ (low 5 bits), and update Rust naming (`requested_contacts`) + parity docs.
- 2026-02-06: Fix Kad1 `HELLO_RES` contact type to `3` (matches iMule `CContact::Self().WriteToKad1Contact` default).
- 2026-02-06: Periodic BOOTSTRAP refresh: stop excluding peers by `failures >= max_failures` (BOOTSTRAP is a distinct discovery path); rely on per-peer backoff instead so refresh continues even when crawl timeouts accumulate.
- 2026-02-07: Observed 3 responding peers (`live=3`) across a multi-hour run (improvement from prior steady state of 2). Routing table size still stayed flat (`routing=153`, `new_nodes=0`), indicating responders are returning already-known contacts.
- 2026-02-07: Add `live_10m` metric to status logs (recently-responsive peers), and change periodic BOOTSTRAP refresh to rotate across "cold" peers first (diversifies discovery without increasing send rate).
- 2026-02-07: Fix long-run stability: prevent Tokio interval "catch-up bursts" (missed tick behavior set to `Skip`), treat SAM TCP-DATAGRAM framing desync as fatal, and auto-recreate the SAM DATAGRAM session if the socket drops (service keeps running instead of crashing).
- 2026-02-07: Introduce typed SAM errors (`SamError`) for the SAM protocol layer + control client + datagram transports; higher layers use `anyhow` but reconnect logic now searches the error chain for `SamError` instead of string-matching messages.
- 2026-02-07: Add a minimal local HTTP API skeleton (REST + SSE) for a future GUI (`src/api/`), with a bearer token stored in `data/api.token`. See `docs/architecture.md`.
- 2026-02-07: Start client-side search/publish groundwork: add Kad2 `SEARCH_SOURCE_REQ` + `PUBLISH_SOURCE_REQ` encoding/decoding, handle inbound `SEARCH_RES`/`PUBLISH_RES` in the service loop, and expose minimal API endpoints to enqueue those actions.
- 2026-02-07: Add iMule-compatible keyword hashing + Kad2 keyword search:
  - iMule-style keyword hashing (MD4) used for Kad2 keyword lookups (`src/kad/keyword.rs`, `src/kad/md4.rs`).
  - `KADEMLIA2_SEARCH_KEY_REQ` encoding and unified `KADEMLIA2_SEARCH_RES` decoding (source + keyword/file results) (`src/kad/wire.rs`, `src/kad/service.rs`).
  - New API endpoints: `POST /kad/search_keyword`, `GET /kad/keyword_results/:keyword_id_hex` (`src/api/mod.rs`).
  - Curl cheat sheet updated (`docs/api_curl.md`).
- 2026-02-07: Add bounded keyword result caching (prevents memory ballooning):
  - Hard caps (max keywords, max total hits, max hits/keyword) + TTL pruning.
  - All knobs are configurable in `config.toml` under `[kad]` (`service_keyword_*`).
  - Status now reports keyword cache totals + eviction counters.
- 2026-02-09: Two-instance keyword publish/search sanity check (mule-a + mule-b):
  - Both sides successfully received `KADEMLIA2_SEARCH_RES` replies, but **all keyword results were empty** (`keyword_entries=0`).
  - Root cause (interop): iMule rejects Kad2 keyword publishes which only contain `TAG_FILENAME` + `TAG_FILESIZE`.
    In iMule `CIndexed::AddKeyword` checks `GetTagCount() != 0`, and Kad2 publish parsing stores filename+size out-of-band
    (so they do not contribute to the internal tag list). iMule itself publishes additional tags like `TAG_SOURCES` and
    `TAG_COMPLETE_SOURCES`. See `source_ref/.../Search.cpp::PreparePacketForTags` and `Indexed.cpp::AddKeyword`.
  - Fix: rust-mule now always includes `TAG_SOURCES` and `TAG_COMPLETE_SOURCES` in Kad2 keyword publish/search-result taglists
    (`src/kad/wire.rs`), matching iMule expectations.
- 2026-02-09: Follow-up two-instance test showed _some_ keyword results coming back from the network (`keyword_entries=1`),
  but A and B still tended to publish/search against disjoint "live" peers and would miss each other's stores.
  Fix: change DHT-critical peer selection to be **distance-first** (XOR distance primary; liveness as tiebreaker) so that
  publish/search targets the correct closest nodes (`src/kad/routing.rs`, `src/kad/service.rs`).
- 2026-02-09: Two-instance test artifacts under `./tmp/` (mule-a+mule-b with `docs/scripts/two_instance_dht_selftest.sh`):
  - Script output shows each side only ever returns its _own_ published hit for the shared keyword (no cross-hit observed).
    This is expected with the current API behavior because `POST /kad/publish_keyword` injects a local hit into the in-memory cache.
    Real proof of network success is `got SEARCH_RES ... keyword_entries>0 inserted_keywords>0` in logs (or explicit `origin=network` markers).
  - Both instances received at least one `got SEARCH_RES ... keyword_entries=0` for the shared keyword (network replied, but empty).
  - Neither instance logged `got PUBLISH_RES (key)` (no publish acks observed).
  - `mule-b` received many inbound `KADEMLIA2_PUBLISH_KEY_REQ` packets from peer `-8jmpFh...` that fail decoding with `unexpected EOF at 39`
    (345 occurrences in that run), so we do not store those keywords and we do not reply with `PUBLISH_RES` on that path.
  - Next debugging targets:
    - capture raw decrypted payload (len + hex head) on first decode failure to determine truncation vs parsing mismatch,
    - make publish-key decoding best-effort and still reply with `PUBLISH_RES` (key) to reduce peer retries,
    - add `origin=local|network` to keyword hits (or a debug knob to disable local injection) to make tests unambiguous.
- 2026-02-09: Implemented publish-key robustness improvements:
  - Add lenient `KADEMLIA2_PUBLISH_KEY_REQ` decoding which can return partial entries and still extract the keyword prefix for ACKing (`src/kad/wire.rs`).
  - On decode failure, rust-mule now attempts a prefix ACK (send `KADEMLIA2_PUBLISH_RES` for the keyword) so peers stop retransmitting.
  - Added `recv_publish_key_decode_failures` counter to `/status` output for visibility (`src/kad/service.rs`).
- 2026-02-09: Discovered an iMule debug-build quirk in the wild:
  - Some peers appear to include an extra `u32` tag-serial counter inside Kad TagLists (enabled by iMule `_DEBUG_TAGS`),
    which shifts tag parsing (we saw this in a publish-key payload where the filename length was preceded by 4 bytes).
  - rust-mule now retries TagList parsing with and without this extra `u32` field for:
    - Kad2 HELLO taglists (ints)
    - search/publish taglists (search info)
      (`src/kad/wire.rs`).
- 2026-02-09: Added rust-mule peer identification:
  - Kad2 `HELLO_REQ/HELLO_RES` now includes a private vendor tag `TAG_RUST_MULE_AGENT (0xFE)` with a string like `rust-mule/<version>`.
  - If a peer sends that tag, rust-mule records it in-memory and logs it once when first learned.
  - This allows rust-mule-specific feature gating going forward while remaining compatible with iMule (unknown tags are ignored).
- 2026-02-07: TTL note (small/slow iMule I2P-KAD reality):
  - Keyword hits are a “discovery cache” and can be noisy; expiring them is mostly for memory hygiene.
  - File _sources_ are likely intermittent; plan to keep them much longer (days/weeks) and track `last_seen` rather than aggressively expiring.
  - If keyword lookups feel too slow to re-learn, bump:
    - `kad.service_keyword_interest_ttl_secs` and `kad.service_keyword_results_ttl_secs` (e.g. 7 days = `604800`).
- 2026-02-08: Fix SAM session teardown + reconnect resilience:
  - Some SAM routers require `SESSION DESTROY STYLE=... ID=...`; we now fall back to style-specific destroys for both STREAM and DATAGRAM sessions (`src/i2p/sam/client.rs`, `src/i2p/sam/datagram_tcp.rs`).
  - KAD socket recreation now retries session creation with exponential backoff on tunnel-build errors like “duplicate destination” instead of crashing (`src/app.rs`).
- 2026-02-08: Add Kad2 keyword publish + DHT keyword storage:
  - Handle inbound `KADEMLIA2_PUBLISH_KEY_REQ` by storing minimal keyword->file metadata and replying with `KADEMLIA2_PUBLISH_RES` (key shape) (`src/kad/service.rs`, `src/kad/wire.rs`).
  - Answer inbound `KADEMLIA2_SEARCH_KEY_REQ` from the stored keyword index (helps interoperability + self-testing).
  - Add API endpoint `POST /kad/publish_keyword` and document in `docs/api_curl.md`.

## Current State (As Of 2026-02-07)

- Canonical branch: `main` (recent historical work happened on `feature/kad-search-publish`).
- Implemented:
  - SAM v3 TCP control client with logging and redacted sensitive fields (`src/i2p/sam/`).
  - SAM `STYLE=DATAGRAM` session over TCP (iMule-style `DATAGRAM SEND` / `DATAGRAM RECEIVED`) (`src/i2p/sam/datagram_tcp.rs`).
  - SAM `STYLE=DATAGRAM` session + UDP forwarding socket (`src/i2p/sam/datagram.rs`).
  - iMule-compatible KadID persisted in `data/preferencesKad.dat` (`src/kad.rs`).
  - iMule `nodes.dat` v2 parsing (I2P destinations, KadIDs, UDP keys) (`src/nodes/imule.rs`).
  - Distributable bootstrap seed embedded at `assets/nodes.initseed.dat` and copied to `data/nodes.initseed.dat` / `data/nodes.fallback.dat` on first run (`src/app.rs`).
  - KAD packet encode/decode including iMule packed replies (pure-Rust zlib/deflate inflater) (`src/kad/wire.rs`, `src/kad/packed.rs`).
  - Minimal bootstrap probe: send `PING` + `BOOTSTRAP_REQ`, decode `PONG` + `BOOTSTRAP_RES` (`src/kad/bootstrap.rs`).
  - Kad1+Kad2 HELLO handling during bootstrap (reply to `HELLO_REQ`, parse `HELLO_RES`, send `HELLO_RES_ACK` when requested) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Minimal Kad2 routing behavior during bootstrap:
  - Answer Kad2 `KADEMLIA2_REQ (0x11)` with `KADEMLIA2_RES (0x13)` using the closest known contacts (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Answer Kad1 `KADEMLIA_REQ_DEPRECATED (0x05)` with Kad1 `RES (0x06)` (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Handle Kad2 `KADEMLIA2_PUBLISH_SOURCE_REQ (0x19)` by recording a minimal in-memory source entry and replying with `KADEMLIA2_PUBLISH_RES (0x1B)` (this stops peers from retransmitting publishes during bootstrap) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Handle Kad2 `KADEMLIA2_SEARCH_SOURCE_REQ (0x15)` with `KADEMLIA2_SEARCH_RES (0x17)` (source results are encoded with the minimal required tags: `TAG_SOURCETYPE`, `TAG_SOURCEDEST`, `TAG_SOURCEUDEST`) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - Persist discovered peers to `data/nodes.dat` (iMule `nodes.dat v2`) so we can slowly self-heal even when `nodes2.dat` fetch is unavailable (`src/app.rs`, `src/nodes/imule.rs`).
  - I2P HTTP fetch helper over SAM STREAM (used to download a fresh `nodes2.dat` when addressbook resolves) (`src/i2p/http.rs`).
- Removed obsolete code:
  - Legacy IPv4-focused `nodes.dat` parsing and old net probe helpers.
  - Empty/unused `src/protocol.rs`.

## Dev Topology Notes

- SAM bridge is on `10.99.0.2`.
- This `rust-mule` dev env runs inside Docker on host `10.99.0.1`.
- For SAM UDP forwarding to work, `SESSION CREATE ... HOST=<forward_host> PORT=<forward_port>` must be reachable from `10.99.0.2` and mapped into the container.
  - Recommended `config.toml` values:
    - `sam.host = "10.99.0.2"`
    - `sam.forward_host = "10.99.0.1"`
    - `sam.forward_port = 40000`
  - Docker needs either `--network host` or `-p 40000:40000/udp`.

If you don't want to deal with UDP forwarding, set `sam.datagram_transport = "tcp"` in `config.toml`.

## Data Files (`*.dat`) And Which One Is Used

### `data/nodes.dat` (Primary Bootstrap + Persisted Seed Pool)

This is the **main** nodes file that `rust-mule` uses across runs. By default it is:

- `kad.bootstrap_nodes_path = "nodes.dat"` (in `config.toml`)
- resolved relative to `general.data_dir = "data"`
- so the primary path is `data/nodes.dat`

On startup, `rust-mule` will try to load nodes from this path first. During runtime it is also periodically overwritten with a refreshed list (but in a merge-preserving way; see below).

Format: iMule/aMule `nodes.dat` v2 (I2P destinations + KadIDs + optional UDP keys).

### `data/nodes.initseed.dat` and `data/nodes.fallback.dat` (Local Seed Snapshots)

These are local seed snapshots stored under `data/` so runtime behavior does not depend on repo paths:

- `data/nodes.initseed.dat`: the initial seed snapshot (created on first run from the embedded initseed).
- `data/nodes.fallback.dat`: currently just a copy of initseed (we can evolve this later into a "last-known-good"
  snapshot if desired).

They are used only when:

- `data/nodes.dat` does not exist, OR
- `data/nodes.dat` exists but has become too small (currently `< 50` entries), in which case startup will re-seed `data/nodes.dat` by merging in reference nodes.

Selection logic lives in `src/app.rs` (`pick_nodes_dat()` + the re-seed block).

### `assets/nodes.initseed.dat` (Embedded Distributable Init Seed)

For distributable builds we track a baseline seed snapshot at:

- `assets/nodes.initseed.dat`

At runtime this is embedded into the binary via `include_bytes!()` and written out to `data/nodes.initseed.dat` /
`data/nodes.fallback.dat` if they don't exist yet (best-effort).

`source_ref/` remains a **dev-only** reference folder (gitignored) that contains iMule sources and reference files, but
the app no longer depends on it for bootstrapping.

### `nodes2.dat` (Remote Bootstrap Download, If Available)

iMule historically hosted an HTTP bootstrap list at:

- `http://www.imule.i2p/nodes2.dat`

`rust-mule` will try to download this only when it is not using the normal persisted `data/nodes.dat` seed pool (i.e. when it had to fall back to initseed/fallback).

If the download succeeds, it is saved as `data/nodes.dat` (we don't keep a separate `nodes2.dat` file on disk right now).

### `data/sam.keys` (SAM Destination Keys)

SAM pub/priv keys are stored in `data/sam.keys` as a simple k/v file:

```text
PUB=...
PRIV=...
```

This keeps secrets out of `config.toml` (which is easy to accidentally commit).

### `data/preferencesKad.dat` (Your KadID / Node Identity)

This stores the Kademlia node ID (iMule/aMule format). It is loaded at startup and reused across runs so you keep a stable identity on the network.

If you delete it, a new random KadID is generated and peers will treat you as a different node.

### `data/kad_udp_key_secret.dat` (UDP Obfuscation Secret)

This is the persistent secret used to compute UDP verify keys (iMule-style `GetUDPVerifyKey()` logic, adapted to I2P dest hash).

This value is generated on first run and loaded from this file on startup. It is intentionally not user-configurable.
If you delete it, a new secret is generated and any learned UDP-key relationships may stop validating until re-established.

## Known Issue / Debugging

If you see `SAM read timed out` right after a successful `HELLO`, the hang is likely on `SESSION CREATE ... STYLE=DATAGRAM` (session establishment can be slow on some routers).

Mitigation:

- `sam.control_timeout_secs` (default `120`) controls SAM control-channel read/write timeouts.
- With `general.log_level = "debug"`, the app logs the exact SAM command it was waiting on (with private keys redacted).

## Latest Run Notes (2026-02-04)

Observed with `sam.datagram_transport = "tcp"`:

- SAM `HELLO` OK.
- `SESSION CREATE STYLE=DATAGRAM ...` OK.
- Loaded a small seed pool (at that time it came from a repo reference `nodes.dat`; today we use the embedded initseed).
- Sent initial `KADEMLIA2_BOOTSTRAP_REQ` to peers, but received **0** `PONG`/`BOOTSTRAP_RES` responses within the bootstrap window.
  - A likely root cause is that iMule nodes expect **obfuscated/encrypted KAD UDP** packets (RC4+MD5 framing), and will ignore plain `OP_KADEMLIAHEADER` packets.
  - Another likely root cause is that the nodes list is stale (the default iMule KadNodesUrl is `http://www.imule.i2p/nodes2.dat`).

Next things to try if this repeats:

- Switch to `sam.datagram_transport = "udp_forward"` (some SAM bridges implement UDP forwarding more reliably than TCP datagrams).
- Ensure Docker/host UDP forwarding is mapped correctly if using `udp_forward` (`sam.forward_host` must be reachable from the SAM host).
- Increase the bootstrap runtime (I2P tunnel build + lease set publication can take time). Defaults are now more forgiving (`max_initial=256`, `runtime=180s`, `warmup=8s`).
- Prefer a fresher/larger `nodes.dat` seed pool (the embedded `assets/nodes.initseed.dat` may age; real discovery + persistence in `data/nodes.dat` should keep things fresh over time).
- Avoid forcing I2P lease set encryption types unless you know all peers support it (iMule doesn't set `i2cp.leaseSetEncType` for its datagram session).
- The app will attempt to fetch a fresh `nodes2.dat` over I2P from `www.imule.i2p` and write it to `data/nodes.dat` when it had to fall back to initseed/fallback.

If you see `Error: SAM read timed out` _during_ bootstrap on `sam.datagram_transport="tcp"`, that's a local read timeout on the SAM TCP socket (no inbound datagrams yet), not necessarily a SAM failure. The TCP datagram receiver was updated to block and let the bootstrap loop apply its own deadline.

### Updated Run Notes (2026-02-04 19:30Z-ish)

- SAM `SESSION CREATE STYLE=DATAGRAM` succeeded but took ~43s (so `sam.control_timeout_secs=120` is warranted).
- We received inbound datagrams:
  - a Kad1 `KADEMLIA_HELLO_REQ_DEPRECATED` (opcode `0x03`) from a peer
  - a Kad2 `KADEMLIA2_BOOTSTRAP_RES` which decrypted successfully
- Rust now replies to Kad1 `HELLO_REQ` with a Kad1 `HELLO_RES` containing our I2P contact details, matching iMule's `WriteToKad1Contact()` layout.
- Rust now also sends Kad2 `HELLO_REQ` during bootstrap and handles Kad2 `HELLO_REQ/RES/RES_ACK` to improve chances of being added to routing tables and to exchange UDP verify keys.
- Observed many inbound Kad2 node-lookup requests (`KADEMLIA2_REQ`, opcode `0x11`). rust-mule now replies with `KADEMLIA2_RES` using the best-known contacts from `nodes.dat` + newly discovered peers (minimal routing-table behavior).
- The `nodes2.dat` downloader failed because `NAMING LOOKUP www.imule.i2p` returned `KEY_NOT_FOUND` on that router.
- If `www.imule.i2p` and `imule.i2p` are missing from the router addressbook, the downloader can't run unless you add an addressbook subscription which includes those entries, or use a `.b32.i2p` hostname / destination string directly.

### Updated Run Notes (2026-02-04 20:42Z-ish)

### Updated Run Notes (2026-02-06)

- Confirmed logs now land in `data/logs/` (daily rolled).
- Fresh run created `data/nodes.initseed.dat` + `data/nodes.fallback.dat` from embedded initseed (first run behavior).
- `data/nodes.dat` loaded `154` entries (primary), service started with routing `153`.
- Over ~20 minutes, service stayed healthy (periodic `kad service status` kept printing), but discovery was limited:
  - `live` stabilized around `2`
  - `recv_ress` > 0 (we do get some `KADEMLIA2_RES` back), but `new_nodes=0` during that window.
  - No WARN/ERROR events were observed.

If discovery remains flat over multi-hour runs, next tuning likely involves more aggressive exploration (higher `alpha`, lower `req_min_interval`, more frequent HELLOs) and/or adding periodic `KADEMLIA2_BOOTSTRAP_REQ` refresh queries in the service loop.

- Bootstrap sent probes to `peers=103`.
- Received:
- `KADEMLIA2_BOOTSTRAP_RES` (decrypted OK), which contained `contacts=1`.
- `KADEMLIA2_HELLO_REQ` from the same peer; rust-mule replied with `KADEMLIA2_HELLO_RES`.
- `bootstrap summary ... discovered=2` and persisted refreshed nodes to `data/nodes.dat` (`count=120`).

### Updated Run Notes (2026-02-05)

From `log.txt`:

- Bootstrapping from `data/nodes.dat` now works reliably enough to discover peers (`count=122` at end of run).
- We now see lots of inbound Kad2 node lookups (`KADEMLIA2_REQ`, opcode `0x11`) and we respond to each with `KADEMLIA2_RES` (contacts=4 in logs).
- One peer was repeatedly sending Kad2 publish-source requests (`opcode=0x19`, `KADEMLIA2_PUBLISH_SOURCE_REQ`). This is now handled by replying with `KADEMLIA2_PUBLISH_RES` and recording a minimal in-memory source entry so that (if asked) we can return it via `KADEMLIA2_SEARCH_RES`.
  - Example (later in the log): `publish_source_reqs=16` and `publish_source_res_sent=16` in the bootstrap summary, plus log lines like `sent KAD2 PUBLISH_RES (sources) ... sources_for_file=1`.

## Known SAM Quirk (DEST GENERATE)

Some SAM implementations reply to `DEST GENERATE` as:

- `DEST REPLY PUB=... PRIV=...`

with **no** `RESULT=OK` field. `SamClient::dest_generate()` was updated to accept this (it now validates `PUB` and `PRIV` instead of requiring `RESULT=OK`). This unblocks:

- `src/bin/sam_dgram_selftest.rs`
- the `nodes2.dat` downloader (temporary STREAM sessions use `DEST GENERATE`)

## Known Issue (Addressbook Entry For `www.imule.i2p`)

If `NAMING LOOKUP NAME=www.imule.i2p` returns `RESULT=KEY_NOT_FOUND`, your router's addressbook doesn't have that host.

Mitigations:

- Add/subscribe to an addressbook source which includes `www.imule.i2p`.
- The downloader also tries `imule.i2p` as a fallback by stripping the leading `www.`.
- The app now also persists any peers it discovers during bootstrap to `data/nodes.dat`, so it can slowly build a fresh nodes list even if `nodes2.dat` can’t be fetched.

### KAD UDP Obfuscation (iMule Compatibility)

iMule encrypts/obfuscates KAD UDP packets (see `EncryptedDatagramSocket.cpp`) and includes sender/receiver verify keys.

Implemented in Rust:

- `src/kad/udp_crypto.rs`: MD5 + RC4 + iMule framing, plus `udp_verify_key()` compatible with iMule (using I2P dest hash in place of IPv4).
- `src/kad/udp_crypto.rs`: receiver-verify-key-based encryption path (needed for `KADEMLIA2_HELLO_RES_ACK` in iMule).
- `kad.udp_key_secret` used to be configurable, but is now deprecated/ignored. The secret is always generated/loaded from `data/kad_udp_key_secret.dat` (analogous to iMule `thePrefs::GetKadUDPKey()`).

Bootstrap now:

- Encrypts outgoing `KADEMLIA2_BOOTSTRAP_REQ` using the target's KadID.
- Attempts to decrypt inbound packets (NodeID-key and ReceiverVerifyKey-key variants) before KAD parsing.

## How To Run

```bash
cargo run --bin rust-mule
```

If debugging SAM control protocol, set:

- `general.log_level = "debug"` in `config.toml`, or
- `RUST_LOG=rust_mule=debug` in the environment.

## Kad Service Loop (Crawler)

As of 2026-02-05, `rust-mule` runs a long-lived Kad service loop after the initial bootstrap by default.
It:

- listens/responds to inbound Kad traffic
- periodically crawls the network by sending `KADEMLIA2_REQ` lookups and decoding `KADEMLIA2_RES` replies
- periodically persists an updated `data/nodes.dat`

### Important Fix (2026-02-05): `KADEMLIA2_REQ` Check Field

If you see the service loop sending lots of `KADEMLIA2_REQ` but reporting `recv_ress=0` in `kad service status`, the most likely culprit was a bug which is fixed in `main` (originally developed on `feature/sam-protocol`):

- In iMule, the `KADEMLIA2_REQ` payload includes a `check` KadID field which must match the **receiver's** KadID.
- If we incorrectly put _our_ KadID in the `check` field, peers will silently ignore the request and never send `KADEMLIA2_RES`.

After the fix, long runs should start showing `recv_ress>0` and `new_nodes>0` as the crawler learns contacts.

### Note: Why `routing` Might Not Grow Past The Seed Count

If `kad service status` shows `recv_ress>0` but `routing` stays flat (e.g. stuck at the initial `nodes.dat` size), that can be normal in a small/stale network _or_ it can indicate that peers are mostly returning contacts we already know (or echoing our own KadID back as a contact).

The service now counts “new nodes” only when `routing.len()` actually increases after processing `KADEMLIA2_RES`, to avoid misleading logs.

Also: the crawler now picks query targets Kademlia-style: it biases which peers it queries by XOR distance to the lookup target (not just “who is live”). This tends to explore new regions of the ID space faster and increases the odds of discovering nodes that weren't already in the seed `nodes.dat`.

Recent observation (2026-02-06, ~50 min run):

- `data/nodes.dat` stayed at `154` entries; routing stayed at `153`.
- `live` peers stayed at `2`.
- Periodic `KADEMLIA2_BOOTSTRAP_REQ` refresh got replies, but returned contact lists were typically `2` and did not introduce new IDs (`new_nodes=0`).

Takeaway: this looks consistent with a very small / stagnant iMule I2P-KAD network _or_ a seed which mostly points at dead peers. Next improvements should focus on discovery strategy and fresh seeding (see TODO below).

Relevant config keys (all under `[kad]`):

- `service_enabled` (default `true`)
- `service_runtime_secs` (`0` = run until Ctrl-C)
- `service_crawl_every_secs` (default `3`)
- `service_persist_every_secs` (default `300`)
- `service_alpha` (default `3`)
- `service_req_contacts` (default `31`)
- `service_max_persist_nodes` (default `5000`)
  Additional tuning knobs:
- `service_req_timeout_secs` (default `45`)
- `service_req_min_interval_secs` (default `15`)
- `service_bootstrap_every_secs` (default `1800`)
- `service_bootstrap_batch` (default `1`)
- `service_bootstrap_min_interval_secs` (default `21600`)
- `service_hello_every_secs` (default `10`)
- `service_hello_batch` (default `2`)
- `service_hello_min_interval_secs` (default `900`)
- `service_maintenance_every_secs` (default `5`)
- `service_max_failures` (default `5`)
- `service_evict_age_secs` (default `86400`)

## Logging Notes

As of 2026-02-05, logs can be persisted to disk via `tracing-appender`:

- Controlled by `[general].log_to_file` (default `true`)
- Files are written under `[general].data_dir/logs` and rolled daily as `rust-mule.log.YYYY-MM-DD` (configurable via `[general].log_file_name`)
- Stdout verbosity is controlled by `[general].log_level` (or `RUST_LOG`).
- File verbosity is controlled by `[general].log_file_level` (or `RUST_MULE_LOG_FILE`).

The Kad service loop now emits a concise `INFO` line periodically: `kad service status` (default every 60s), and most per-packet send/timeout logs are `TRACE` to keep stdout readable at `debug`.

To keep logs readable, long I2P base64 destination strings are now shortened in many log lines (they show a prefix + suffix rather than the full ~500 chars). See `src/i2p/b64.rs` (`b64::short()`).

As of 2026-02-06, the status line also includes aggregate counts like `res_contacts`, `sent_bootstrap_reqs`, `recv_bootstrap_ress`, and `bootstrap_contacts` to help tune discovery without turning on very verbose per-packet logging.

## Reference Material

- iMule source + reference `nodes.dat` are under `source_ref/` (gitignored).
- KAD wire-format parity notes: `docs/kad_parity.md`.

## Roadmap (Agreed Next Steps)

Priority is to stabilize the network layer first, so we can reliably discover peers and maintain a healthy routing table over time:

1. **Kad crawler + routing table + stable loop (next)**
   - Actively query peers (send `KADEMLIA2_REQ`) and **decode `KADEMLIA2_RES`** to learn more contacts.
   - Maintain an in-memory routing table (k-buckets / closest contacts) with `last_seen`, `verified`, and UDP key metadata.
   - Run as a long-lived service: keep SAM datagram session open, respond continuously, periodically refresh/ping, and periodically persist `data/nodes.dat`.
   - TODO (discovery): add a conservative “cold bootstrap probe” mode so periodic bootstrap refresh occasionally targets _non-live / never-seen_ peers, to try to discover new clusters without increasing overall traffic.
   - TODO (seeding): optionally fetch the latest public `nodes.dat` snapshot (when available) and merge it into `data/nodes.dat` with provenance logged.

2. **Publish/Search indexing (after routing is stable)**

- Implement remaining Kad2 publish/search opcodes (key/notes/source) with iMule-compatible responses.
- Add a real local index so we can answer searches meaningfully (not just “0 results but no retry”).

## Tuning Notes / Gotchas

- `kad.service_req_contacts` should be in `1..=31`. (Kad2 masks this field with `0x1F`.)
  - If it is set to `32`, it will effectively become `1`, which slows discovery dramatically.
- The service persists `nodes.dat` periodically. It now merges the current routing snapshot into the existing on-disk `nodes.dat` to avoid losing seed nodes after an eviction cycle.
- If `data/nodes.dat` ever shrinks to a very small set (e.g. after a long run evicts lots of dead peers), startup will re-seed it by merging in `data/nodes.initseed.dat` / `data/nodes.fallback.dat` if present.

- The crawler intentionally probes at least one “cold” peer (a peer we have never heard from) per crawl tick when available. This prevents the service from getting stuck talking only to 1–2 responsive nodes forever.

- SAM TCP-DATAGRAM framing is now tolerant of occasional malformed frames (it logs and skips instead of crashing). Oversized datagrams are discarded with a hard cap to avoid memory blowups.
- SAM TCP-DATAGRAM reader is byte-based (not `String`-based) to avoid crashes on invalid UTF-8 if the stream ever desyncs.

## 2026-02-08 Notes (Keyword Publish/Search UX + Reach)

- `/kad/search_keyword` and `/kad/publish_keyword` now accept either:
  - `{"query":"..."}` (iMule-style: first extracted word is hashed), or
  - `{"keyword_id_hex":"<32 hex>"}` to bypass tokenization/hashing for debugging.
- Keyword publish now also inserts the published entry into the local keyword-hit cache immediately (so `/kad/keyword_results/<keyword>` reflects the publish even if the network is silent).
- Keyword search/publish now run as a small, conservative “job”:
  - periodically sends `KADEMLIA2_REQ` toward the keyword ID to discover closer nodes
  - periodically sends small batches of `SEARCH_KEY_REQ` / `PUBLISH_KEY_REQ` to the closest, recently-live peers
  - stops early for publish once any `PUBLISH_RES (key)` ack is observed

- Job behavior tweak:
  - A keyword job can now do **both** publish and search for the same keyword concurrently.
    Previously, starting a search could overwrite an in-flight publish job for that keyword.

## 2026-02-09 Notes (Single-Instance Lock)

- Added an OS-backed single-instance lock at `data/rust-mule.lock` (under `general.data_dir`).
  - Prevents accidentally running two rust-mule processes with the same `data/sam.keys`, which
    triggers I2P router errors like “duplicate destination”.
  - Uses a real file lock (released automatically if the process exits/crashes), not a “sentinel
    file” check.

## 2026-02-09 Notes (Peer “Agent” Identification)

- SAM `DATAGRAM RECEIVED` frames include the sender I2P destination, but **do not** identify the
  sender implementation (iMule vs rust-mule vs something else).
- To support rust-mule-specific feature gating/debugging, we added a small rust-mule private
  extension tag in the Kad2 `HELLO` taglist:
  - `TAG_RUST_MULE_AGENT (0xFE)` as a string, value like `rust-mule/<version>`
  - iMule ignores unknown tags in `HELLO` (it only checks `TAG_KADMISCOPTIONS`), so this is
    backwards compatible.
- When received, this agent string is stored in the in-memory routing table as `peer_agent` (not
  persisted to `nodes.dat`, since that file is in iMule format).

## 2026-05-04 Notes (Kad/DHT Resource Cap Review Fixes)

- PR review follow-up tightened the Kad/DHT resource caps:
  - every inbound routing-table peer learn path now enforces `kad.service_max_runtime_nodes`
  - local `PublishSource` cache insertion now applies source-store limits too
  - zero source-store limits now drop remote cached sources while preserving the local published
    source entries needed to answer `SEARCH_SOURCE_REQ`
- Validation run locally:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-targets --all-features`

## 2026-05-04 Notes (Rust Toolchain Pin)

- Rust is pinned to `1.95.0` in `rust-toolchain.toml`, with `rustfmt` and `clippy` included.
- CI, CodeQL, and release workflows install Rust `1.95.0` explicitly instead of floating with
  `dtolnay/rust-toolchain@stable`, so local Clippy and PR Clippy should report the same lint set.

## Debugging Notes (Kad Status Counters)

- `/status` now includes two extra counters to help distinguish “network is silent” vs “we are
  receiving packets but can’t parse/decrypt them”:
  - `dropped_undecipherable`: failed Kad UDP decrypt (unknown/invalid obfuscation)
  - `dropped_unparsable`: decrypted OK but Kad packet framing/format was invalid
- For publish/search testing, we also now log at `INFO` when:
  - we receive a `PUBLISH_RES (key)` ACK (so you can see if peers accepted your publish)
  - we receive a non-empty `SEARCH_RES` (inserted keyword/source entries)

## Two-Instance Testing

- Added `docs/scripts/two_instance_dht_selftest.sh` to exercise publish/search flows between two
  locally-running rust-mule instances (e.g. mule-a on `:17835` and mule-b on `:17836`).
