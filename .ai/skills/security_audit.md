# Skill: SecurityAudit

## Purpose

Perform a security-focused review of a bounded scope of code (file(s), module(s), or a PR).
Identify vulnerabilities, attack surfaces, and unsafe assumptions relevant to a networked P2P client.

## When to Use

- Reviewing KAD networking, message parsing/serialization, routing table logic
- Reviewing I2P SAM integration, session handling, stream/datagram boundaries
- Reviewing config loading, persistence, file IO of protocol artifacts (nodes.dat, keys, caches)
- Reviewing crypto-related code (hashing, MACs, key derivation, secrets)

## Scope Checklist

Focus on:

- **Untrusted input handling**: parsing, bounds checks, type conversions, length prefixes
- **Crypto misuse**: weak primitives, nonces, secrets too small, predictable randomness
- **Authentication/integrity**: spoofing, lack of MAC/signatures where needed
- **Replay resistance**: token lifetimes, monotonic counters, timestamp validation
- **DoS vectors**: unbounded loops, allocations, map growth, per-packet expensive ops
- **Async/concurrency**: races, cancellation hazards, deadlocks, shared state invariants
- **Information leaks**: logs, error messages, protocol fingerprinting, identifiers in clear
- **File/FS**: path traversal, permissions, atomic writes, partial writes, corrupted state recovery
- **Dependency risks**: unsafe crates, known footguns, feature flags enabling risky behavior

## Constraints

- Do **NOT** modify code.
- Do **NOT** provide patches unless explicitly asked.
- Prefer concrete findings. If you must speculate, label as "hypothesis" + how to verify.

## Output Format

Return a list of findings using this template:

### Finding <n>: <short title>

- severity: HIGH | MEDIUM | LOW
- category: (from `.ai/meta.md`)
- location: <file>:<line range> (best effort)
- impact: <what goes wrong, who can exploit, what they gain>
- evidence: <what in code/behavior supports this>
- recommendation: <conceptual fix, constraints, tradeoffs>
- verification: <how to test or reproduce / what to log>

## Extra: rust-mule Specific Red Flags

Call out explicitly if you see any of these:

- secrets <= 64 bits used for integrity/authentication
- MD5 or CRC used as a security boundary (not just non-adversarial checks)
- "capability inferred from behavior" instead of explicit negotiated flags
- any "accept if parsing fails" or fallback to insecure mode
- any unbounded parsing into Vec/HashMap based on attacker-controlled lengths
- per-packet expensive operations (KDF, signature verify) without rate limiting
- routing table growth without eviction / quotas / per-peer caps
