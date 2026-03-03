# RFC-KAD3-INTEROP-CHECKLIST

## KAD3 Interoperability Requirements

**Status:** Draft  
**Category:** Informational  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Purpose

This document defines the minimum requirements for an independent
implementation to interoperate with KAD3 nodes.

It is intended as a practical checklist for implementers.

Passing this checklist implies KAD3-Core compatibility.

---

## 2. Mandatory Requirements (MUST)

An implementation MUST satisfy all of the following.

---

### 2.1 Identity

- [ ] Generate a long-term asymmetric keypair.
- [ ] Derive NodeID = HASH(public_key).
- [ ] Use the same hash function as defined by the KAD3 network.
- [ ] Reject peers where HASH(public_key) ≠ node_id.

---

### 2.2 Canonical Encoding

- [ ] Encode all messages using canonical CBOR.
- [ ] Use definite-length maps and arrays only.
- [ ] Sort map keys according to canonical CBOR rules.
- [ ] Sign the canonical CBOR encoding (excluding `signature` field).
- [ ] Reject non-canonical encodings (strict mode).

---

### 2.3 Signature Validation

- [ ] Verify signature for every received message.
- [ ] Reject unsigned messages.
- [ ] Reject messages with invalid signature.
- [ ] Reject messages with NodeID/public-key mismatch.

---

### 2.4 HELLO Handshake

- [ ] Send HELLO before routing table insertion.
- [ ] Validate `echo_nonce` in response.
- [ ] Enforce timestamp skew limits.
- [ ] Enforce replay protection (nonce or timestamp tracking).
- [ ] Only promote peers after successful HELLO exchange.

---

### 2.5 Routing Table Semantics

- [ ] Use XOR distance metric.
- [ ] Maintain k-buckets with bounded size.
- [ ] Implement least-recently-verified replacement policy.
- [ ] Do not split buckets outside local NodeID range.
- [ ] Do not insert unverified contacts into buckets.

---

### 2.6 Lookup Algorithm

- [ ] Implement iterative lookup with α parallelism.
- [ ] Terminate lookup when no closer nodes found.
- [ ] Validate contacts returned during lookup before insertion.
- [ ] Do not trust returned contacts blindly.

---

### 2.7 Transport Independence

- [ ] Ensure routing logic does not depend on transport type.
- [ ] Ensure message semantics identical across transports.
- [ ] Do not switch transport mid-request.

---

## 3. Recommended Requirements (SHOULD)

These improve robustness and security but are not required for minimal interop.

- [ ] Enforce bucket diversity (NodeID prefix or endpoint diversity).
- [ ] Track liveness per endpoint.
- [ ] Rate-limit invalid or malicious peers.
- [ ] Apply exponential backoff on failed probes.
- [ ] Separate hint cache from verified routing table.
- [ ] Log protocol violations.

---

## 4. Optional Extensions (MAY)

Implementations MAY support:

- Proof-of-work admission control
- Transport preference negotiation
- Bridge support for legacy KAD2
- Additional message types beyond core set

Unknown extensions MUST be ignored safely.

---

## 5. Interop Test Cases

An implementation MUST pass the following runtime tests:

### Test 1: HELLO Roundtrip

- Send HELLO.
- Receive HELLO response.
- Validate signature and echo_nonce.

### Test 2: Invalid Signature

- Send HELLO with tampered signature.
- Peer MUST reject.

### Test 3: NodeID Mismatch

- Send HELLO with mismatched NodeID.
- Peer MUST reject.

### Test 4: Canonical Encoding Violation

- Send map with unsorted keys.
- Peer MUST reject (strict mode).

### Test 5: FIND_NODE Exchange

- Perform iterative lookup.
- Ensure returned contacts validated before insertion.

---

## 6. Compliance Levels

### Level 1 – Core Interop

Meets all MUST requirements.

### Level 2 – Hardened

Meets MUST + SHOULD requirements.

### Level 3 – Extended

Supports optional extensions.

---

## 7. Protocol Negotiation and Upgrade

### 7.1 Overview

