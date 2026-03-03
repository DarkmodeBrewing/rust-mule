# I2P Endpoint Semantics for KAD3 - v0.1

This document defines how **I2P endpoints** are represented, advertised, selected, and used inside KAD3 **without leaking transport assumptions into DHT logic**.

---

## 1. Design Constraints (Why I2P is special)

I2P differs from clearnet UDP/TCP in **three critical ways**:

1. **Identity = Address**

   * An I2P destination *is* a cryptographic identity.
   * There is no concept of IP ownership or NAT.

2. **Latency & Variance**

   * High RTT
   * Bursty delivery
   * Temporary unreachability is normal

3. **Multiple Messaging Modes**

   * SAM **datagrams** (UDP-like)
   * SAM **streams** (TCP-like)

KAD3 **MUST** treat I2P as a first-class transport, not a degraded UDP clone.

---

## 2. Endpoint Taxonomy

KAD3 defines **transport-neutral endpoints**, but I2P needs **subtypes**.

### 2.1 Endpoint Type Identifiers

```text
i2p_sam_datagram
i2p_sam_stream
```

These are **distinct**, not interchangeable.

---

## 3. Endpoint Representation (Normative)

### 3.1 I2P Endpoint Structure

```rust
pub struct I2pEndpoint {
    pub dest: I2pDestination,     // full public destination
    pub mode: I2pMode,             // Datagram | Stream
    pub sam_addr: SamAddress,      // local SAM control socket
    pub ttl: Option<u32>,          // optional advertisement lifetime
}
```

```rust
pub enum I2pMode {
    Datagram,
    Stream,
}
```

### 3.2 Destination Encoding

* Destinations **MUST** be advertised as full binary destinations
* Base64 MAY be used for wire encoding
* Hash-only representations are **NOT SUFFICIENT** for reachability

**Rationale:**
The hash identifies the destination, but cannot be contacted.

---

## 4. Advertising I2P Endpoints in KAD3

### 4.1 Multi-Endpoint Contacts

A KAD3 contact MAY advertise:

* only I2P endpoints
* only clearnet endpoints
* both

Example (conceptual):

```text
Contact {
  node_id: X,
  endpoints: [
    i2p_sam_datagram://<dest>,
    udp4://1.2.3.4:4665
  ]
}
```

### 4.2 Capability Flags

Contacts advertising I2P endpoints **SHOULD** include:

```text
supports_i2p = true
prefers_anonymous = true
```

This allows lookup logic to:

* prefer I2P for privacy-sensitive operations
* fall back intelligently

---

## 5. Endpoint Selection Rules (CRITICAL)

KAD3 lookup logic **MUST NOT** assume:

* low RTT
* symmetric reachability
* reliable delivery

### 5.1 Transport Preference Policy

Transport selection is **policy-driven**, not semantic.

Example policy (non-normative):

1. Prefer I2P endpoint if:

   * local node has I2P
   * target advertises I2P
2. Otherwise use clearnet
3. Never mix transports mid-request

### 5.2 Endpoint Stickiness

For a given request:

* exactly **one endpoint** MUST be selected
* retries MUST use the same endpoint type

**Reason:**
Mixing endpoints destroys failure semantics and pollutes liveness scoring.

---

## 6. Datagram vs Stream Semantics

### 6.1 SAM Datagram Mode (Preferred)

**Characteristics:**

* Message-oriented
* Lower overhead
* No connection state
* Best match for KAD traffic

**Rules:**

* Each KAD3 message → one SAM datagram
* Size MUST respect SAM datagram limits
* No ordering guarantees

**Usage:**

* PING
* HELLO
* FIND_NODE
* FIND_VALUE
* STORE

### 6.2 SAM Stream Mode (Fallback / Optional)

**Characteristics:**

* Higher latency
* Reliable
* Connection-oriented

**Rules:**

* Length-prefixed CBOR frames
* Connection lifetime MAY span multiple messages
* Stream MUST NOT imply trust or stability

**Usage:**

* bootstrap only
* large responses (if ever needed)
* environments where datagrams are blocked

---

## 7. Message Framing Over I2P

### 7.1 Datagram Framing

```text
[ CBOR(KAD3 envelope bytes) ]
```

* No extra framing
* Signature verification happens at protocol layer

### 7.2 Stream Framing

```text
[ u32 length ][ CBOR message ][ u32 length ][ CBOR message ] ...
```

Framing is **transport-local** and invisible to KAD3 logic.

---

## 8. Liveness Semantics (VERY IMPORTANT)

### 8.1 I2P Reachability Rules

* Failure to respond **MUST NOT** immediately penalize contact
* Multiple failures required before demotion
* Time-based decay preferred over binary alive/dead

### 8.2 Separate Liveness per Endpoint

A contact MAY be:

```text
alive via i2p
dead via udp
```

Routing table MUST track liveness **per endpoint**, not per node.

---

## 9. Bucket & Diversity Implications

### 9.1 Diversity Constraints

Buckets **SHOULD** enforce:

* endpoint-type diversity
* destination-hash prefix diversity

Example:

> no more than N contacts sharing the same first K bits of I2P dest hash

This reduces eclipse risk inside I2P.

---

## 10. I2P-Specific Security Notes

### 10.1 What I2P Gives You

* Strong endpoint anonymity
* No IP leakage
* Built-in crypto identity

### 10.2 What It Does NOT Give You

* Sybil resistance
* Trust
* Message authenticity (that’s KAD3’s job)

KAD3 signatures remain **mandatory** even over I2P.

---

## 11. Failure Modes & Correct Interpretation

| Situation           | Correct Interpretation |
| ------------------- | ---------------------- |
| No response         | network variance       |
| Long RTT            | normal                 |
| Temporary blackhole | expected               |
| Endpoint disappears | tunnel rebuild         |

**Never** treat I2P failures like UDP failures.

---

## 12. Clean Integration into rust-mule

### 12.1 Transport Adapter Trait (Concrete)

```rust
impl TransportAdapter for I2pSamDatagramAdapter {
    type Endpoint = I2pEndpoint;

    fn send(&self, ep: &I2pEndpoint, data: &[u8]) -> Result<(), TransportError> {
        // send via SAM DATAGRAM
    }

    fn is_reachable(&self, ep: &I2pEndpoint) -> bool {
        // soft heuristic, not binary truth
    }
}
```

### 12.2 Absolutely Forbidden

* KAD core checking `.mode == Datagram`
* Buckets knowing about SAM
* Protocol assuming ordered delivery

---
