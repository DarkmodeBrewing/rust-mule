# RFC-KAD3-POLICY

## Operational Policy Profiles for KAD3

**Status:** Draft  
**Category:** Informational  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Abstract

This document defines operational policy parameters for KAD3.

KAD3 semantics are transport-agnostic and fixed.
Operational behavior (timeouts, retries, eviction thresholds, etc.)
is controlled by policy profiles.

This separation allows KAD3 to operate correctly over heterogeneous
transports such as UDP, I2P, and QUIC without altering core protocol rules.

---

## 2. Design Principles

1. Semantics MUST remain transport-agnostic.
2. Policy MAY vary per transport.
3. Policy MUST NOT change:
   - XOR distance metric
   - bucket indexing
   - message structure
   - handshake validation
4. Policy SHOULD be configurable at runtime.

---

## 3. Policy Categories

Policy parameters are grouped into the following categories:

- Lookup behavior
- Timeout and retry behavior
- Liveness scoring
- Admission control
- Endpoint selection
- Bucket diversity

---

## 4. Lookup Policy Parameters

| Parameter                 | Description                   |
| ------------------------- | ----------------------------- |
| `alpha`                   | Parallel lookup fan-out       |
| `max_concurrent_requests` | Upper bound of in-flight RPCs |
| `lookup_timeout_ms`       | Max time for full lookup      |
| `per_request_timeout_ms`  | Timeout per RPC               |

---

## 5. Liveness Policy Parameters

| Parameter              | Description                       |
| ---------------------- | --------------------------------- |
| `failure_threshold`    | Failures before demotion          |
| `decay_half_life_secs` | Liveness score decay period       |
| `probe_interval_secs`  | Interval between health probes    |
| `soft_failure_weight`  | Penalty for transient failure     |
| `hard_failure_weight`  | Penalty for confirmed unreachable |

Liveness MUST be tracked per endpoint.

---

## 6. Admission Control Policy

| Parameter                        | Description                         |
| -------------------------------- | ----------------------------------- |
| `max_new_contacts_per_minute`    | Rate limit on new contact promotion |
| `require_handshake`              | Always true (normative requirement) |
| `pow_required`                   | Optional proof-of-work requirement  |
| `max_hints_processed_per_minute` | KAD2 bridge safety limit            |

---

## 7. Endpoint Selection Policy

| Parameter                  | Description                        |
| -------------------------- | ---------------------------------- |
| `preferred_transports`     | Ordered list of transports         |
| `allow_transport_fallback` | Whether to try alternate transport |
| `sticky_transport`         | Lock transport per request         |

Rules:

- Exactly one endpoint MUST be selected per request.
- Retries MUST use same transport.

---

## 8. Bucket Diversity Policy

| Parameter                    | Description                    |
| ---------------------------- | ------------------------------ |
| `max_same_prefix_per_bucket` | Limit NodeID prefix clustering |
| `max_same_transport_ratio`   | Avoid transport monoculture    |
| `enforce_diversity`          | Toggle enforcement             |

---

## 9. Reference Profiles

### 9.1 I2P-First Profile

- `alpha = 2`
- `per_request_timeout_ms = 8000`
- `failure_threshold = 5`
- `decay_half_life_secs = 1800`
- `preferred_transports = ["i2p_sam_datagram"]`
- `allow_transport_fallback = false`
- `soft_failure_weight` > UDP default
- Conservative eviction

Interpretation:
High-latency tolerant, slow demotion, low concurrency.

---

### 9.2 UDP Profile

- `alpha = 3`
- `per_request_timeout_ms = 2000`
- `failure_threshold = 3`
- `decay_half_life_secs = 600`
- `preferred_transports = ["udp"]`
- `allow_transport_fallback = true`

Interpretation:
Aggressive, lower tolerance for packet loss, faster churn.

---

### 9.3 QUIC Profile

- `alpha = 3`
- `per_request_timeout_ms = 3000`
- Moderate failure threshold
- Connection reuse preferred

Interpretation:
Stable peers, heavier resources, less churn.

---

## 10. Invariants

The following MUST always hold:

- Policy changes MUST NOT alter protocol semantics.
- Policy MUST NOT bypass HELLO verification.
- Policy MUST NOT override signature validation.
- Policy MUST NOT insert unverified contacts.

---

## 11. Conclusion

Policy defines behavior tuning, not protocol definition.

This separation allows KAD3 to remain stable while adapting
to transport-specific realities.
