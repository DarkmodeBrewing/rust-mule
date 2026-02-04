# Repository Guidelines

- Instruction for this repo:
- Read handoff.md before doing anything.
- After each meaningful change (or after tests run), update handoff.md with: status, decisions, next steps, and a change log entry.
- Keep it short and factual.

## Project Structure & Module Organization

- `src/` contains the Rust crate. `src/main.rs` is the CLI/entrypoint and `src/lib.rs` exposes modules.
- Core modules live under `src/` (e.g., `config.rs`, `app.rs`, `protocol.rs`, `kad.rs`). Subsystems are grouped in folders like `src/i2p/`, `src/net/`, and `src/nodes/`.
- `config.toml` in the repo root is the default configuration file loaded at startup.
- `data/` and `datfiles/` hold runtime artifacts (e.g., `datfiles/nodes.dat`). `target/` is the Cargo build output.

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
