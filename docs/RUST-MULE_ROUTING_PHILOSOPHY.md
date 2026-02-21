# rust-mule Routing Philosophy

## Purpose

This document describes the **routing philosophy** of `rust-mule`.

While `rust-mule` speaks the **KAD2 wire protocol** for interoperability, its
internal routing behavior is intentionally **more conservative, selective, and adaptive**
than a naïve KAD2 implementation.

This document defines *how* and *why* routing decisions are made, independent of
wire-level compatibility.

---

## Design Principles

### 1. Hostile Network Assumption

The network is assumed to be:

- Noisy
- Partially malicious
- Heavily NATed
- High-churn
- Latency-variable
- Full of legacy and broken clients

Correct behavior must emerge **despite** this environment, not because of it.

---

### 2. Compatibility ≠ Trust

Wire compatibility does **not** imply trust.

A peer may:
- Speak valid KAD2
- Respond correctly to some messages
- Still be unreliable, transient, or adversarial

All trust is **earned locally over time**, never granted by protocol compliance alone.

---

### 3. Local Knowledge Only

`rust-mule` maintains **no global truth** and participates in **no shared reputation system**.

All routing decisions are based on:
- Locally observed behavior
- Locally measured reliability
- Locally recorded history

No node’s opinion is exported or accepted.

---

## Node Classification

Each observed peer implicitly falls into one of four internal classes:

### 1. Unknown
- Newly discovered
- Unverified
- No reliability history

Unknown nodes are:
- Probed cautiously
- Used sparingly
- Never trusted with critical routing responsibility

---

### 2. Verified
- Successfully completed identity validation
- Responded correctly at least once

Verification is **necessary but insufficient** for long-term trust.

---

### 3. Stable
- Responds consistently over time
- Maintains stable latency characteristics
- Survives churn cycles

Stable nodes are:
- Preferred for routing
- Preferred for publish targets
- Retained longer in buckets

---

### 4. Unreliable
- Frequent timeouts
- High response variance
- Incorrect or empty responses

Unreliable nodes are:
- De-prioritized
- Evicted earlier
- Still tolerated at low volume

They are **not banned**, only deprioritized.

---

## Routing Table Philosophy

### Buckets Are Health-Based, Not Time-Based

Buckets are refreshed based on:
- Node responsiveness
- Timeout ratios
- Bucket density health

Not simply on fixed timers.

---

### Eviction Is Conservative but Decisive

When eviction is required:
- Unreliable nodes go first
- New but promising nodes are given opportunity
- Long-lived stable nodes are strongly protected

No eviction occurs without a *reason*.

---

### Density Is Not a Goal

A full bucket is not automatically a healthy bucket.

Quality > Quantity.

---

## Identity and Transport

### Identity Is Logically Separate from Transport

Internally, a node is represented by:
- NodeID
- Observed transports
- Capabilities
- Reliability history

Transport-specific details (UDP, I2P, etc.) are treated as **attributes**, not identity.

---

### Transport Latency Is Contextual

High latency does not imply low quality.

Latency expectations are evaluated **per transport class**:
- Direct UDP
- NAT traversal
- Anonymity networks (e.g. I2P)

Nodes are judged relative to their transport context.

---

## Request Strategy

### Cautious Parallelism

`rust-mule` favors:
- Fewer concurrent requests
- Longer patience windows
- Adaptive retry behavior

Over:
- Aggressive fan-out
- Request flooding
- Speculative retries

---

### Timeouts Are Information

A timeout is not just failure — it is a **signal**.

Timeout patterns contribute to:
- Node reliability scoring
- Future routing decisions
- Eviction prioritization

---

## Publish & Search Philosophy

### Publishing Is Earned

Nodes do not receive publish traffic merely because they are “close” in XOR space.

Preference is given to nodes that have demonstrated:
- Responsiveness
- Correct forwarding behavior
- Stability over time

---

### Search Feedback Is Remembered

Search paths that produce results are remembered internally and influence:
- Future routing order
- Publish target selection

This feedback is **local and ephemeral**, never shared.

---

## Legacy and Noise Handling

### KAD1 Traffic

- KAD1 packets are accepted at the socket level
- Parsed minimally
- Dropped politely
- Never allowed to influence routing state

KAD1 compatibility is tolerated, not supported.

---

### Garbage and Malformed Traffic

Malformed or undecipherable packets are:
- Dropped immediately
- Not logged verbosely
- Not allowed to consume routing resources

Politeness must never be expensive.

---

## Security Posture

### Sybil Resistance by Cost

`rust-mule` does not attempt to prevent Sybil nodes from joining.

Instead, it ensures that:
- Sybils are expensive to maintain
- Time, consistency, and correctness are required to gain influence

Presence is cheap. Influence is not.

---

### No Absolute Trust

No node is ever:
- Fully trusted
- Immune from eviction
- Exempt from scrutiny

Longevity raises confidence — it never grants immunity.

---

## Non-Goals

`rust-mule` explicitly does **not** attempt to provide:

- Global reputation systems
- Consensus guarantees
- Content authenticity
- Network-wide optimization
- Centralized bootstrapping authority

These are incompatible with Kademlia’s design philosophy.

---

## Closing Notes

`rust-mule` treats KAD not as a rigid protocol, but as a **living, adversarial system**.

Correctness is defined not by strict adherence to historical behavior,
but by long-term network health, stability, and resistance to abuse.

If future clients independently converge on similar internal behavior,
that evolution may one day be called “KAD3”.

Until then, this philosophy governs every routing decision.
