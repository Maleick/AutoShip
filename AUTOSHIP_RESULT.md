# Result: #746 — Auto Patch: Runtime Offset Auto-Detection via Pattern Scanning

## Status: PARTIAL

## Changes Made
- `docs/wiki/Configuration.md`: documented the real runtime offset flow, including compiled fallback, JSON overlay, and opt-in shadow scanning via `TEXTQUEST_SCAN_OFFSETS=1`.
- `feature-list.json`: recorded issue #746 as blocked on verification, with the current implementation status and the failing workspace checks called out.

## Tests
- Command: `cargo test -p textquest-common --lib pattern_db::tests::shadow_scan_entries_include_eqmain_placeholders -- --nocapture`
- Result: PASS
- Command: `cargo test -p textquest-common --lib offset_db::tests::overlay_from_preserves_base_offsets_while_overriding_runtime_values -- --nocapture`
- Result: PASS
- Command: `cargo test -p textquest-common --lib`
- Result: FAIL
- Command: `cargo test -p textquest-dll --lib`
- Result: FAIL
- Command: `python3 scripts/dev-preflight.py`
- Result: FAIL
- New tests added: no

## Notes
The runtime offset overlay pipeline is already present in the tree, but the full workspace remains red for unrelated reasons: `textquest-common --lib` fails in `account_safety` and `risk_model`, `textquest-dll --lib` fails on pre-existing missing `bandit`/overlay/combat exports, and `python3 scripts/dev-preflight.py` also reports unrelated formatting and missing-module issues outside this issue.
