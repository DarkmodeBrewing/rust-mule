# Security Review — rust-mule

> Reviewed from the perspective of a senior Rust developer.  
> Issues are sorted **HIGH → LOW**.  
> Items marked **[FIXED]** were patched in this review pass.

---

## HIGH

### 1. Timing Side-Channel on Bearer Token Comparison **[FIXED]**

**File:** `src/api/auth.rs`  
**Line (before patch):** 49

```rust
// BEFORE
if provided != current_token {
    return Err(StatusCode::FORBIDDEN);
}
```

Rust's `!=` on `String` / `&str` short-circuits on the first differing byte.  A local process measuring response-time variance across many requests can leak the token one character at a time.  While the API is loopback-only, the threat model still includes local processes (e.g., a compromised user-space program on the same machine).

**Fix:** replaced with `subtle::ConstantTimeEq` so the comparison always takes the same time regardless of where bytes first differ.

```rust
// AFTER
if !bool::from(provided.as_bytes().ct_eq(current_token.as_bytes())) {
    return Err(StatusCode::FORBIDDEN);
}
```

---

### 2. SAM Command-Line Injection via Session Name **[FIXED]**

**File:** `src/i2p/sam/protocol.rs` (`encode_value`) and  
`src/api/handlers/settings.rs` (`validate_settings`)

The SAM v3 protocol is line-delimited (`\r\n`).  `encode_value` wraps values containing whitespace in double-quotes, but passes embedded `\n` / `\r` bytes through literally:

```
SESSION CREATE ID="rust-mule\nHELLO VERSION MIN=99.99" ...
```

When written to the TCP socket this becomes two lines, injecting an arbitrary SAM command.  An attacker who already holds a valid API token could craft a `sam.session_name` payload (via `PATCH /api/v1/settings`) that forces the SAM bridge into a different state.

**Fix (protocol layer):** `encode_value` now strips `\r` / `\n` from values before encoding.  
**Fix (validation layer):** `validate_settings` explicitly rejects session names that contain `\n` or `\r` with `400 Bad Request`.

---

### 3. No Upper Bound on `file_size` in Download Creation **[FIXED]**

**File:** `src/download/service.rs`

```rust
// BEFORE: only lower-bound checked
if req.file_size == 0 { ... }
file.set_len(req.file_size).await?;
```

`file.set_len(u64::MAX)` will fail on most file-systems, but an attacker-supplied value close to the disk limit could still trigger unexpected sparse-file behaviour, fill the disk, or expose arithmetic edge-cases elsewhere in range bookkeeping.

**Fix:** capped at **100 GiB**, well above any realistic eMule/iMule file.

```rust
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024 * 1024;
if req.file_size > MAX_FILE_SIZE { ... }
```

---

### 4. MD5 K-Table Computed via Floating Point **[FIXED]**

**File:** `src/kad/udp_crypto.rs`

```rust
// BEFORE – computed at every call
let x = ((f64::sin((i + 1) as f64).abs()) * (2u64.pow(32) as f64)).floor() as u64;
```

The MD5 specification lists 64 exact integer constants (Table T in RFC 1321).  Deriving them at runtime via `f64::sin` introduces platform-specific floating-point rounding risk: on hardware with 80-bit extended-precision FPUs (or non-IEEE modes) one or more constants could differ from the standard, silently producing a non-standard hash.  This would break cross-node KAD packet decryption.

**Fix:** replaced with the 64 hardcoded constants from RFC 1321.

---

## MEDIUM

### 5. Unbounded Session Map Growth (Memory DoS) **[FIXED]**

**File:** `src/api/handlers/core.rs`

A caller with a valid token can invoke `POST /api/v1/session` in a tight loop.  Sessions live for 8 hours; the sweeper runs every 5 minutes.  With a 30-req/window rate limit (default), a local attacker could accumulate ~2 400 live sessions before the first sweep, and keep growing them over time.

**Fix:** a hard cap of `MAX_SESSIONS = 1024` concurrent sessions is enforced at insert time (after expired sessions are pruned).  Exceeding the cap returns `503 Service Unavailable`.

---

