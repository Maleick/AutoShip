//! Pattern database for runtime offset auto-detection.
//!
//! Maps symbolic offset names to IDA-style byte-pattern signatures and resolution
//! strategies. The scan engine (`scan_engine.rs`) iterates these entries, runs the
//! scanner, and resolves matched offsets into preferred-base addresses.
//!
//! Patterns are placeholder stubs until the Ghidra export script populates real
//! function prologues. See #746 (Auto Patch) for the roadmap.

use crate::offsets;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which loaded module to scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanModule {
    /// eqgame.exe
    EqGame,
    /// eqmain.dll (login screen)
    EqMain,
}

/// Whether the scan entry targets a function address or a global pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetCategory {
    /// A callable function (e.g. `CastSpell`, `ProcessGameEvents`).
    Function,
    /// A global pointer (e.g. `pinstLocalPlayer`, `pinstTarget`).
    Global,
}

/// How to convert a raw pattern-match offset into the target address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMode {
    /// The match offset IS the function address (RVA from module base).
    /// Used for function prologue patterns scanned directly.
    Direct,

    /// The matched instruction contains a RIP-relative 32-bit displacement.
    /// Read the `i32` at `match_offset + disp_offset`, then compute:
    ///   target = match_addr + disp_offset + 4 + displacement
    ///
    /// Used for `mov rax, [rip+disp]` patterns that reference global pointers.
    RipRelative {
        /// Byte offset within the matched pattern where the 4-byte displacement
        /// begins (e.g. 3 for `48 8B 05 <disp32>`).
        disp_offset: usize,
    },
}

/// A single offset scan specification.
///
/// Maps a symbolic offset name (matching keys in `OffsetDatabase`) to an
/// IDA-style byte pattern and a resolution strategy.
#[derive(Debug, Clone)]
pub struct ScanEntry {
    /// Symbolic name matching `OffsetDatabase` keys (e.g. `"castSpell"`,
    /// `"pinstLocalPlayer"`).
    pub name: &'static str,

    /// Whether this is a function or global pointer.
    pub category: OffsetCategory,

    /// Which module to scan (eqgame.exe or eqmain.dll).
    pub module: ScanModule,

    /// IDA-style pattern string (e.g. `"48 89 5C 24 ?? 57 48 83 EC 30"`).
    pub pattern: &'static str,

    /// How to resolve the match offset into the target address.
    pub resolve: ResolveMode,

    /// Expected preferred-base address from compiled constants (`offsets.rs`).
    /// Used for validation — if the scan result differs, the offset moved
    /// (likely due to a patch). `None` if no compiled constant exists.
    pub expected_preferred: Option<u64>,
}

// ---------------------------------------------------------------------------
// Scan entries
// ---------------------------------------------------------------------------

