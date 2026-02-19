# rust-mule Behavior Contract

## Anonymity-Preserving Invariants for KAD over I2P

> This document defines **non-negotiable behavioral invariants** for rust-mule.
> These rules exist to preserve anonymity, resist fingerprinting, and ensure
> that all correct nodes are **behaviorally interchangeable**.

Performance, efficiency, and elegance are always secondary to these invariants.

---

## Core Principle

> **All correct nodes must be behaviorally indistinguishable on the wire.**

Anonymity in KAD-style networks does **not** come from scale alone.
It comes from:

- homogeneity
- predictability at the protocol level
- unpredictability at the timing and ordering level

rust-mule intentionally prioritizes **boring behavior**.

---

## Threat Model Summary

This contract defends primarily against:

- passive traffic analysis
- active fingerprinting
- timing correlation
- protocol differentiation
- low-to-medium Sybil presence

It does **not** attempt to defeat:

- global adversaries with full tunnel compromise
- endpoint compromise
- hostile runtime environments

---

## Invariant Categories

1. Timing
2. Message Ordering
3. Participation Symmetry
4. Error & Failure Handling
5. Rate Limiting
6. Persistence & Identity
7. Anti-Optimization Rule

All categories are **mandatory**.

---

## 1. Timing Invariants

### Rule

All outbound network messages MUST be delayed by a randomized window.

### Requirements

- No immediate sends
- No deterministic delays
- No fast paths for “cheap” messages

### Properties

- Delay = base + jitter
- Same logic for all message types
- Distribution MUST be bounded

### Rationale

Prevents:

- RTT fingerprinting
- CPU / runtime performance leaks
- “this node is faster” classification

---

## 2. Message Ordering Invariants

### Rule

Outbound messages MUST NOT preserve logical or processing order.

### Requirements

- Messages are enqueued, shuffled, then sent
- Applies to:
  - lookup responses
  - publish acknowledgements
  - peer lists
  - maintenance traffic

### Rationale

Prevents:

- implementation fingerprinting
- language/runtime detection
- bucket traversal inference

---

## 3. Participation Symmetry Invariant

### Rule

Nodes MUST appear equally useful and equally lazy.

### Requirements

- Routing participation MUST be symmetric with client activity
- Nodes SHOULD generate cover lookups unrelated to their own interests
- Routing traffic MUST NOT be minimized based on “not strictly needed”

### Rationale

Prevents:

- role-based deanonymization
- “pure router” vs “pure client” classification
- low-effort traffic analysis

---

## 4. Error & Failure Handling Invariants

### Rule

Failures MUST degrade slowly and identically.

### Requirements

- No descriptive error messages
- No early rejection
- No fast failure paths

### Handling Guidelines

- Unknown message → delay → silent drop
- Malformed message → delay → generic failure
- Unreachable peer → indistinguishable from congestion

### Rationale

Errors are high-entropy fingerprinting vectors.
Uniform failure behavior collapses attacker signal.

---

## 5. Rate Limiting Invariants

### Rule

All activity MUST be capped by hard upper bounds.

### Requirements

- Fixed maximums for:
  - lookups per time window
  - publishes per time window
  - responses per peer
- Limits MUST NOT scale with CPU, memory, or uptime

### Rationale

Prevents:

- performance-based fingerprinting
- Sybil amplification
- “enthusiastic node” identification

---

## 6. Persistence & Identity Invariants (KAD-Specific)

### Rule

Nodes MUST appear long-lived and decay slowly.

### Requirements

- Gradual bucket aging
- Graceful shutdown behavior
- No abrupt identity disappearance when possible

### Rationale

KAD assumes persistence.
Sudden churn leaks identity transitions and restart events.

---

## 7. Anti-Optimization Rule (Critical)

### Rule

No optimization is permitted if it violates any invariant.

### Mandatory Review Question

Before introducing any change, ask:

> Does this alter timing, ordering, rate, participation symmetry, or failure behavior?

If yes → the change MUST NOT be merged.

### Rationale

Most anonymity failures are introduced by “harmless” optimizations.

Inefficiency is not a bug — it is a defense.

---

## Implementation Guidance (Non-Normative)

- Use centralized delay queues for outbound traffic
- Prefer uniform schedulers over event-driven immediacy
- Avoid conditional behavior based on local state when observable
- Treat protocol boredom as a feature

---

## Testing Expectations

rust-mule SHOULD include tests or harnesses that:

- detect timing variance drift
- detect ordering stability
- flag behavior dependent on load or hardware

Behavioral regressions are security regressions.

---

## Final Statement

rust-mule does not attempt to be the fastest, smartest, or most efficient node.

It attempts to be:

> **indistinguishable, persistent, and aggressively average**

This is the foundation of practical anonymity in KAD-style networks.

---

End of document.
