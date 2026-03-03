# RFC-KAD3
## A Transport-Agnostic, Identity-Bound Kademlia DHT

**Status:** Draft  
**Category:** Standards Track  
**Version:** 0.3  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Abstract

KAD3 defines a Kademlia-based Distributed Hash Table (DHT) that is
transport-agnostic, identity-bound, and explicitly designed for hostile
networks.

KAD3 provides peer discovery and key lookup functionality.
It does not define packet routing, anonymity, or content transport.

---

## 2. Conformance Language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**,
and **MAY** are to be interpreted as described in RFC 2119.

---

## 3. Design Goals

- Deterministic lookup convergence (O(log N))
- Strong separation of concerns
- Cryptographic binding between NodeID and identity
- Support for multiple transports per node
- Resistance to routing table poisoning

Non-goals include packet forwarding, anonymity guarantees, and Sybil
elimination.

---

## 4. Layered Architecture

Implementations MUST separate the following layers:

1. **Semantics Layer**
   - XOR distance
   - k-buckets
   - lookup convergence
   - publish/republish rules

2. **Protocol Layer**
   - message formats
   - validation rules
   - replay protection
   - state machines

3. **Transport Layer**
   - message framing
   - delivery
   - timeouts
   - reachability feedback

No layer may directly depend on a lower layer’s internal state.

---

## 5. Identity Model

### 5.1 Keypair

Each node MUST possess a long-lived asymmetric keypair.

### 5.2 NodeID Derivation

NodeID MUST be derived as:

    NodeID = HASH(public_key)

The hash function MUST be cryptographically secure and fixed per network.

Nodes MUST NOT advertise NodeIDs not derived from their public key.

---

## 6. Distance Metric

KAD3 MUST use XOR distance:

    distance(A, B) = A XOR B

This metric MUST be used consistently for routing, bucket placement,
and lookup ordering.

---

## 7. Routing Table

### 7.1 Buckets

- Buckets represent contiguous XOR-distance ranges.
- Each bucket holds up to **k** contacts.
- Buckets covering the local NodeID MAY be split.
- Buckets not covering the local NodeID MUST NOT be split.

### 7.2 Replacement Policy

When a bucket is full:
1. The least-recently-verified contact MUST be probed.
2. If unresponsive, it MUST be replaced.
3. If responsive, the new contact MUST be discarded.

### 7.3 Liveness

Contacts MUST be promoted only after protocol-level verification.
One-off observations MUST NOT be treated as stable.

---

## 8. Contacts and Endpoints

A Contact consists of:

- NodeID
- Public Key
- One or more Endpoints
- Capability flags

Endpoints MUST specify:
- Transport identifier
- Address data
- Optional priority or expiry

---

## 9. Message Envelope

All protocol messages MUST include:

- message_type
- request_id
- src_node_id
- src_public_key
- timestamp or nonce
- payload
- signature

Unsigned or invalid messages MUST be rejected.

---

## 10. Core Messages

The following messages MUST be supported:

- PING / PONG
- HELLO
- FIND_NODE
- FIND_VALUE
- STORE
- ERROR

---

## 11. Lookup Algorithm

Lookups MUST be iterative and parallelized with factor α.

Termination MUST occur when:
- no closer nodes are discovered
- no unqueried candidates remain
- timeout budget is exceeded

---

## 12. Storage Semantics

- Values MUST be stored on the k closest nodes.
- Values MUST include an expiry.
- Nodes MAY refuse storage under resource pressure.
- Republish mechanisms SHOULD exist.

---

## 13. Transport Adapters

Transport adapters MUST:
- send messages
- receive messages
- report delivery success/failure

Transport adapters MUST NOT:
- implement routing logic
- mutate routing tables

---

## 14. Security Considerations

KAD3 mitigates:
- NodeID spoofing
- replay attacks
- basic routing table poisoning

KAD3 does not claim to solve:
- Sybil attacks
- traffic analysis
- endpoint deanonymization

---

## 15. Backward Compatibility

Bridges to legacy KAD2 networks MAY exist but MUST preserve trust-domain
separation and MUST NOT promote unverified contacts.

---

## 16. Conclusion

KAD3 preserves Kademlia’s proven semantics while modernizing identity,
transport abstraction, and implementation safety.