### 6. RC4 Stream Cipher — Cryptographically Broken

**File:** `src/kad/udp_crypto.rs`

RC4 has well-documented biases and is considered broken by modern standards.  Here it is used as "obfuscation" to avoid plain-protocol fingerprinting, not for confidentiality, which matches how iMule/aMule uses it.  The limitation is clearly documented in the code.

**Recommendation:** no functional change needed for iMule interoperability.  If a future protocol version is designed independently of iMule, replace with ChaCha20 or AES-CTR.

---

### 7. MD5 Used as a Key Derivation Function

**File:** `src/kad/udp_crypto.rs`  
**Functions:** `encrypt_kad_packet`, `try_decrypt_kad_with_node_id`, `try_decrypt_kad_with_receiver_key`

MD5 has known preimage weaknesses (collision attacks in practice; second-preimage resistance is weaker than SHA-2).  Using it as a KDF to derive RC4 key material is insufficient for a security-critical channel but is acceptable for its stated purpose: protocol-level obfuscation that matches the iMule/aMule wire format.

**Recommendation:** same as above — acceptable for the current compatibility goal; document this clearly in the module header.

---

### 8. 32-Bit UDP Key Secret — Brute-Forceable

**File:** `src/kad/udp_key.rs`, `src/kad/udp_crypto.rs` (`udp_verify_key`)

```rust
pub async fn load_or_create_udp_key_secret(path: &Path) -> Result<u32>
```

The `kad_udp_key_secret` is 32 bits (≈ 4 × 10⁹ possible values).  It feeds into `udp_verify_key`, which computes `MD5(dest_hash || secret)` and XOR-folds the result into a 32-bit receiver verify key.  A passive attacker who observes multiple `HELLO_REQ` packets to different destinations could brute-force the secret in ≤ 2³² MD5 evaluations — feasible on modern hardware in minutes.

**Recommendation:** extend the secret to 128 bits and use a proper PRF (HKDF-SHA256) for key derivation.  This is a breaking change against the iMule wire format so it must be gated on a protocol version negotiation.

---

### 9. Debug Endpoints Enabled by Default **[FIXED]**

**File:** `src/config.rs`, `config.toml`

```rust
// BEFORE
fn default_api_enable_debug_endpoints() -> bool { true }
```

`/api/v1/debug/*` endpoints expose internal routing-table dumps, live lookup triggers, and peer probe capabilities.  Shipping with these on by default leaks operational details and widens the attack surface.

**Fix:** default changed to `false`; `config.toml` updated to match.

---

## LOW / NITPICK

### 10. Custom MD4, MD5, and DEFLATE Implementations

**Files:** `src/kad/md4.rs`, `src/kad/udp_crypto.rs`, `src/kad/packed.rs`

Rolling bespoke implementations of established algorithms is high-maintenance and error-prone.  The existing implementations appear correct (the test vectors pass), but any future change risks subtle bugs.

**Recommendation:** consider replacing with well-audited crates from the RustCrypto ecosystem:
- `md-5` (MD5), `md4` (MD4) — `digest` trait compatibility  
- `flate2` (zlib/deflate, uses miniz_oxide or zlib-ng)

The main trade-off is adding external dependencies; the current self-contained approach keeps the dependency graph minimal.

---

### 11. `choose_semi_random_marker` Returns `0x00` as Hardcoded Fallback

**File:** `src/kad/udp_crypto.rs`

```rust
// Extremely unlikely.
Ok(0x00)
```

After 128 failed attempts, the function returns `0x00` instead of an error.  The probability of this path being reached is astronomically small (`(8/256)^128`), but `0x00` is not in the reserved-protocol list, so the packet would be structurally valid.  Returning a hard error here would be safer and more explicit.

---

### 12. `sam.host` Validates as IP but Runtime Accepts Hostnames

**File:** `src/api/handlers/settings.rs` (`validate_settings`)

```rust
cfg.sam.host.parse::<std::net::IpAddr>().map_err(|_| StatusCode::BAD_REQUEST)?;
```

