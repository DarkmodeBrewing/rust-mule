# SAM Transport State Machine

Last Reviewed: 2026-03-17

## Purpose

Define a stricter SAM transport/session lifecycle for `rust-mule` so runtime
degradation is detected, surfaced, and recovered explicitly instead of being
treated as an optimistic one-shot reconnect.

This note is based on:

- observed alpha failures where a client logged `SAM DATAGRAM session ready` but
  did not appear to recover fully at the router/session level
- review of `source_ref/yosemite`, which models SAM as an explicit protocol
  state machine with strict response validation

This is not a request to copy `yosemite`. It is a design extraction note.

## Problem

Current `rust-mule` behavior can be too optimistic after runtime SAM failure:

- control channel framing can fail
- reconnect/recreate logic runs
- `SESSION STATUS RESULT=OK` may be treated as sufficient
- the client can still appear effectively inert afterwards

This creates a bad operator state:

- process alive
- UI/API may still look mostly healthy
- KAD transport may no longer be functionally usable

## Design Goals

1. Model SAM control/session lifecycle explicitly.
2. Distinguish control-plane health from datagram-plane health.
3. Distinguish "session created" from "transport verified healthy".
4. Make degraded state visible in logs, `/api/v1/status`, and UI.
5. Make recovery bounded, observable, and retryable.

## Proposed State Model

Suggested transport states:

1. `Disconnected`
  - no SAM control connection
  - no session

2. `HelloPending`
  - TCP control connection established
  - awaiting `HELLO REPLY`

3. `SessionCreatePending`
  - `HELLO` succeeded
  - awaiting `SESSION CREATE` result

4. `SessionCreated`
  - router accepted session creation
  - destination exists
  - not yet verified as usable

5. `DatagramReady`
  - datagram socket/session exists
  - local send path is initialized
  - still not enough to call the transport healthy by itself

6. `TransportVerifying`
  - waiting for post-create proof of usable transport
  - examples:
    - successful datagram send path
    - expected inbound KAD packet
    - successful HELLO / bootstrap / request-response traffic

7. `Healthy`
  - SAM control channel healthy
  - datagram transport healthy
  - recent KAD traffic proves usability

8. `Degraded`
  - process still running
  - transport/session partially unavailable or uncertain
  - automatic recovery in progress or pending

9. `Recovering`
  - old session being torn down or replaced
  - retry/backoff active

## Required Health Signals

Track separately:

1. `control_connected`
  - SAM TCP control connection is alive

2. `session_created`
  - router accepted `SESSION CREATE`

3. `datagram_ready`
  - local datagram plumbing exists

4. `transport_verified`
  - post-create proof that the session is actually usable

5. `last_transport_ok_unix_secs`
  - last known successful transport-level activity

6. `last_transport_error`
  - structured last failure cause

7. `recovery_attempts`
  - current bounded retry count

## Verification Requirement

Do not treat:

- `SESSION STATUS RESULT=OK`

as sufficient proof of recovery by itself.

Recovery should only transition to `Healthy` after a post-create verification
condition passes.

Candidate verification signals:

1. first successful outbound KAD request after recreation
2. first successful inbound KAD packet after recreation
3. first successful expected request/response pair after recreation

The exact probe can vary, but the key rule is:

- "session created" is not the same as "transport healthy"

## Response Validation

SAM response handling should be tied to expected state:

- `HELLO REPLY` only valid in `HelloPending`
- `SESSION STATUS` only valid in `SessionCreatePending` or explicit destroy/add
  paths
- unexpected responses should be treated as protocol/state errors, not silently
  accepted

This follows the useful pattern seen in `yosemite`:

- strict parser
- explicit controller state
- command/response pairing

## Failure Classification

Recovery/logging/status should distinguish at least:

1. `duplicate_id`
2. `duplicate_destination`
3. `control_framing_error`
4. `router_disconnect`
5. `session_create_rejected`
6. `tunnel_build_failed`
7. `verification_timeout`

These should appear as structured causes in runtime state and logs.

## Logging and Status Surface

At `info` level, operator-visible events should include:

- transport degraded
- recovery started
- session destroy/recreate attempted
- recovery succeeded
- recovery failed after retries

At `debug` level, include:

- raw SAM response classification
- retry backoff details
- short destination/session fingerprint

Status/UI should expose something like:

- `sam_state`
- `kad_transport_ready`
- `kad_transport_degraded`
- `last_transport_error`
- `recovery_attempts`

## Recovery Flow

Suggested recovery sequence:

1. detect control/datagram failure
2. mark `Degraded`
3. close old control/session resources locally
4. attempt explicit destroy if meaningful
5. reconnect TCP control
6. `HELLO`
7. `SESSION CREATE`
8. initialize datagram path
9. enter `TransportVerifying`
10. only mark `Healthy` after verification succeeds

If verification fails:

- remain `Degraded`
- retry with bounded backoff
- expose failure clearly

## Non-Goals

This note does not require:

- adopting `yosemite` APIs or code shape
- adding SAM subsessions right now
- redesigning KAD itself

The immediate goal is runtime correctness and observability.

## Why This Matters

Without this, long-running clients can:

- lose effective SAM/KAD transport
- appear active
- stop participating in the network meaningfully

That is a correctness and soak-test reliability problem, not just a logging
polish issue.
