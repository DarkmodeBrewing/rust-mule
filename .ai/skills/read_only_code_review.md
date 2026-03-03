# Skill: ReadOnlyCodeReview

## Purpose

Perform a senior-level, read-only code review focused on correctness, maintainability, and idiomatic Rust.
This is NOT a security audit and NOT a performance/DoS audit (those have separate skills).

## When to Use

- Reviewing PRs for code quality before merge
- Identifying Rust antipatterns and non-idiomatic code
- Improving clarity, structure, error handling, and invariants
- Finding likely bugs (logic, edge cases) without threat-model framing

## Scope Checklist

Focus on:

- **Correctness & edge cases**: off-by-one, wrong defaults, invalid states
- **Rust idioms**:
  - prefer `Result`/`?` patterns over nested matches
  - avoid unnecessary clones/allocations
  - prefer iterators where clearer (but not “iterator golf”)
  - appropriate use of `Cow`, `Arc`, `Bytes`, `SmallVec` (only if already in repo)
- **API boundaries**:
  - module structure, visibility (`pub` hygiene)
  - types that encode invariants (newtypes, enums)
  - avoid “stringly typed” identifiers when better types exist
- **Error handling**:
  - meaningful error context (`anyhow::Context` / `thiserror`)
  - avoid swallowing errors
  - avoid `unwrap`/`expect` in non-test code
- **Async sanity** (light pass):
  - `tokio::select!` correctness and cancellation behavior
  - avoid holding locks across `.await`
  - backpressure handling where relevant (channels/streams)
- **Logging/telemetry**:
  - helpful structured logs (fields), not spam
  - avoid logging secrets (but don’t deep-audit security here)

## Constraints

- Do NOT modify code.
- Do NOT propose large refactors unless they are clearly justified.
- Prefer small, actionable improvements with clear tradeoffs.
- If a finding looks security-relevant, note it and recommend running `SecurityAudit`.

## Output Format

Return a list of findings using this template:

### Finding <n>: <short title>

- severity: HIGH | MEDIUM | LOW
- category: (use one from `.ai/meta.md`, typically: concurrency/async, logging/observability, config/ops)
- location: <file>:<line range> (best effort)
- impact: <what breaks / developer pain / likely bug>
- evidence: <what in code supports this>
- recommendation: <conceptual change; may include small code sketch ONLY if user asked for patches>
- verification: <how to confirm via tests / reasoning>

## “Rust-mule style” preferences

- Favor clarity and boring correctness over cleverness.
- Prefer bounded state machines and typed invariants.
- Keep compatibility-sensitive logic explicit and well-commented.
