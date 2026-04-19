# Changelog

All notable changes to TextQuest are documented in this file.

## [0.7.0-alpha] - 2026-04-19

200 PRs merged since v0.6.1 — major feature push across combat, economy, TUI, testing, and CI.

### Added

- **Lua scripting foundation** — Core Lua 5.4 API bindings via mlua; `ScriptLoader` with sandboxed execution (#2083, #1161)
- **Melee automation** — Discipline scheduler, endurance tracking, 16-class melee skill rotation engine (#2040, #1623)
- **Chat log writer** — Orchestrator pipeline for structured chat capture and sink routing (#2081, #2020)
- **GM detection** — Account safety monitoring, suspicious-name scoring, tell pattern matching (#2035, #1153)
- **Audio alerts** — `AlertType` enum, `AudioPlayer` with configurable TOML-driven alert mappings (#2042)
- **Packet hook activation** — Explicit WSASend/WSARecv validation path for injection testing (#2013)
- **Clipboard + scratchpad** — Export to clipboard, operator scratchpad with note persistence (#2041, #1881)
- **Admin backup/restore** — `textquest-admin` CLI: `backup list`, `backup restore` commands (#2031, #1733)
- **Economy tracking** — Wishlist intent (Keep/Sell/Bank/Distribute), ledger trend summaries, EconomyLedger entropy calc (#2024, #2025, #1534-#1537)
- **Baseline scorecard** — M9 behavior optimization: per-client performance baseline and delta measurement (#2023)
- **Common mock infrastructure** — Shared test mocks for IPC, client registry, orchestrator state (#2090, #2091, #870)

### Fixed

- **Security** — LLM `base_url` userinfo bypass hardened (#2074); trusted-origin enforcement on admin session endpoints (#2071)
- **PEB unlink** — Widened pointer guard for x64 user-space addresses (#2070)
- **Web SDK** — `TimestampConfig` payload shape restored for timestamp-config endpoints (#2059)
- **Session control** — Record creation capped to `max_records` limit (was ignored in tests) (#2046)
- **Spawn delta** — Duplicate `spawn_type` assignment removed (#2075); pattern-specific watchlist endpoint fixed (#2076)
- **Ability cooldowns** — Duplicate constant/struct definitions removed (#2056, #2058)
- **textquest.toml permissions** — Box-chat settings update now preserves file mode (#2072)

### Testing

- Map rendering coordinate transforms, zone exit markers, full render pipeline integration (#1986, #2000, #1994, #1990)
- 23 CLI integration tests for full admin flow (#1968, #1116)
- Admin dashboard performance, log pagination, backup panel coverage (#2082)
- Expanded `textquest-common` unit coverage (#1958)

### CI

- Clippy enforced as hard error across all workflows (#2039, #1971)
- CI/CD quality gates + pre-commit hooks (#1963, #1255)
- Self-hosted runners across all workflows — no GitHub-hosted runners (#1930)
- Docs-only gate bypass patched (#2069)

### Docs

- Troubleshooting decision tree (26 failure modes) (#2036, #1212)
- Map rendering pipeline docs and inline comments (#2038, #1195)
- Comprehensive MQ2/KissAssist/RGMercs/RedGuides research consolidated
- Docs cleanup: 13 stale files deleted, 12 archived, 15 wiki pages consolidated

### Accounts / Platform

- 26 of 36 Daybreak accounts created; `accounts.toml` updated with group assignments
- macOS: compile and test only — demo mode temporarily disabled pending module ungating
- Windows: full pipeline operational on Frostreaver (Tailscale SSH, MSVC nightly)

---

## [0.6.1] - 2026-04-07

### Fixed

- **README metrics hardening** — `update_readme_metrics.py` now includes a source-based fallback (regex scan for `#[test]`) to accurately count tests when the `cargo test --workspace` runner is unavailable (`FileNotFoundError`) or a successful run yields zero parsed tests (common in macOS demo mode). A non-zero `cargo test` exit now surfaces the failure immediately (`sys.exit(1)`) instead of silently falling back.
- **Handoff automation** — `scripts/gen-handoff.sh` now auto-generates `HANDOFF.md` with live repo stats (lines of code, crate versions, recent commits, and test results).

### Removed

- **Stale MCP configuration** — Removed legacy `.mcp.json` referencing local development server paths.

## [0.6.0] - 2026-04-06

### Added

- **Main-thread login hook** — `LoginController::GiveTime()` HWBP hook (DR1) provides main-thread execution during eqmain, matching MQ2 AutoLogin's `OnPulse()` architecture. All login UI operations now run on EQ's main thread instead of a background thread.
- **`LoginServerAPI::JoinServer()`** — Direct API call for server selection, bypassing the "PLAY EVERQUEST!" button click entirely. Falls back to vtable button click if the API isn't available.
- **`textquest autologin` CLI command** — End-to-end spawn → inject → login pipeline. Reads accounts from `config/accounts.toml`, supports per-account passwords from `config/.credentials` (TSV), `--spawn` flag for launching new EQ processes, configurable inject delay.
- **`click_button_for_phase()` helper** — Phase-aware button clicking that selects direct vtable click (eqmain) vs game loop queue (eqgame) based on context, preventing the recurring "queued click never drains" bug class.
- **Screen-state tracing** — Login phases log which SIDL windows are visible (connect, serverselect, yesnodialog, okdialog) for diagnostic purposes.
- **InteractTarget command** — `CEverQuest::RightClickedOnPlayer` for NPC interaction via IPC.
- **MQ2-style `/door` and `/click right target` slash handling** — rewrites `/door` to `/doortarget` and routes `/click right target` through the DLL interaction path.
- **Accounts groups 3-6** — 24 placeholder accounts in `accounts.toml` for 36-box setup.

### Fixed

- **Critical: `click_yesno_yes()` wrong argument** — Was called with `eqmain_base` (DLL base address) instead of dialog window pointer. Would crash EQ or silently fail when "already logged in" dialog appeared.
- **Login button false positive** — "PLAY EVERQUEST!" text exists on login screen as branding label. Phase 2 now uses SIDL window detection (`serverselect`) instead of text scan.
- **Login button wrong candidate** — Early-break optimization found menu tab instead of form submit button. Now scans for both LOGIN candidates (>= 2).
- **`queue_button_click()` during eqmain** — `ProcessGameEvents` hook only activates after eqgame.exe loads. During eqmain, queued clicks never execute. Now uses direct vtable clicks.
- **Cross-thread HWBP** — Debug registers are per-thread. `SetThreadContext` now targets EQ's main thread via `NtSetContextThread` indirect syscall, not the init pool thread.
- **EQ window class** — Corrected from `"EverQuest"` to `"_EverQuestwndclass"` for `FindWindowA`.
- **Credential zeroization** — `load_credentials_file()` returns `Zeroizing<String>` passwords. Consistent with all other credential paths.
- **Navigation heading formula** — Corrected to match MQ2's `atan2(dx, dy)`.

### Removed

- **Background thread login** — `login_chain_phase2()` removed. All login operations now run on EQ's main thread via GiveTime hook.
- **PostMessage credential entry** — `type_password_wm_char()` and `simulate_enter_key()` removed from login flow. MQ2 proves CXStr write + vtable click on main thread is sufficient.

### Architecture

- Login flow now matches MQ2 AutoLogin: FSM-driven, main-thread execution, direct EQ API calls. No `PostMessage`, no background threads, no message-based input simulation.

## [0.5.0] - 2026-03-29

- M5 Anti-Cheat milestone (~70%): reflective injection, HWBP hooks, sleep obfuscation, indirect syscalls
- M6 Web Dashboard (~55%): axum + React/Vite/Tailwind SPA
- Login automation: credential store, process spawner, login FSM, launch coordinator
