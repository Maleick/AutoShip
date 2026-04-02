# Anti-Detection and Operator Risk

This document summarizes DMFT's current anti-detection posture and the rules for promoting outside research into roadmap work.

It is intentionally evidence-focused. It does not promise stealth or guarantee safety.

## Canonical Inputs

Use these sources in this order:

1. current code and repo docs
2. `docs/external-research/daybreak-detection-digest.md`
3. official Daybreak policy pages linked from that digest
4. clearly labeled community reporting

## Current Measures in the Repo

Current practical measures include:

- randomized DLL staging names before injection
- session-derived IPC names instead of simple fixed names
- per-session authenticated IPC using the raw token as the first pipe message
- constant-time token comparison inside the DLL
- restrictive current-user DACLs for named pipes and shared memory
- movement humanization and timing variation in higher-level behavior
- render strobing for background clients
- GM flag visibility in operator tooling

## Current Operator Implications

- authenticated IPC is tied to the injected session, not only to a PID
- reconnect-style flows depend on the retained `login_token_<pid>.bin` file
- if clients are relaunched or copied, stale token files should not be trusted
- machine hygiene matters because official Daybreak policy is broader than one single client session

## Evidence Rules

Use the following handling model:

- official Daybreak guidance can tighten milestone gates immediately
- primary docs and public code repos can create `Research-backed` slice candidates when repo fit is clear
- community reporting can suggest validation tasks or provisional slices
- exploit-oriented claims stay low-confidence until corroborated
- no community claim alone can satisfy an anti-cheat milestone exit gate

## What Not to Claim

Do not document any of the following as established fact unless they are backed by current code or stronger evidence:

- exact internal Daybreak scan mechanics
- reliable bypass claims
- speculative hook-restoration or scanner-evasion behavior
- exploit-grade zoning or travel shortcuts as normal operator features

## Near-Term `M7` Hardening Focus

Current roadmap work should focus on:

- official-policy-driven anti-cheat gates
- hook, module, string, and environment exposure review
- timing, naming, and operator-hygiene hardening
- validation tasks for packet, zoning, and orchestrator changes

## Related Docs

- `docs/external-research/daybreak-detection-digest.md`
- `docs/implementation-roadmap.md`
- `docs/wiki/Security-and-Anti-Detection-Notes.md`
