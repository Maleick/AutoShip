# Offset Verification: eqgame.exe 20260310

- Verification date: 2026-04-24
- Binary: `/Users/maleick/Projects/TextQuest-Ghidra/staging/live/2026-04-11/eqgame.exe`
- Binary SHA-256: `25eaa879af050017b994d005013929a284c819cfd2972d228bdf4a90e7bf9a2d`
- Embedded client string observed with `strings`: `Mar 10 2026`
- PE image base: `0x0000000140000000`; entry RVA: `0x6DCD58`
- Source constants: `textquest-common/src/offsets.rs`
- Expected signatures available in repo: `textquest-common/src/pattern_db.rs` active-hack entries only

## Scope Notes

Issue #959 requested checking 52 function/global addresses. The current compiled
eqgame offset surface has 71 function/global entries in
`OffsetDatabase::from_compiled_offsets()`: 58 non-zero addresses and 13 zero
placeholders. This report checks every current non-zero eqgame.exe address and
lists the placeholders separately, making the table a superset of the original
52-address acceptance target.

Rows marked `verified` have a non-placeholder IDA signature in `pattern_db.rs`
and that signature resolved to the expected address in the 20260310 binary. Rows
marked `unknown` had bytes collected when present in the PE image, but no
independent expected signature was available in this repository. Global pointers
in the zero-fill portion of `.data` have no raw file bytes to compare; the
loader/runtime initializes that memory.

## Summary

- Pattern-backed offsets checked: 3
- Pattern-backed verified: 3
- Pattern-backed mismatches: 0
- Compiled non-zero function/global addresses checked: 58
- Compiled entries without independent expected bytes: 58
- Zero placeholder function/global entries listed: 13
- Follow-up mismatch issues filed: none; no signature-backed mismatch was found

## Pattern-Backed Verification

| Name | Category | Constant | Address | Expected bytes / resolver | Actual bytes at address | Section | Status | Evidence |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| `packetScrambler` | global | `OFFSET_PACKET_SCRAMBLER` | `0x0000000140E909A8` | `48 8B 1D ?? ?? ?? ?? 48 8B 43 08 48 63 50 04 48 8D 4B 10 48 03 CA E8 ?? ?? ?? ?? 4C 8B C0 41 8B D6 48 8B CB E8 ?? ?? ?? ??` (RipRelative +3) | `not present in raw file (virtual-zero-fill)` | .data | verified | 3 match(es), resolved `0x140e909a8`; `0x140e909a8`; `0x140e909a8` |
| `opcodeScramblerHton` | function | `OFFSET_HTON` | `0x0000000140679380` | `48 89 5C 24 10 48 89 6C 24 18 56 48 83 EC 20 49 8B E8 8B DA 48 8B F1 83 FA 15 75 ?? 48 8B 41 08 4C 63 48 04 41 8B 84 09 B0 02 00 00 48 0F BA E0 0C` (Direct) | `48 89 5c 24 10 48 89 6c 24 18 56 48 83 ec 20 49` | .text | verified | 1 match, resolved `0x140679380` |
| `networkSend` | function | `OFFSET_NETWORK_SEND` | `0x0000000140563130` | `48 89 5C 24 08 48 89 6C 24 10 56 57 41 56 48 83 EC 40 49 63 E9 49 8B F0 44 8B F2 48 8B F9 45 85 C9 0F 84 ?? ?? ?? ?? 4D 85 C0` (Direct) | `48 89 5c 24 08 48 89 6c 24 10 56 57 41 56 48 83` | .text | verified | 1 match, resolved `0x140563130` |

## Compiled Function And Global Address Sweep

