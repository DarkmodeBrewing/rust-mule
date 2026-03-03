# RFC-KAD3-TRANSPORTS
## Transport Adapters for KAD3

**Status:** Draft  
**Category:** Informational  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Abstract

This document defines how transport adapters integrate with KAD3.
It specifies adapter responsibilities, guarantees, and constraints.

Transport adapters provide message delivery only.
They MUST NOT implement routing or protocol semantics.

---

## 2. Transport Abstraction Model

KAD3 separates:

- **Message encoding** (CBOR, protocol layer)
- **Message framing and delivery** (transport layer)

Transport adapters are interchangeable components.

---

## 3. Transport Adapter Requirements

All transport adapters MUST provide:

- send(endpoint, bytes)
- receive(bytes, endpoint)
- timeout handling
- reachability feedback

Transport adapters MUST NOT:
- interpret message payloads
- reorder semantic operations
- mutate routing tables

---

## 4. Supported Transports

This document defines three reference adapters:

- UDP
- I2P (SAM v3)
- QUIC

Additional transports MAY be added without changing KAD3 semantics.

---

## 5. UDP Adapter

### 5.1 Characteristics
- Datagram-based
- Unreliable
- Low latency
- Spoofable

### 5.2 Usage
- One KAD3 message per datagram
- Canonical CBOR payload
- No fragmentation beyond MTU

### 5.3 Considerations
- Expect loss and duplication
- Signature verification is mandatory
- Rate limiting REQUIRED

---

## 6. I2P Adapter

### 6.1 Characteristics
- Anonymous overlay
- High latency
- Cryptographic destination addressing
- Probabilistic reachability

### 6.2 Modes
- SAM Datagram (preferred)
- SAM Stream (fallback)

### 6.3 Usage
- Datagram: one message per SAM datagram
- Stream: length-prefixed CBOR frames

### 6.4 Liveness Semantics
- Soft failure interpretation
- Endpoint-specific liveness tracking
- No aggressive eviction

---

## 7. QUIC Adapter

### 7.1 Characteristics
- Connection-oriented
- Encrypted
- Multiplexed streams
- Congestion-controlled

### 7.2 Usage
- Length-prefixed CBOR messages
- One logical stream per peer recommended

### 7.3 Considerations
- Higher resource cost
- Connection establishment latency
- Useful for stable peers and bootstrap nodes

---

## 8. Endpoint Selection Policy

Endpoint selection is policy-driven.

Rules:
- Exactly one endpoint per request
- Retries MUST use same endpoint type
- No mid-request transport switching

---

## 9. Transport Failure Semantics

Failures MUST be classified as:
- transient
- suspected permanent
- unknown

Transport adapters SHOULD provide:
- failure classification hints
- RTT estimates (optional)

KAD3 core MUST treat failures conservatively.

---

## 10. Transport Independence Invariants

The following MUST hold:

- KAD3 lookup logic is transport-agnostic
- Routing table structure is identical regardless of transport
- Security validation is identical across transports

Violating these invariants is a protocol bug.

---

## 11. Implementation Notes (Non-Normative)

Recommended adapter layout:

- kad_transport_udp
- kad_transport_i2p
- kad_transport_quic

Each adapter should be independently testable.

---

## 12. Conclusion

Transport adapters are delivery mechanisms, not participants in routing.
By enforcing strict boundaries, KAD3 remains robust across heterogeneous
and adversarial networks.