# Research: Test Offset Reconciliation

## Scope

This run reconciles TextQuest's Test-client offset work against local evidence only.

Promotion authority:

- `TextQuest-Ghidra/test/*`
- the local GhidraMCP headless server

Checklist only:

- public `eqlib` live/test diffs
- local `forkvanilla/src/eqlib` history

Do not promote an upstream value into TextQuest just because `eqlib` moved. Use upstream changes to decide what to inspect, then prove the final value from local Ghidra evidence.

## Harvest

- Variant: `test`
- Harvested at: `2026-04-09T01:42:16Z`
- Host: `frostreaver`
- Transport: `ghidra-mcp`
- Binaries:
  - `eqgame.exe`
  - `eqmain.dll`
  - `EQGraphics.dll`

Source: `TextQuest-Ghidra/test/harvest-info.json`

## Repro Workflow

1. Verify GhidraMCP connectivity:

```bash
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py check
```

2. Use local export files as the stable evidence store:

- `TextQuest-Ghidra/test/eqgame/ghidra-export/metadata.json`
- `TextQuest-Ghidra/test/eqmain/ghidra-export/metadata.json`
- `TextQuest-Ghidra/test/eqgraphics/ghidra-export/metadata.json`
- `TextQuest-Ghidra/test/eqgame/decompiled/*.c`
- `TextQuest-Ghidra/test/eqmain/decompiled/*.c`

3. Use GhidraMCP for quick address and xref checks:

```bash
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call \
  GET /get_function_by_address --query address=ram:1405654e0

python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call \
  GET /get_xrefs_to --query address=ram:140ea9a68 --query limit=5

python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call \
  GET /disassemble_function --query address=ram:1402792b0
```

4. Compare against `eqlib` only after local evidence exists.

## Evidence Ledger

### What worked

- `LocalPC -> me` is the strongest current Test player-construction chain.
- Actor-backed class resolution now beats the stale direct `PlayerZoneClient::CharClass` read for the local player.
- The current Test `eqmain` login globals are locally supported and promoted.
- Packet hook coverage now includes both Winsock families.
- The `ZoneGuideManagerClient` singleton miswire was identified and corrected from local evidence.

### What failed

- `pinstSpawnManager` still lacks a direct local-Test proof.
- `spawn_manager::PLAYER_LIST` still lacks a direct local-Test proof.
- `PINST_CDISPLAY`, `PINST_CEVERQUEST`, and `PINST_CXWND_MANAGER` still lack direct local-Test proof.
- Several `eqgame` hook entry points still need rebinding from local evidence.
- The direct `instEQZoneInfo` strings were blank in the live dump, so the zone-name fallback path still needs runtime confirmation.

### Exact verification commands

```bash
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py check
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /get_function_by_address --query address=ram:1405654e0
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /get_xrefs_to --query address=ram:140ea9a68 --query limit=5
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /disassemble_function --query address=ram:1402792b0
```

Evidence:

- `TextQuest-Ghidra/test/harvest-info.json`
- `TextQuest-Ghidra/test/eqgame/ghidra-export/metadata.json`
- `TextQuest-Ghidra/test/eqgame/decompiled/140309ad0_FUN_140309ad0.c`
- local dump logs from frostreaver showing `actor_class=3`, `direct_class=0`, `pinstLocalPC` valid, and `pinstSpawnManager = 0`

### Next blocker state

- Test blocker: prove `PINST_SPAWN_MANAGER` and `spawn_manager::PLAYER_LIST` from local Test evidence.
- Test blocker: rebind the unresolved `eqgame` hooks from local Test evidence.
- Test blocker: rerun a fresh Windows live test to confirm the zone-name fallback and fresh packet row behavior on the rebuilt desktop drop.
- Demo blocker: none for local map rendering; demo mode already exercises map rendering, viewport reset, and Map-to-Debug handoff. The remaining demo risk is accidental confusion with live validation, not a code gap.

## Checklist Pattern From eqlib History

Over roughly the last year, the stable patch-review pattern is:

- `include/eqlib/offsets/eqgame.h` is the primary patch heartbeat.
- `include/eqlib/offsets/eqmain.h` is the secondary patch heartbeat.
- `include/eqlib/offsets/eqgraphics.h` moves on some patches, but not all.
- Struct churn repeats in a narrow cluster:
  - `PlayerClient.h`
  - `PcClient.h`
  - `PcProfile.h`
  - `EQClasses.h`
  - `LoginFrontend.h`
  - `CXWnd.h`
  - adjacent UI and item headers

That pattern is useful for deciding what TextQuest should inspect after a patch, but not for deciding the final value to promote.

## Promoted Evidence

### eqgame

| Surface | Value | Evidence | Confidence | TextQuest action |
| --- | --- | --- | --- | --- |
| `PINST_LOCAL_PC` | `0x140EA9A68` | `140309ad0_FUN_140309ad0.c` gates player construction on `DAT_140ea9a68` and emits `LocalPC is NULL in Player constr. for %s` | High | Promoted in `textquest-common/src/offsets.rs` |
| `character_zone::ME` | `0x2848` | `140309ad0_FUN_140309ad0.c` stores the constructed player pointer at `LocalPC + 0x2848` | High | Promoted in `textquest-common/src/offsets.rs`, `textquest-common/src/offset_db.rs`, and `config/offsets.json` |
| `actor_client::RACE` | `0x0FF4` | Current Test/openvanilla reference headers place `PlayerClient::mActorClient` at `0x0FE0`; `Actors.h` keeps `ActorBase::Race` at `+0x14` | Medium | Promoted in `textquest-common/src/offsets.rs` and used for spawn class/race reads |
| `actor_client::RACE_OVERRIDE` | `0x0FF8` | Same `mActorClient` + `ActorBase` derivation as above | Medium | Promoted in `textquest-common/src/offsets.rs` |
| `actor_client::CHAR_CLASS` | `0x0FFC` | Same `mActorClient` + `ActorBase::Class` derivation as above; runtime dump showed `actor_class=3` while direct class was `0` for the local PAL | Medium-high | Promoted in `textquest-common/src/offsets.rs`; local-player class now resolves through actor class first |
| `profile::CLASS` | `0x17CC` | Current Test/openvanilla `PcProfile.h` reference; used only as a fallback when live actor/direct class reads are absent | Medium | Promoted in `textquest-common/src/offsets.rs` and used as the local-player fallback |
| `packetScrambler` | `0x140EA9A68` | Current send-path research still sees the same global in opcode-scrambler contexts, but the same object is also on the LocalPC/player-construction path | Medium | Kept as a research-facing name with explicit ambiguity noted |
| `networkConnection` | `0x140EA9CF0` | Local GhidraMCP xref and disassembly checks on current Test | High | Already promoted |
| `netSend` | `0x1405654E0` | Local GhidraMCP function lookup on current Test | High | Already promoted |
| `opcodeScramblerHton` | `0x14067B9E0` | Local GhidraMCP function lookup on current Test | High | Already promoted |

### eqmain

| Surface | Value | Evidence | Confidence | TextQuest action |
| --- | --- | --- | --- | --- |
| `LOGIN_SERVER_API` | `0x1801804E0` | `18002fa10_FUN_18002fa10.c` gates on `DAT_1801804e0` and passes it to `JOIN_SERVER` | Medium-high | Promoted in `textquest-common/src/offsets.rs` |
| `PINST_LOGIN_CLIENT` | `0x1801804F0` | `18000a0d0_FUN_18000a0d0.c` and `180009eb0_FUN_180009eb0.c` follow `DAT_1801804f0 -> pLoginData -> hEQWnd` | High | Promoted in `textquest-common/src/offsets.rs` |
| `PINST_LOGIN_CONTROLLER` | `0x180180500` | `180011ef0_FUN_180011ef0.c` logs `g_pLoginController->GiveTime()` and calls `FUN_180016640(DAT_180180500)` | High | Promoted in `textquest-common/src/offsets.rs` |
| `LOGIN_CONTROLLER_GIVE_TIME` | `0x180016640` | Present in current Test export and called from `180011ef0_FUN_180011ef0.c` | High | Kept |
| `JOIN_SERVER` | `0x180018050` | Present in current Test export and called from `18002fa10_FUN_18002fa10.c` | High | Kept |
| `EQLOGIN_HWND` | `0x408` | Verified in current Test decompilation | High | Kept |
| `CEDITBASEWND_INPUT_TEXT` | `0x278` | Verified in `18002e490_FUN_18002e490.c` | High | Kept |