/// All scan entries for auto-detection.
///
/// Patterns are placeholder stubs (`CC CC CC`) until real function prologues
/// are exported from Ghidra. The `expected_preferred` values are populated
/// from `offsets.rs` to enable the validation framework.
///
/// Covers all function addresses and global pointers in `offsets.rs`.
pub const SCAN_ENTRIES: &[ScanEntry] = &[
    // ═══════════════════════════════════════════════════════════════════
    // Functions (eqgame.exe) — hook targets and callable addresses
    // ═══════════════════════════════════════════════════════════════════
    ScanEntry {
        name: "processGameEvents",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::PROCESS_GAME_EVENTS),
    },
    ScanEntry {
        name: "realRenderWorld",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::REAL_RENDER_WORLD),
    },
    ScanEntry {
        name: "interpretCmd",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::INTERPRET_CMD),
    },
    ScanEntry {
        name: "executeCmd",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::EXECUTE_CMD),
    },
    ScanEntry {
        name: "castSpell",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CAST_SPELL),
    },
    ScanEntry {
        name: "doCombatAbility",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::DO_COMBAT_ABILITY),
    },
    ScanEntry {
        name: "useSkill",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::USE_SKILL),
    },
    ScanEntry {
        name: "canUseItem",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CAN_USE_ITEM),
    },
    ScanEntry {
        name: "doAttack",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::DO_ATTACK),
    },
    ScanEntry {
        name: "rightClickedOnPlayer",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::RIGHT_CLICKED_ON_PLAYER),
    },
    ScanEntry {
        name: "clickedPlayer",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CLICKED_PLAYER),
    },
    ScanEntry {
        name: "issuePetCommand",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::ISSUE_PET_COMMAND),
    },
    ScanEntry {
        name: "getConLevel",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::GET_CON_LEVEL),
    },
    ScanEntry {
        name: "getPcClient",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::GET_PC_CLIENT),
    },
    ScanEntry {
        name: "doLoot",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::DO_LOOT),
    },
    ScanEntry {
        name: "dspChat",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::DSP_CHAT),
    },
    ScanEntry {
        name: "fixHeading",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::FIX_HEADING),
    },
    ScanEntry {
        name: "getBearing",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::GET_BEARING),
    },
    ScanEntry {
        name: "charListEnterWorld",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CHAR_LIST_ENTER_WORLD),
    },
    ScanEntry {
        name: "charListSelectChar",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CHAR_LIST_SELECT_CHAR),
    },
    ScanEntry {
        name: "freeTargetCastSpell",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::FREE_TARGET_CAST_SPELL),
    },
    ScanEntry {
        name: "changeHeight",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CHANGE_HEIGHT),
    },
    ScanEntry {
        name: "zoneGuideManager",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::ZONE_GUIDE_MANAGER),
    },
    ScanEntry {
        name: "cchatMgrGetRgba",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CCHAT_MGR_GET_RGBA),
    },
    ScanEntry {
        name: "cchatMgrInitContextMenu",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CCHAT_MGR_INIT_CONTEXT_MENU),
    },
    ScanEntry {
        name: "cchatMgrFreeChatWindow",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CCHAT_MGR_FREE_CHAT_WINDOW),
    },
    ScanEntry {
        name: "cchatMgrSetLockedActiveChat",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT),
    },
    ScanEntry {
        name: "cchatMgrCreateChatWindow",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CCHAT_MGR_CREATE_CHAT_WINDOW),
    },
    ScanEntry {
        name: "invSlotMgrFindSlot",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::INV_SLOT_MGR_FIND_SLOT),
    },
    ScanEntry {
        name: "invSlotMgrMoveItem",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::INV_SLOT_MGR_MOVE_ITEM),
    },
    ScanEntry {
        name: "invSlotMgrSelectSlot",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::INV_SLOT_MGR_SELECT_SLOT),
    },
    ScanEntry {
        name: "invSlotGetItemBase",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::INV_SLOT_GET_ITEM_BASE),
    },
    ScanEntry {
        name: "spellBookWndMemorizeSet",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::SPELL_BOOK_WND_MEMORIZE_SET),
    },
    ScanEntry {
        name: "contextMenuMgrHandleMenu",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::CONTEXT_MENU_MGR_HANDLE_MENU),
    },
    // ─── Anti-cheat / network functions ─────────────────────────────
    ScanEntry {
        name: "netSend",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::NET_SEND),
    },
    ScanEntry {
        name: "fileIntegrityDispatcher",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::FILE_INTEGRITY_DISPATCHER),
    },
    ScanEntry {
        name: "serverMemcheckHandler",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::SERVER_MEMCHECK_HANDLER),
    },
    ScanEntry {
        name: "worldAuthenticate",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::WORLD_AUTHENTICATE),
    },
    ScanEntry {
        name: "systemFingerprint",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::SYSTEM_FINGERPRINT),
    },
    ScanEntry {
        name: "memcheck4ProcessEnum",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::MEMCHECK4_PROCESS_ENUM),
    },
    // ═══════════════════════════════════════════════════════════════════
    // Global pointers (eqgame.exe) — RIP-relative resolution
    // ═══════════════════════════════════════════════════════════════════
    // These patterns match instructions like `mov rax, [rip+disp32]`
    // that reference global pointers. The disp_offset points to the
    // 4-byte displacement within the instruction.
    ScanEntry {
        name: "pinstLocalPlayer",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_LOCAL_PLAYER),
    },
    ScanEntry {
        name: "pinstControlledPlayer",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CONTROLLED_PLAYER),
    },
    ScanEntry {
        name: "pinstTarget",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_TARGET),
    },
    ScanEntry {
        name: "pinstSpawnManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_SPAWN_MANAGER),
    },
    ScanEntry {
        name: "pinstLocalPC",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_LOCAL_PC),
    },
    ScanEntry {
        name: "pinstSpellManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_SPELL_MANAGER),
    },
    ScanEntry {
        name: "pinstCDisplay",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CDISPLAY),
    },
    ScanEntry {
        name: "pinstCEverQuest",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CEVERQUEST),
    },
    ScanEntry {
        name: "pinstCChatWindowManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CCHAT_WINDOW_MANAGER),
    },
    ScanEntry {
        name: "pinstCInvSlotMgr",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CINV_SLOT_MGR),
    },
    ScanEntry {
        name: "pinstCXWndManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CXWND_MANAGER),
    },
    ScanEntry {
        name: "pinstActiveCorpse",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_ACTIVE_CORPSE),
    },
    ScanEntry {
        name: "pinstSGraphicsEngine",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_SGRAPHICSENGINE),
    },
    ScanEntry {
        name: "pinstCContextMenuManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CONTEXT_MENU_MANAGER),
    },
    ScanEntry {
        name: "instEQZoneInfo",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::zone_info::INST_EQ_ZONE_INFO),
    },
    ScanEntry {
        name: "outboundMsgCounter",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::OUTBOUND_MSG_COUNTER),
    },
    ScanEntry {
        name: "inboundMsgCounter",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::INBOUND_MSG_COUNTER),
    },
    // ═══════════════════════════════════════════════════════════════════
    // Functions (eqmain.dll) — login/server select
    // ═══════════════════════════════════════════════════════════════════
    ScanEntry {
        name: "eqmain_joinServer",
        category: OffsetCategory::Function,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::eqmain::JOIN_SERVER),
    },
    ScanEntry {
        name: "eqmain_loginViewManager",
        category: OffsetCategory::Function,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::eqmain::LOGIN_VIEW_MANAGER),
    },
    ScanEntry {
        name: "eqmain_loginControllerGiveTime",
        category: OffsetCategory::Function,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::Direct,
        expected_preferred: Some(offsets::eqmain::LOGIN_CONTROLLER_GIVE_TIME),
    },
    // ─── eqmain.dll global pointers ─────────────────────────────────
    ScanEntry {
        name: "eqmain_pinstSidlManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::eqmain::SIDL_MANAGER),
    },
    ScanEntry {
        name: "eqmain_pinstLoginServerAPI",
        category: OffsetCategory::Global,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::eqmain::LOGIN_SERVER_API),
    },
    ScanEntry {
        name: "eqmain_pinstCXWndManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::eqmain::CXWND_MANAGER),
    },
    ScanEntry {
        name: "eqmain_pinstLoginClient",
        category: OffsetCategory::Global,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::eqmain::PINST_LOGIN_CLIENT),
    },
    ScanEntry {
        name: "eqmain_pinstLoginController",
        category: OffsetCategory::Global,
        module: ScanModule::EqMain,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::eqmain::PINST_LOGIN_CONTROLLER),
    },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::Pattern;
    use std::collections::HashSet;

    #[test]
    fn all_patterns_parse_without_panic() {
        for entry in SCAN_ENTRIES {
            // from_ida panics on invalid patterns — this is the test.
            let p = Pattern::from_ida(entry.pattern);
            assert!(
                p.len() >= 3,
                "pattern for {} is suspiciously short ({} bytes)",
                entry.name,
                p.len()
            );
        }
    }

    #[test]
    fn no_duplicate_names() {
        let mut seen = HashSet::new();
        for entry in SCAN_ENTRIES {
            assert!(
                seen.insert(entry.name),
                "duplicate scan entry name: {}",
                entry.name
            );
        }
    }

    #[test]
    fn eqgame_expected_preferred_above_preferred_base() {
        for entry in SCAN_ENTRIES {
            if entry.module != ScanModule::EqGame {
                continue;
            }
            if let Some(expected) = entry.expected_preferred {
                assert!(
                    expected >= crate::offsets::EQ_PREFERRED_BASE,
                    "{}: EqGame expected_preferred {:#x} is below EQ_PREFERRED_BASE",
                    entry.name,
                    expected
                );
            }
        }
    }

    #[test]
    fn rip_relative_entries_have_valid_disp_offset() {
        for entry in SCAN_ENTRIES {
            if let ResolveMode::RipRelative { disp_offset } = entry.resolve {
                let p = Pattern::from_ida(entry.pattern);
                assert!(
                    disp_offset + 4 <= p.len(),
                    "{}: disp_offset {} + 4 exceeds pattern length {}",
                    entry.name,
                    disp_offset,
                    p.len()
                );
            }
        }
    }

    #[test]
    fn function_entries_use_direct_resolve() {
        for entry in SCAN_ENTRIES {
            if entry.category == OffsetCategory::Function {
                assert_eq!(
                    entry.resolve,
                    ResolveMode::Direct,
                    "{}: function entries should use Direct resolve",
                    entry.name
                );
            }
        }
    }

    #[test]
    fn global_entries_use_rip_relative_resolve() {
        for entry in SCAN_ENTRIES {
            if entry.category == OffsetCategory::Global {
                assert!(
                    matches!(entry.resolve, ResolveMode::RipRelative { .. }),
                    "{}: global entries should use RipRelative resolve",
                    entry.name
                );
            }
        }
    }

    #[test]
    fn entry_count() {
        // 40 eqgame functions + 17 eqgame globals + 3 eqmain functions + 5 eqmain globals = 65
        assert_eq!(SCAN_ENTRIES.len(), 65);
    }
}
