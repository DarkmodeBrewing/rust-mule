# Task Plan

## Current Priority

1. Complete typed-error migration for boundary/runtime layers.

Scope:
- `src/app.rs`
- `src/main.rs`
- `src/api/mod.rs` (internal helpers; keep HTTP boundary ergonomics)
- `src/single_instance.rs`
- high-churn paths in `src/kad/service.rs`

Why this is next:
- Subsystem modules now expose typed errors; remaining `anyhow` hotspots are orchestration paths.
- Finishing this gives consistent error provenance across runtime/control-plane behavior.
- It will also make API/logging error mapping cleaner and safer.

Definition of done:
- Replace internal `anyhow` usage in the listed files with typed error enums + `Result` aliases.
- Preserve existing runtime behavior and HTTP responses.
- Add/update tests where behavior mapping changes.
- `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` pass.