### EQGraphics

`EQGraphics.dll` is present in the Test harvest and analyzable, but current TextQuest code does not dereference any direct `EQGraphics.dll` offsets. Current render work still depends on `eqgame` surfaces such as `CDisplay::RealRender_World` plus DXGI/D3D11 interception.

For now, review `EQGraphics` as a patch checklist surface, not a promotion target.

## Unresolved Or Not Yet Promoted

- Exact local-Test proof for `PINST_SPAWN_MANAGER`
- Exact local-Test proof for `spawn_manager::PLAYER_LIST`
- Exact local-Test proof for `PINST_CDISPLAY`
- Exact local-Test proof for `PINST_CEVERQUEST`
- Exact local-Test proof for `PINST_CXWND_MANAGER`
- Current Test rebinding for these `eqgame` functions:
  - `PROCESS_GAME_EVENTS`
  - `REAL_RENDER_WORLD`
  - `RIGHT_CLICKED_ON_PLAYER`
  - `EXECUTE_CMD`
  - `INTERPRET_CMD`
  - `CHAR_LIST_ENTER_WORLD`
  - `CHAR_LIST_SELECT_CHAR`

These surfaces still exist in the binary, but they were not promoted in this run without stronger local proof.

## TextQuest Changes From This Run

- The external spawn reader now prefers the locally-proven `LocalPC -> me` chain and only falls back to `pinstLocalPlayer` if that path is unavailable.
- The external and DLL-side spawn readers now treat `mActorClient.Class` as the preferred live class source and only fall back to the older direct `PlayerZoneClient::CharClass` / `PcProfile::Class` paths when needed.
- `config/offsets.json` now records the locally-proven `pinstLocalPC` value and `character_zone.me`.
- `config/offsets.json` now stores `standState` under `player_zone`, matching the code and `OffsetDatabase` schema.
- `eqmain` login globals now reflect the current Test evidence instead of the older baseline.
- Zone reads now try both direct and pointer-indirect `instEQZoneInfo` before falling back to `ZoneGuideManagerClient`, and dump mode logs both shapes for the next live Test proof.
- TextQuest had also wired `ZoneGuideManagerClient` to the wrong nearby symbol.
  - incorrect TextQuest value: `0x1403571F0`
  - corrected Test singleton: `0x1403572C0`
  - reference-only checklist source: `openvanilla` Test `eqgame.h` `ZoneGuideManagerClient__Instance_x`
  - `OffsetDatabase` and `config/offsets.json` now treat `zoneGuideManager` as a global, not as a function.
- Packet capture now covers both Winsock families:
  - `send` / `recv`
  - `WSASend` / `WSARecv`
  - this change was driven by a live Test state where the DLL initialized, IPC came up, packet hooks logged as installed, and PacketMonitor still showed zero rows.

## Current Runtime Interpretation

- The orchestrator warning `No session token for client` is not a direct read of the on-disk token files. It comes from the orchestrator's in-memory `session_tokens` map.
- The DLL consumes `%TEMP%/textquest/token_{pid}.bin`, not `%TEMP%/textquest/login_token_{pid}.bin`.
- If `token_{pid}.bin` still exists after a supposedly successful inject, the DLL did not reach `generate_session_token()` and therefore did not start authenticated IPC for that session.
- The current debug build writes `%TEMP%/textquest/textquest-dll-init-{pid}.log` so the next Windows retest can distinguish:
  - callback never reached
  - callback hit `already_initialized`
  - `initialize()` panicked
  - tracing initialized but token or IPC startup stalled
