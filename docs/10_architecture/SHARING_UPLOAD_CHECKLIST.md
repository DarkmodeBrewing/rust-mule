Status: DRAFT
Last Reviewed: 2026-03-08

# Sharing and Upload Checklist

## Purpose

Track implementation guardrails for real file sharing and disk-backed upload serving.

## Scope

- Shared folder configuration and settings UI/API
- File indexing/hash metadata for publish
- Real upload serving path for inbound part requests
- Safety, abuse controls, and observability

## Implementation Checklist

- Shared folders
  - Add config field for multiple share roots.
  - Expose share roots in settings API and settings UI.
  - Canonicalize paths before storage/use.
  - Reject duplicates and unsafe overlaps by policy.
  - Reject unsafe roots by default (`/`, core OS dirs, app/runtime data dirs).
  - Define symlink policy (deny by default unless explicitly enabled).

- Library indexing
  - Scan configured share roots and discover candidate files.
  - Compute and persist stable metadata (MD4, size, normalized path, mtime).
  - Publish sources only for files that pass hash/size verification.
  - Revalidate or invalidate entries when file content changes.
  - Surface scanner errors and permission failures in status/UI.

- Publish/source binding
  - Bind each published source to an indexed local file record.
  - Keep source-store entries traceable to local path metadata.
  - Refuse publish when local file mapping is missing or stale.

- Upload serving (`OP_REQUESTPARTS` -> `OP_SENDINGPART`)
  - Parse and validate requested ranges (bounds, ordering, max block constraints).
  - Read exact byte ranges from disk.
  - Return correct `OP_SENDINGPART` payloads per requested blocks.
  - Handle short reads and I/O failures deterministically with counters.
  - Ensure idempotent behavior for duplicate/out-of-order requests.

- Concurrency and backpressure
  - Enforce per-peer and global upload limits.
  - Add fair scheduling across peers/files.
  - Use bounded queues and explicit timeout/drop behavior.

- Safety and abuse controls
  - Apply request validation and rate limits for malformed/flood behavior.
  - Add cooldown/ban policy hooks for repeat offenders.
  - Ensure no path traversal or unintended file exposure outside approved share roots.

- Observability
  - Add counters: requests served, bytes sent, read failures, denied/invalid requests.
  - Add per-file/per-peer debug diagnostics for triage.
  - Keep verbose bucket/routing internals behind debug gating.

- Testing
  - Unit tests:
    - path policy validation
    - range parsing/bounds checks
    - disk range read correctness
  - Integration tests:
    - A shares real file, B discovers source, B downloads and verifies bytes/hash
    - restart behavior with index rebuild + republish
  - Negative tests:
    - unsafe folder rejection
    - symlink/path traversal denial
    - permission/read failure handling

## Non-Goals (initial slice)

- Full media-library UX and advanced tagging/search.
- Remote share management outside localhost auth model.
