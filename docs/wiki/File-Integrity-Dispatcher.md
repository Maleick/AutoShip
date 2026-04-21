# File Integrity Dispatcher

**Module**: `textquest-dll/src/hooks/file_integrity_dispatcher.rs`
**Scope**: M5.5 — Server-initiated integrity defense (issue #2177)

## Purpose

EQ's `FILE_INTEGRITY_DISPATCHER = 0x140564BC0` runs three file-integrity checks on `WorldAuthenticate` (`FUN_1402C9C80`):

| Check | File | Opcode |
|---|---|---|
| `1x` | `eqgame.exe` | `0x8bdc` |
| `1sa` | `Resources/BaseData.txt` | `0xe91d` |
| `1sa` | `Resources/SkillCaps.txt` | `0x9562` |

Each check hashes the file and samples 256 random DWORDs using a Lagged Fibonacci PRNG (`FUN_14025ABD0`, state at `DAT_140E8D148`). PRNG is deterministic — server knows which positions were sampled, so hash spoofing requires matching per-position samples.

## Design

- **`retour::static_detour!`** byte-patch detour on `FILE_INTEGRITY_DISPATCHER`. Replaces HWBP approach from earlier revision after DR3 collision with `#2175` server memcheck responder. No HWBP debug registers consumed.
- **`LfgPrng`** — Rust port of the additive lagged Fibonacci generator (p=55, q=24, state is 55 `u32`s). Supports `from_state(&[u32; 55])` construction from a live memory snapshot for cross-check.
- **`FileHashCache`** — precomputed `(full_hash, samples)` per file variant. Gated behind `CACHE_ACTIVE: AtomicBool` (default `false` = passthrough).
- **Opcodes constants** — `OPCODE_EXE_HASH = 0x8bdc`, `OPCODE_BASEDATA_HASH = 0xe91d`, `OPCODE_SKILLCAPS_HASH = 0x9562`.

## Operator Workflow

No runtime control surface. Detour installs at DLL init, removes on `hooks::remove_all()`. In default configuration (passthrough), the original dispatcher runs unmodified. To enable hash substitution: populate `FileHashCache` with expected server-side hashes and set `CACHE_ACTIVE` true.

## Related

- Epic: #2173 (M5.5)
- Siblings: #2175 (server memcheck), #2176 (counter watchdog)
