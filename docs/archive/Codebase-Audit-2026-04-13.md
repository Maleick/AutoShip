# Codebase Audit — April 13, 2026

## Scope

This audit pass focused on three goals:

1. Identify immediately repairable code defects.
2. Measure gap coverage for unwired/placeholder paths.
3. Consolidate findings into the canonical wiki surface with concrete follow-up slices.

## Checks Run

- `cargo test -q`
- `rg -n "TODO|FIXME|TBD|placeholder|unimplemented!\(|todo!\(" textquest textquest-common textquest-dll textquest-web docs/wiki tests --glob '!web/package-lock.json'`

## Immediate Repairs Landed

1. Removed a duplicate `Command::SetTimingCorrection` enum variant that broke `textquest-common` compilation.
2. Closed a missing-brace/function-boundary defect in DLL hook installation flow so `install_hooks()` now terminates correctly and delegates to `install_remaining_hooks()`.
3. Closed a missing loop delimiter in `SoulCoordinator::can_request()` to restore syntactic correctness.
4. Fixed missing `HwbpSlot` import in `packet_hook.rs`.
5. Re-enabled `hooks::detours` module export in `hooks/mod.rs`.
6. Removed obsolete shadow-scan function duplication in `textquest-dll/src/lib.rs` (duplicate `scan_offsets` definitions).

## Gap Coverage Findings (Unwired / Placeholder)

### High-signal unwired paths

- TUI injection command remains a placeholder message path (`:inject`) and is still documented as such.
- Web API still has explicit placeholder endpoints and TODO-backed persistence paths.
- DLL EQ offsets include explicit `0x0` placeholders for move/cast/target.
- Soul suppression and zone/looting signals still carry TODO placeholders.

### Build-integrity gap (critical)

Current workspace build is red with broad unresolved integration drift. The most critical unresolved classes are:

- duplicate/partial module wiring and exports
- missing fields/methods across Soul/TUI/Web integration surfaces
- stale API references and struct shape mismatches

This is a **repo-wide integration issue** that exceeds a safe single-pass repair and should be decomposed into focused GitHub sub-issues.

## Recommended Decomposition (Parent + Sub-issues)

Parent issue: **Restore workspace compile/test integrity after integration drift**

Suggested sub-issues (single-concern slices):

1. `textquest/src/lib.rs` duplicate module declaration cleanup (`testing` duplicated).
2. `textquest/src/loot/mod.rs` wishlist module wiring restore.
3. `textquest/src/soul/*` config/schema reconciliation (`suppression`, audit, request counters, validator fields).
4. `textquest/src/tui/*` hook-rotation state schema reconciliation.
5. `textquest/src/tui/app.rs` return-type and command-path compile fixes.
6. `textquest-web/src/api/economy.rs` + `AppState` field contract repair.
7. `textquest-web/src/main.rs` soul route wiring parity (`list_soul_states`, `get_soul_state`).

## Documentation Consolidation Notes

- This page is now the canonical point-in-time audit snapshot for April 13, 2026.
- Historical research pages remain valid, but active remediation status should be tracked from this page + child issues until compile integrity returns.