- Current live frostreaver dump evidence before the process exited:
  - `pinstLocalPlayer = 0`
  - `pinstLocalPC` and `LocalPC->me` valid
  - local player resolved as `Xuramtine (PAL)` from `actor_class=3` while direct class stayed `0`
  - `instEQZoneInfo` direct short/long strings were blank
  - `pinstSpawnManager = 0`
  - linked-list fallback still enumerated `688` spawns successfully
  - the old `ZoneGuideManagerClient` symbol path was also invalid at runtime because the singleton address itself was miswired
- For clean Test evidence, restart the EQ process before reinjecting. Repeated injections into the same live process can leave prior DLL state resident and confuse token/session matching.

## Runtime Read Completeness Slice (2026-04-10)

### What worked

- Zone reads now normalize placeholder strings and preserve source diagnostics (`inst_direct`, `inst_indirect`, `zone_guide`, `none`) so `Unknown` does not silently mask fallback behavior.
- `read_zone_name` / `read_zone_short_name` now resolve through one diagnostic path and emit explicit unresolved-source context when all candidates fail.
- Target reads now return a reason state (`none`, `invalid_pointer`, `read_failed`, `self_target_fallback`) and preserve self-target visibility when `pinstTarget` resolves to the local-player identity path.
- TUI character summary now surfaces non-empty target-read reason labels when target is blank, avoiding silent empty panes.
- CLI dump mode now logs target-read reason labels, and status mode now prints stand-state raw IDs alongside class IDs for faster patch-day sanity checks.
- Tactical-map reloads now stay aligned with unresolved-zone handling, and map discovery now includes macOS bundle-style `Contents/Resources/config/maps` fallback for local demo smoke.
- Tactical `r` reset is now reserved for map-view reset across the Tactical screen, even when focus is on a side panel.

### What failed or remains open

- We still do not have direct local-Test proof for `pinstSpawnManager` and `spawn_manager::PLAYER_LIST`; linked-list fallback remains authoritative for spawn enumeration.
- We still do not have a fresh in-world proof capture on the newest binaries for the `self_target_fallback` branch under a true self-target null-pointer edge case.
- `instEQZoneInfo` direct strings may still be blank in some Test sessions; fallback behavior is now explicit, but exact root-cause ownership remains a binary-level follow-up.
- PacketMonitor still lacks fresh in-world proof that the new pipe-open token recovery path yields visible packet rows on the rebuilt Windows desktop drop.

### Proof matrix (local evidence + expected runtime signals)

| Surface | Evidence source | Runtime check | Expected |
| --- | --- | --- | --- |
| Zone long/short source selection | `TextQuest-Ghidra/test/harvest-info.json` + local dump logs | `textquest.exe --dump` zone diagnostics | Source label is `inst_direct`, `inst_indirect`, or `zone_guide`; only `none` when all reads fail |
| Zone fallback hardening | `instEQZoneInfo` direct/indirect blank behavior from prior Test dumps | TUI selected-client zone field while attached | No silent collapse to stale `Unknown`; fallback source is traceable |
| Target pane completeness | `pinstTarget` Test instability evidence + local-player identity chain proof | TUI Character panel target line | Target text present or explicit reason suffix (`invalid_pointer`, `read_failed`, `self_target_fallback`) |
| Class source sanity | Local dump evidence (`actor_class=3`, `direct_class=0`, profile fallback) | CLI dump `Local player class sources` | Actor class preferred; profile remains local-player fallback |
| Stand/class projection sanity | Shared-state `SpawnData` and TUI player snapshot | `--status PID` + Character panel | Class ID and stand-state stay non-empty/observable even when target/zone are degraded |

## Next Checklist

1. Re-test the Windows Test build and confirm the TUI stops falling back to demo/no-player behavior.
2. Rebind `PINST_SPAWN_MANAGER` and `spawn_manager::PLAYER_LIST` from local Test evidence.
3. Rebind the unresolved `eqgame` hook functions from local Test evidence.
4. Only after those proofs exist, widen the shared snapshot schema further.
