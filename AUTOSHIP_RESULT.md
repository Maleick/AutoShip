# Result: #1025 — #861.3: Implement LogoutSequencer

Status: DONE

Changes Made:
- Added `textquest/src/launcher/logout_sequencer.rs` with `LogoutSequencer`, `LogoutStateMachine`, command retry handling, `/disband` group leave, `/quit` IPC routing, process-exit polling, and force-kill fallback.
- Added production adapters for `CommandPipe` IPC and OS process status/termination.
- Exported the new launcher module from `textquest/src/launcher/mod.rs`.
- Added focused unit tests for group leave -> quit -> exit, timeout -> force-kill, and IPC delivery retry.

Tests:
- `cargo check` passed.
- `cargo test` was not run per instruction to use `cargo check` only.

Notes:
- Default process-exit timeout is 5 seconds.
- Default IPC command retry count is 3 attempts.
COMPLETE
