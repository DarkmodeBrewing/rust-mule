# Repository Guidelines

## Instruction for this repo:

- Read handoff.md before doing anything.
- After each meaningful change (or after tests run), update handoff.md with: status, decisions, next steps, and a change log entry.
- Keep it short and factual.
- Please write/update tests where applicable, run `cargo fmt`, run `cargo clippy`, run `cargo test`, and after each iteration commit and push to the remote.
- Prefer proposing a minimal design or patch first when making non-trivial changes.
- If assumptions are unclear, ask before coding.

### Rust-Mule Development Rules

1. Do NOT translate iMule code line-by-line.
2. Extract architecture and intent, not implementation.
3. Prefer Rust-native design:
   - ownership & borrowing
   - async/await
   - message enums instead of class hierarchies
   - Avoid `Arc<Mutex<...>>` unless concurrency requires shared mutation.
   - Prefer message passing and ownership transfer.
   - Avoid premature optimization; correctness and clarity first.
4. Build in layers:
   - Transport (UDP sockets, async runtime)
   - Message protocol (encode/decode)
   - Node identity & routing table
   - Bootstrap logic
5. No global state.
6. No C++-style singletons.
7. Every subsystem must be testable in isolation.
8. If stuck, stop coding and explain the architectural problem.

## Porting old code into rust

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
  - Message parsing/serialization”

## Project Structure & Module Organization

- `src/` contains the Rust crate. `src/main.rs` is the CLI/entrypoint and `src/lib.rs` exposes modules.
- Core modules live under `src/` (e.g., `config.rs`, `app.rs`, `protocol.rs`, `kad.rs`). Subsystems are grouped in folders like `src/i2p/`, `src/net/`, and `src/nodes/`.
- `config.toml` in the repo root is the default configuration file loaded at startup.
- `data/` holds runtime artifacts (e.g., `data/nodes.dat`, `data/preferencesKad.dat`). `target/` is the Cargo build output.
- `assets/` holds repo-tracked bootstrapping snapshots used for first-run seeding (e.g., `assets/nodes.initseed.dat`).
- `assets/` contains repo-tracked, _static_ bootstrap data.
  - These files are NOT modified at runtime.
  - They may be embedded at compile time or copied on first run.
- Runtime-generated or mutable data belongs in `data/` only.

## Build, Test, and Development Commands

- `cargo build` — compile the project in debug mode.
- `cargo run` — build and run the binary; uses `config.toml` by default.
- `cargo run -- <args>` — pass CLI args if added in the future.
- `cargo test` — run tests (currently none in the repo).

## Coding Style & Naming Conventions

- Use standard Rust formatting (rustfmt defaults: 4-space indentation, trailing commas, etc.).
- Prefer `snake_case` for functions/modules, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Keep modules cohesive: add new subsystem code under `src/<subsystem>/mod.rs` with focused helper files.

## Testing Guidelines

- No automated tests are checked in yet. When adding tests, use `#[cfg(test)] mod tests` in the relevant module or create `tests/` integration tests.
- Name test functions descriptively (e.g., `parses_nodes_dat`). Run with `cargo test`.

## Commit & Pull Request Guidelines

- Commit messages in history use a short prefix (e.g., `chore: ...`, `wip: ...`). Follow this convention for consistency.
- PRs should include a short summary, the motivation/issue link if applicable, and any config or data file changes noted explicitly.

## Configuration & Data Notes

- `config.toml` is validated on startup; keep `sam.host`, `sam.port`, and `sam.session_name` valid to avoid runtime errors.
- Avoid committing generated artifacts from `target/` or runtime data in `data/` unless required for reproducibility.

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

## Architectural Invariants

- Node identity is immutable once created.
- Routing table mutations must be explicit and testable.
- Network I/O is isolated from protocol parsing.
- No module may depend on concrete transport details unless it is in the transport layer.
