# Result: #744 — Timing-based anti-debug evasion (116 GetTickCount checks)

## Status: DONE

## Changes Made
- `textquest-common/src/ipc.rs`: added `Command::SetTimingCorrection { enabled: bool }` IPC command.
- `textquest/src/config.rs`: added `timing_correction: bool` to app config with default `false` and serde defaults; added tests.
- `config/frostreaver.toml`: added `timing_correction = false`.
- `textquest-dll/src/hooks/timing.rs` (new): added Windows timing hook module with `GetTickCount` and `QueryPerformanceCounter` hooks, overhead accounting, enable/disable behavior, and platform-independent unit tests.
- `textquest-dll/src/hooks/game_loop.rs`: recorded hook runtime with `Instant::now()` into timing module and added debug-assertion trace logging of measured overhead.
- `textquest-dll/src/hooks/mod.rs`: exported timing module and included removal path.
- `textquest-dll/src/lib.rs`: installed timing hooks in hook initialization flow.
- `textquest/src/orchestrator_loop.rs`: propagated `timing_correction` config into orchestrator and sent timing command to injected process on `ClientReady`.
- `textquest/src/cli.rs`: sends timing-correction IPC command when injection mode is used and config enables it.
- `textquest/src/orchestrator.rs`: made `send_ipc_command` public for orchestrator loop usage.

## Tests
- Command: `cargo test`
- Result: PASS
- New tests added: yes

## Notes
- `#[cfg(windows)]` and `#[cfg(not(windows))]` paths were used for timing hooks; non-Windows remains stubbed.
- QueryPerformanceCounter normalization and clamp logic was implemented with saturating subtraction behavior for safety on large hook overhead values.