| Name | Category | Constant | Address | Expected bytes | Actual bytes at address | Section | Status |
| --- | --- | --- | ---: | --- | --- | --- | --- |
| `pinstLocalPlayer` | global | `PINST_LOCAL_PLAYER` | `0x0000000140E8E380` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstControlledPlayer` | global | `PINST_CONTROLLED_PLAYER` | `0x0000000140E8E430` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstTarget` | global | `PINST_TARGET` | `0x0000000140E8E428` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstSpawnManager` | global | `PINST_SPAWN_MANAGER` | `0x0000000140F0CD90` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstLocalPC` | global | `PINST_LOCAL_PC` | `0x0000000140E909A8` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstSpellManager` | global | `PINST_SPELL_MANAGER` | `0x0000000140F0E6F0` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCDisplay` | global | `PINST_CDISPLAY` | `0x0000000140E8E450` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCEverQuest` | global | `PINST_CEVERQUEST` | `0x0000000140F11758` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCChatWindowManager` | global | `PINST_CCHAT_WINDOW_MANAGER` | `0x0000000140F22B20` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCInvSlotMgr` | global | `PINST_CINV_SLOT_MGR` | `0x0000000140DDD5F0` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCXWndManager` | global | `PINST_CXWND_MANAGER` | `0x0000000140F37B28` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstActiveCorpse` | global | `PINST_ACTIVE_CORPSE` | `0x0000000140E8E390` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstSGraphicsEngine` | global | `PINST_SGRAPHICSENGINE` | `0x0000000140F36B68` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `pinstCContextMenuManager` | global | `PINST_CONTEXT_MENU_MANAGER` | `0x0000000140F21BD0` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `instEQZoneInfo` | global | `INST_EQ_ZONE_INFO` | `0x0000000140E95CD4` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `outboundMsgCounter` | global | `OUTBOUND_MSG_COUNTER` | `0x0000000140F60FC8` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `inboundMsgCounter` | global | `INBOUND_MSG_COUNTER` | `0x0000000140F60FC4` | unknown - no independent Ghidra/pattern signature in repo | `not present in raw file (virtual-zero-fill)` | .data | unknown |
| `castSpell` | function | `CAST_SPELL` | `0x00000001400D9F20` | unknown - no independent Ghidra/pattern signature in repo | `48 8b c4 55 53 56 57 41 54 41 55 41 56 41 57 48` | .text | unknown |
| `doCombatAbility` | function | `DO_COMBAT_ABILITY` | `0x00000001402ED490` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 48 89 6c 24 20 56 57 41 54 41 56` | .text | unknown |
| `useSkill` | function | `USE_SKILL` | `0x00000001401052A0` | unknown - no independent Ghidra/pattern signature in repo | `40 53 56 57 48 83 ec 50 48 83 3d d8 93 e0 00 00` | .text | unknown |
| `canUseItem` | function | `CAN_USE_ITEM` | `0x00000001400EDDB0` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 08 48 89 6c 24 10 48 89 74 24 18 57` | .text | unknown |
| `doAttack` | function | `DO_ATTACK` | `0x000000014031B890` | unknown - no independent Ghidra/pattern signature in repo | `48 8b c4 48 89 58 08 4c 89 48 20 44 88 40 18 88` | .text | unknown |
| `executeCmd` | function | `EXECUTE_CMD` | `0x00000001402235B0` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 10 55 56 57 41 54 41 55 41 56 41 57` | .text | unknown |
| `interpretCmd` | function | `INTERPRET_CMD` | `0x0000000140283FB0` | unknown - no independent Ghidra/pattern signature in repo | `48 85 d2 0f 84 60 05 00 00 55 53 56 57 41 54 41` | .text | unknown |
| `rightClickedOnPlayer` | function | `RIGHT_CLICKED_ON_PLAYER` | `0x0000000140296D30` | unknown - no independent Ghidra/pattern signature in repo | `40 53 57 41 56 48 81 ec 80 04 00 00 48 8b 05 15` | .text | unknown |
| `clickedPlayer` | function | `CLICKED_PLAYER` | `0x00000001402724F0` | unknown - no independent Ghidra/pattern signature in repo | `48 83 ec 58 48 8b 0d 55 bf c1 00 48 8d 44 24 30` | .text | unknown |
| `issuePetCommand` | function | `ISSUE_PET_COMMAND` | `0x00000001402856A0` | unknown - no independent Ghidra/pattern signature in repo | `40 53 48 83 ec 50 83 fa 1e 0f 85 08 01 00 00 48` | .text | unknown |
| `getConLevel` | function | `GET_CON_LEVEL` | `0x00000001402E3C10` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 57 48 83 ec 20 80 ba 35 01 00 00` | .text | unknown |
| `getPcClient` | function | `GET_PC_CLIENT` | `0x0000000140307970` | unknown - no independent Ghidra/pattern signature in repo | `48 8b 91 10 04 00 00 48 85 d2 75 03 33 c0 c3 48` | .text | unknown |
| `doLoot` | function | `DO_LOOT` | `0x000000014022C0D0` | unknown - no independent Ghidra/pattern signature in repo | `48 83 ec 48 48 8b 15 4d 23 c6 00 48 85 d2 75 4e` | .text | unknown |
| `processGameEvents` | function | `PROCESS_GAME_EVENTS` | `0x000000014028E0F0` | unknown - no independent Ghidra/pattern signature in repo | `48 83 ec 28 e8 47 17 00 00 b9 ff ff ff ff e8 9d` | .text | unknown |
| `dspChat` | function | `DSP_CHAT` | `0x000000014010CFC0` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 08 48 89 74 24 10 48 89 7c 24 18 55` | .text | unknown |
| `realRenderWorld` | function | `REAL_RENDER_WORLD` | `0x00000001401A4650` | unknown - no independent Ghidra/pattern signature in repo | `48 85 c9 74 2a e8 b6 ac 17 00 48 8b d8 48 85 c0` | .text | unknown |
| `fixHeading` | function | `FIX_HEADING` | `0x0000000140661520` | unknown - no independent Ghidra/pattern signature in repo | `f3 0f 10 0d 94 0e 27 00 33 c0 0f 2f c1 8b c8 0f` | .text | unknown |
| `getBearing` | function | `GET_BEARING` | `0x0000000140258850` | unknown - no independent Ghidra/pattern signature in repo | `48 83 ec 68 0f 29 74 24 50 0f 28 f3 0f 29 7c 24` | .text | unknown |
| `freeTargetCastSpell` | function | `FREE_TARGET_CAST_SPELL` | `0x00000001402B5740` | unknown - no independent Ghidra/pattern signature in repo | `4c 8b dc 49 89 5b 08 49 89 6b 10 49 89 73 18 57` | .text | unknown |
| `changeHeight` | function | `CHANGE_HEIGHT` | `0x000000014031AB80` | unknown - no independent Ghidra/pattern signature in repo | `40 53 48 83 ec 40 0f 29 74 24 30 0f 28 c2 f3 0f` | .text | unknown |
| `zoneGuideManager` | function | `ZONE_GUIDE_MANAGER` | `0x00000001403571F0` | unknown - no independent Ghidra/pattern signature in repo | `40 53 48 83 ec 20 8b 0d f4 2a 24 01 65 48 8b 04` | .text | unknown |
| `charListEnterWorld` | function | `CHAR_LIST_ENTER_WORLD` | `0x00000001400D4B20` | unknown - no independent Ghidra/pattern signature in repo | `40 53 48 83 ec 40 48 8b d9 ba 03 00 00 00 e8 3d` | .text | unknown |
| `charListSelectChar` | function | `CHAR_LIST_SELECT_CHAR` | `0x00000001400D5D20` | unknown - no independent Ghidra/pattern signature in repo | `40 53 56 57 41 57 48 81 ec d8 01 00 00 48 8b 05` | .text | unknown |
| `cchatMgrGetRgba` | function | `CCHAT_MGR_GET_RGBA` | `0x00000001403B2D40` | unknown - no independent Ghidra/pattern signature in repo | `48 83 ec 28 81 fa ff 00 00 00 7e 34 8b ca e8 ed` | .text | unknown |
| `cchatMgrInitContextMenu` | function | `CCHAT_MGR_INIT_CONTEXT_MENU` | `0x00000001403B2ED0` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 48 89 74 24 20 55 57 41 54 41 56` | .text | unknown |
| `cchatMgrFreeChatWindow` | function | `CCHAT_MGR_FREE_CHAT_WINDOW` | `0x00000001403B1D40` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 10 48 89 6c 24 18 57 48 83 ec 20 48` | .text | unknown |
| `cchatMgrSetLockedActiveChat` | function | `CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT` | `0x00000001403BB240` | unknown - no independent Ghidra/pattern signature in repo | `44 8b 89 24 02 00 00 33 c0 45 85 c9 7e 1e 4c 8d` | .text | unknown |
| `cchatMgrCreateChatWindow` | function | `CCHAT_MGR_CREATE_CHAT_WINDOW` | `0x00000001403B1780` | unknown - no independent Ghidra/pattern signature in repo | `40 56 57 41 56 48 83 ec 20 4d 8b f1 48 8b f2 48` | .text | unknown |
| `invSlotMgrFindSlot` | function | `INV_SLOT_MGR_FIND_SLOT` | `0x0000000140421100` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 08 48 89 6c 24 10 48 89 74 24 18 48` | .text | unknown |
| `invSlotMgrMoveItem` | function | `INV_SLOT_MGR_MOVE_ITEM` | `0x0000000140421C90` | unknown - no independent Ghidra/pattern signature in repo | `40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac` | .text | unknown |
| `invSlotMgrSelectSlot` | function | `INV_SLOT_MGR_SELECT_SLOT` | `0x0000000140423FC0` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 55 56 57 48 81 ec 80 01 00 00 48` | .text | unknown |
| `invSlotGetItemBase` | function | `INV_SLOT_GET_ITEM_BASE` | `0x0000000140419520` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 08 57 48 83 ec 20 48 8b da 48 8b f9` | .text | unknown |
| `spellBookWndMemorizeSet` | function | `SPELL_BOOK_WND_MEMORIZE_SET` | `0x000000014050EFE0` | unknown - no independent Ghidra/pattern signature in repo | `40 53 56 41 55 41 56 48 83 ec 68 48 8b 05 66 8e` | .text | unknown |
| `netSend` | function | `NET_SEND` | `0x0000000140563330` | unknown - no independent Ghidra/pattern signature in repo | `08 48 8b d5 49 8b c9 e8 24 fc ff ff 48 8d 8f 48` | .text | unknown |
| `fileIntegrityDispatcher` | function | `FILE_INTEGRITY_DISPATCHER` | `0x0000000140564BC0` | unknown - no independent Ghidra/pattern signature in repo | `41 89 9e 0c 02 00 00 4d 89 a6 c8 02 00 00 4d 89` | .text | unknown |
| `serverMemcheckHandler` | function | `SERVER_MEMCHECK_HANDLER` | `0x00000001400B5720` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 55 56 57 41 54 41 55 41 56 41 57` | .text | unknown |
| `worldAuthenticate` | function | `WORLD_AUTHENTICATE` | `0x00000001402C9C80` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 10 48 89 74 24 20 55 57 41 56 48 8d` | .text | unknown |
| `systemFingerprint` | function | `SYSTEM_FINGERPRINT` | `0x0000000140594840` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 18 55 56 57 48 83 ec 20 48 8b da 48` | .text | unknown |
| `memcheck4ProcessEnum` | function | `MEMCHECK4_PROCESS_ENUM` | `0x0000000140299120` | unknown - no independent Ghidra/pattern signature in repo | `48 89 5c 24 08 4c 89 44 24 18 48 89 54 24 10 55` | .text | unknown |
| `contextMenuMgrHandleMenu` | function | `CONTEXT_MENU_MGR_HANDLE_MENU` | `0x000000014046E770` | unknown - no independent Ghidra/pattern signature in repo | `c0 e8 ba 24 13 00 4c 8b c0 48 8d 15 00 2b 46 00` | .text | unknown |
| `eqBeginZone` | function | `EQ_BEGIN_ZONE` | `0x000000014028D0E0` | unknown - no independent Ghidra/pattern signature in repo | `a8 00 00 00 4c 03 c9 41 8b d6 49 8b c9 4c 8d 05` | .text | unknown |

## Zero Placeholder Entries

These entries remain intentionally unresolved in `offsets.rs`; no binary address
can be checked until a scan signature or Ghidra address is curated.

| Name | Category | Constant | Address | Status |
| --- | --- | --- | ---: | --- |
| `eqEndZone` | function | `EQ_END_ZONE` | `0x0000000000000000` | unknown placeholder |
| `eqFinishZone` | function | `EQ_FINISH_ZONE` | `0x0000000000000000` | unknown placeholder |
| `eqZoneChange` | function | `EQ_ZONE_CHANGE` | `0x0000000000000000` | unknown placeholder |
| `eqInvitePlayer` | function | `EQ_INVITE_PLAYER` | `0x0000000000000000` | unknown placeholder |
| `eqDisband` | function | `EQ_DISBAND` | `0x0000000000000000` | unknown placeholder |
| `eqFollowPlayer` | function | `EQ_FOLLOW_PLAYER` | `0x0000000000000000` | unknown placeholder |
| `eqMakeLeader` | function | `EQ_MAKE_LEADER` | `0x0000000000000000` | unknown placeholder |
| `eqBuyItem` | function | `EQ_BUY_ITEM` | `0x0000000000000000` | unknown placeholder |
| `eqSellItem` | function | `EQ_SELL_ITEM` | `0x0000000000000000` | unknown placeholder |
| `eqOpenTrade` | function | `EQ_OPEN_TRADE` | `0x0000000000000000` | unknown placeholder |
| `eqCompleteTrade` | function | `EQ_COMPLETE_TRADE` | `0x0000000000000000` | unknown placeholder |
| `eqBuffPlayer` | function | `EQ_BUFF_PLAYER` | `0x0000000000000000` | unknown placeholder |
| `eqRemoveBuff` | function | `EQ_REMOVE_BUFF` | `0x0000000000000000` | unknown placeholder |
