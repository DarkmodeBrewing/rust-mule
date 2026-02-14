# Download Subsystem Design (iMule-Compatible, Rust-Native)

## Goal

Implement chunked file downloads compatible with iMule/eMule behavior, including:

- active temp state in `data/download/` (`.part`, `.part.met`, backup, optional seeds),
- completed files in `data/incoming/`,
- persisted known-file metadata (`known.met`) and AICH hashsets (`known2_64.met`),
- wire-compatible block request/receive flow for client-to-client transfers.

This document extracts architecture and behavior from iMule source without line-by-line translation.

## iMule Findings (Deep Dive Summary)

### Core files and responsibilities

- `source_ref/iMule-2.3.1.5-src/src/PartFile.cpp`:
  - creates numbered temp files (`%03d.part.met` + `.part`),
  - persists gap map + file tags to `.part.met`,
  - tracks requested blocks and chooses next blocks by chunk-selection policy,
  - buffers incoming block data, flushes to disk, hashes completed parts, finalizes file.
- `source_ref/iMule-2.3.1.5-src/src/DownloadClient.cpp`:
  - sends `OP_REQUESTPARTS`,
  - receives `OP_SENDINGPART` / `OP_COMPRESSEDPART`,
  - maps packets to pending requested blocks and writes to partfile buffer.
- `source_ref/iMule-2.3.1.5-src/src/UploadClient.cpp`:
  - validates and serves requested block ranges from complete/part files.
- `source_ref/iMule-2.3.1.5-src/src/DownloadQueue.cpp`:
  - orchestrates active downloads, source selection, source requests, and queue lifecycle.
- `source_ref/iMule-2.3.1.5-src/src/KnownFileList.cpp`:
  - persists `known.met` for known/shared/completed files.
- `source_ref/iMule-2.3.1.5-src/src/SHAHashSet.h` and `source_ref/iMule-2.3.1.5-src/src/SHAHashSet.cpp`:
  - manages AICH hashsets in `known2_64.met`.

### On-disk formats and filenames

- Part metadata versions:
  - `PARTFILE_VERSION=0xE0`, `PARTFILE_VERSION_LARGEFILE=0xE2`
  - from `source_ref/iMule-2.3.1.5-src/src/include/common/DataFileVersion.h`.
- Known list headers:
  - `MET_HEADER=0x0E`, `MET_HEADER_WITH_LARGEFILES=0x0F`
  - from `source_ref/iMule-2.3.1.5-src/src/include/common/DataFileVersion.h`.
- AICH hashset file:
  - `known2_64.met` (`KNOWN2_MET_FILENAME`)
  - from `source_ref/iMule-2.3.1.5-src/src/SHAHashSet.h`.
- Block/part sizing:
  - `PARTSIZE=9728000`, `BLOCKSIZE=184320`, `EMBLOCKSIZE=184320`
  - from `source_ref/iMule-2.3.1.5-src/src/include/protocol/ed2k/Constants.h`.

### Transfer behavior to preserve

- Requests are grouped as up to N pending blocks; each block is a byte range.
- Block boundaries are aligned to ed2k block semantics (~180 KiB), but requests may be smaller to fill gaps.
- Chunk selection is rarity-aware and completion-aware (Maella enhanced chunk selection in `PartFile.cpp`).
- Received data is matched against pending requested ranges, buffered, persisted, then verified by part hash.
- Completed download is moved from temp (`.part`) into incoming dir and state is persisted.

## Rust Module Boundaries (Proposed)

New subsystem under `src/download/`:

- `src/download/mod.rs`
  - public facade, commands/events, subsystem wiring.
- `src/download/types.rs`
  - immutable identifiers and DTOs:
  - `DownloadId`, `FileHashMd4`, `PartIndex`, `BlockRange`, `DownloadStatus`.
- `src/download/errors.rs`
  - typed `DownloadError` and sub-errors (`StoreError`, `WireError`, `HashError`, `IoError`).
- `src/download/store.rs`
  - partfile metadata persistence:
  - read/write `.part.met`, `.part.met.bak`, allocate numbered slots.
- `src/download/gap.rs`
  - gap/range tracking and merge/split logic.
- `src/download/scheduler.rs`
  - chunk selection + block request scheduling (rarity + completion + request spread).
- `src/download/transfer.rs`
  - per-peer pending blocks, request building, receive-path matching, timeout/retry bookkeeping.
- `src/download/hash.rs`
  - MD4 part/full-file verification; optional AICH integration hooks.
- `src/download/known.rs`
  - known-file persistence interface (`known.met`, `known2_64.met` phase-gated).
- `src/download/service.rs`
  - async event loop actor for download queue, source updates, and state transitions.

Integration points:

- `src/app.rs`: start/stop `DownloadService` and route commands.
- `src/api/*`: add endpoints for create/list/detail/pause/resume/cancel and per-download metrics.
- UI controllers can then consume these endpoints for active downloads.

## Data Layout (rust-mule)

Under `general.data_dir`:

- `download/`
  - `001.part`
  - `001.part.met`
  - `001.part.met.bak`
  - optional: `001.part.met.seeds`
- `incoming/`
  - finalized files (safe rename/move target).
- `known.met`
- `known2_64.met` (when AICH support is enabled).

Notes:

- Keep temp and incoming split explicit.
- Startup scan should rebuild in-memory queue from `download/*.part.met`.
- Write operations should be atomic where possible (temp + rename).

## Delivery Phases

1. Phase 0: scaffolding + typed errors
- Create `src/download/*` modules and typed errors.
- Add config defaults for `download/` and `incoming/` subdirs under `data_dir`.
- No wire traffic yet.

2. Phase 1: local partfile lifecycle
- Create/load/save `.part.met` and `.part`.
- Persist gap map and requested ranges.
- Add queue operations: add/pause/resume/cancel/delete.
- Add unit tests for persistence and gap math.

3. Phase 2: block transfer receive path
- Implement incoming block packet validation and write pipeline.
- Buffer + flush behavior, part completion detection, MD4 verification.
- Add deterministic tests for range matching, duplicate packets, and boundary checks.

4. Phase 3: block request scheduler
- Implement rarity/completion-aware part selection compatible with iMule intent.
- Add per-peer pending block windows and timeout/retry handling.
- Add tests for selection fairness and starvation avoidance.

5. Phase 4: finalize + known files
- Move completed files to `data/incoming/`.
- Persist known entries to `known.met`.
- Phase-gated AICH/`known2_64.met` integration.

6. Phase 5: API + UI integration
- Add REST endpoints for download lifecycle and progress.
- Surface queue details in UI (active, paused, completed, errored).

## Compatibility Rules

- Maintain ed2k file/block constants and range semantics.
- Keep `.part.met` metadata stable and recoverable (`.bak` fallback).
- Preserve source compatibility at protocol level; internal scheduling may be Rust-native as long as behavior is equivalent.
- No global mutable state; use actor-style command/event loop for subsystem core.

## Test Plan

- Unit tests:
  - gap/range operations,
  - `.part.met` roundtrip,
  - requested/pending block lifecycle,
  - hash verification rules.
- Integration tests:
  - startup recovery from existing temp files,
  - download completion and move to `incoming`,
  - cancellation cleanup semantics.
- Compatibility tests:
  - fixture-based comparison for key `.part.met` fields and part/gap interpretation.

## Open Decisions

- `data/incoming` spelling is recommended (legacy typo `incomming` should not become canonical).
- AICH can be introduced in phase 4 after MD4-first baseline is stable.
- `known2_64.met` can be feature-gated initially if full AICH recovery path is deferred.
