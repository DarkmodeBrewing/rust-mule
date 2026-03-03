# RFC-KAD3-TEST-VECTORS
## Canonical Encoding and Signature Test Vectors

**Status:** Draft  
**Category:** Informational  
**Version:** 0.1  
**Author:** rust-mule project  
**Updated:** 2026-02

---

## 1. Purpose

This document provides canonical CBOR encoding examples
for KAD3 interoperability testing.

All byte sequences are hexadecimal.

---

## 2. HELLO (Unsigned Envelope)

### 2.1 Logical Structure

```json
{
  "type": "HELLO",
  "version": 3,
  "node_id": 32 bytes of 0x11,
  "public_key": 32 bytes of 0x22,
  "nonce": 1,
  "timestamp": 1700000000,
  "capabilities": {},
  "endpoints": []
}
````

---

### 2.2 Canonical CBOR Encoding (Hex)

```text
A8
  6A 63 61 70 61 62 69 6C 69 74 69 65 73 A0
  69 65 6E 64 70 6F 69 6E 74 73 80
  69 6E 6F 64 65 5F 69 64 58 20 11...11
  65 6E 6F 6E 63 65 01
  6A 70 75 62 6C 69 63 5F 6B 65 79 58 20 22...22
  69 74 69 6D 65 73 74 61 6D 70 1A 65 CD 1D 00
  64 74 79 70 65 65 48 45 4C 4C 4F
  67 76 65 72 73 69 6F 6E 03
```

(ellipses represent repeated bytes)

---

## 3. Signature Input

The signature MUST be computed over the exact canonical CBOR
byte sequence above.

Signature algorithm MUST be specified by implementation
(e.g., Ed25519 RECOMMENDED).

---

## 4. HELLO With Signature (Example Layout)

```text
A9
  ... (same as above)
  69 73 69 67 6E 61 74 75 72 65 58 40 <64-byte-signature>
```

---

## 5. PING Example

Logical:

```json
{
  "type": "PING",
  "request_id": 42
}
```

Canonical CBOR:

```text
A2
  6A 72 65 71 75 65 73 74 5F 69 64 18 2A
  64 74 79 70 65 64 50 49 4E 47
```

---

## 6. Compliance Requirements

An implementation is compliant if:

* It produces identical canonical encoding for identical logical structures.
* It verifies signatures correctly against canonical encoding.
* It rejects non-canonical encodings when strict mode is enabled.

---

## 7. Notes

Implementers MUST ensure:

* Map keys sorted by canonical CBOR rules.
* No indefinite-length containers.
* No alternative integer encodings.

Failure to enforce canonical encoding breaks signature validation.

---
