# Dev Notes (SAM + KAD Over I2P)

This repo is currently focused on bootstrapping an iMule-compatible Kademlia (KAD) overlay over I2P using SAM v3.

## Branch

Active work has been happening on the feature branch `feature/sam-protocol`.

## What Works Today

- SAM v3 control protocol client (`src/i2p/sam/`).
- SAM `STYLE=DATAGRAM` session with UDP forwarding (`src/i2p/sam/datagram.rs`).
- iMule-compatible identity:
  - `preferencesKad.dat` persisted KadID (random 128-bit) (`src/kad.rs`).
- iMule-compatible `nodes.dat` parsing (I2P destinations, KadIDs, UDP keys) (`src/nodes/imule.rs`).
- KAD2 bootstrap "probe":
  - Sends `KADEMLIA2_PING` and `KADEMLIA2_BOOTSTRAP_REQ` to peers from `nodes.dat`.
  - Receives and decodes `KADEMLIA2_PONG` and `KADEMLIA2_BOOTSTRAP_RES`.
  - Supports iMule "packed" KAD replies (deflate+zlib) via a pure-Rust inflater (`src/kad/packed.rs`).

This is not yet a full routing table implementation. It is a minimal end-to-end connectivity check to real peers.

## Local Reference Sources

- `source_ref/` contains iMule sources and reference files. It is intentionally gitignored.
- `datfiles/nodes.dat` is a small nodes list.

The app will prefer `data/nodes.dat`, but falls back to `datfiles/nodes.dat` and `source_ref/nodes.dat` if those exist.

## Running

1. Ensure SAM is reachable.
2. Ensure UDP forwarding from SAM to this process works (see next section).
3. Run:

```bash
cargo run
```

Optional tooling:

```bash
cargo run --bin imule_nodes_inspect -- datfiles/nodes.dat
```

## Remote SAM Bridging (Docker Scenario)

If the SAM bridge is on a different machine than the `rust-mule` process, SAM UDP forwarding must be configured so that:
- The SAM bridge can send UDP packets to the host+port you provide in `SESSION CREATE ... HOST=<forward_host> PORT=<forward_port>`.
- That UDP host+port ultimately reaches the `rust-mule` UDP socket.

Example topology used during development:
- SAM bridge: `10.99.0.2`
- This dev environment: Docker container on host `10.99.0.1`

Recommended `config.toml` values for that topology:
- `sam.host = "10.99.0.2"` (SAM TCP control)
- `sam.port = 7656`
- `sam.udp_port = 7655` (SAM UDP datagrams)
- `sam.forward_host = "10.99.0.1"` (the Docker host IP, reachable from `10.99.0.2`)
- `sam.forward_port = 40000` (fixed UDP port, so Docker/firewalls can be configured)

Docker requirements (choose one):
- Use host networking (`--network host`), or
- Publish UDP port mapping (`-p 40000:40000/udp`).

If you leave `sam.forward_port = 0` (ephemeral), it is usually hard to forward through Docker or firewalls.

## Obsolete Code Cleanup

As part of the KAD-over-I2P alignment, the following obsolete/unused modules were removed:
- `src/net/` (TCP probe helpers)
- The older IPv4-centric `nodes.dat` parser in `src/nodes/parse.rs` and `src/nodes/detect.rs`

