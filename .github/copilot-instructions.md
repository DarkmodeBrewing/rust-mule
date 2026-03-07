# Copilot Instructions for rust-mule

## What This Repository Is

`rust-mule` is an experimental, I2P-only, iMule-compatible Kademlia (KAD) node and control plane written in Rust.
It uses SAM v3 datagram sessions and exposes a local HTTP API (`/api/v1`) plus an embedded UI.

Primary goals:
- Correctness and robustness under hostile/untrusted network input
- Rust-native architecture (not direct C++ translation)
- Measurable behavior via tests and baseline/soak scripts

## Core Architecture Rules

- Do **not** translate iMule/aMule/eMule code line-by-line.
- Extract intent and redesign using Rust idioms:
  - ownership/borrowing
  - async/await
  - message enums and actor-style command loops
- Avoid global state and singleton-style patterns.
- Avoid `Arc<Mutex<...>>` unless shared mutable concurrency is clearly required.
- Keep transport I/O isolated from protocol parsing and routing logic.
- Ensure subsystem behavior is testable in isolation.

## Layering Expectations

Keep boundaries explicit:
- Transport (I2P SAM datagram TCP/UDP-forward)
- Protocol encode/decode and validation
- KAD routing/search/publish/store logic
- Session/state and app orchestration
- Download lifecycle and persistence (`.part`, `.part.met`, `known.met`)

Do not leak transport-specific details into higher layers.

## Security and Input Handling

- Treat all network payloads and API input as untrusted.
- Prefer typed errors over panics.
- Enforce explicit bounds for lengths/counts/payload sizes.
- Reject malformed inputs early with clear error paths.
- Preserve compatibility envelope behavior while keeping implementation Rust-native.

## Coding and Naming Conventions

- Rustfmt defaults; idiomatic naming:
  - `snake_case` for functions/modules
  - `CamelCase` for types
  - `SCREAMING_SNAKE_CASE` for constants
- Keep modules cohesive and focused.
- Prefer neutral naming (`Mule*`/protocol-neutral) over direct `Imule*` identifiers.
- In code comments, prefer compatibility wording over product-specific historical wording.

## Testing and Validation Requirements

For meaningful changes, add or update tests and run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Focus tests on:
- Hostile/malformed input handling
- State-machine transitions
- Recovery and restart behavior
- API contract stability for UI consumers

## Documentation and Workflow Rules

- Read `docs/handoff.md` before implementing changes.
- After meaningful changes, update `docs/handoff.md` with:
  - status
  - decisions
  - next steps
  - change log
- Keep `docs/TODO.md` and `docs/TASKS.md` aligned with delivered work.
- Use feature branches and pull requests; do not bypass PR flow.

## Review Priorities

When proposing changes, optimize for:
1. Behavioral correctness
2. Safety/hardening
3. Test coverage and reproducibility
4. Maintainability and module boundaries
5. Compatibility constraints (without direct source translation)

