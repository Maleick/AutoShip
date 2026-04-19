# Offset Placeholder Audit

**Audit date:** 2026-04-16
**Branch:** `claude/audit-and-merge-prs-kb4Pv`
**CLIENT_DATE in `offsets.rs`:** `20260310` (March 10, 2026)
**Last Ghidra harvest:** 2026-04-09 (per `Research-Patch-Day-Reproduction.md`)
**Last live patch:** 2026-04-15 (per EverQuest weekly maintenance cadence)

## Why this audit exists

Every `pub const … = 0x0;` in `textquest-common/src/offsets.rs` is a live behavior gap — the DLL skips or stubs any feature depending on that address, but compiles and links clean. This makes drift invisible until an operator hits the feature at runtime. This file enumerates the gaps so the next patch-day session has a single punch list instead of grepping `= 0x0;` fresh.

## Version drift flags

- `offsets.rs:18` `CLIENT_DATE = "20260310"` predates the 2026-04-09 Ghidra harvest that updated `config/offsets.json`. The constants in `offsets.rs` and the values in `config/offsets.json` should be reconciled — they serve two different consumers (the compiled DLL vs. the `OffsetDatabase` runtime loader) but represent the same patch state.
- `offsets.rs:23` `ACTUAL_VERSION_DATE = 0x140B38830` and `offsets.rs:26` `EXPECTED_VERSION_DATE = "Mar 10 2026"` are both tied to the March 10 patch. When the next real live-client Ghidra harvest lands, these must be updated in lockstep with `CLIENT_DATE`.
- `data/ghidra.db` currently reports `0` globals and `0` opcodes per the April 9 reproduction doc — the `functions` and `strings` tables populate cleanly but globals/opcodes don't import. Tracked separately; fix in `textquest/src/bin/import_ghidra.rs` when the harvest pipeline is next touched.

## 34 unresolved placeholders

All addresses below are `0x0` placeholders in `textquest-common/src/offsets.rs`. None of these can be promoted from this session's evidence alone — the sister `TextQuest-Ghidra` repo, where the Ghidra exports live, is not in this workspace. Each row links the constant to its file location and the operator action required to land a real value.

### Player / character data pointers (13)

| Constant | file:line | Blocker |
|---|---|---|
| `PINST_CHAR_DATA` | `textquest-common/src/offsets.rs:62` | Scan signature needs RE. No MQ upstream name maps cleanly. |
| `PINST_PC_DATA` | `:66` | Same. May be redundant with `PINST_LOCAL_PC` once RE'd. |
| `PINST_GROUP` | `:70` | Needs `pinstCGroup_x` promotion from `TextQuest-Ghidra/test/eqgame/`. |
| `PINST_RAID` | `:74` | Needs `pinstCRaid_x` promotion. |
| `PINST_ALT_ADV_MANAGER` | `:78` | `pinstAltAdvManager_x` — available in MQ eqlib but needs local Ghidra proof. |
| `PINST_MERC_MANAGER` | `:82` | `pinstMercenaryManager_x` — same. |
| `PINST_ACTIVE_BANKER` | `:86` | `pinstActiveBanker_x` — same. |
| `PINST_ACTIVE_MERCHANT` | `:90` | `pinstActiveMerchant_x` — same. |
| `PINST_ACTIVE_TRADE` | `:94` | `pinstTradeTarget_x` — same. |
| `PINST_TASK_MANAGER` | `:98` | `pinstTaskManager_x` — blocks PR #1796 reward automation edge cases. |
| `PINST_FELLOWSHIP` | `:102` | Low priority; no feature currently depends on this. |
| `PINST_ADVANCED_LOOT_WND` | `:106` | Blocks planned adv-loot automation. |
| `PINST_REAL_ESTATE_ITEMS` | `:110` | Not yet wired to a feature; defer. |

### UI / render hook candidates (6)

| Constant | file:line | Blocker |
|---|---|---|
| `SIDL_SCREEN_WND_INIT` | `:336` | `CSidlScreenWnd::Init` — UI hook for popup detection. Line 335 lost its `//` comment prefix (should be `///`). |
| `CXWND_MANAGER_REMOVE_WND` | `:339` | Comment prefix typo at `:338` — `/` not `///`. |
| `CMERCHANTWND_PURCHASEPAGEHANDLER_UPDATELIST` | `:342` | Comment prefix typo at `:341`. |
| `PROCESS_MOUSE_EVENTS` | `:345` | Hook candidate for anti-detection research. |
| `PROCESS_KEYBOARD_EVENTS` | `:348` | Same. |
| `CRENDER_RESET_DEVICE` | `:351` | Comment prefix typo at `:350`. Blocks graphics-recovery hook. |

