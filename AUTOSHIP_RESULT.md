# AutoShip Result — Issue #748: Ghidra function prologue export

## Status: COMPLETE

## Plan Executed

- Add a headless-compatible Ghidra Python script under `scripts/`.
- Keep core formatting and filtering logic importable for local Python unit tests.
- Export labeled functions and bookmarked functions as JSON records with `name`, `address`, `size`, and raw prologue `bytes`.
- Document the operator workflow in the canonical wiki docs.
- Run the required verification commands where feasible.

## Files Created / Modified

| File | Action |
|------|--------|
| `scripts/ghidra_export_function_prologues.py` | Created — Ghidra headless script for exporting 32-64 byte function prologues as JSON |
| `tests/test_ghidra_export_function_prologues.py` | Created — unit tests for byte formatting, default-name filtering, bookmarked/labeled selection, argument parsing, and record construction |
| `docs/wiki/Offsets-EQ-Internals-and-MacroQuest-References.md` | Modified — added Ghidra prologue export workflow notes |

## What Was Implemented

- `ghidra_export_function_prologues.py` supports headless use via:

```bash
analyzeHeadless <project_dir> <project_name> \
  -process eqgame.exe \
  -scriptPath scripts \
  -postScript ghidra_export_function_prologues.py function_prologues.json 64
```

- The script exports a JSON array shaped like:

```json
[
  {
    "address": "0x140D9F20",
    "bytes": "48 89 5C 24 08",
    "name": "CastSpell",
    "size": 1234
  }
]
```

- Function selection includes functions with non-default labels plus functions covered by bookmarks.
- Byte count is clamped to the requested 32-64 byte range and to the function body size.
- Byte formatting normalizes signed Jython byte values into uppercase two-digit hex.

## Verification

- `python3 -m unittest tests.test_ghidra_export_function_prologues` — PASS, 5 tests.
- `cargo fmt --check` — PASS.
- `cargo clippy --all-targets --all-features -- -D warnings` — PASS.
- `cargo test` — PASS.
- `python3 scripts/dev-preflight.py` — PASS on rerun, 18 passed / 0 warnings / 0 failed.

## Notes

- The first `scripts/dev-preflight.py` run failed in `textquest-web` test `put_player_watch_config_does_not_update_state_when_disk_write_fails` after package-cache contention. The same test passed in isolation, and a full preflight rerun passed cleanly.
