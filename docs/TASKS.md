# Task Plan

## Current Priority

1. KAD search/publish reliability pass (network-level verification).

Scope:
- verify ACK behavior and retry/backoff tuning against live peers
- validate `search_sources` and keyword search/publish success rates over longer runs
- tighten metrics/logging around request->response conversion and timeout causes

Why this is next:
- typed-error migration is now complete across runtime and subsystem modules
- remaining delivery risk is network behavior (timeouts, retries, peer variance), not error typing

Definition of done:
- measurable improvement in successful search/publish round-trips over baseline
- clear counters/log events for ACKs, retries, and timeout buckets
- `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` pass
