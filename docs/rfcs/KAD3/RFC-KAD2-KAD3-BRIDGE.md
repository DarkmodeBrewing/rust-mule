# RFC-KAD2-KAD3-BRIDGE
## Bridging Legacy KAD2 Networks into KAD3

**Status:** Draft  
**Category:** Informational / Experimental  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Abstract

This document specifies a bridge mechanism allowing nodes implementing
KAD3 to interact with legacy KAD2 networks in a controlled and safe manner.

The bridge is designed to support:
- bootstrapping,
- peer discovery,
- optional search/publish interoperability,

while preserving KAD3’s stricter identity, validation, and routing guarantees.

This bridge explicitly defines **trust boundaries** and **promotion rules**
to prevent routing table poisoning and identity confusion.

---

## 2. Conformance Language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**,
and **MAY** are to be interpreted as described in RFC 2119.

---

## 3. Design Goals

- Allow KAD3 nodes to leverage existing KAD2 networks for discovery
- Prevent unverified KAD2 contacts from contaminating KAD3 routing tables
- Preserve KAD3’s identity-bound trust model
- Support gradual migration from KAD2 to KAD3

---

## 4. Non-Goals

This bridge does NOT aim to:
- provide transparent identity equivalence between KAD2 and KAD3
- guarantee full interoperability
- make KAD3 dependent on KAD2
- solve Sybil attacks originating from KAD2

---

## 5. Trust Domains

KAD2 and KAD3 operate in **separate trust domains**.

- KAD2 lacks cryptographic identity binding.
- KAD3 requires cryptographic NodeID verification.

The bridge MUST treat all KAD2-derived information as **untrusted hints**
until independently verified via KAD3 protocol exchanges.

---

## 6. Bridge Modes

Implementations MAY support one or more of the following modes.

### 6.1 Mode 1: Bootstrap-Only Bridge (RECOMMENDED)

Purpose:
- Initial network entry
- Recovery from routing table starvation

Behavior:
- KAD2 is queried for peer addresses only.
- Returned peers are treated as hints.
- Hints MUST be probed using KAD3 `HELLO`.
- Only peers that complete KAD3 verification MAY enter the KAD3 routing table.

This mode MUST be supported by compliant implementations.

---

### 6.2 Mode 2: Directory Bridge (RECOMMENDED)

Purpose:
- Ongoing discovery support
- Supplemental peer hints

Behavior:
- KAD2-derived peers are stored in a separate hint cache.
- Hint cache entries MUST NOT affect:
  - bucket splitting
  - routing distance calculations
- Promotion to KAD3 contacts requires full KAD3 handshake and verification.

---

### 6.3 Mode 3: Semantic Bridge (OPTIONAL, HIGH RISK)

Purpose:
- Enable KAD3 nodes to search and publish into KAD2 networks.

Behavior:
- KAD3 `FIND_VALUE` and `STORE` are translated to KAD2 equivalents.
- Returned results MUST be labeled as originating from KAD2.
- KAD2-originated results MUST NOT:
  - influence KAD3 routing
  - be treated as trusted
  - cause contact promotion without KAD3 verification

This mode SHOULD be disabled by default.

---

## 7. Data Structures and Separation

Implementations MUST maintain at least two distinct stores:

### 7.1 KAD3 Routing Table
- Contains only fully verified KAD3 contacts
- Enforced by identity and signature validation

### 7.2 KAD2 Hint Cache
- Contains unverified peer endpoints
- Stores metadata such as:
  - origin
  - first_seen
  - last_seen
  - success_rate

No module MAY bypass this separation.

---

## 8. Contact Promotion Rules

### 8.1 Promotion Preconditions

A KAD2 hint MAY be promoted to a KAD3 contact ONLY IF:
- the peer successfully completes a KAD3 `HELLO` exchange
- NodeID is verified as derived from the advertised public key
- message signatures validate
- endpoint policies permit the transport

Failure to meet ANY requirement MUST prevent promotion.

---

### 8.2 Identity Mapping Constraints

- KAD2 NodeIDs MUST NOT be treated as equivalent to KAD3 NodeIDs.
- The bridge MUST NOT project KAD3 NodeIDs into KAD2 space as identities.
- A bridge MAY operate with a distinct local KAD2 identity solely for KAD2 participation.

---

## 9. Protocol Translation Rules

### 9.1 FIND_NODE

- KAD3 `FIND_NODE(target)` MAY be translated into a KAD2 node lookup.
- Results MUST be placed into the KAD2 hint cache only.

### 9.2 FIND_VALUE / STORE

If semantic bridging is enabled:
- KAD3 `STORE` MAY be translated to KAD2 publish semantics.
- KAD3 `FIND_VALUE` MAY be translated to KAD2 search semantics.

Returned values MUST be tagged:
- `origin = kad2`
- `verification = none`
- `confidence = low`

User-facing systems SHOULD surface this distinction clearly.

---

## 10. Anti-Poisoning Requirements

The bridge MUST enforce:
- rate limits on KAD2 hint ingestion
- endpoint diversity constraints
- malformed reply detection
- failure tracking and eviction

The bridge MUST NOT:
- insert KAD2 hints directly into KAD3 buckets
- trigger bucket splits based on KAD2 data
- override KAD3 distance ordering

---

## 11. Bootstrapping Algorithm

A RECOMMENDED bootstrap flow:

1. Load persisted KAD3 contacts.
2. Attempt KAD3-only network join.
3. If insufficient contacts:
   - query KAD2 for hints
   - store hints in hint cache
4. Probe hints using KAD3 `HELLO`.
5. Promote verified peers.
6. Reduce KAD2 bridge activity as KAD3 stabilizes.

---

## 12. Deployment Patterns

### 12.1 Dual-Stack Node

A single process implements:
- KAD3 protocol
- KAD2 bridge client

This is the RECOMMENDED deployment model.

---

### 12.2 External Bridge Service

A separate service:
- interacts with KAD2
- supplies signed hint feeds to KAD3 nodes

This model centralizes risk and SHOULD be treated as semi-trusted.

---

## 13. Security Considerations

This bridge mitigates:
- direct routing table poisoning
- identity confusion between KAD2 and KAD3

This bridge does NOT mitigate:
- Sybil attacks originating in KAD2
- traffic analysis
- malicious content propagation

---

## 14. Conclusion

The KAD2 → KAD3 bridge provides a pragmatic migration path while
maintaining KAD3’s stricter trust and validation guarantees.

The bridge is intentionally conservative: discovery is permitted,
trust is not inherited.

---

## 15. References

- Kademlia: A Peer-to-Peer Information System Based on the XOR Metric
- KAD1 / KAD2 (eMule/aMule)
- RFC 2119