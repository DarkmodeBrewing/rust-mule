# Handoff / Continuation Notes

This file exists because chat sessions are not durable project memory. In the next session, start here, then check `git log` on `feature/sam-protocol`.

## Goal

Implement an iMule-compatible Kademlia (KAD) overlay over **I2P only**, using **SAM v3** `STYLE=DATAGRAM` sessions (UDP forwarding) for peer connectivity.

## Current State (As Of 2026-02-04)

- Active branch: `feature/sam-protocol`
- Implemented:
  - SAM v3 TCP control client with logging and redacted sensitive fields (`src/i2p/sam/`).
  - SAM `STYLE=DATAGRAM` session over TCP (iMule-style `DATAGRAM SEND` / `DATAGRAM RECEIVED`) (`src/i2p/sam/datagram_tcp.rs`).
  - SAM `STYLE=DATAGRAM` session + UDP forwarding socket (`src/i2p/sam/datagram.rs`).
  - iMule-compatible KadID persisted in `data/preferencesKad.dat` (`src/kad.rs`).
  - iMule `nodes.dat` v2 parsing (I2P destinations, KadIDs, UDP keys) (`src/nodes/imule.rs`).
  - KAD packet encode/decode including iMule packed replies (pure-Rust zlib/deflate inflater) (`src/kad/wire.rs`, `src/kad/packed.rs`).
  - Minimal bootstrap probe: send `PING` + `BOOTSTRAP_REQ`, decode `PONG` + `BOOTSTRAP_RES` (`src/kad/bootstrap.rs`).
  - Kad1+Kad2 HELLO handling during bootstrap (reply to `HELLO_REQ`, parse `HELLO_RES`, send `HELLO_RES_ACK` when requested) (`src/kad/bootstrap.rs`, `src/kad/wire.rs`).
  - I2P HTTP fetch helper over SAM STREAM (used to download a fresh `nodes2.dat`) (`src/i2p/http.rs`).
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

## Known Issue / Debugging

If you see `SAM read timed out` right after a successful `HELLO`, the hang is likely on `SESSION CREATE ... STYLE=DATAGRAM` (session establishment can be slow on some routers).

Mitigation:
- `sam.control_timeout_secs` (default `120`) controls SAM control-channel read/write timeouts.
- With `general.log_level = "debug"`, the app logs the exact SAM command it was waiting on (with private keys redacted).

## Latest Run Notes (2026-02-04)

Observed with `sam.datagram_transport = "tcp"`:
- SAM `HELLO` OK.
- `SESSION CREATE STYLE=DATAGRAM ...` OK.
- Loaded `datfiles/nodes.dat` (35 contacts).
- Sent initial `KADEMLIA2_BOOTSTRAP_REQ` to peers, but received **0** `PONG`/`BOOTSTRAP_RES` responses within the bootstrap window.
  - A likely root cause is that iMule nodes expect **obfuscated/encrypted KAD UDP** packets (RC4+MD5 framing), and will ignore plain `OP_KADEMLIAHEADER` packets.
  - Another likely root cause is that the nodes list is stale (the default iMule KadNodesUrl is `http://www.imule.i2p/nodes2.dat`).

Next things to try if this repeats:
- Switch to `sam.datagram_transport = "udp_forward"` (some SAM bridges implement UDP forwarding more reliably than TCP datagrams).
- Ensure Docker/host UDP forwarding is mapped correctly if using `udp_forward` (`sam.forward_host` must be reachable from the SAM host).
- Increase the bootstrap runtime (I2P tunnel build + lease set publication can take time). Defaults are now more forgiving (`max_initial=256`, `runtime=180s`, `warmup=8s`).
- Prefer a fresher/larger `nodes.dat` (this repo has both `datfiles/nodes.dat` and `source_ref/nodes.dat`; the app now prefers the `source_ref` one if `data/nodes.dat` is absent).
- Avoid forcing I2P lease set encryption types unless you know all peers support it (iMule doesn't set `i2cp.leaseSetEncType` for its datagram session).
- The app will attempt to fetch a fresh `nodes2.dat` over I2P from `www.imule.i2p` and write it to `data/nodes.dat` when it had to fall back to `source_ref/` or `datfiles/`.

If you see `Error: SAM read timed out` *during* bootstrap on `sam.datagram_transport="tcp"`, that's a local read timeout on the SAM TCP socket (no inbound datagrams yet), not necessarily a SAM failure. The TCP datagram receiver was updated to block and let the bootstrap loop apply its own deadline.

### Updated Run Notes (2026-02-04 19:30Z-ish)

- SAM `SESSION CREATE STYLE=DATAGRAM` succeeded but took ~43s (so `sam.control_timeout_secs=120` is warranted).
- We received inbound datagrams:
  - a Kad1 `KADEMLIA_HELLO_REQ_DEPRECATED` (opcode `0x03`) from a peer
  - a Kad2 `KADEMLIA2_BOOTSTRAP_RES` which decrypted successfully
- Rust now replies to Kad1 `HELLO_REQ` with a Kad1 `HELLO_RES` containing our I2P contact details, matching iMule's `WriteToKad1Contact()` layout.
- Rust now also sends Kad2 `HELLO_REQ` during bootstrap and handles Kad2 `HELLO_REQ/RES/RES_ACK` to improve chances of being added to routing tables and to exchange UDP verify keys.
- The `nodes2.dat` downloader failed because `NAMING LOOKUP www.imule.i2p` returned `KEY_NOT_FOUND` on that router.
- If `www.imule.i2p` and `imule.i2p` are missing from the router addressbook, the downloader can't run unless you add an addressbook subscription which includes those entries, or use a `.b32.i2p` hostname / destination string directly.

### Updated Run Notes (2026-02-04 20:42Z-ish)

- Bootstrap sent probes to `peers=103`.
- Received:
- `KADEMLIA2_BOOTSTRAP_RES` (decrypted OK), which contained `contacts=1`.
- `KADEMLIA2_HELLO_REQ` from the same peer; rust-mule replied with `KADEMLIA2_HELLO_RES`.
- `bootstrap summary ... discovered=2` and persisted refreshed nodes to `data/nodes.dat` (`count=120`).

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
- `kad.udp_key_secret` can be configured explicitly. If left as `0`, the app will generate one and persist it under `data/kad_udp_key_secret.dat` (analogous to iMule `thePrefs::GetKadUDPKey()`), without mutating `config.toml`.

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

## Reference Material

- iMule source + reference `nodes.dat` are under `source_ref/` (gitignored).
