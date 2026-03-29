# Anti-Detection: Warden & EQ Anti-Cheat

## What is Warden?

EverQuest uses a proprietary anti-cheat system commonly referred to as **Warden** (originally a Blizzard term, but the EQ community uses it generically). Daybreak's implementation scans for:

1. **Known DLL signatures** — Warden periodically scans loaded modules in the eqgame.exe process. It checks module names and hashes against a blacklist of known cheat DLLs.
2. **Memory pattern scanning** — The client sends memory regions to the server for analysis. Warden can read arbitrary memory pages looking for known byte patterns (e.g., hook trampolines, specific string constants).
3. **Window enumeration** — Checks for known cheat tool window titles/classes.
4. **Process enumeration** — Scans running processes for known cheat executables.
5. **Integrity checks** — Validates that certain game functions haven't been modified (detour detection). Checks for inline hooks at known function entry points.

## How MacroQuest2 Avoids Detection

MQ2 has evolved its anti-detection over many years:

### Module hiding
- MQ2 unlinks its DLL from the PEB (Process Environment Block) loaded module list, making it invisible to `EnumProcessModules` and similar APIs.
- The DLL name is randomized at load time — not "MacroQuest2.dll" but a random string.
- MQ2 erases PE headers from the DLL's memory region after loading, so memory scans can't find a valid PE signature.

### String obfuscation
- Identifying strings like "MacroQuest", "MQ2", plugin names, etc. are encrypted or obfuscated in the binary.
- Log file paths and names avoid obvious identifiers.

### Hook stealth
- MQ2 uses "trampoline" hooks that preserve the original function bytes and restore them before Warden integrity checks.
- Some builds use hardware breakpoint hooks (debug registers) instead of inline patching — these leave no modified bytes in code sections.
- The detour library (Detours/MinHook) is configured to use page-aligned trampolines that don't stand out in memory scans.

### Timing
- Actions are not executed instantly — MQ2 adds human-like delays between commands.
- Frame-perfect actions are avoided; jitter is added to casting, movement, and targeting.
- The `/stick` and `/nav` commands include configurable randomization.

### Warden evasion
- MQ2 hooks `ReadProcessMemory` and `NtReadVirtualMemory` to return clean (unmodified) memory when Warden scans known regions.
- Some builds hook Warden's scan entry point to skip or neuter the scan entirely.
- The community maintains updated "Warden offsets" that track where the scan routines live in each client patch.

## Frostreaver Anti-Detection Strategy

### Current measures (implemented)

1. **Command jitter** — All IPC commands from the orchestrator are queued with a random 1-10 tick delay (250ms-2.5s) before execution. Uses Xorshift32 PRNG for deterministic per-client randomness.

2. **GM detection** — The `is_gm` flag is read from every spawn in the spawn list (offset `0x03ec` in PlayerClient). The TUI can highlight GM spawns and trigger alerts.

3. **String audit** — The dmft-dll crate has been audited for identifying strings. Tracing/log strings go to our own file appender (not visible to Warden). IPC identifiers like shared memory names (`dmft_state_*`) and pipe names (`dmft_cmd_*`) are flagged for future obfuscation.

### Future measures (planned)

4. **DLL name randomization** — Generate a random DLL filename at injection time instead of `dmft_dll.dll`.

5. **PE header erasure** — Zero out the PE headers in memory after DLL initialization completes.

6. **PEB unlinking** — Remove the DLL from the loaded modules list in the PEB.

7. **Hook restoration for scans** — Implement a mechanism to temporarily restore original bytes at hook points when Warden scans are detected.

8. **IPC name obfuscation** — Replace `dmft_state_*` and `dmft_cmd_*` with randomized names, communicated via a bootstrap channel.

## Strings of Concern in dmft-dll

These strings appear in the compiled DLL and could be flagged by pattern scanning:

| String | Location | Risk | Notes |
|--------|----------|------|-------|
| `dmft_state_{id}` | `ipc/shared.rs` | **Medium** | Shared memory name visible to Warden |
| `dmft-ipc-{id}` | `ipc/mod.rs` | **Low** | Thread name, less visible |
| `\\.\pipe\dmft_cmd_{id}` | `ipc/pipe.rs` | **Medium** | Named pipe visible to Warden |
| `dmft-dll.log` | `lib.rs` | **Low** | Log file in temp dir |
| `DMFT DLL *` | `lib.rs` (tracing) | **Low** | Goes to file appender only |

Tracing messages (e.g., "DMFT DLL initializing", "Game loop hook installed") are safe — they go to our rolling file appender in `%TEMP%/dmft/`, not to game memory or any channel Warden scans.

## References

- MacroQuest2 source: `mq2-reference/` (local clone)
- RedGuides community: Primary source for Warden bypass discussion
- EQ Emulator forums: Anti-detection techniques for private servers
