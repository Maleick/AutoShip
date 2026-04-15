# Charm Break Detection + Auto Re-Charm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect charm breaks in the DLL combat FSM, immediately attempt to re-charm the broken pet, and stop retrying after bounded terminal failures so normal combat can clean up.

**Architecture:** Keep the behavior inside `textquest-dll`'s existing combat FSM instead of adding a second automation loop. Track the currently charmed mob from successful charm casts, detect a break when that mob leaves `MyPet` and becomes hostile again, then reuse the existing cast retry machinery for retryable re-charm failures and add a separate bounded counter for terminal re-charm failures.

**Tech Stack:** Rust, `textquest-dll`, `textquest-common`, existing combat FSM/unit-test harness

---

### Task 1: Lock the behavior with focused FSM tests

**Files:**
- Modify: `textquest-dll/src/combat/state.rs`
- Test: `textquest-dll/src/combat/state.rs`

- [ ] Add failing unit tests that cover charm-break detection, immediate re-charm intent, and terminal failure fallback.
- [ ] Run targeted Rust tests for the new cases and confirm they fail for the expected missing behavior.

### Task 2: Implement charm-state tracking in the combat FSM

**Files:**
- Modify: `textquest-dll/src/combat/state.rs`

- [ ] Add internal charm-tracking state to `Combatant` and helper functions to resolve the `Charm` spell line, detect breaks from pet/xtarget state, and decide when re-charm should preempt the normal rotation.
- [ ] Wire the new state into cast lifecycle handling so successful charm casts establish ownership, retryable failures continue to use `CastRetryPolicy`, and terminal failures count toward bounded fallback.

### Task 3: Document the new runtime behavior

**Files:**
- Modify: `docs/wiki/Class-Combat-Rotations.md`

- [ ] Update the wiki page to describe automated charm-break handling, the retry/fallback behavior, and the current scope of supported charm automation in the DLL.

### Task 4: Validate the final behavior

**Files:**
- Modify: `textquest-dll/src/combat/state.rs`
- Modify: `docs/wiki/Class-Combat-Rotations.md`

- [ ] Run targeted Rust tests for `textquest-dll` combat state and enchanter-related coverage.
- [ ] Review the diff for issue-scope correctness, then prepare the branch/PR flow against `TextQuest#1582`.
