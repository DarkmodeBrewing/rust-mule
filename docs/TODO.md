# TODO

Sync this document with the `next steps` notes in the `docs/handover.md`

Here are some TODO's divided into the categories

- SAM
- KAD
- API
- UI
- General
- Documentation
- CLI

We focus at one category a time. If we find another category or a TODO item, this file should be updated.
The points in this document together with `/docs/handover.md` should be treated as a foundation for real task planning.
Put tasks in `/docs/TASKS.md`
If any of the desing documents (`/docs/UI_DESIGN.md`, `/docs/API_DSIGN.md`) contains tasks that aren't covered in this `TODO`, please add more todos in this document

## SAM

## KAD

- [/] Search/Publish - ongoing, get network communication work, verify
- [/] General KAD Protokol alingment to `iMule` (see sources `./source_ref` for iMule C++ sourcecode)
- [/] Get ACK's from peers
- [/] Grow network

## API

- Overview of API design is located in `/docs/API_DESIGN.md`
- [ ] API should bind to `127.0.0.1` and `localhost`, not `0.0.0.0`
- [ ] API `CORS` rules should only accept connections from `127.0.0.1`/`localhost`, and allow headers `Authorization`, `Content-Type` (at least not more headers then required)
- [ ] API should be served from base URL `/api/<version>`
- [ ] API should expose an ednpoint for the UI to get the bearer token from `/data/api.token` (`GET:/api/<version>/dev/auth`)
  - [] Endpoint should be loopback only

## UI

- Overview of the design of the UI is documented in `/docs/UI_DESIGN.md`
- [ ] The UI will consist of static HTML-,CSS-, JS-files with Alpine.JS and Chart.js
- [ ] While bootstrapping the UI, a call to the API's endpoint (`GET:/api/<version>/dev/auth`) to get the bearer token (`/data/api.token`)
- [ ] Static files (JavasScrip files and CSS files) will be included in `./ui/assets/<type>`, but embedded in the binary (`/ui/assets/css/base.css`, `/ui/assets/js/app.js`, `/ui/assets/js/alpine.min.js`, `/ui/assets/js/chart.min.js` etc.)
  - [ ] Alpine.JS - used for state, interactive form controls, list and for composing components
  - [ ] Chart.JS - used for plot charts for statistics and the like
  - [ ] bootstrap.js - bootstrap initial functions, fetch bearer token, functions for setting theme etc.
  - [ ] base.css - UI styling, set up themes using CSS variables
  - [ ] Rule for CSS - always use variables, never litterals, if not using `em` inside components, or `px` when drawing borders/hairlines
  - [ ] perhaps svg's or images used for logo, icons etc. - not descided
  - [ ] All static files (`.html`, `.js`, `.css` etc.) should be embedded into the application, and should be served via http over the same port on the api, protected by the bearer token
- [ ] Routes: API is served from `/api/` base route, UI is server from `/` (folder: `/ui/<page>.html`) -> (`/ui/index.html`, `/ui/statistics.html`, `/ui/settings.html`, `/ui/search.html` etc.)
- [ ] Bearer token should never be exposed in the UI (HTML, query params etc.), bootstrapping javascript should get it and store it in `sessionStorage` for usage in subsequent API requests (question: how to handle SSE endpoints??)
- [ ] UI should leverage
  - [ ] Start page / overview
  - [ ] Search interface: File search by keywords
  - [ ] Network status (Peers), with statistics (Network throughput, Live / Last seen (peers), streaming events (SSE))
  - [ ] Application settings (from config.toml) backed by the API (`PATCH: /api/settings`, `GET: /api/settings`)
- [ ] Create start/overview page
- [ ] Create statistics page, use chart.js to draw statistical charts where needed
- [ ] Create the search form, levaraging Alpine.js
- [ ] UI should be auto started in the current platforms default browser
- [ ] Settings toggle to prevent auto open of UI should exist, to be able to run headless
- [ ] Auto open must wait for the bootstrap of API and HTTP services, and the `/data/api.token` must exist

## General

- [ ] Clean up trace logging / file logging, want a more clean `INFO` log, hide `verbosity` behind `DEBUG` flag
  - [ ] Never log keys or unredacted hashes
- [ ] Do a `clippy` round on the complete repo
- [ ] Memory pressure logging using `jmalloc`, and memory management
- [ ] Memory pressure logs size of routing table, lookups etc. to have
- [ ] API should be mandatory, i.e. no config toggle to load the API, only port number should be configurable (since the API is the control plane)
- [ ] Logfile names should be written as `rust-mule.YYYY-mm-DD.log` and continue daily rotation, but there should be a check on startup to delete logfiles older than 30 days
- [ ] `rust-mule` client shows up in I2P Router console as `SAM UDP Client` - investigate if we can report a client name to the router (just like `I2P Snark`)
- [ ] TLS for headless, remote UI - low priority
- [ ] Possibly add a SQLite database for storage - 

## Documentation

- [ ] Make sure to align the documentation to the latest version

## CLI

- [ ] Investigate posibility to run headless, but still access the control plane - low priority
