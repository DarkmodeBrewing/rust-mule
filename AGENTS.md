# Repository Guidelines

## AI Agent Rules

This repository supports structured AI code reviews and development assistance via defined "skills".

- Skills are defined under `.ai/skills/`
- Shared definitions (severity levels, threat model, finding categories) live in `.ai/meta.md`
- Only one skill may be active per review pass unless explicitly requested
- Default mode is **read-only analysis**
- No architectural, protocol, or cryptographic changes without explicit instruction
- When reporting findings, use the structured format defined in `.ai/meta.md`
- Do not suggest patches unless explicitly asked

If reviewing code:

- Choose the appropriate skill (e.g., `ReadOnlyCodeReview`, `SecurityAudit`, `PerformanceDosReview`, `ProtocolUpgradeNegotiationReview`)
- Do not mix scopes
- Stay within the defined boundaries of the selected skill

---

## Instruction for this repo

- Prefer 80–150 lines max per read
- Use `rg -n` to pinpoint, then `sed` a small window
- Avoid `sed -n '1,320p'` style unless necessary
- Read `handoff.md` before doing anything
- After each meaningful change (or after tests run), update `handoff.md` with:
  - status
  - decisions
  - next steps
  - change log entry
- Keep updates short and factual
- Write/update tests where applicable
- Run `cargo fmt`
- Run `cargo clippy --all-targets --all-features`
- Run `cargo test`
- After each iteration: commit and push to remote
- Prefer proposing a minimal design or patch first when making non-trivial changes
- If assumptions are unclear, ask before coding

---

## Rust-Mule Development Rules

1. Do NOT translate iMule code line-by-line.
2. Extract architecture and intent, not implementation.
3. Prefer Rust-native design:
   - ownership & borrowing
   - async/await
   - message enums instead of class hierarchies
   - Avoid `Arc<Mutex<...>>` unless concurrency requires shared mutation
   - Prefer message passing and ownership transfer
   - Avoid premature optimization; correctness and clarity first
4. Build in layers:
   - Transport (UDP sockets, async runtime)
   - Message protocol (encode/decode)
   - Node identity & routing table
   - Bootstrap logic
5. No global state.
6. No C++-style singletons.
7. Every subsystem must be testable in isolation.
8. If stuck, stop coding and explain the architectural problem.

---

## Porting old code into Rust

Map the iMule codebase into these conceptual layers:

- Networking / transport
- KAD routing logic
- DHT storage
- Message protocol handling
- Session/state management
- File sharing logic
- Utility layers

For each:

1. What does iMule do here?
2. What Rust module should exist?
3. What data structures belong there?
4. What should be async?

- iMule C++ code is located in `./source_ref`
- STOP direct translation. Switch to architecture extraction mode.
- Identify high-level subsystems in iMule
- Describe responsibilities of each
- Propose Rust module boundaries
- Only then begin implementing from scratch in Rust idioms
- Do NOT implement everything. Focus on:
  - Networking layer
  - KAD node + routing table
  - Message parsing/serialization

---

## Project Structure & Module Organization

- `src/` contains the Rust crate. `src/main.rs` is the CLI/entrypoint and `src/lib.rs` exposes modules.
- Core modules live under `src/` (e.g., `config.rs`, `app.rs`, `protocol.rs`, `kad.rs`).
- Subsystems are grouped in folders like `src/i2p/`, `src/net/`, and `src/nodes/`.
- `config.toml` in the repo root is the default configuration file loaded at startup.
- `data/` holds runtime artifacts (e.g., `data/nodes.dat`, `data/preferencesKad.dat`).
- `assets/` holds repo-tracked bootstrapping snapshots used for first-run seeding (e.g., `assets/nodes.initseed.dat`).
- `assets/` contains repo-tracked, static bootstrap data.
  - These files are NOT modified at runtime.
  - They may be embedded at compile time or copied on first run.
- Runtime-generated or mutable data belongs in `data/` only.
- `target/` is the Cargo build output and must not be committed.

---

## Build, Test, and Development Commands

- `cargo build` — compile the project in debug mode
- `cargo run` — build and run the binary (uses `config.toml` by default)
- `cargo run -- <args>` — pass CLI args if added
- `cargo test` — run tests (tests are expected for new subsystems and protocol logic)

---

## Coding Style & Naming Conventions

- Use standard Rust formatting (rustfmt defaults)
- Prefer:
  - `snake_case` for functions/modules
  - `CamelCase` for types
  - `SCREAMING_SNAKE_CASE` for constants
- Keep modules cohesive: add subsystem code under `src/<subsystem>/mod.rs` with focused helper files
- Avoid large multi-responsibility modules

---

## Testing Guidelines

- Use `#[cfg(test)] mod tests` in relevant modules or create `tests/` integration tests
- Name test functions descriptively (e.g., `parses_nodes_dat`)
- Tests must validate:
  - routing invariants
  - protocol encode/decode round trips
  - state transitions
  - edge cases and invalid input

---

## Commit & Pull Request Guidelines

- Work must be done in branches (`main` is locked)
- Commit messages should use short prefixes (e.g., `chore:`, `feat:`, `fix:`, `refactor:`, `wip:`)
- PRs must include:
  - short summary
  - motivation or issue link
  - note any config or data file changes explicitly

---

## Configuration & Data Notes

- `config.toml` is validated on startup
- Keep `sam.host`, `sam.port`, and `sam.session_name` valid to avoid runtime errors
- Avoid committing runtime-generated artifacts from `data/`
- Avoid committing build artifacts from `target/`

---

## Definition of Done (before commit)

- `cargo fmt` produces no changes
- `cargo clippy --all-targets --all-features` has no warnings (or warnings are explicitly justified)
- `cargo test` passes
- No public API changes unless explicitly stated
- `handoff.md` updated with:
  - current status
  - decisions made
  - next steps
  - brief change log entry

---

## AI Review Protocol

When a PR touches:

- Core logic → run `ReadOnlyCodeReview`
- Networking, parsing, routing → also run `PerformanceDosReview`
- Crypto, identity, integrity → run `SecurityAudit`
- Handshake, versioning, capability negotiation → run `ProtocolUpgradeNegotiationReview`

Reviews must:

- Tag severity: HIGH / MEDIUM / LOW
- Identify category
- Provide evidence and reasoning
- Suggest conceptual fixes only (unless patches are explicitly requested)

Downgrade protection rule:

- If both peers advertise support for a stronger protocol mode, legacy fallback must not occur silently.

---

## Architectural Invariants

- Node identity is immutable once created
- Routing table mutations must be explicit and testable
- Network I/O is isolated from protocol parsing
- No module may depend on concrete transport details unless it is in the transport layer
- All externally sourced input is untrusted
- All protocol negotiation must be explicit and versioned
