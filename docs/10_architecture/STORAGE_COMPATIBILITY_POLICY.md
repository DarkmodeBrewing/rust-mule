Status: ACTIVE
Last Reviewed: 2026-03-19

# Storage Compatibility Policy

## Decision

`rust-mule` treats compatibility as a boundary, not as a blanket rule.

- Wire protocol compatibility with iMule is release-critical.
- User-visible network behavior should remain compatibility-oriented where it
  materially affects interoperability.
- Internal persistence should default to Rust-native formats and data models.
- Legacy on-disk formats should only be implemented directly when they are
  required for a concrete interoperability goal.

## Why

Full legacy on-disk parity is expensive, brittle, and constrains internal
architecture around historical implementation details that do not map cleanly to
Rust-native design.

For local working state, Rust-native persistence gives us:

- simpler schema evolution
- lower parser and recovery complexity
- clearer tests and failure handling
- less accidental coupling to iMule internal storage assumptions

## Current Policy

For download/storage-related work:

- `.part` file payload data remains naturally compatible as file content.
- active download metadata may use a Rust-native internal representation.
- `known.met` / `known2_64.met` parity is optional unless a specific interop
  requirement makes it necessary.
- import/export or migration adapters are preferred over making the internal
  storage model fully legacy-native.

## When To Implement Legacy File Parity

Implement direct legacy format compatibility only when one of these is a real
project requirement:

- users must resume active/incomplete downloads across iMule and rust-mule
- users must move runtime state between clients without conversion
- preserving existing user runtime files without migration tooling is a release
  goal

Absent those requirements, Rust-native storage remains the default.

## Consequences

- Download implementation should prioritize protocol behavior and transfer
  correctness over byte-for-byte `.part.met` parity.
- If cross-client state portability becomes important later, add explicit
  import/export or one-way migration paths.
- Docs and backlog should distinguish:
  - wire/protocol compatibility
  - user-visible behavior compatibility
  - local persistence compatibility

## Review Rule

Before adding or expanding a legacy on-disk format, ask:

> Does this unlock a concrete interoperability workflow that users actually need?

If not, prefer Rust-native persistence.
