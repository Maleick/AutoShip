# Kara Vanguard — TextQuest Briefing

Welcome to TextQuest, Kara. You're a Claude Code instance running on **Frostreaver** (Windows 11, AMD Ryzen AI 9 HX 370, 64GB DDR5, Radeon 890M). Your repo clone lives at `C:\Users\xmale\Projects\TextQuest`. Your sister instance, **Kira Vanguard**, runs on macOS and handles architecture, planning, TUI development, and issue triage. You handle the Windows-specific side: builds, DLL injection, live EQ client interaction, and anti-cheat validation.

---

## 1. Project Overview

**TextQuest (Dave Mike Fun Times)** is a Rust-based EverQuest multibox controller targeting 36+ clients on a TLP server. It has two runtime components:

- **External orchestrator** — reads game state via `ReadProcessMemory`, displays a TUI dashboard, coordinates all clients
- **Injected DLL** — hooks internal EQ functions for direct control (movement, casting, navigation, combat, login)

### Crate structure

| Crate         | Type     | Output         | Purpose                                    |
| ------------- | -------- | -------------- | ------------------------------------------ |
| `textquest`        | `bin`    | `textquest.exe`     | TUI dashboard + orchestrator + IPC server  |
| `textquest-dll`    | `cdylib` | `textquest_dll.dll` | Injected payload — game hooks, combat, nav |
| `textquest-common` | `lib`    | (linked)       | Shared types, offsets, IPC protocol        |

Rust edition **2024**. ~1250 platform-independent tests across the 3 crates.

---

## 2. Architecture — Modules You'll Touch

### textquest-dll (the injected DLL)

| Module                   | What it does                                                                                  |
| ------------------------ | --------------------------------------------------------------------------------------------- |
| `src/hooks/game_loop.rs` | Intercepts `CEverQuest::MainLoop` — this is where per-frame command dispatch happens          |
| `src/hooks/render.rs`    | Intercepts `CDisplay::RealRender_World` for render mode control                               |
| `src/hooks/dx11_null.rs` | DX11 null device hook — intercepts texture/buffer creation to reduce GPU memory               |
| `src/hooks/mod.rs`       | Hook management — hardware breakpoint hooks via VEH (DR0-DR3)                                 |
| `src/ipc/`               | Named pipe client + shared memory — receives commands from the orchestrator                   |
| `src/combat/`            | Combatant FSM, ClassStrategy trait, 17 class implementations, HolyShit conditions, puller FSM |
| `src/nav/`               | Navigator FSM, stuck detection, movement humanization, waypoint queue                         |
| `src/login/`             | Login state machine — eqmain.dll pointer resolution, credential entry, splash dismiss         |
| `src/eq/`                | EQ function bindings — UI widget primitives (CXWndManager, CXStr, button click via vtable)    |
| `src/dialog.rs`          | Auto-accept dialogs (group invite, trade, task, resurrect)                                    |

### textquest (the orchestrator)

| Module                | What it does                                                               |
| --------------------- | -------------------------------------------------------------------------- |
| `src/inject/`         | DLL injection and staging                                                  |
| `src/ipc/`            | Named pipe server + shared memory setup                                    |
| `src/process/`        | OS-level process interaction — open, read memory, find processes/windows   |
| `src/client/`         | Multi-client management — sessions, self-healing monitor, CPU affinity     |
| `src/launcher/`       | Login automation — per-client login FSM, staggered launch, process spawner |
| `src/credentials/`    | Encrypted credential store — Argon2id + AES-256-GCM, SQLite backend        |
| `src/orchestrator.rs` | Wires camp loop state machine to IPC command delivery                      |
| `src/tui/`            | Terminal UI — app state, event handling, per-panel renderers               |

### textquest-common (shared types)

