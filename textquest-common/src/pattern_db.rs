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
/// Phase 1 covers the most critical offsets: hook targets and core globals.
pub const SCAN_ENTRIES: &[ScanEntry] = &[
    // ─── Functions (hook targets) ───────────────────────────────────
    ScanEntry {
        name: "processGameEvents",
        category: OffsetCategory::Function,
        module: ScanModule::EqGame,
        // Placeholder — replace with real prologue from Ghidra export
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
    // ─── Globals (RIP-relative resolution) ──────────────────────────
    // These patterns match instructions like `mov rax, [rip+disp32]`
    // that reference global pointers. The disp_offset points to the
    // 4-byte displacement within the instruction.
    ScanEntry {
        name: "pinstLocalPlayer",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        // Placeholder — real pattern would be e.g. "48 8B 05 ?? ?? ?? ?? 48 85 C0 74"
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_LOCAL_PLAYER),
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
        name: "pinstCEverQuest",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_CEVERQUEST),
    },
    ScanEntry {
        name: "pinstSpawnManager",
        category: OffsetCategory::Global,
        module: ScanModule::EqGame,
        pattern: "CC CC CC CC CC CC CC CC",
        resolve: ResolveMode::RipRelative { disp_offset: 3 },
        expected_preferred: Some(offsets::PINST_SPAWN_MANAGER),
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
    fn all_expected_preferred_above_preferred_base() {
        for entry in SCAN_ENTRIES {
            if let Some(expected) = entry.expected_preferred {
                assert!(
                    expected >= crate::offsets::EQ_PREFERRED_BASE,
                    "{}: expected_preferred {:#x} is below EQ_PREFERRED_BASE",
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
        // 6 functions + 4 globals = 10 entries in Phase 1
        assert_eq!(SCAN_ENTRIES.len(), 10);
    }
}
