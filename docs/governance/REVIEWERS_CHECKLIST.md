# Reviewer Checklist (Mandatory for All PRs)

This checklist MUST be reviewed for every pull request that:

- touches networking
- affects message flow
- alters timing, retries, or scheduling
- introduces optimizations
- refactors protocol logic
- changes error handling
- adds caching, batching, shuffling, or shortcuts
- modifies routing behavior
- touches `src/kad/**`

If any answer below is **“Yes”**, the PR MUST be rejected or explicitly justified in the PR description.

---

# 1. Timing & Scheduling (Privacy Critical)

☐ Does this introduce any immediate sends?  
☐ Does it reduce, bypass, or special-case message delays?  
☐ Does it make one message type observably faster than others?  
☐ Does it react faster under low load or idle conditions?  
☐ Does it change behavior based on CPU speed or async wake timing?  
☐ Does it alter retry cadence or backoff patterns?

**Rule:**

> Outbound behavior must remain timing-homogeneous and hardware-agnostic.

---

# 2. Message Ordering & Entropy

☐ Does this preserve deterministic iteration order?  
☐ Does it reduce shuffling or randomization?  
☐ Does it respond in the same order requests were received?  
☐ Does it make bucket traversal predictable?  
☐ Does it remove randomness for "efficiency"?

**Rule:**

> Observable ordering must remain unstable and non-deterministic.

---

# 3. Participation Symmetry

☐ Does this reduce routing participation?  
☐ Does it avoid relays because “not strictly required”?  
☐ Does it make client-only behavior cheaper than routing behavior?  
☐ Does it reduce cover traffic?  
☐ Does it bias behavior based on node role?

**Rule:**

> Nodes must remain equally useful and equally lazy.

No optimization may reduce network contribution asymmetrically.

---

# 4. Failure & Error Surface

☐ Does this distinguish failure causes more precisely on the wire?  
☐ Does it introduce more specific error codes?  
☐ Does it reject malformed input earlier than before?  
☐ Does it expose internal state transitions in logs or protocol replies?  
☐ Does it change retry/timeout behavior based on failure type?

**Rule:**

> Failures must degrade slowly and indistinguishably.

Externally observable failure modes must not increase fingerprintability.

---

# 5. Rate & Resource Bounds (DoS Guardrail)

☐ Does this remove or relax hard caps?  
☐ Does it allow unbounded collection growth?  
☐ Does it scale behavior with CPU/memory availability?  
☐ Does it spawn tasks without explicit bounds?  
☐ Does it increase per-packet computational cost?  
☐ Does it increase amplification (small request → large response)?

**Rule:**

> Activity rates must remain bounded and hardware-agnostic.

All attacker-influenced resource usage must have explicit upper limits.

---

# 6. Identity & Persistence Stability

☐ Does this cause more frequent reconnects?  
☐ Does it regenerate identity or keys more aggressively?  
☐ Does it alter bucket decay or refresh cadence?  
☐ Does it make lifecycle transitions externally observable?  
☐ Does it accelerate state churn?

**Rule:**

> Node identity and routing state must decay slowly and predictably.

---

# 7. Protocol Upgrade & Downgrade Safety

☐ Does this introduce new capability negotiation?  
☐ Could a peer strip or suppress capability signals?  
☐ Does this allow silent fallback to legacy behavior?  
☐ Is negotiation bound to identity and nonce?  
☐ Are upgraded semantics versioned explicitly?

**Rule:**

> No silent downgrade.  
> If both peers support stronger mode, it must be used.

---

# 8. Fingerprinting Risk (Critical)

☐ Does this introduce new observable behavior?  
☐ Could long-term observation distinguish rust-mule?  
☐ Does it alter message cadence, entropy, or response symmetry?  
☐ Would this change look “clever” or “efficient” on the wire?

**Rule:**

> Clever behavior is suspicious behavior.

Indistinguishability from the baseline network is prioritized over efficiency.

---

# 9. Optimization Sanity Gate

☐ Is this primarily an optimization?  
☐ Is the optimization externally observable?

If BOTH are true → ❌ REJECT unless justified with:

- explicit anonymity impact analysis
- confirmation via baseline capture
- documented invariants unchanged

**Rule:**

> Anonymity-preserving inefficiency is intentional.

---

# 10. KAD/Wire Baseline Evidence Gate

For PRs touching `src/kad/**`:

☐ Includes before/after baseline artifact paths from `scripts/test/kad_phase0_baseline.sh`  
☐ PR description summarizes deltas for:

- `pending_overdue`
- `pending_max_overdue_ms`
- `tracked_out_requests`
- `tracked_out_matched`
- `tracked_out_unmatched`
- `tracked_out_expired`
  ☐ Any significant drift is explicitly explained and tied to contract-compatible intent

---

# Reviewer Declaration

Before approving, the reviewer MUST be able to state:

> “This change does not meaningfully alter observable timing, ordering, rate,
> routing symmetry, downgrade safety, or failure surface.”

If this cannot be stated confidently, the PR MUST NOT be merged.

---
