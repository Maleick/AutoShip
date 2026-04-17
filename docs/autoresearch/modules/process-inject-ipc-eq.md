# Process / Injection / IPC / Game Modules

Files: `process/{memory,mod,window}.rs`, `inject/{dll_prep,loader,mod,reflective}.rs`, `ipc/{mod,pipe,shared}.rs`, `eq/{cheater,gm_detector,hvt,log_parser,log_watcher,map_parser,mod,named_db,named_tracker,spawn,spawn_alert,spawn_filter,structs}.rs`.

This group is where `textquest` bridges "host-side Rust" and "EQ client memory / DLL". All four submodules share a single constraint: operate on live game state without being detected.

---

## `process/memory.rs`

- **Purpose.** Safe, RAII wrapper around `ReadProcessMemory` for remote reads from EQ clients.
- **Public API.**
  - `ProcessHandle::open(pid: u32)` — opens with `PROCESS_VM_READ`.
  - `ProcessHandle::module_base()` — via `EnumProcessModules`.
  - `ProcessHandle::read<T: Copy>(addr)` — typed read.
  - `ProcessHandle::read_ptr(addr)` — 64-bit pointer read.
  - `ProcessHandle::chase_ptr(base, offsets: &[usize])` — follow a pointer chain.
  - `ProcessHandle::read_bytes(addr, count)` — raw buffer for hex dumps.
  - `ProcessHandle::read_string(addr, max_len)` — reads `max_len` bytes, truncates at the first NUL, decodes with `String::from_utf8_lossy` (UTF-8 bytes; **not** a wide-string reader).
  - `is_probably_valid_process_ptr(addr)`, `MIN_VALID_PROCESS_PTR = 0x0000_0000_0001_0000`, `MAX_VALID_PROCESS_PTR = 0x0000_7FFF_FFFF_FFFF` (x64 user-mode range).
- **Invariants.**
  - `is_probably_valid_process_ptr` is a **helper callers opt into** — `ProcessHandle::read`, `read_ptr`, `read_bytes`, and `read_string` do _not_ validate pointers internally. Callers must guard their own reads (most `eq::spawn` call sites already do).
  - `chase_ptr` accumulates an error stack with step index on failure.
  - `HANDLE` auto-closes on `Drop`.
  - Non-Windows stub returns `0x140000000` for `module_base()` so tests compile.

## `process/window.rs`

- **Purpose.** Enumerate top-level Windows owned by a given PID for focus/activation checks.
- **Invariants.** Uses `EnumWindows` + `GetWindowThreadProcessId`; invisible, non-top-level windows are filtered.

## `inject/dll_prep.rs`

- **Purpose.** Stage the compiled DLL onto disk under a legitimate-looking Microsoft DLL name, tracked by SHA-256.
- **Public API.**
  - `StagedDll` (`path`, `hash`, `_guard: std::fs::File`).
  - `compute_file_hash(path)`.
  - `StagingNamePool::{new, capacity, used_count, next_name, release}`.
  - `prepare_dll(source_dll)`, `prepare_dll_locked(source_dll)`, `cleanup_dll(path)`.
  - `LEGITIMATE_DLL_NAMES` (~60 entries: `mscorlib.ni.dll`, `clrjit.dll`, `dbghelp.dll`, `d3d11.dll`, …).
- **Invariants.**
  - The file-handle guard (`_guard`) keeps the staged file open so it cannot be replaced between `CreateFile` and `LoadLibraryW` (TOCTOU guard).
  - SHA-256 hash captured at staging time is re-verified pre-injection.
  - Name pool is thread-safe and supports up to 60 concurrent injections.

## `inject/loader.rs`

- **Purpose.** `CreateRemoteThread` + `LoadLibraryW` injection with stealth verification.
- **Public API.**
  - `inject_dll(pid: u32, dll_path: &Path, expected_hash: &str)`.
  - `eject_dll(pid: u32, dll_name: &str)`.
  - `dll_module_name(dll_path)`, `INJECTION_TIMEOUT_MS = 10_000`.
- **Invariants.**
  - **Verification trusts the remote thread's exit code (HMODULE)**, _not_ PEB module enumeration — the DLL deliberately unlinks itself from the loader list for stealth. Using PEB walks here is a known anti-pattern.
  - DLL path must end in `.dll` and fit `DLL_PATH_LEN_LIMIT = 8192` bytes.
  - May require `SeDebugPrivilege` depending on target process permissions, integrity level, and ownership. Same-user targets can often be injected without it; the loader does **not** enable the privilege itself.
  - Non-Windows compiles are a successful no-op so tests run.

## `inject/reflective.rs`

- **Purpose.** Parse a PE from raw bytes and (eventually) map it into the target without touching the filesystem.
- **Public API.** `InjectError`, `ParsedPe`, `parse_pe`, `apply_relocation`, `is_system_dll`, `PeSection`, `Relocation`, `ImportEntry`.
- **Invariants.**
  - `e_lfanew` bounds-checked to avoid OOB reads on malformed inputs.
  - `IMAGE_REL_AMD64_*` relocation types supported (x64 only).
  - System imports (kernel32, ntdll) are resolved from the target; payload imports fail gracefully.
  - Feature is staged but not the active injection path yet.

## `ipc/pipe.rs`