The API validates that `sam.host` is a parseable IP address, but the underlying `TcpStream::connect((host, port))` call accepts any hostname string.  A user who bypasses the API and edits `config.toml` directly can supply a hostname, while the API falsely suggests only IPs are acceptable.  Either the runtime should restrict to IPs as well, or the validator should be relaxed to also accept hostnames (with DNS resolution awareness).

---

### 13. Token File Permissions Set After Rename (TOCTOU Window) **[FIXED]**

**File:** `src/api/token.rs`

```rust
// BEFORE
tokio::fs::write(&tmp, token.as_bytes()).await?;
tokio::fs::rename(&tmp, path).await?;
// Best-effort to restrict secrets on Unix
let perm = std::fs::Permissions::from_mode(0o600);
let _ = std::fs::set_permissions(path, perm);
```

The temp file is created without restricted permissions; `chmod 0600` is applied after the atomic rename.  Between the rename and the chmod there is a brief window in which the token file is world-readable (or group-readable, depending on the umask).

**Fix:** on Unix, the temp file is opened with `mode(0o600)` before any bytes are written, so the atomic rename moves a file that is already restricted.

---

### 14. Rate Limit Window Boundary Allows Request Burst

**File:** `src/api/rate_limit.rs`

The rate limiter uses a fixed-window algorithm.  A client can send `limit` requests at the end of one window and `limit` more at the start of the next, achieving `2 × limit` requests in a very short period.  This is a well-known property of fixed-window limiters.

**Recommendation:** for the most sensitive endpoint (`POST /api/v1/session`), consider a sliding-window or token-bucket algorithm.  For a loopback-only API the practical risk is low.

---

### 15. `CORS`: Missing `Access-Control-Allow-Credentials`

**File:** `src/api/cors.rs`

The CORS middleware reflects the `Origin` header back as `Access-Control-Allow-Origin` for loopback origins, which is correct.  However, it does not emit `Access-Control-Allow-Credentials: true`.  This is actually the **secure** default (credentials are not sent cross-origin), but a developer who later adds a browser-side `fetch(..., { credentials: 'include' })` call will find it silently rejected by the browser.  The intent should be made explicit in the code comment.

---

### 16. Session Sweep Only in Background Task

**File:** `src/api/mod.rs`

Expired sessions are cleaned up in two places:
1. In `create_session` (per-request, explicit).
2. In a background sweeper (every 5 minutes).

The `has_valid_session` function does not call `cleanup_expired_sessions`; it checks and removes only the **specific** session it is looking up.  Stale sessions therefore linger in the map for up to 5 minutes after expiry, contributing to the count checked by the new `MAX_SESSIONS` cap.  This is benign but worth noting.

---

## Summary Table

| # | Severity | File(s) | Fixed? |
|---|----------|---------|--------|
| 1 | HIGH | `api/auth.rs` | ✅ |
| 2 | HIGH | `i2p/sam/protocol.rs`, `api/handlers/settings.rs` | ✅ |
| 3 | HIGH | `download/service.rs` | ✅ |
| 4 | HIGH | `kad/udp_crypto.rs` | ✅ |
| 5 | MEDIUM | `api/handlers/core.rs`, `api/mod.rs` | ✅ |
| 6 | MEDIUM | `kad/udp_crypto.rs` | — (by design) |
| 7 | MEDIUM | `kad/udp_crypto.rs` | — (by design) |
| 8 | MEDIUM | `kad/udp_key.rs`, `kad/udp_crypto.rs` | — (breaking change) |
| 9 | MEDIUM | `config.rs`, `config.toml` | ✅ |
| 10 | LOW | `kad/md4.rs`, `kad/udp_crypto.rs`, `kad/packed.rs` | — (trade-off) |
| 11 | LOW | `kad/udp_crypto.rs` | — (cosmetic) |
| 12 | LOW | `api/handlers/settings.rs` | — (clarification needed) |
| 13 | LOW | `api/token.rs` | ✅ |
| 14 | LOW | `api/rate_limit.rs` | — (by design) |
| 15 | NITPICK | `api/cors.rs` | — (comment only) |
| 16 | NITPICK | `api/mod.rs` | — (cosmetic) |
