# Result: #774 — Add spawn create/destroy event hooks

## Status: DONE

## Changes Made
- `textquest-common/src/offsets.rs`: Added `PLAYER_MANAGER_CREATE_PLAYER` and `PLAYER_MANAGER_PREP_DESTROY_PLAYER` placeholders set to `0x0`.
- `textquest-common/src/ipc.rs`: Added `SpawnEventKind` and `SpawnEvent` types plus command/response plumbing for spawn event polling.
- `textquest-dll/src/hooks/game_loop.rs`: Implemented spawn-list delta detection (`Created`/`Destroyed`) and unit tests for transition logic.
- `textquest-dll/src/ipc/mod.rs`: Added handling and queuing of spawn deltas from the game loop frame.
- `textquest/src/orchestrator.rs`: Exposed poll API to fetch spawn events for a PID and clear batches safely.
- `textquest/src/tui/app.rs`: Added spawn alert feed handling for incoming spawn events.
- `textquest/src/tui/run.rs`: Wired spawn event polling each tick and fed events into the TUI app state.

## Tests
- Command: `cargo test`
- Result: PASS
- New tests added: yes

## Notes
- `cargo test` passes with one existing warning: `warning: unused variable: zone` in `textquest/src/tui/app.rs`.
- IPC wire framing tests were initially affected by enum variant ordering changes; resolved by keeping command/response enum variants in order-safe positions.