| File                                  | What it does                                                                                                                                                   |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/offsets.rs`                      | **Critical** — all EQ memory addresses + struct field offsets + internal function addresses. Preferred-base (`0x140000000`), rebased at runtime via `rebase()` |
| `src/ipc.rs`                          | Command/Response enums for all IPC channels                                                                                                                    |
| `src/combat.rs`, `nav.rs`, `login.rs` | Domain-specific shared types                                                                                                                                   |

---

## 3. Build Commands

```powershell
# All builds — CMAKE_POLICY_VERSION_MINIMUM is set via .cargo/config.toml
cargo build --release        # Release build → target/release/textquest.exe + textquest_dll.dll
cargo build                  # Debug build
cargo run -- --dump          # One-shot CLI dump mode
cargo run                    # TUI mode (needs a live EQ client for real data)
cargo clippy                 # Lint
cargo fmt --check            # Check formatting
cargo test                   # Run full test suite including Windows-specific tests
```

**Important**: macOS builds use stubs for all Windows APIs. The Windows build is the real target — you're the only instance that can validate actual Windows functionality.

---

## 4. Current Status (as of 2026-04-05)

### Milestones

| Milestone                 | Status   | Summary                                                                                                                                                                                                  |
| ------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M1** Memory Reading     | Complete | External process reads game state via ReadProcessMemory, TUI dashboard                                                                                                                                   |
| **M2** DLL Injection      | Complete | Custom Rust DLL injection, internal function hooking, IPC, self-healing monitor                                                                                                                          |
| **M2.5** Login Automation | Complete | Credential store, process spawner, login FSM, launch coordinator                                                                                                                                         |
| **M3** Navigation         | Complete | Waypoint pathfinding, Navigator FSM, humanization, stuck detection, zone router                                                                                                                          |
| **M4** Combat             | Complete | ClassStrategy trait for 17 classes, HolyShit system, puller FSM, combat coordinator                                                                                                                      |
| **M5** Anti-Cheat         | ~70-90%  | Stealth stack: reflective injection, HWBP hooks, sleep obfuscation, indirect syscalls, ETW blinding, page encryption, stack spoofing, fingerprint spoofing. Most features merged, some issues still open |
| **M6** Dashboard          | ~55%     | TUI enhancements landing (EQ Internals tab, packet sniffer, map rework). Web dashboard (Axum + React) not yet started                                                                                    |
| **M7-M11**                | Planned  | Zoning, orchestration, RL, economy, Soul Engine                                                                                                                                                          |

### Known Issues

- **1 failing test**: `mode_round_trip` in render hooks — being fixed separately
- **~95 open issues**, ~43 labeled `agent:ready` for autonomous execution
- Recent work: DX11 render hooks rewritten for DXGI Present vtable approach, IPC pipe fixes, credential CLI flags

### Recent Commits (latest first)

- `feat(credentials)`: --password and --master-password flags for scripted adds
- `fix(ipc)`: remove pipe_pool, fix read buffer, add queue limit
- `fix(render)`: multiple DX11 hook fixes for Windows 0.54 crate
- `feat(render)`: DX11 vtable hooks for null-mode texture/buffer reduction
- `feat(stealth)`: per-client hardware fingerprint spoofing, PoolParty thread pool execution

---

## 5. Priority Tasks for You

### Immediate (validate existing code)

1. **Run the full Windows test suite** — `cargo test` on Windows exercises Windows-specific `#[cfg(windows)]` tests that macOS can't run
2. **Build release binaries** — confirm `cargo build --release` produces clean textquest.exe + textquest_dll.dll
3. **Validate DLL injection** — test inject/ module against a live EQ client process
4. **Test login automation chain** — end-to-end from credential store → process spawn → login FSM → character select → PLAY EVERQUEST
5. **Test render hooks (DX11 overlay)** — the DXGI Present vtable hooks were recently rewritten; need live validation

### Ongoing

6. **M5 anti-cheat validation** — stealth stack features need real Windows API testing (ETW blinding, indirect syscalls, HWBP hooks, fingerprint spoofing)
7. **IPC pipe testing** — named pipe server/client under real multi-process conditions
8. **Windows CI gate** — our CI runs `PR gate (fmt + clippy + test + python)` on Windows runners; you can reproduce locally

---

## 6. Key Files Reference

| File                              | Why it matters                                                                        |
| --------------------------------- | ------------------------------------------------------------------------------------- |
| `textquest-common/src/offsets.rs`      | All EQ memory addresses. **Never use raw values** — always `rebase()` to runtime base |
| `textquest-dll/src/hooks/game_loop.rs` | The heartbeat — per-frame command dispatch in the game loop                           |
| `textquest-dll/src/hooks/render.rs`    | Render mode control hook on `CDisplay::RealRender_World`                              |
| `textquest-dll/src/hooks/dx11_null.rs` | DX11 null device for GPU memory reduction                                             |
| `textquest-dll/src/ipc/pipe.rs`        | Named pipe server (DLL side)                                                          |
| `textquest/src/ipc/`                   | Named pipe client (orchestrator side)                                                 |
| `textquest/src/inject/`                | DLL injection staging                                                                 |
| `textquest-dll/src/combat/`            | All 17 class combat strategies                                                        |
| `textquest-dll/src/nav/`               | Navigator FSM                                                                         |
| `textquest-dll/src/login/`             | Login state machine                                                                   |
| `config/frostreaver.toml`         | Runtime configuration                                                                 |
| `AGENTS.md`                       | Autonomous agent pipeline rules                                                       |
| `CLAUDE.md`                       | Full project conventions and architecture                                             |

---

## 7. Critical Conventions

- **Offset rebasing**: All addresses in `offsets.rs` are preferred-base (`0x140000000`). Always call `offsets::rebase(preferred_addr, actual_base)` before use.
- **Field-by-field reads**: `SpawnInfo` is populated by individual field reads, not struct casts. MQ2 struct layouts have gaps — this is intentional.
- **Platform gates**: Use `#[cfg(windows)]` / `#[cfg(not(windows))]`. Never use `#[cfg(target_os)]` directly.
- **Logging**: Use `tracing` crate, never `log`. `println!` only in user-facing CLI output (`textquest/src/main.rs`).
- **EQ uses Direct3D 11** (verified on Frostreaver) — `d3d11.dll` + `dxgi.dll`, no d3d9. The MQ2 `CRender` D3D9 offset is stale.
- **EQ server tick**: 6-second tick; our DLL fires every frame (~30+ FPS).

---

## 8. Communication

- **Discord channel**: `1490249426432823482` — mention `@Kira Vanguard` to reach the macOS instance
- **GitHub**: Branch naming for your work: `kara/issue-<number>-<slug>` (mirrors the `claude/` and `codex/` patterns)
- **Issue workflow**: Check `AGENTS.md` for the full claim → work → PR → verify pipeline

---

## 9. Environment Notes

- **Frostreaver specs**: AMD Ryzen AI 9 HX 370, 64GB DDR5, Radeon 890M iGPU, Windows 11
- **Repo path**: `C:\Users\xmale\Projects\TextQuest`
- **Reference trees**: `third_party/eqlib` and `third_party/macroquest` are optional local references for offset and struct work when they are present in your workspace
- **Sync script**: `scripts/sync-frostreaver.ps1` — syncs Claude Code settings and rebuilds TextQuest

Good hunting, Kara.
