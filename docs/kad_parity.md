# KAD Parity Notes (iMule vs rust-mule)

This document is a **sanity-check map** between iMule's Kademlia-over-I2P implementation and
`rust-mule`'s Rust-native implementation.

Goal: follow the **wire format + interoperability behavior** of iMule, without copying C++ architecture.

## Reference Pointers (iMule)

Primary files in `source_ref/iMule-2.3.1.5-src/`:

- UDP obfuscation (RC4+MD5 framing): `src/EncryptedDatagramSocket.cpp`
- Verify key derivation: `src/kademlia/kademlia/Prefs.cpp` (`CPrefs::GetUDPVerifyKey`)
- UDP listener (opcodes / packet formats / send path): `src/kademlia/net/KademliaUDPListener.cpp`
- Contact serialization + Kad1/Kad2 contact layouts:
  - `src/kademlia/routing/Contact.cpp` (`WriteToKad1Contact`, `WriteToKad2Contact`, `WriteToFile`)
  - `src/kademlia/routing/RoutingZone.cpp` (`WriteFile` / `ReadFile` for `nodes.dat`)
- I2P dest hash: `src/libs/i2p/CI2PAddress.h` (`hashCode()` = first 4 bytes as `uint32`)

## rust-mule Modules

- Wire formats + opcodes: `src/kad/wire.rs`
- UDP obfuscation (encrypt/decrypt + verify keys): `src/kad/udp_crypto.rs`
- Long-lived crawler/service loop: `src/kad/service.rs`
- Bootstrap probe + minimal responders (startup phase): `src/kad/bootstrap.rs`
- Routing table (Rust-native, not bucketed yet): `src/kad/routing.rs`
- `nodes.dat` read/write (iMule v2): `src/nodes/imule.rs`

## Confirmed Wire Formats

- **Kad packet header**
  - iMule: `OP_KADEMLIAHEADER (0x05)` and `OP_KADEMLIAPACKEDPROT (0x06)`
  - rust-mule: `KadPacket::{encode, decode}` handles both, including zlib inflate for `0x06`.

- **KADEMLIA2_BOOTSTRAP_REQ (0x0D)**: empty payload.

- **KADEMLIA2_BOOTSTRAP_RES (0x0E)**:
  - iMule (`Process2BootstrapRequest`): `<sender_id u128><sender_ver u8><sender_tcp_dest 387><count u16><contact>*count>`
  - rust-mule: `decode_kad2_bootstrap_res()` matches layout.
  - Per-contact layout is `WriteToKad2Contact`: `<ver u8><id u128><udp_dest 387>`.

- **KADEMLIA2_HELLO_REQ/RES (0x0F/0x10)**:
  - Base contact payload is `WriteToKad2Contact` followed by TagList.
  - rust-mule: `encode_kad2_hello()` + `decode_kad2_hello()` match this (we parse only numeric tags we care about).
  - ACK request is `TAG_KADMISCOPTIONS (0x58)` with bit `0x04` set (Kad v8+).

- **KADEMLIA2_HELLO_RES_ACK (0x12)**:
  - iMule: `<myKadID u128><tagCount u8 (0)>`
  - rust-mule: same.

- **KADEMLIA2_REQ (0x11)**:
  - iMule (`Search.cpp::SendFindPeersForTarget`): `<contactCount u8><target u128><check(receiver_id) u128><sender(my_id) u128>`
  - rust-mule: `encode_kad2_req()` includes sender id; `decode_kad2_req()` parses it when present.

- **KADEMLIA2_RES (0x13)**:
  - iMule: `<target u128><count u8><contact>*count` where contact is `WriteToKad2Contact`.
  - rust-mule: `encode_kad2_res()` / `decode_kad2_res()` match.

- **Kad1 (deprecated) REQ/RES + HELLO**
  - iMule uses the *same* request payload for Kad1+Kad2 searches:
    `<contactCount u8><target u128><check(receiver_id) u128><sender(my_id) u128>`; opcode differs.
  - rust-mule: `decode_kad1_req()` matches; Kad1 HELLO_RES now uses contact type `3` (matches iMule `WriteToKad1Contact` default).

## UDP Obfuscation (Crypto) Parity

- iMule "encrypted datagram" framing is implemented in `src/kad/udp_crypto.rs`:
  - NodeID-key and receiver-key variants (iMule `DecryptReceivedClient` try 0 + try 2).
  - Magic: `0x395F2EC1` (KAD UDP sync client).
  - `pad_len = 0` (matches iMule's default; padding is allowed but unused).
  - Verify key derivation matches iMule `CPrefs::GetUDPVerifyKey`:
    `udp_verify_key(secret, dest_hash)`.
- I2P dest hash matches iMule `CI2PAddress::hashCode()`:
  first 4 destination bytes reinterpreted as a little-endian `u32`.

## `nodes.dat` v2 Parity

- iMule `RoutingZone::WriteFile` writes:
  - `u32 0` (old client guard)
  - `u32 2` (file version)
  - `u32 count`
  - contacts via `CContact::WriteToFile`:
    `<kad_ver u8><id u128><udp_dest 387><udp_key u32><udp_key_ip u32><verified u8>`
- rust-mule implements exactly that in `src/nodes/imule.rs`.

## Intentional Differences (Rust-Native)

- Routing table is currently a stable in-memory set (`RoutingTable`) with selection helpers, not a full k-bucket tree.
  - This is a conscious Rust-native stepping stone.
- iMule packet tracking / legacy challenge logic isn't replicated 1:1.
  - We use receiver-key validity + observed traffic as the primary "verified" signal.
