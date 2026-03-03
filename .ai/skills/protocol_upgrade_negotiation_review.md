# Skill: ProtocolUpgradeNegotiationReview

## Purpose

Review or design protocol capability negotiation / upgrade paths between:

- Legacy protocol (e.g., KAD2/iMule-compatible behavior)
  and
- A future protocol (e.g., KAD3 with stronger crypto / new wire formats)

Goal: prevent downgrade, confusion, and partial-deployment failures.

## When to Use

- You’re adding a "supports KAD3" capability
- You’re defining a new handshake/HELLO extension
- You’re defining "dual-stack" behavior where nodes speak both versions
- You’re introducing new crypto, IDs, or message formats that must coexist

## Design Requirements (non-negotiable)

1. **Explicit capability signaling**
   - No inference from timing or message content.
2. **No silent downgrade**
   - If both peers claim support, use the stronger mode unless policy forbids it.
3. **Transcript binding**
   - Negotiation result must be integrity-protected and bound to:
     - peer identity (or stable identifier),
     - selected version,
     - selected capability set,
     - nonce/challenge to prevent replay.
4. **Feature separation**
   - Avoid "same message type, different semantics" unless versioned clearly.
5. **Partial deployment safe**
   - Must behave sensibly when:
     - only one peer supports KAD3,
     - middleboxes drop unknown fields,
     - peers lie about capabilities.
6. **Operational resilience**
   - Timeouts, retries, and caching must not create amplification or memory growth.

## Threats to Explicitly Check

- Downgrade attacks (strip capability, force legacy crypto)
- Version confusion (peer interprets message as different version)
- Replay of negotiation tokens
- Reflection/amplification (negotiation triggers big responses)
- State exhaustion (many half-open negotiation states)
- Fingerprinting (capabilities reveal client identity too easily)

## Preferred Negotiation Pattern (recommended baseline)

- Step 1: Legacy-compatible HELLO (minimal, safe)
- Step 2: Capability offer/answer using:
  - a compact capability bitmap + version range
  - a fresh nonce/challenge from each side
- Step 3: Upgrade confirmation message protected by:
  - MAC (preferred) using a derived session key
  - or signature if you already have long-term identity keys
- Step 4: Switch to upgraded message formats only after confirmation

## Constraints

- Do NOT implement; this skill is review/design only.
- Do NOT change existing compatibility promises unless explicitly instructed.

## Output Format

Return:

### A) Summary

- proposed modes (legacy, upgraded)
- how negotiation works at a high level

### B) Compatibility Matrix

A small matrix showing behaviors for:

- legacy ↔ legacy
- legacy ↔ upgraded
- upgraded ↔ upgraded
  and what each side sends/accepts.

### C) Risks & Findings

List findings using the standard template in `.ai/meta.md`.

### D) Recommendations

Numbered, actionable design recommendations:

- "must"
- "should"
- "nice-to-have"

### E) Test Plan

A short list of test cases:

- downgrade attempt
- replay attempt
- capability stripping
- mixed-version network
- fuzzed negotiation fields
- timeout & retry behavior under packet loss
