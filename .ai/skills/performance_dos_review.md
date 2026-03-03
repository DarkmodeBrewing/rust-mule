# Skill: PerformanceDosReview

## Purpose

Review code for performance pitfalls and resource-exhaustion / DoS vectors in a packet-heavy, networked app.
This is not a full security audit: it focuses on **cost**, **bounds**, **amplification**, and **resilience**.

## When to Use

- Any PR touching packet parsing, routing tables, caches, queues, timers, async tasks
- Anything on hot paths: per-packet handlers, periodic maintenance loops
- Any change that adds new collections keyed by peer/ID, or new background tasks

## Key Questions (always answer)

1. What attacker-controlled inputs influence CPU, memory, disk, or outbound traffic?
2. Are there hard bounds on:
   - allocations (`Vec::with_capacity`, `Bytes`, buffers)
   - collection growth (maps/sets/lists)
   - spawned tasks / outstanding futures
   - retries/timeouts/backoff
3. Is there a path for amplification?
   - small request → huge response
   - one packet → many lookups / disk writes / network fanout
4. Are expensive operations rate-limited or cached?
5. What happens under sustained packet loss and reordering?

## Scope Checklist

Focus on:

- **Parsing costs**
  - length-prefixed fields validated before allocation/copy
  - avoid `to_vec()`/`clone()` on hot paths
  - prefer `Bytes` / slice views when possible (only if already used)
- **Collection growth**
  - eviction policies (LRU/TTL), quotas per peer, global caps
  - avoid “unbounded HashMap keyed by attacker input”
  - ensure maintenance tasks actually shrink structures
- **Timers & retries**
  - exponential backoff + jitter (or at least backoff)
  - no tight loops on failure
  - cap max in-flight requests per peer and globally
- **Work scheduling**
  - avoid spawning per-packet tasks without a limit
  - prefer worker loops / bounded channels
  - ensure cancellation is handled (select/timeout)
- **Disk IO**
  - avoid writing to disk per packet / per event
  - ensure atomic writes for periodic snapshots
  - validate file sizes before reading into memory
- **Logging cost**
  - avoid per-packet `info!` logs in hot paths
  - ensure debug logs are behind appropriate levels

## Constraints

- Do NOT modify code.
- Provide concrete bounds you recommend (numbers) when possible, but make it clear they’re defaults to tune.
- If you detect a pure vulnerability (auth/integrity/crypto), recommend running `SecurityAudit`.

## Output Format

Return:

### A) Hot Path Map

- Identify the likely hot functions/loops touched (names + why they are hot)

### B) Findings

Use this template:

#### Finding <n>: <short title>

- severity: HIGH | MEDIUM | LOW
- category: dos/resource | concurrency/async | input-validation | logging/observability | config/ops
- location: <file>:<line range> (best effort)
- impact: <CPU spike / memory growth / queue buildup / disk churn / amplification>
- evidence: <what in code supports this>
- recommendation: <bounded strategy: caps, eviction, backoff, batching>
- verification: <benchmark idea / stress test / metrics to add>

### C) Suggested Metrics (optional but encouraged)

List counters/gauges/histograms that would prove you’re safe:

- in-flight requests (global, per-peer)
- queue depths
- routing table size / cache sizes
- dropped packets by reason
- parse failures by type
- retry counts / timeout counts
- per-message handler duration histogram (if available)

## Default Bound Recommendations (starting points)

These are conservative defaults to propose if none exist:

- max in-flight requests per peer: 8–32
- max in-flight requests global: 1k–10k (depending on node size)
- max contacts processed per inbound message: 64–256
- max routing table growth per hour without eviction: 0 (must have eviction)
- max accepted message size: set explicitly (protocol-specific)
- retry backoff: base 250–500ms, cap 30–60s, include jitter
