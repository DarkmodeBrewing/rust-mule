# TODO

Sync this document with the `next steps` notes in `docs/handoff.md`.

Here are some TODOs divided into categories:

- SAM
- KAD
- API
- UI
- General
- Documentation
- CLI

We focus on one category at a time. If we find another category or a TODO item, this file should be updated.
The points in this document together with `docs/handoff.md` should be treated as a foundation for real task planning.
Put tasks in `docs/TASKS.md`.
If any design documents (`docs/UI_DESIGN.md`, `docs/API_DESIGN.md`) contain tasks not covered in this TODO, add them here.

## SAM

## KAD

- [/] Search/Publish - ongoing, get network communication work, verify
- [/] General KAD protocol alignment to `iMule` (see `./source_ref` for iMule C++ source code)
- [/] Get ACKs from peers
- [/] Grow network

## API

- Overview of API design is located in `/docs/API_DESIGN.md`
- [ ] API should bind to `127.0.0.1` and `localhost`, not `0.0.0.0`
- [x] API `CORS` rules should only accept connections from `127.0.0.1`/`localhost`, and allow headers `Authorization`, `Content-Type` (at least not more headers than required)
- [x] API should be served from base URL `/api/<version>`
- [x] API should expose an endpoint for the UI to get the bearer token from `/data/api.token` (`GET:/api/<version>/dev/auth`)
  - [x] Endpoint should be loopback only

## UI

- Overview of the design of the UI is documented in `/docs/UI_DESIGN.md`
- [x] The UI will consist of static HTML-,CSS-, JS-files with Alpine.JS and Chart.js
- [x] While bootstrapping the UI, a call to the API's endpoint (`GET:/api/<version>/dev/auth`) to get the bearer token (`/data/api.token`)
- [x] Static files (JavaScript files and CSS files) will be included in `./ui/assets/<type>`, but embedded in the binary (`/ui/assets/css/base.css`, `/ui/assets/js/app.js`, `/ui/assets/js/alpine.min.js`, `/ui/assets/js/chart.min.js`, etc.)
  - [x] Alpine.JS - used for state, interactive form controls, list and for composing components
  - [x] Chart.JS - used for plot charts for statistics and the like
  - [x] Bootstrap/init JS exists (`theme-init.js`, `helpers.js`) for startup functions including token bootstrap and theme setup
  - [x] base.css - UI styling, set up themes using CSS variables
  - [/] Rule for CSS - always use variables, never literals, using `em` inside components and `px` for borders/hairlines
  - [ ] Perhaps SVGs or images for logo/icons - not decided
  - [/] All static files (`.html`, `.js`, `.css` etc.) should be embedded into the application, and should be served via http over the same port on the api, protected by frontend session auth
- [/] Routes: API is served from `/api/` base route, UI is served from `/` (folder: `/ui/<page>.html`) -> (`/ui/index.html`, `/ui/statistics.html`, `/ui/settings.html`, `/ui/search.html`, etc.)
- [x] Bearer token should never be exposed in the UI (HTML, query params etc.), bootstrapping javascript should get it and store it in `sessionStorage` for usage in subsequent API requests (SSE now uses session-cookie auth, no token query param)
- [ ] UI should leverage
  - [x] Start page / overview
  - [x] Search interface: File search by keywords
  - [x] Network status (Peers), with statistics (Network throughput, Live / Last seen (peers), streaming events (SSE))
  - [x] Application settings (from config.toml) backed by the API (`PATCH: /api/settings`, `GET: /api/settings`)
- [x] Create start/overview page
- [/] Create statistics page, use chart.js to draw statistical charts where needed (charts + controls implemented on `node_stats`; dedicated statistics page still open)
- [x] Create the search form, leveraging Alpine.js
- [x] UI should be auto started in the current platforms default browser
- [x] Settings toggle to prevent auto open of UI should exist, to be able to run headless
- [x] Auto open must wait for the bootstrap of API and HTTP services, and the `/data/api.token` must exist

## General

- [ ] Clean up trace logging / file logging, want a more clean `INFO` log, hide `verbosity` behind `DEBUG` flag
  - [ ] Never log keys or unredacted hashes
- [ ] Do a `clippy` round on the complete repo
- [ ] Memory pressure logging using `jmalloc`, and memory management
- [ ] Memory pressure logs size of routing table, lookups etc. to have
- [x] API should be mandatory, i.e. no config toggle to load the API, only port number should be configurable (since the API is the control plane)
- [ ] Logfile names should be written as `rust-mule.YYYY-mm-DD.log` and continue daily rotation, but there should be a check on startup to delete logfiles older than 30 days
- [ ] `rust-mule` client shows up in I2P Router console as `SAM UDP Client` - investigate if we can report a client name to the router (just like `I2P Snark`)
- [ ] TLS for headless, remote UI - low priority
- [ ] Possibly add a SQLite database for storage - 

## Documentation

- [ ] Make sure to align the documentation to the latest version

## CLI

- [ ] Investigate possibility to run headless, but still access the control plane - low priority