KAD2 does not provide protocol negotiation. KAD2 peers will send KAD2
messages unprompted according to their own behavior. KAD3 negotiation
therefore MUST be implemented as a KAD3-side capability upgrade attempt
that does not depend on any KAD2 negotiation support.

A node implementing both KAD2 and KAD3 ("dual-stack node") MUST be able
to communicate with legacy KAD2 peers without requiring them to change,
while opportunistically upgrading to KAD3 when a peer supports it.

### 7.2 Protocol Preference

A dual-stack node MUST prefer KAD3 when it is available. When a peer is
known to support KAD3, KAD3 MUST be used for all DHT operations with that
peer (HELLO, FIND_NODE, FIND_VALUE, STORE, PING/PONG). Implementations
MUST NOT silently downgrade mid-request.

### 7.3 Upgrade Attempt Trigger

A dual-stack node MAY attempt to upgrade a peer to KAD3 under any of the
following conditions:

- the peer was discovered via KAD2 (bootstrap or discovery), OR
- the peer was discovered via out-of-band configuration, OR
- the peer was learned through KAD3 but lacks current verification.

Upgrade attempts SHOULD be rate-limited and SHOULD avoid repeated probes
to unresponsive peers.

### 7.4 Upgrade Procedure (Normative)

To attempt a KAD3 upgrade, the node MUST perform:

1. Select a candidate endpoint for KAD3 transport (policy-driven).
2. Send a KAD3 HELLO request to that endpoint.
3. Await a valid KAD3 HELLO response.
4. If validation succeeds, mark the peer as KAD3-capable and promote it
   into the KAD3 routing table.

If the HELLO attempt fails (timeout, invalid signature, malformed
response), the peer MUST NOT be treated as KAD3-capable.

A failed upgrade attempt MUST NOT prevent continuing KAD2 communication.

### 7.5 Peer Capability State

Implementations MUST track peer capability state as one of:

- `Unknown`: no KAD3 upgrade attempted or insufficient evidence
- `Kad2Only`: confirmed non-KAD3 after repeated failed attempts (optional)
- `Kad3Capable`: successful HELLO verified
- `Kad3Banned`: KAD3 responses are invalid/malicious (rate-limited)

This state MUST be stored separately from routing-table membership.

### 7.6 Capability Advertisement (KAD3)

KAD3 capability advertisement occurs only within KAD3 messages and MUST
NOT be inferred from KAD2 behavior.

A KAD3 HELLO `capabilities` map MUST include:

- `transports`: array of supported transport identifiers
- `extensions`: array of supported extension identifiers (may be empty)

Unknown capabilities MUST be ignored.

### 7.7 Extension Negotiation (KAD3)

KAD3 extensions (e.g., stronger token scheme, PoW admission) MUST be
negotiated via intersection of advertised capability sets.

A node MUST only use an extension with a peer if:

- the local node supports the extension, AND
- the peer explicitly advertised support for the extension in HELLO.

If an extension is required for a message type, and negotiation fails,
the node MUST fall back to the non-extension baseline behavior for KAD3,
or refuse the operation if no baseline exists.

### 7.8 Transport Selection

Transport selection MUST be policy-driven and performed per-request.

For any request:

- exactly one endpoint MUST be selected
- retries MUST use the same endpoint type
- mid-request transport switching MUST NOT occur

### 7.9 KAD2 Coexistence Rules (Strict)

KAD2-derived peer information MUST be treated as "hints" and MUST NOT be
inserted into the KAD3 routing table without successful KAD3 HELLO
verification.

A KAD2 peer MUST NOT be assumed to support KAD3 unless it responds with
a valid KAD3 HELLO response.

### 7.10 Observability Requirements (Recommended)

Implementations SHOULD emit telemetry for:

- upgrade attempts (success/failure)
- validation failures (signature/node_id mismatch)
- downgrade/fallback decisions
- extension negotiation outcomes

## 8. Conclusion

An implementation that satisfies all MUST requirements
is KAD3-Core compatible and should interoperate
with other compliant implementations.