- **Purpose.** Orchestrator-side client for the DLL's named-pipe command channel.
- **Public API.** `CommandPipe::{connect(client_id, session_id), send(cmd), send_correlated(cmd, corr)}`.
- **Invariants.**
  - `client_id` **must be the EQ process PID** (the DLL uses `std::process::id()` when creating the pipe).
  - Pipe name comes from `textquest_common::ipc::pipe_name(session_id, client_id)`.
  - Session-token file is written **before** `LoadLibraryW` so the DLL can authenticate during init.
  - Correlation IDs are echoed in responses; mismatches cause the call to bail.
  - PIPE_TYPE_BYTE mode — callers loop writes until all bytes land.
  - When the pipe is unavailable, upstream (Soul coordinator) buffers commands with a priority queue.
- **Depends on.** `textquest_common::ipc::{Command, Response, CorrelationIdGenerator, IpcCommand}`, `protocol` module.

## `ipc/shared.rs`

- **Purpose.** Read-only orchestrator view of the DLL's published shared memory region.
- **Public API.** `SharedStateReader`, `SharedNavSnapshot`, `read_game_state()`.
- **Invariants.**
  - Mapped read-only.
  - Struct layout mirrors EQ's in-memory format; offsets are owned by `textquest_common::offsets`.
  - Drop impl tears down the view.

## `eq/structs.rs`

- **Purpose.** Canonical Rust types for game objects.
- **Public API.**
  - `EqClass` (16 playable classes, `from_id`, `short_name`).
  - `SpawnType` (Player, Npc, Corpse, Unknown).
  - `StandState` (Standing, Frozen, Looting, Sitting, Ducking, Feigned, Dead — includes an ASCII `sprite()`).
  - `BuffSlot`, `SpellSlot`, `CastDurationSource`, `CastState`.
  - `SpawnInfo` (spawn_id, name, displayed_name, lastname, type, level, class_id, stand_state, x/y/z, heading, hp_current/max, mana, endurance, race_id, gm_flag, cast_state).
  - `GroupInfo`.
- **Invariants.**
  - `EqClass` reads `ActorClient::CHAR_CLASS` (i32 @ `0x0FDC`) — `PlayerZoneClient::CharClass` is often zero for NPCs.
  - Mana/endurance are only reliable for the local player; remote spawns clamp to zero.
  - Positions near `(0,0,0)` log a diagnostic (likely an offset mismatch, never a fatal error).
  - Never log raw addresses, only metadata.

## `eq/spawn.rs`

- **Purpose.** Linked-list traversal of live spawns + read helpers for buffs, spellbook, memmed spells, cast state, target, zone name, group info.
- **Public API.** `read_spawn`, `read_local_player`, `read_all_spawns`, `read_buff_slots`, `read_spellbook`, `read_memorized_spells`, `read_cast_state`, `read_target`, `read_zone_name`, `read_group_info`.
- **Invariants.**
  - The spawn list is doubly-linked. `read_all_spawns` backtracks from `pinstLocalPlayer` to the head by following `PREV` pointers (max 4096 steps) then walks forward with a `HashSet` cycle guard; hop limit is `max_count × 4`.
  - Critical fields (name, type, x/y/z, level) hard-fail a spawn; non-critical fields default to `0`.
  - Sentinel pointer `0xFFFF` means "empty slot".
  - Valid pointer range is enforced through `is_probably_valid_process_ptr`.

## `eq/log_parser.rs`

- **Purpose.** Convert EQ log lines into structured `LogEvent`s.
- **Public API.** `LogEvent` (`Loot`, `Kill`, `Money`, `Experience`, `Death`, `ZoneEnter`, `Chat`), `parse_log_line`.
- **Invariants.**
  - EQ timestamp `[Day Mon DD HH:MM:SS YYYY]` is stripped before matching.
  - Non-matching lines return `None` (graceful degradation).
  - Chat events delegate to `textquest_common::chat::parse_stripped_chat_text`.

## `eq/log_watcher.rs`

- **Purpose.** Tail EQ log files and stream parsed events to subscribers.
- **Invariants.** File rotation detected via size-shrink + inode change. Backfill is bounded to avoid replaying historical logs on startup.

## `eq/map_parser.rs`

- **Purpose.** Parse ChatWithThisName-style `.map` files (see `reference_cwtn_map_code`).
- **Invariants.** Handles `L` (line), `P` (point) records; preserves color tuples for TUI map rendering.

## `eq/named_db.rs` / `eq/named_tracker.rs`

- **Purpose.** Track contested/named spawns in a SQLite-backed DB; persistence used by the Discord bot's spawn feed.
- **Invariants.** Observations deduped per `(zone, mob, timestamp window)`.

## `eq/spawn_alert.rs` / `eq/spawn_filter.rs`

- **Purpose.** Combine filter rules with the named DB to emit targeted spawn alerts.
- **Invariants.** Filter evaluation is pure; alert emission is idempotent per `(spawn_id, rule_id)`.

## `eq/hvt.rs` / `eq/cheater.rs` / `eq/gm_detector.rs`

- **Purpose.** High-value-target heuristics, cheater/suspicious-player tracking, and GM detection from spawn metadata.
- **Invariants.**
  - GM detection reads both the GM flag and name/level/anonymity heuristics.
  - Cheater tracking is advisory — it drives alerts, it does not gate actions.
