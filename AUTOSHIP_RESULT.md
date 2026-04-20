# AUTOSHIP_RESULT

## Issue
- #2136 M8 BLOCKER: Remove or gate relay_command() HTTP backdoor (arch + security violation)

## Scope
- Updated: `textquest-web/src/api/control.rs` (plus inline test module in same file)

## What was changed
- Added a strict allowlist for relay commands via `RelayCommandAllowed::{ReadStatus, GetLog, GetMetrics}`.
- `relay_command()` now rejects disallowed commands with `403 Forbidden` and logs a warning (`tracing::warn!`).
- Empty `command` payloads are rejected with `400 Bad Request`.
- Allowed commands are normalized and relayed as read-only command names only.
- Added module-level security note documenting why the relay is restricted.
- Added tests:
  - `control_relay_blocks_disallowed_command`
  - `control_relay_allows_readonly_commands`
- Tests assert blocked commands are not emitted on `event_tx`.

## Verification
- Ran: `cargo test -p textquest-web control_relay`
- Result: pass (2 tests)
  - `control_relay_allows_readonly_commands`
  - `control_relay_blocks_disallowed_command`

## Notes
- No edits were made outside `textquest-web/src/api/control.rs` and its local test module.
