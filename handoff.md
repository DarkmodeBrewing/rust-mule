# Handoff

## Status
Security review complete. All HIGH and most LOW findings fixed.

## Decisions
- Added `subtle` crate for constant-time Bearer token comparison (timing side-channel fix).
- SAM `encode_value` now strips `\r`/`\n` to prevent command injection.
- `validate_settings` rejects session names containing `\r`/`\n`.
- MD5 K-table uses hardcoded RFC 1321 constants instead of `f64::sin`.
- Debug endpoints now default to `false` (was `true`).
- Token temp file uses `mode(0o600)` before rename on Unix.
- Download `file_size` capped at 100 GiB.
- Session map capped at 1 024 entries to prevent memory DoS.

## Next Steps
- Consider replacing custom MD4/MD5/inflate with RustCrypto crates (low priority, dependency trade-off).
- Extend `kad_udp_key_secret` from 32-bit to 128-bit (breaking protocol change).
- Consider sliding-window rate limiter for `POST /api/v1/session`.
- Clarify `sam.host` validator vs. runtime hostname acceptance.

## Change Log
- 2026-02-28: Security review pass. See `SECURITY_REVIEW.md` for full findings.
  - Fixed: timing side-channel on token compare, SAM newline injection, MD5 float K-table,
    debug endpoint default, token file TOCTOU, download file_size cap, session map cap.
