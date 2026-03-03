# 3️⃣ Wire Format Proposals (CBOR vs bincode vs Protobuf)

---

## 3.1 Requirements

KAD3 wire encoding MUST support:

* Deterministic canonical encoding (for signatures)
* Compact representation
* Streaming / datagram use
* Multi-language compatibility (optional but preferred)
* Schema evolution

---

## 3.2 Candidate Formats

### Option A: CBOR (Recommended)

**Pros**

* Binary, compact
* Canonical encoding defined (RFC 8949)
* Self-describing
* Excellent for signed messages
* Good Rust support (`serde_cbor`)

**Cons**

* Slightly larger than bincode
* Needs canonical-mode enforcement

**Verdict**
✅ **Best balance for KAD3**

---

### Option B: bincode

**Pros**

* Extremely fast
* Very compact
* Excellent Rust ergonomics

**Cons**

* Rust-only ecosystem
* No canonical spec across languages
* Fragile schema evolution
* Dangerous for cryptographic signing unless frozen

**Verdict**
❌ Good for internal IPC, bad for protocol

---

### Option C: Protocol Buffers

**Pros**

* Strong schema evolution
* Multi-language
* Mature tooling

**Cons**

* Canonical encoding NOT guaranteed
* Varint ordering complicates signing
* Heavier runtime
* Less friendly to datagram-style traffic

**Verdict**
⚠️ Acceptable, but awkward for signed DHT messages

---

## 3.3 Recommendation

**KAD3 SHOULD use CBOR in canonical mode.**

Reasons:

* Deterministic byte output → safe signatures
* Flexible message evolution
* Good balance of size vs clarity
* Transport-agnostic framing

---

## 3.4 Canonical Message Encoding Rule

All KAD3 messages MUST be encoded as:

```text
CBOR(map)
  key: "type"
  key: "request_id"
  key: "src_node_id"
  key: "src_public_key"
  key: "timestamp"
  key: "payload"
```

* Canonical CBOR ordering MUST be enforced
* Signature is computed over the encoded envelope (excluding signature field)
* Signature appended as final field or transport wrapper

---

## 3.5 Transport Framing

Encoding ≠ framing.

Examples:

* UDP: single CBOR message per datagram
* QUIC/TCP: length-prefixed CBOR frames
* I2P: SAM stream message boundaries

Transport adapters MUST NOT reinterpret CBOR payloads.

---
