# AI Meta

This file defines shared terms, severity levels, and security/protocol assumptions used by skills.

## Severity Levels

- HIGH: Exploitable vulnerability, serious data loss, remote crash, auth bypass, key compromise, downgrade to insecure mode, or high-probability DoS.
- MEDIUM: Meaningful weakness requiring additional conditions, moderate DoS, unsafe defaults, or footguns likely to be misused.
- LOW: Hard-to-exploit issues, best-practice gaps, minor information leaks, or robustness improvements.

## Finding Categories (use one)

- crypto
- protocol
- input-validation
- authn/authz
- dos/resource
- memory-safety
- concurrency/async
- privacy/metadata
- logging/observability
- supply-chain
- config/ops

## Evidence Guidelines

Evidence should be concrete:

- reference a function/type name, constant, or specific behavior
- describe trigger conditions and expected vs actual behavior
- avoid speculation; if uncertain, label as "hypothesis" and explain what to verify

## Protocol Design Goals (rust-mule context)

- Preserve KAD2/iMule compatibility unless explicitly breaking for KAD3.
- Avoid introducing downgrade paths.
- Capability negotiation must be:
  - explicit (not inferred),
  - authenticated (or integrity-protected),
  - replay-resistant where relevant,
  - safe under partial deployment (mixed versions).

## Threat Model (baseline)

Assume adversaries can:

- observe and inject packets (on-path or adjacent, depending on transport)
- replay packets
- spoof endpoints unless authenticated
- attempt CPU/memory exhaustion
- attempt downgrade/feature-confusion attacks during handshake/negotiation
- enumerate metadata (timing, identifiers, protocol fingerprints)
