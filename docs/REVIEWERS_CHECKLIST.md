---

## Reviewer Checklist (Mandatory for All PRs)

This checklist MUST be reviewed for every pull request that:
- touches networking
- affects message flow
- alters timing, retries, or scheduling
- introduces optimizations
- refactors protocol logic
- changes error handling
- adds caching, batching, or shortcuts

If any answer below is **“Yes”**, the PR MUST be rejected or revised.

---

## 1. Timing & Scheduling

☐ Does this change introduce any immediate sends?  
☐ Does it reduce, bypass, or special-case message delays?  
☐ Does it make any message type faster than others?  
☐ Does it react faster under low load or idle conditions?  
☐ Does it change behavior based on CPU speed or async wakeups?

**Rule:**

> All outbound messages must remain delayed and timing-homogeneous.

---

## 2. Message Ordering

☐ Does this preserve request or processing order?  
☐ Does it iterate buckets, peers, or lists deterministically?  
☐ Does it reduce or remove shuffling steps?  
☐ Does it respond in the same order requests were received?

**Rule:**

> Observable ordering must remain randomized and unstable.

---

## 3. Participation Symmetry

☐ Does this reduce routing participation?  
☐ Does it avoid “unnecessary” lookups or relays?  
☐ Does it make client-only behavior cheaper than routing?  
☐ Does it skip cover activity because “it’s not needed”?

**Rule:**

> Nodes must remain equally useful and equally lazy.

---

## 4. Error & Failure Behavior

☐ Does this add descriptive error messages?  
☐ Does it reject malformed input earlier than before?  
☐ Does it distinguish between failure causes on the wire?  
☐ Does it log or expose protocol errors differently per case?

**Rule:**

> Failures must degrade slowly and indistinguishably.

---

## 5. Rate & Throughput

☐ Does this increase throughput under good conditions?  
☐ Does it scale behavior with available resources?  
☐ Does it increase message volume opportunistically?  
☐ Does it remove or relax hard caps?

**Rule:**

> Activity rates must remain bounded and hardware-agnostic.

---

## 6. Persistence & Identity

☐ Does this cause more frequent reconnects?  
☐ Does it reset identity, state, or buckets more aggressively?  
☐ Does it make restarts faster or more abrupt?  
☐ Does it expose lifecycle transitions externally?

**Rule:**

> Node identity must decay slowly and predictably.

---

## 7. Fingerprinting Risk (Critical)

☐ Does this introduce any new observable behavior?  
☐ Could an attacker detect this change with long-term observation?  
☐ Does this make rust-mule easier to distinguish from other nodes?  
☐ Would this change look “clever” or “efficient” on the wire?

**Rule:**

> Clever behavior is suspicious behavior.

---

## 8. Optimization Sanity Check

☐ Is this change primarily an optimization?  
☐ Is the optimization observable externally?

If **both** are true → ❌ REJECT.

**Rule:**

> Anonymity-preserving inefficiency is intentional.

---

## Reviewer Declaration

Before approving, the reviewer MUST be able to say:

> “This change does not alter observable timing, ordering, rate,
> participation symmetry, or failure behavior in any meaningful way.”

If this cannot be stated confidently, the PR MUST NOT be merged.

## KAD/Wire Baseline Evidence Gate

For PRs touching `src/kad/**` or KAD wire behavior:

☐ PR includes before/after baseline capture artifact paths from `scripts/test/kad_phase0_baseline.sh`.  
☐ PR description summarizes deltas for:
- `pending_overdue`, `pending_max_overdue_ms`
- `tracked_out_requests`, `tracked_out_matched`, `tracked_out_unmatched`, `tracked_out_expired`
☐ Any significant drift is explained and tied to explicit contract-compatible intent.

---

End of checklist.
