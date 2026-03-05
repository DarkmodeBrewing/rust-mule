Status: DRAFT
Last Reviewed: 2026-03-05

# KAD Trace Lookup (Debug Endpoint)

## Purpose

Provide a debug-only API that traces iterative KAD lookup behavior toward a target key and returns hop-by-hop execution data for diagnosis.

This is for operator troubleshooting and protocol verification, not end-user UI workflows.

## Proposed Endpoint

- Method: `POST`
- Path: `/api/v1/debug/trace_lookup`
- Availability: only when `[api].enable_debug_endpoints = true`
- Auth: standard bearer/session auth (same as other debug routes)
- Rate limit: strict, separate bucket from normal API traffic

## Execution Mode (chosen)

Use asynchronous execution to avoid blocking API workers during slow network traces.

Proposed control flow:

1. `POST /api/v1/debug/trace_lookup` returns `202 Accepted` with `trace_id`.
2. `GET /api/v1/debug/trace_lookup/{trace_id}` returns current state/result.
3. Optional: `DELETE /api/v1/debug/trace_lookup/{trace_id}` to cancel active traces.

State model (proposed): `queued | running | completed | failed | cancelled | expired`.

Retention/safety:

- keep bounded in-memory trace registry (`max_active_traces`)
- add TTL for completed/failed traces (`trace_ttl_secs`)
- reject new traces when capacity is exceeded

## Request Schema (proposed)

```json
{
  "target_key_hex": "0123456789abcdef0123456789abcdef",
  "max_hops": 16,
  "parallelism": 3,
  "timeout_ms": 15000
}
```

Constraints:

- `target_key_hex` must be 128-bit hex (32 chars).
- `max_hops` bounded (for example `1..=32`).
- `parallelism` bounded (for example `1..=8`).
- `timeout_ms` bounded (for example `500..=60000`).

## Response Schema (proposed)

```json
{
  "target_key_hex": "0123456789abcdef0123456789abcdef",
  "started_at": "2026-03-05T12:00:00Z",
  "duration_ms": 1243,
  "status": "completed",
  "best_distance_hex": "0000000000000000000000000000000f",
  "visited_count": 27,
  "timeouts": 2,
  "hops": [
    {
      "hop": 0,
      "queried_peer_key_hex": "....",
      "queried_peer_dest_b64": "....",
      "distance_hex": "....",
      "rtt_ms": 87,
      "returned_contacts": 16,
      "closer_contacts": 4,
      "error": null
    }
  ]
}
```

`status` values (proposed): `completed | timeout | hop_limit | no_closer_contacts | aborted`.

## Runtime Behavior

1. Validate input and enforce bounds.
2. Seed shortlist with closest local routing contacts to `target_key_hex`.
3. Execute iterative lookup with bounded concurrency (`parallelism`):
   - send lookup requests to shortlist candidates not yet queried
   - on response, parse and filter returned contacts
   - update shortlist and best-distance tracker
   - emit hop record for each query attempt (including errors/timeouts)
4. Stop when one stop condition occurs:
   - global timeout reached
   - no closer contacts discovered
   - hop limit reached
   - exact/near-enough target convergence reached
5. Return trace summary + hop list.

## Integration Notes

- Reuse existing KAD service lookup machinery where possible.
- Implement as a debug command path in `kad::service`, not as standalone socket logic in API layer.
- Keep network I/O in transport/service layers; API handler only validates input and maps result to JSON.

## Security and Safety Requirements

- Debug feature-flag gate is mandatory.
- Enforce per-request bounds regardless of client input.
- Add dedicated rate-limit bucket to avoid traffic amplification.
- Do not leak internal secrets in response payloads.
- Return normalized errors (`{code,message}`) on validation/runtime failures.

## Observability

Add counters/logs for:

- trace requests started/completed/failed/timed_out
- total queried peers per trace
- average trace duration
- rejected traces by validation/rate-limit/debug-disabled

## Test Plan (minimum)

- Unit:
  - request validation bounds and target key format
  - stop-condition precedence
  - response shape and status mapping
- Integration:
  - endpoint available only when debug endpoints enabled
  - bounded runtime under forced timeout
  - deterministic behavior in small mocked lookup topology

## Rollout Plan

1. Implement service-level trace collector with internal command.
2. Add API handler and route under `/api/v1/debug`.
3. Add tests + counters.
4. Document operator usage in `docs/30_operations/api_curl.md`.
