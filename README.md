# rust-mule

<p align="center">
  <img src="assets/this_is_fine_mule.png" alt="This is fine – rust-mule KAD bootstrap" width="420">
</p>

<p align="center">
  <em>
    Bootstrapping a decentralized KAD network over I2P.<br>
    Sometimes it works immediately. Sometimes it sets the room on fire.
  </em>
</p>

`rust-mule` is an experimental, **I2P-only** iMule-compatible Kademlia (KAD) crawler/overlay.
It talks to the local (or remote) I2P router via **SAM v3** using `STYLE=DATAGRAM` sessions.

This repo is currently focused on:

- Getting the SAM datagram client stable (TCP datagram framing or UDP forwarding).
- Running a long-lived KAD service loop to discover peers and maintain a routing table.
- Persisting a reusable bootstrap seed pool (`data/nodes.dat`).

## Quick Start

1. Edit `config.toml`:

- `sam.host` / `sam.port`: where the SAM bridge is
- `sam.datagram_transport`: `tcp` (default) or `udp_forward`

2. Run:

```bash
cargo run --bin rust-mule
```

Logs:

- Stdout is controlled by `[general].log_level` (or `RUST_LOG`)
- File logs roll daily under `data/logs/` when `[general].log_to_file=true`

## Data Files

Runtime state lives under `data/` (gitignored):

- `data/nodes.dat`: primary persisted bootstrap pool (iMule `nodes.dat` v2 format)
- `data/nodes.initseed.dat`: initial seed snapshot (created from embedded initseed on first run)
- `data/nodes.fallback.dat`: fallback seed snapshot (currently same as initseed)
- `data/preferencesKad.dat`: your KadID (stable node identity)
- `data/kad_udp_key_secret.dat`: UDP obfuscation secret (iMule-style verify key seed; generated on first run)
- `data/sam.keys`: SAM destination keys (`PUB=...` / `PRIV=...`)

For more details and current development notes, see `docs/handoff.md`.
