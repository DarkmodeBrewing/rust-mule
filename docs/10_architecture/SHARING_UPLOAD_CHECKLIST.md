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

## Security Edge Cases

- Hidden/sensitive file leakage
  - Prevent accidental sharing of secrets in dotfiles/profile dirs/key stores.
  - Add default deny patterns with explicit allow override policy.

- TOCTOU file mutation between index and serve
  - File content/size may change after indexing and before serving.
  - Re-validate file identity (size/mtime/inode or hash window) at serve time.

- Symlink/hardlink escape
  - Links can resolve outside allowed share roots.
  - Re-check resolved canonical path on each open, not only during scan.

- Oversized/sparse file abuse
  - Large or sparse files can exhaust resources during hashing/serving.
  - Enforce max file-size and bounded hashing/serving rates.

- Unicode/path normalization confusion
  - Different normalization forms can bypass duplicate/policy checks.
  - Normalize and compare paths/filenames consistently.

- Overlapping share-root ambiguity
  - Nested roots can create policy gaps.
  - Reject overlap by default or enforce explicit precedence with warnings.

- Metadata disclosure through API/logs
  - Absolute local paths should not leak to default API responses/logs/events.
  - Use redacted/logical identifiers in default operator views.

- Upload amplification patterns
  - Tiny, fragmented range requests can maximize overhead.
  - Enforce batching, per-peer budgets, and minimum practical block policy.

- MD4 compatibility risk
  - Protocol requires MD4 but MD4 is weak.
  - Track stronger local integrity metadata alongside MD4 for local trust decisions.

- Settings/debug authorization drift
  - Share-root management and debug endpoints are sensitive.
  - Enforce same auth/rate-limit rigor and ensure debug does not bypass share policy.

## Network Interop Numbers and Edge Cases

- Current rust-mule transfer defaults (as of 2026-03-08)
  - transfer pump reserve block size: `64 KiB` (`src/app.rs`, `BLOCK_SIZE = 64 * 1024`)
  - max reserved blocks per pump call: `4` (`src/app.rs`, `MAX_BLOCKS_PER_RESERVE`)
  - protocol request range slots per `OP_REQUESTPARTS`: `3` (`src/download/protocol.rs`)
  - lease caps:
    - per-peer inflight leases: `32` (`src/download/service.rs`)
    - per-download inflight leases: `256` (`src/download/service.rs`)

- iMule reference values (for compatibility baseline)
  - `BLOCKSIZE` / `EMBLOCKSIZE`: `184320` bytes (`source_ref/.../Constants.h`)
  - `PARTSIZE`: `9728000` bytes (`source_ref/.../Constants.h`)
  - `OP_REQUESTPARTS` uses 3 ranges and `OP_SENDINGPART` payload shape is compatible (`source_ref/.../Client2Client/TCP.h`)

- Interop edge cases to track
  - Smaller-than-expected block size (64 KiB vs ~180 KiB) may increase overhead and degrade behavior with peers tuned for iMule-sized blocks.
  - Request/response pacing windows can drift under mixed-client timing assumptions.
  - Retry/timeout behavior can become noisy when block granularity differs from peer expectations.
  - Completion latency can increase due to fragmented block scheduling.

- Implementation guidance
  - Keep protocol payload compatibility, but make transfer sizing policy configurable.
  - Prefer iMule-aligned default block size for interop unless data proves otherwise.
  - Validate changes via mixed-client soak runs before locking defaults.

## Non-Goals (initial slice)

- Full media-library UX and advanced tagging/search.
- Remote share management outside localhost auth model.
