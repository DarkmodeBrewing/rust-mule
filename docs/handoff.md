# Handoff / Continuation Notes

This file exists because chat sessions are not durable project memory. In the next session, start here, then check `git log` on `main` and the active feature branch(es).

## Goal

Implement an iMule-compatible Kademlia (KAD) overlay over **I2P only**, using **SAM v3** `STYLE=DATAGRAM` sessions (UDP forwarding) for peer connectivity.

## Status (2026-02-12)

- Long-haul two-instance run (25 rounds) confirmed network-origin keyword hits on both instances:
  - A received non-empty `SEARCH_RES` at 2026-02-11 19:41:41.
  - B received non-empty `SEARCH_RES` at 2026-02-11 19:50:02.
- Routing snapshot at end of run: total_nodes=157, verified=135, buckets_empty=121, bucket_fill_max=80, last_seen_max≈35060s (~9.7h), last_inbound_max≈29819s (~8.3h). Routing still not growing (`new_nodes=0`).
- Observed SAM `SESSION STATUS RESULT=I2P_ERROR MESSAGE="PONG timeout"` on both instances at 2026-02-12 06:49:20; service auto-recreated SAM session.
- Source publish/search remained empty in the script output.

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

- 2026-02-11: Tune two-instance selftest script with polling + peer snapshot controls; update `tmp/test_script_command.txt` to use new flags.
- 2026-02-11: Add routing snapshot controls and end-of-run routing dumps for the two-instance selftest; update `tmp/test_script_command.txt`.
- 2026-02-12: Long-haul run confirmed network-origin keyword hits; routing table still flat; SAM session recreated after PONG timeout on both instances.
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

- Active branch (latest work): `feature/kad-search-publish`
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

If you see the service loop sending lots of `KADEMLIA2_REQ` but reporting `recv_ress=0` in `kad service status`, the most likely culprit was a bug which was fixed on `feature/sam-protocol`:

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
