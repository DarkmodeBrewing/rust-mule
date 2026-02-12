# Handoff / Continuation Notes

This file exists because chat sessions are not durable project memory. In the next session, start here, then check `git log` on `main` and the active feature branch(es).

## Goal

Implement an iMule-compatible Kademlia (KAD) overlay over **I2P only**, using **SAM v3** `STYLE=DATAGRAM` sessions (UDP forwarding) for peer connectivity.

## Status (2026-02-12)

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
- Publish/search candidate selection now truncates by **distance first**, then optionally reorders the *same* set by liveness to avoid skipping closest nodes.
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
- 2026-02-12: Return distance-ordered peer lists with fallback (max*4) to avoid empty batches when closest peers are skipped.
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
- 2026-02-06: Clarify iMule `KADEMLIA2_REQ` first byte is a *requested contact count* (low 5 bits), and update Rust naming (`requested_contacts`) + parity docs.
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
- 2026-02-09: Follow-up two-instance test showed *some* keyword results coming back from the network (`keyword_entries=1`),
  but A and B still tended to publish/search against disjoint "live" peers and would miss each other's stores.
  Fix: change DHT-critical peer selection to be **distance-first** (XOR distance primary; liveness as tiebreaker) so that
  publish/search targets the correct closest nodes (`src/kad/routing.rs`, `src/kad/service.rs`).
- 2026-02-09: Two-instance test artifacts under `./tmp/` (mule-a+mule-b with `docs/scripts/two_instance_dht_selftest.sh`):
  - Script output shows each side only ever returns its *own* published hit for the shared keyword (no cross-hit observed).
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
  - File *sources* are likely intermittent; plan to keep them much longer (days/weeks) and track `last_seen` rather than aggressively expiring.
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

If you see `Error: SAM read timed out` *during* bootstrap on `sam.datagram_transport="tcp"`, that's a local read timeout on the SAM TCP socket (no inbound datagrams yet), not necessarily a SAM failure. The TCP datagram receiver was updated to block and let the bootstrap loop apply its own deadline.

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
- If we incorrectly put *our* KadID in the `check` field, peers will silently ignore the request and never send `KADEMLIA2_RES`.

After the fix, long runs should start showing `recv_ress>0` and `new_nodes>0` as the crawler learns contacts.

### Note: Why `routing` Might Not Grow Past The Seed Count

If `kad service status` shows `recv_ress>0` but `routing` stays flat (e.g. stuck at the initial `nodes.dat` size), that can be normal in a small/stale network *or* it can indicate that peers are mostly returning contacts we already know (or echoing our own KadID back as a contact).

The service now counts “new nodes” only when `routing.len()` actually increases after processing `KADEMLIA2_RES`, to avoid misleading logs.

Also: the crawler now picks query targets Kademlia-style: it biases which peers it queries by XOR distance to the lookup target (not just “who is live”). This tends to explore new regions of the ID space faster and increases the odds of discovering nodes that weren't already in the seed `nodes.dat`.

Recent observation (2026-02-06, ~50 min run):
- `data/nodes.dat` stayed at `154` entries; routing stayed at `153`.
- `live` peers stayed at `2`.
- Periodic `KADEMLIA2_BOOTSTRAP_REQ` refresh got replies, but returned contact lists were typically `2` and did not introduce new IDs (`new_nodes=0`).

Takeaway: this looks consistent with a very small / stagnant iMule I2P-KAD network *or* a seed which mostly points at dead peers. Next improvements should focus on discovery strategy and fresh seeding (see TODO below).

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
   - TODO (discovery): add a conservative “cold bootstrap probe” mode so periodic bootstrap refresh occasionally targets *non-live / never-seen* peers, to try to discover new clusters without increasing overall traffic.
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
