# iMule Compatibility vs Anonymity Contract
## Timing & Scheduling Guidance for rust-mule

This document defines how rust-mule balances **iMule protocol compatibility**
with the **Behavior Contract’s anonymity invariants**, specifically regarding
timing, scheduling, retries, and maintenance.

---

## Core Rule

> **Honor the Behavior Contract first.  
> Honor iMule compatibility within the contract’s safe envelope.**

rust-mule is **iMule-compatible**, not an iMule clone.
We match protocol semantics and timing *expectations*, not exact schedules.

---

## Compatibility Philosophy

- **Protocol semantics MUST match iMule**
  - message formats
  - state transitions
  - retry logic
  - timeout expectations

- **Timing behavior MUST satisfy the Behavior Contract**
  - randomized
  - bounded
  - non-deterministic
  - indistinguishable across nodes

If there is a conflict:
> **Compatibility adapts to the contract, not the other way around.**

---

## Three Layers of Timing (Mental Model)

### 1. Semantic Deadlines (Hard Constraints)
These exist for interoperability.

Examples:
- request timeouts
- retry windows
- bucket refresh intervals
- cooldown / ban periods

**Rules:**
- Keep the same *order of magnitude* as iMule
- Match retry *shape* (e.g. exponential backoff), not exact spacing
- Never exceed peer patience expectations

These define the **outer envelope**.

---

### 2. Scheduling Policy (Non-Negotiable)
All outbound traffic MUST pass through a centralized scheduling layer.

This layer enforces:
- base delay + jitter
- randomized ordering
- per-peer rate caps
- global rate caps

**No message may bypass this layer.**

This is the core anonymity defense.

---

### 3. Distribution Shaping (Compatibility Accent)
Within the allowed envelope, rust-mule MAY resemble iMule statistically.

Allowed:
- similar delay ranges
- similar retry cadence on average
- similar maintenance frequency *with drift*

Forbidden:
- exact delays
- exact periodicity
- deterministic retry spacing

Goal:
> Long-term observers conclude  
> “This looks like a normal node,”  
> not “this is a specific implementation.”

---

## Concrete Guidance

### What MUST match iMule
- Timeout *ranges*
- Retry *counts*
- Backoff *shape*
- Maintenance *frequency order of magnitude*

### What MUST NOT match exactly
- Absolute delays
- Retry spacing
- Refresh timestamps
- Ordering of responses
- Burst patterns

---

## Conflict Resolution Rule

If changing a timing value could cause peers to:
- give up before we respond
- retry excessively
- blacklist us as unresponsive
- fail routing convergence

➡️ **Honor iMule’s timing envelope (with jitter).**

Otherwise:
➡️ **Honor the Behavior Contract fully.**

---

## Implementation Rules (Normative)

### Central Traffic Shaper
All outbound messages MUST:
- be delayed
- be jittered
- be rate-limited
- be shuffled

Applies to:
- requests
- responses
- maintenance traffic
- error handling
- cover traffic

---

### Urgency Classes (Optional but Recommended)

Instead of message-type special cases, define urgency classes:

- **Interactive**
  - lookup replies
  - essential acknowledgements

- **Routine**
  - bucket refresh
  - keepalive
  - maintenance

- **Opportunistic**
  - cover lookups
  - extra relays

Each class:
- has different delay *ranges*
- still obeys randomness, caps, and ordering rules

---

### Timeouts & Retries

- Use iMule-compatible timeout envelopes
- Randomize per transaction slightly
- Never retry on exact schedules
- Never retry immediately after timeout

---

### Maintenance Scheduling

Forbidden:
- fixed intervals (e.g. “every 30 minutes exactly”)

Required:
- jittered intervals
- slow drift
- event-triggered maintenance that still goes through the shaper

---

## Common Fingerprinting Traps (Avoid)

- Immediate replies on cache hits
- Exact periodic refresh cycles
- Identical retry spacing
- Deterministic ordering of contacts
- Fast failures with distinct behavior
- Consistently lower latency than peers

Any of the above makes a node *observable*.

---

## Final Guideline

rust-mule aims to be:

- **Protocol-correct**
- **Timing-boring**
- **Statistically plausible**
- **Behaviorally interchangeable**

We emulate the *behavioral envelope* of iMule,
not its stopwatch.

---

End of document.