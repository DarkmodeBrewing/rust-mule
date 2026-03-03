## 2.1 KAD3 HELLO Handshake

### Purpose

* Bind NodeID ⇄ Public Key
* Verify reachability
* Establish protocol compatibility

### Mermaid Diagram

````md
```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> HelloSent : send HELLO
    HelloSent --> HelloReceived : receive HELLO
    HelloReceived --> Verified : signature + NodeID valid
    HelloReceived --> Rejected : invalid identity

    HelloSent --> Timeout : no response
    Timeout --> [*]

    Verified --> [*]
    Rejected --> [*]
````

````

### Plain-text equivalent (for sanity checks)

```text
IDLE
  |
  | send HELLO
  v
HELLO_SENT
  |
  | receive HELLO
  v
HELLO_RECEIVED
  |
  |-- valid signature + NodeID --> VERIFIED
  |
  |-- invalid ------------------> REJECTED
````

**Rules:**

* VERIFIED peers may become contacts
* REJECTED peers must not be retried aggressively
* Timeout peers are demoted, not blacklisted

---

## 2.2 KAD2 → KAD3 Promotion Pipeline

### Purpose

Prevent routing table poisoning from legacy KAD2 data.

### Mermaid Diagram

````md
```mermaid
stateDiagram-v2
    [*] --> Hint

    Hint --> Probed : attempt KAD3 HELLO
    Probed --> Verified : handshake OK
    Probed --> Failed : timeout / invalid

    Failed --> Hint : retry later (rate-limited)
    Verified --> Contact

    Contact --> [*]
````

````

### Plain-text equivalent

```text
HINT (KAD2)
  |
  | probe with KAD3 HELLO
  v
PROBED
  |
  |-- success --> VERIFIED --> CONTACT (KAD3 routing table)
  |
  |-- failure --> FAILED --> back to HINT (with penalties)
````

**Hard invariant:**

> A `Hint` can NEVER become a `Contact` without a full KAD3 HELLO.

---