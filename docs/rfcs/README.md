# rust-mule RFC Index

This directory contains design documents and protocol specifications
for the rust-mule project.

All RFCs follow an incremental, implementation-driven approach.
They are not frozen standards.

---

## Active RFCs

### RFC-KAD3

**File:** `RFC-KAD3.md`  
**Status:** Draft  
**Summary:**  
Defines a transport-agnostic, identity-bound Kademlia DHT (KAD3),
with strict separation between semantics, protocol, and transport.

Key properties:

- Cryptographic NodeID binding
- Multi-endpoint contacts
- Explicit anti-poisoning rules
- No transport coupling

---

### RFC-KAD2-KAD3-BRIDGE

**File:** `RFC-KAD2-KAD3-BRIDGE.md`  
**Status:** Experimental  
**Summary:**  
Defines a controlled bridge from legacy KAD2 networks into KAD3,
supporting bootstrapping, discovery, and optional semantic interop,
while preserving KAD3 trust boundaries.

Key properties:

- Strict trust-domain separation
- Hint → probe → verify promotion pipeline
- Safe migration path from KAD2 to KAD3

---

## Supporting Documents (Planned / In Progress)

- `RFC-KAD3-WIRE-FORMAT.md` — canonical encoding & framing
- `RFC-KAD3-STATE-MACHINES.md` — handshake & lookup state diagrams
- `RFC-KAD3-SECURITY.md` — extended threat model
- `RFC-KAD3-TRANSPORTS.md` — UDP, I2P, QUIC adapters
- `RFC-KAD3-HANDSHAKE.md` — KAD3 HELLO handshake protocol
- `RFC-KAD3-TEST-VECTORS.md` — canonical CBOR encoding examples
- `RFC-KAD3-INTEROP-CHECKLIST.md` — the minimum requirements for an independent implementation to interoperate with KAD3 node
- `RFC-KAD3-POLICY.md` — defines operational policy parameters for KAD3

---

## Status Legend

- **Draft** – Actively evolving, implementation-backed
- **Experimental** – Optional or high-risk features
- **Stable** – Semantics unlikely to change
- **Deprecated** – Retained for reference only
