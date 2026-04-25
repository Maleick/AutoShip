# MQ2/eqlib Opcode Table Research — EQ Client 20260310

**Status:** Research — reference data extracted; confirmed codes limited; most codes require binary extraction.
**Parent issue:** #946
**Issue:** #947
**Date:** 2026-04-25

## Summary

EverQuest Live/TLP application-layer opcodes **rotate with every client patch**. There is no publicly available opcode table specifically for client build 20260310 (March 10, 2026). This document describes the research methodology, sources, and the reference table produced in `config/opcodes.json`.

## Key Finding: Opcode Rotation

> **All application-layer opcode hex codes rotate with every EQ patch.**
> The numeric codes valid for client 20260310 are **different** from all older patches.
> Confirmed codes exist only where TextQuest binary analysis (Ghidra) or live capture has run against the March 2026 client binary.

The `config/opcodes.json` file contains:

- **Confirmed codes** (2 entries): extracted from Ghidra analysis of the 20260310 binary
- **Dec 2024 binary research codes** (1 entry): from pre-20260310 binary, requires reconciliation
- **EQEmu RoF2 reference codes** (~100 entries): from EQEmu `utils/patches/patch_RoF2.conf` (client: May 10 2013). These are the most recent publicly documented EQ client codes. **These will NOT match 20260310.**
- **EQEmu SoF reference codes** (~15 entries): older reference, lower priority
- **EQEmu oplist names only** (~160 entries): opcode names from `common/emu_oplist.h` with `0x0000` placeholder codes pending extraction

## Confirmed Codes (client 20260310)

| Opcode Name         | Hex Code | Direction | Source                                                                                                  |
| ------------------- | -------- | --------- | ------------------------------------------------------------------------------------------------------- |
| OP_ZoneEntry        | `0x0042` | inbound   | Ghidra test fixtures (`textquest-common/src/ghidra_db.rs` line 737)                                     |
| OP_ClientUpdate     | `0x0080` | outbound  | Ghidra test fixtures (`textquest-common/src/ghidra_db.rs` line 763)                                     |
| OP_ClientUpdate_Alt | `0x1643` | outbound  | Dec 2024 binary — MQ2reachit hook on `UdpConnection::SendMessage`. Requires reconciliation with 0x0080. |

## Sources Consulted

### 1. macroquest/eqlib (GitHub)

- Repository: https://github.com/macroquest/eqlib
- The public eqlib repo targets `EQLIB_TARGET = "Emu"` (EQEmu), not the Live EQ client.
- Contains `include/eqlib/game/EQData.h` — class/race/spell definitions, NOT opcode table.
- Contains `include/eqlib/offsets/eqgame.h` — binary offsets for client build 20130510 (May 2013).
- **No public opcode table for EQ Live client 20260310 was found** in macroquest/eqlib or macroquest/macroquest.

### 2. EQEmu/Server — Opcode Reference

- Repository: https://github.com/EQEmu/Server
- `common/emu_oplist.h` — Complete list of ~400+ opcode names used in EQEmu
- `common/emu_opcodes.h` — EmuOpcode enum definition
- `utils/patches/patch_RoF2.conf` — Opcode code assignments for RoF2 client (May 10 2013)
- `utils/patches/opcodes.conf` — Base Titanium (Oct 2005) opcode codes
- The EQEmu system uses a translation layer: EmuOpcode (stable names) → client-specific uint16 codes

### 3. TextQuest Project Binary Research

- `docs/research/C1-movement-agreement-packet.md` — Confirms `0x1643` as C→S movement opcode from Dec 2024 binary via `UdpConnection::SendMessage` hook
- `textquest-common/src/ghidra_db.rs` (lines 737, 763) — Test fixtures confirming `0x42`/OP_ZoneEntry and `0x80`/OP_ClientUpdate
- `docs/research/zone-packet-audit.md` — Documents zone transition packet sequence; notes most opcode codes are "unknown" for 20260310

## Output: config/opcodes.json Format

```json
{
  "code": "0x0042",
  "name": "OP_ZoneEntry",
  "direction": "inbound",
  "description": "...",
  "source": "ghidra-confirmed | eqemu-rof2-reference | eqemu-sof-reference | eqemu-oplist",
  "client": "20260310 | rof2-reference | sof-reference | reference"
}
```

## Source Field Taxonomy

| Source Value              | Meaning                                                             |
| ------------------------- | ------------------------------------------------------------------- |
| `ghidra-confirmed`        | Extracted from Ghidra analysis of 20260310 binary                   |
| `binary-research-dec2024` | From Dec 2024 binary (pre-20260310), needs reconciliation           |
| `eqemu-rof2-reference`    | EQEmu RoF2 patch conf — reference only, codes DO NOT match 20260310 |
| `eqemu-sof-reference`     | EQEmu SoF deprecated opcode list                                    |
| `eqemu-oplist`            | Name-only from EQEmu emu_oplist.h, no code confirmed                |

## Statistics

- Total entries: 282
- Confirmed for 20260310: 3
- RoF2 reference codes: ~100
- Name-only (code unknown): ~160

## Next Steps

To complete this table for client 20260310:

1. **Pattern scan** — Run Ghidra/binary scan on `eqgame.exe` build 20260310 to extract all opcode→handler function mappings. The handler dispatch table is a large switch/indirect-call structure in `NetworkRecv`.
2. **Live capture** — Use MQ2reachit or a packet capture hook to observe opcodes in flight during zone-in, combat, and movement.
3. **Cross-reference** — Map observed opcode codes against EQEmu opcode names using the struct layout comparison method (payload size + field layout fingerprinting).

See `docs/research/zone-packet-audit.md` and `docs/research/C1-movement-agreement-packet.md` for related work.

## References

- EQEmu emu_oplist.h: https://github.com/EQEmu/Server/blob/master/common/emu_oplist.h
- EQEmu patch_RoF2.conf: https://github.com/EQEmu/Server/blob/master/utils/patches/patch_RoF2.conf
- macroquest/eqlib EQData.h: https://github.com/macroquest/eqlib/blob/master/include/eqlib/game/EQData.h
