# Result: #1028 — #861.4: Add QuitGame IPC command

Status: DONE

Changes Made:
- Added `Command::QuitGame { account_name }` to the IPC protocol, appended to preserve existing bincode enum discriminants.
- Routed `QuitGame` in the DLL game-loop dispatcher to a login-layer handler.
- Implemented `/quit` dispatch plus a 5-second game-loop monitor that reports failure if the loop keeps ticking.
- Added focused coverage for IPC roundtrip, DLL dispatch routing, command validation, and timeout detection.

Tests:
- `cargo check --workspace --tests` passed.
- Full `cargo test` and clippy were not run per issue instruction to use cargo check only.

Notes:
- Process exit is triggered through EQ's `/quit`; on failure to stop the game loop within 5 seconds, the DLL emits a failed `CommandResult`.

COMPLETE
