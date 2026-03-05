Status: DRAFT
Last Reviewed: 2026-03-05

# Debug Bootstrap Restart Endpoint

## Purpose

Provide a debug-only API operation to force a bootstrap refresh cycle without restarting the process.

This endpoint is operational/diagnostic and should not be exposed as a normal user workflow.

## Endpoint Shape (planned)

- `POST /api/v1/debug/bootstrap/restart`
  - returns `202 Accepted` with `job_id`
- `GET /api/v1/debug/bootstrap/jobs/{job_id}`
  - returns job status/result
- optional `DELETE /api/v1/debug/bootstrap/jobs/{job_id}`
  - cancel queued/running job when possible

## Security

- Only available when `[api].enable_debug_endpoints = true`.
- Require standard auth plus debug second-factor:
  - config: `api.debug_token`
  - header: `X-Debug-Token`
- Response behavior:
  - debug disabled: `404`
  - debug enabled + missing/invalid debug token: `403`

## Execution Model (chosen)

- Asynchronous execution only (`202 + job_id`), to avoid blocking API workers.
- Job states: `queued | running | completed | failed | cancelled | expired`.

## Runtime Guardrails

- single-flight bootstrap restart (only one active at a time)
- cooldown window between accepted runs
- bounded in-memory job registry (`max_active_jobs`)
- TTL cleanup for terminal jobs (`job_ttl_secs`)
- explicit rejection when at capacity or cooldown

## Observability

Track counters/events for:

- bootstrap restart requested/accepted/rejected
- rejection reasons (`cooldown`, `already_running`, `rate_limited`, `capacity`)
- completed/failed/cancelled jobs and duration

## Test Plan (minimum)

- endpoint is unavailable when debug endpoints are disabled
- debug token enforcement (`404/403` behavior)
- returns `202` with job id
- single-flight and cooldown behavior
- job status endpoint returns lifecycle transitions
