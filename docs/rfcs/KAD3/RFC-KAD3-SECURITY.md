# RFC-KAD3-SECURITY

## Threat Model and Security Considerations for KAD3

**Status:** Draft  
**Category:** Informational  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Abstract

This document defines the threat model, security assumptions, and
mitigations for KAD3.

KAD3 is designed to operate in hostile, adversarial environments and
explicitly assumes the presence of malicious nodes. This document
describes what KAD3 mitigates, what it does not, and why those boundaries
exist.

---

## 2. Security Philosophy

KAD3 follows these principles:

- Assume the network is hostile
- Assume peers may lie, disappear, or collude
- Prefer containment over prevention
- Prefer verifiable facts over reputation
- Never silently trust legacy systems
- Token/cookie scheme MUST use HMAC-SHA256; token length MUST be ≥ 96 bits; MD5 MUST NOT be used.

KAD3 intentionally avoids claims of “full security” or “Sybil resistance”.

---

## 3. Trust Assumptions

KAD3 makes the following minimal assumptions:

- Cryptographic primitives are secure
- Local node implementation is not compromised
- Randomness used for key generation is sufficient

No assumptions are made about:

- peer honesty
- peer longevity
- network topology
- transport reliability

---

## 4. Adversary Model

KAD3 assumes adversaries may:

- Operate arbitrary numbers of nodes
- Send malformed or misleading messages
- Attempt routing table poisoning
- Attempt eclipse attacks
- Perform traffic analysis
- Exploit transport-specific weaknesses
- Replay previously valid messages

Adversaries are assumed to be computationally bounded.

---

## 5. Threat Categories

### 5.1 Identity Spoofing

**Threat:**  
A node claims a NodeID not bound to its cryptographic identity.

**Mitigation:**

- NodeID MUST be derived from public key
- All messages MUST be signed
- NodeID/public-key mismatch MUST be rejected

Residual risk: none (assuming crypto holds).

---

### 5.2 Routing Table Poisoning

**Threat:**  
Insertion of malicious contacts to bias lookup results.

**Mitigations:**

- Bucket size limits
- Replacement-by-verification
- Liveness tracking
- Contact diversity constraints (RECOMMENDED)
- Strict separation of hints vs verified contacts

Residual risk: partial (bounded by bucket size and diversity rules).

---

### 5.3 Eclipse Attacks

**Threat:**  
Adversary surrounds a node with controlled peers.

**Mitigations:**

- XOR distance distribution
- Bucket diversity rules
- Endpoint diversity
- Slow promotion of new contacts
- No trust inheritance from legacy bridges

Residual risk: present but reduced; global Sybil attacks remain possible.

---

### 5.4 Sybil Attacks

**Threat:**  
Adversary creates many identities to gain influence.

**Mitigation:**  
KAD3 does not claim to solve Sybil attacks.

Optional mitigations (non-normative):

- Proof-of-work admission
- Rate limits
- Diversity heuristics
- Cost asymmetry via transports (e.g., I2P)

Residual risk: high (by design).

---

### 5.5 Replay Attacks

**Threat:**  
Previously valid messages are replayed to manipulate state.

**Mitigations:**

- Timestamps or nonces in message envelope
- Replay window enforcement
- Request correlation via request_id

Residual risk: low.

---

### 5.6 Message Tampering

**Threat:**  
Messages altered in transit.

**Mitigations:**

- Cryptographic signatures over canonical CBOR encoding
- Strict envelope validation

Residual risk: none (assuming crypto holds).

---

### 5.7 Transport-Level Attacks

**Threats include:**

- Packet loss
- Delays
- Reordering
- Blackholing
- Endpoint exhaustion

**Mitigations:**

- Transport abstraction
- Soft liveness scoring
- Retry with backoff
- No assumption of reliable delivery

Residual risk: unavoidable; tolerated by design.

---

### 5.8 Bridge-Induced Attacks (KAD2 → KAD3)

**Threat:**  
Legacy network injects malicious data.

**Mitigations:**

- Hint cache separation
- Mandatory KAD3 HELLO verification
- No direct routing table insertion
- Rate limiting and filtering

Residual risk: low if bridge rules are enforced.

---

## 6. What KAD3 Does NOT Protect Against

KAD3 does not attempt to protect against:

- Global Sybil attacks
- Traffic analysis
- Metadata correlation
- Malicious content distribution
- Application-layer abuse

These are explicitly out of scope.

---

## 7. Transport-Specific Considerations

### 7.1 UDP

- Susceptible to spoofing (mitigated by signatures)
- Low latency, low reliability

### 7.2 I2P

- Strong anonymity
- High latency
- No inherent trust

### 7.3 QUIC/TCP

- Stateful connections
- Susceptible to resource exhaustion

Transport choice affects threat surface but not KAD3 semantics.

---

## 8. Implementation Guidance (Non-Normative)

Implementers SHOULD:

- Fail closed on validation errors
- Log security-relevant events
- Keep routing and security logic auditable
- Avoid implicit trust shortcuts

---

## 9. Conclusion

KAD3 prioritizes containment, verification, and explicit trust boundaries.
It does not promise perfect security, but it ensures failures are local,
detectable, and bounded.