### Zone / group / trade functions (13)

| Constant | file:line | Blocker |
|---|---|---|
| `EQ_END_ZONE` | `:419` | `EQFinishZone_x` adjacent — needs Ghidra proof on both. |
| `EQ_FINISH_ZONE` | `:423` | Same. |
| `EQ_ZONE_CHANGE` | `:427` | Needed for zone-transition IPC events. |
| `EQ_INVITE_PLAYER` | `:431` | Blocks cross-client invite automation. |
| `EQ_DISBAND` | `:435` | Same. |
| `EQ_FOLLOW_PLAYER` | `:439` | Core navigation primitive. |
| `EQ_MAKE_LEADER` | `:443` | Group/raid admin automation. |
| `EQ_BUY_ITEM` | `:447` | Needed for merchant automation. |
| `EQ_SELL_ITEM` | `:451` | Same. |
| `EQ_OPEN_TRADE` | `:455` | Trade automation. |
| `EQ_COMPLETE_TRADE` | `:459` | Same. |
| `EQ_BUFF_PLAYER` | `:463` | Buff-rotation automation. |
| `EQ_REMOVE_BUFF` | `:467` | Same. |

### Player manager IPC hooks (2)

| Constant | file:line | Blocker |
|---|---|---|
| `spawn_manager::PLAYER_MANAGER_CREATE_PLAYER` | `:1234` | Blocks event-driven spawn-add IPC — DLL currently polls. |
| `spawn_manager::PLAYER_MANAGER_PREP_DESTROY_PLAYER` | `:1239` | Same — spawn-destroy IPC. |

## Comment typos to fix during next edit pass

Lines 335, 338, 341, and 350 in `offsets.rs` have single-slash comments (`/`) instead of triple-slash doc comments (`///`). They don't break the build but degrade rustdoc output. Worth fixing in the next patch-day commit alongside real offset promotions.

## Cross-check against `config/offsets.json`

`config/offsets.json` carries the runtime-loaded offsets for the `OffsetDatabase` consumer. It was last updated as part of the 2026-04-09 reconciliation (per `Research-Test-Offset-Reconciliation.md`). Fields present in the JSON but not in the Rust constants (or vice versa) are not inherently a bug — they feed different code paths — but the `pinstSpawnManager = 0` observed in the April 9 runtime dump is a real gap the JSON still masks. Next harvest should specifically re-prove:

- `pinstSpawnManager` (gap flagged since April 9)
- `spawn_manager::PLAYER_LIST` (same)
- `pinstCDisplay`, `pinstCEverQuest`, `pinstCXWndManager` (outstanding per research doc)

## Recommended next actions

These changes require the sister `TextQuest-Ghidra` repo and a fresh Windows inject. Do not promote any of these from upstream MQ eqlib headers alone — that is the exact footgun that produced the incorrect `ZoneGuideManagerClient` promotion on 2026-04-09. Sequence:

1. Sync `TextQuest-Ghidra` on the workstation hosting GhidraMCP.
2. Run the harvest: `python scripts/harvest_mcp.py test`.
3. Re-prove each blocker constant using `ghidra_mcp.py call GET /get_function_by_address` / `GET /get_xrefs_to`.
4. Update `textquest-common/src/offsets.rs` in the same commit that bumps `CLIENT_DATE`, `ACTUAL_VERSION_DATE`, and `EXPECTED_VERSION_DATE`.
5. Re-run `python3 scripts/validate_offsets_sync.py` to confirm `offsets.rs` and `config/offsets.json` stay in sync.
6. Re-import `data/ghidra.db`: `cargo run --bin import_ghidra -- data/ghidra.db ../TextQuest-Ghidra/test/eqgame/ghidra-export/`.
7. Run the patch-day runbook's in-world verification slice to confirm the new values don't regress zone reads, target reads, or spawn enumeration.

## Related reading

- [EQ Patch Day Runbook](../patch-day-runbook.md) — operational playbook for bulk-delta vs. full RE paths
- [Research: Patch-Day Reproduction](Research-Patch-Day-Reproduction.md) — canonical evidence workflow
- [Research: Test Offset Reconciliation](Research-Test-Offset-Reconciliation.md) — per-surface promotion ledger
- [Offsets, EQ Internals, and MacroQuest References](Offsets-EQ-Internals-and-MacroQuest-References.md) — reference catalog
