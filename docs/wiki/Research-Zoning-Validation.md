# Zoning Queue Flush and Safe-Coordinate Validation

Curated `M6` validation note for issue `#108`.

This document turns the April 2026 packet/zoning import set into explicit recovery checkpoints. It does not treat imported reverse-engineering notes as live proof. The current evidence state for the checkpoint set remains `Needs Live Proof`.

## Scope

- queue flush before a zone transition begins
- zone loading timeout handling
- safe-coordinate recovery after failure or completion paths

## Source Stack

Primary repo inputs:

- `docs/implementation-roadmap.md`
- `docs/research-imports/2026-04-02-packet-zoning/EQ_Zoning_System.md`

Supporting raw input:

- `docs/research-imports/2026-04-02-packet-zoning/movement-validation.raw.txt`

Evidence handling notes:

- `EQ_Zoning_System.md` is a curated import summary and is the highest-confidence source in this packet/zoning import set.
- `movement-validation.raw.txt` is explicitly quarantined raw input. It may suggest validation targets, but it is not milestone proof on its own.

## Current Imported Findings

### Queue flush before zoning

Imported finding:

- `ExecuteZoneTransition` calls `FlushMovementQueue` before zone validation and before the client enters the loading loop.
- The raw movement note also ties `FlushMovementQueue` to immediate movement-history submission.

Repo-fit implication:

- DMFT should treat queue flush as a pre-zone checkpoint, not an assumed guarantee.
- Any operator-facing zoning or movement recovery work should be able to distinguish "navigation still issuing movement" from "movement queue drained and zone handoff in progress."

Current evidence state:

- `Research-backed` for the existence of the checkpoint in imported notes
- `Needs Live Proof` for DMFT behavior on a current live client

### Zone loading timeout

Imported finding:

- the zone loading state machine polls for zone data and times out after roughly 120 seconds
- imported game-state notes call out `0xFD` as the zone-timeout error state

Repo-fit implication:

- DMFT needs an operator-visible timeout checkpoint for zoning work instead of collapsing all slow or failed transitions into a generic "stuck" bucket.
- Follow-on implementation should preserve the distinction between a route blockage, a zone denial, and a zone load timeout.

Current evidence state:

- `Research-backed` for the imported timeout path
- `Needs Live Proof` for timeout timing, observable client state, and safe recovery on the current build

### Safe-coordinate recovery

Imported finding:

- `MoveLocalPlayerToSafeCoords` is part of the success-path post-load flow in the imported zoning summary.
- the same import set also ties safe-coordinate handling to failure paths such as level-gated denial and certain same-zone teleport or recovery cases.

Repo-fit implication:

- DMFT should model safe-coordinate recovery as a named checkpoint rather than as an opaque movement correction.
- Follow-on operator surfaces should be able to call out when the client has fallen back to a safe-coordinate restore versus when the route is simply recomputing.

Current evidence state:

- `Research-backed` for the existence of safe-coordinate recovery hooks in the imported notes
- `Needs Live Proof` for which zone types and failure paths actually invoke it on the current client

## Validation Checkpoint Matrix

### Pre-zone queue flush

- Imported basis: `ExecuteZoneTransition` calls `FlushMovementQueue` before validation.
- What DMFT can claim now: this is a research-backed checkpoint candidate.
- Live-proof requirement: capture a live zone-line or zone-request transition and confirm movement stops issuing before zone handoff.

### Zone load timeout

- Imported basis: the zone loop waits for data and fails after about 120 seconds; timeout state noted as `0xFD`.
- What DMFT can claim now: timeout is a named provisional failure mode.
- Live-proof requirement: observe a controlled timeout on a current test client and record visible state, duration, and recovery outcome.

### Safe-coordinate recovery

- Imported basis: the imported zoning summary and raw movement notes tie recovery to safe coordinates.
- What DMFT can claim now: safe-coordinate recovery is a validation target.
- Live-proof requirement: observe at least one current-build case where the client restores to safe coordinates and record the trigger category.

## Live Validation Tasks

### 1. Queue flush checkpoint

Target:

- normal zone-line crossing on a current Windows test client

Record:

- whether movement input or nav output is still being issued immediately before the zone handoff
- whether the client enters the zoning path cleanly without a leftover movement burst after the request
- any operator-visible signal that distinguishes "draining movement" from "loading zone"

Pass condition:

- the observed run supports a distinct pre-zone queue-drain checkpoint, or the imported assumption is invalidated with notes

### 2. Timeout checkpoint

Target:

- a controlled test scenario that allows a zone request to stall or fail without using exploit-style inputs

Record:

- elapsed time from zone handoff to timeout or failure
- visible client state during the wait
- whether the client returns to the prior location, safe coordinates, login flow, or a different failure state

Pass condition:

- the current client exposes a reproducible timeout or denial path that can be named in `M6` docs without relying only on imported notes

### 3. Safe-coordinate recovery checkpoint

Target:

- one same-zone recovery case and one denied-or-failed zone case when feasible

Record:

- trigger category
- resulting player position and whether it matches a known safe point
- whether recovery happens after load completion, after denial, or after timeout

Pass condition:

- at least one current-build recovery case is confirmed and documented with a clear trigger category

## Operator and Follow-On Guidance

- Do not describe queue flush, timeout handling, or safe-coordinate recovery as live-validated behavior until a current client run captures them.
- Keep risky travel claims, exploit-style travel, and bypass-oriented movement work out of this validation track.
- When `M6` operator-facing work is implemented, expose zoning timeout and safe-coordinate recovery as distinct states rather than burying them inside a generic stuck label.

## Proposed Follow-On Surfaces

- `M6` docs or TUI slices for explicit zoning state and failure labels
- operator logging that distinguishes route blockage, zone denial, timeout, and safe-coordinate recovery
- validation notes that map specific zone types to confirmed recovery behavior
