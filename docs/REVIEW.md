# Code Review Guidelines

## Always check
- Offset addresses use `rebase()` before dereferencing — raw preferred-base values are never used as pointers
- All OS APIs are behind `#[cfg(windows)]` with `#[cfg(not(windows))]` stubs — prefer shorthand `#[cfg(windows)]` over `#[cfg(target_os = "windows")]`
- Spawn field reads are individual `proc.read::<T>(addr + OFFSET)` calls, not wholesale struct reads
- Logging uses `tracing` — no `log` crate, no `println!` outside CLI/TUI entry points
- No secrets, credentials, or `.env` files in committed code

## Style
- Prefer exhaustive `match` over chains of `if let`
- Keep platform-independent tests runnable on macOS
- New IPC commands should update both the `Command` surface and the matching `Response` handling in `textquest-common`, whether that is a dedicated response variant or `Response::CommandResult`

## Skip
- Generated files under `third_party/`
- Demo data in `textquest/src/tui/run.rs` (macOS stubs are intentionally dummy)
- Formatting-only changes (prettier/rustfmt handles this in CI)
