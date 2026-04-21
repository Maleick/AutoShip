# Server Memcheck Responder

**Module**: `textquest-dll/src/hooks/memcheck.rs`
**Scope**: M5.5 — Server-initiated integrity defense (issue #2175)

## Purpose

Respond correctly to server opcode `0x4f27` memcheck queries. Server can send a counted list of `(address, length)` region specs that the client's handler (`FUN_1400B5720`, `SERVER_MEMCHECK_HANDLER = 0x1400B5720`) copies into 0x100-byte blocks, hashes, and returns. Any in-memory modification to `.text` (or the DLL's own pages if mapped inside a requested range) is server-visible without this responder.

## Design

- **HWBP on Dr3** at `SERVER_MEMCHECK_HANDLER` — fires at handler entry, passthrough-default (returns false, VEH sets RF + resumes). Dr3 is the last free HWBP slot; subsequent hooks must use detour path.
- **CleanHashCache** — pre-computed FNV-1a 64-bit hashes of `.text` at DLL init, keyed by 0x100-byte block address. 32 MiB scan from eqgame.exe base.
- **Region intersection** — `blocks_for_region()` resolves `(addr, len)` → aligned block addresses, handling unaligned starts.
- **Byte substitution** — stubbed pending Ghidra-confirmed struct layout for `FUN_1400B5720` argument registers (RCX / RDX / R8 — region_count + spec array pointer).

## Operator Workflow

No runtime control. Hook installs automatically at DLL init and removes on `hooks::remove_all()`. `cached_block_count()` returns coverage metric for diagnostics.

## Related

- Epic: #2173 (M5.5)
- Siblings: #2176 (message counter watchdog), #2177 (file integrity)
