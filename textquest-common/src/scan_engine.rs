//! Scan engine for runtime offset auto-detection.
//!
//! Orchestrates byte-pattern scanning against loaded EQ modules, resolves
//! matched offsets into preferred-base addresses, validates results against
//! compiled constants, and merges findings into an `OffsetDatabase`.
//!
//! See #746 (Auto Patch) for the roadmap.

use crate::offset_db::OffsetDatabase;
use crate::pattern_db::{OffsetCategory, ResolveMode, ScanEntry, ScanModule};
use crate::scanner::Pattern;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of scanning a single entry.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Symbolic name (matches `ScanEntry::name` and `OffsetDatabase` keys).
    pub name: String,
    /// Whether this is a function or global.
    pub category: OffsetCategory,
    /// Resolved address in preferred-base space.
    pub resolved_preferred: u64,
    /// Raw byte offset where the pattern matched within the module.
    pub matched_at_offset: usize,
    /// `true` if the resolved address matches the compiled constant.
    pub validated: bool,
}

/// Aggregate results from a full scan pass against one module.
#[derive(Debug, Clone)]
pub struct ScanReport {
    /// Which module was scanned.
    pub module: ScanModule,
    /// How many entries were scanned.
    pub entries_scanned: usize,
    /// How many entries produced a match.
    pub entries_found: usize,
    /// How many matched entries agreed with compiled constants.
    pub entries_validated: usize,
    /// Names of entries that failed to match.
    pub entries_failed: Vec<String>,
    /// Entries where the scan result differs from the compiled constant:
    /// `(name, expected_preferred, found_preferred)`.
    pub entries_moved: Vec<(String, u64, u64)>,
    /// Successful scan results.
    pub results: Vec<ScanResult>,
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scan all entries matching `module` against a memory region.
///
/// # Arguments
///
/// * `data` — byte slice of the module image (typically the full loaded image
///   or at minimum the `.text` section).
/// * `module_base` — runtime virtual address of the module (e.g. the actual
///   base of eqgame.exe as returned by `GetModuleHandle`).
/// * `preferred_base` — the compile-time preferred base of the module
///   (e.g. `0x140000000` for eqgame.exe).
/// * `module` — which module this scan targets (entries for other modules are
///   skipped).
/// * `entries` — the full `SCAN_ENTRIES` slice; only entries whose `module`
///   field matches are processed.
#[must_use]
pub fn scan_module(
    data: &[u8],
    module_base: u64,
    preferred_base: u64,
    module: ScanModule,
    entries: &[ScanEntry],
) -> ScanReport {
    let relevant: Vec<&ScanEntry> = entries.iter().filter(|e| e.module == module).collect();

    let mut report = ScanReport {
        module,
        entries_scanned: relevant.len(),
        entries_found: 0,
        entries_validated: 0,
        entries_failed: Vec::new(),
        entries_moved: Vec::new(),
        results: Vec::new(),
    };

    for entry in &relevant {
        let pattern = Pattern::from_ida(entry.pattern);
        let match_offset = crate::scanner::scan_region(data, &pattern);

        let Some(offset) = match_offset else {
            report.entries_failed.push(entry.name.to_string());
            continue;
        };

        // Resolve the match offset into a preferred-base address.
        let resolved = match entry.resolve {
            ResolveMode::Direct => {
                // The match offset is the function RVA.
                preferred_base + offset as u64
            }
            ResolveMode::RipRelative { disp_offset } => {
                resolve_rip_relative(data, offset, disp_offset, module_base, preferred_base)
            }
        };

        // Validate against compiled constant.
        let validated = entry.expected_preferred == Some(resolved);

        if let Some(expected) = entry.expected_preferred
            && !validated
        {
            report
                .entries_moved
                .push((entry.name.to_string(), expected, resolved));
        }

        if validated {
            report.entries_validated += 1;
        }
        report.entries_found += 1;
        report.results.push(ScanResult {
            name: entry.name.to_string(),
            category: entry.category,
            resolved_preferred: resolved,
            matched_at_offset: offset,
            validated,
        });
    }

    report
}

/// Resolve a RIP-relative displacement from a matched pattern.
///
/// The x86-64 RIP-relative addressing mode encodes a 32-bit signed displacement
/// relative to the NEXT instruction (i.e. after the 4-byte displacement field).
///
/// Given: `mov rax, [rip + disp32]` matched at byte offset `match_off` in
/// the module, the target address is:
///
/// ```text
/// instruction_addr = module_base + match_off
/// next_ip          = instruction_addr + disp_offset + 4
/// target_addr      = next_ip + sign_extend(disp32)
/// preferred_addr   = preferred_base + (target_addr - module_base)
/// ```
fn resolve_rip_relative(
    data: &[u8],
    match_off: usize,
    disp_offset: usize,
    module_base: u64,
    preferred_base: u64,
) -> u64 {
    let disp_pos = match_off + disp_offset;
    if disp_pos + 4 > data.len() {
        // Can't read displacement — return 0 to signal failure.
        return 0;
    }

    let disp_bytes: [u8; 4] = data[disp_pos..disp_pos + 4].try_into().unwrap();
    let displacement = i32::from_le_bytes(disp_bytes) as i64;

    // next_ip is the address of the byte after the displacement field.
    let next_ip = module_base as i64 + match_off as i64 + disp_offset as i64 + 4;
    let target = (next_ip + displacement) as u64;

    // Convert from runtime address to preferred-base address.
    preferred_base + (target - module_base)
}

/// Merge scan results into an `OffsetDatabase`, overwriting matching keys
/// in the `globals` and `functions` maps.
pub fn apply_to_offset_db(report: &ScanReport, db: &mut OffsetDatabase) {
    for result in &report.results {
        match result.category {
            OffsetCategory::Function => {
                db.functions
                    .insert(result.name.clone(), result.resolved_preferred);
            }
            OffsetCategory::Global => {
                db.globals
                    .insert(result.name.clone(), result.resolved_preferred);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern_db::{OffsetCategory, ResolveMode, ScanEntry, ScanModule};

    /// Helper: build a scan entry with a specific pattern.
    fn entry(name: &'static str, pattern: &'static str, resolve: ResolveMode) -> ScanEntry {
        ScanEntry {
            name,
            category: if matches!(resolve, ResolveMode::Direct) {
                OffsetCategory::Function
            } else {
                OffsetCategory::Global
            },
            module: ScanModule::EqGame,
            pattern,
            resolve,
            expected_preferred: None,
        }
    }

    #[test]
    fn direct_resolve_finds_pattern() {
        // Place a known byte sequence at offset 0x20 in a 256-byte buffer.
        let mut data = vec![0x00u8; 256];
        data[0x20] = 0x48;
        data[0x21] = 0x89;
        data[0x22] = 0x5C;
        data[0x23] = 0x24;
        data[0x24] = 0x08;

        let entries = [entry("testFunc", "48 89 5C 24 08", ResolveMode::Direct)];

        let module_base = 0x7FF6_0000_0000u64;
        let preferred_base = 0x0001_4000_0000u64;

        let report = scan_module(
            &data,
            module_base,
            preferred_base,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 1);
        assert_eq!(report.entries_failed.len(), 0);
        assert_eq!(report.results[0].matched_at_offset, 0x20);
        assert_eq!(report.results[0].resolved_preferred, preferred_base + 0x20);
    }

    #[test]
    fn rip_relative_resolve_computes_target() {
        // Simulate: `mov rax, [rip+0x12345678]` at offset 0x10 in the module.
        //
        // Instruction encoding: 48 8B 05 <disp32>
        // disp32 = 0x12345678 (little-endian: 78 56 34 12)
        //
        // next_ip     = module_base + 0x10 + 3 + 4 = module_base + 0x17
        // target_addr = next_ip + 0x12345678
        // preferred   = preferred_base + (target_addr - module_base)
        //             = preferred_base + 0x17 + 0x12345678
        //             = preferred_base + 0x1234568F

        let mut data = vec![0x00u8; 256];
        data[0x10] = 0x48;
        data[0x11] = 0x8B;
        data[0x12] = 0x05;
        // disp32 = 0x00000050 (little-endian)
        data[0x13] = 0x50;
        data[0x14] = 0x00;
        data[0x15] = 0x00;
        data[0x16] = 0x00;

        let entries = [ScanEntry {
            name: "testGlobal",
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??",
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: None,
        }];

        let module_base = 0x7FF6_0000_0000u64;
        let preferred_base = 0x0001_4000_0000u64;

        let report = scan_module(
            &data,
            module_base,
            preferred_base,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_found, 1);

        // next_ip = module_base + 0x10 + 3 + 4 = module_base + 0x17
        // target  = module_base + 0x17 + 0x50 = module_base + 0x67
        // preferred = preferred_base + 0x67
        let expected_preferred = preferred_base + 0x67;
        assert_eq!(report.results[0].resolved_preferred, expected_preferred);
    }

    #[test]
    fn rip_relative_negative_displacement() {
        // Test with a negative displacement (target is before the instruction).
        let mut data = vec![0x00u8; 256];
        data[0x80] = 0x48;
        data[0x81] = 0x8B;
        data[0x82] = 0x05;
        // disp32 = -0x50 = 0xFFFFFFB0 (little-endian: B0 FF FF FF)
        data[0x83] = 0xB0;
        data[0x84] = 0xFF;
        data[0x85] = 0xFF;
        data[0x86] = 0xFF;

        let entries = [ScanEntry {
            name: "testNeg",
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??",
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: None,
        }];

        let module_base = 0x7FF6_0000_0000u64;
        let preferred_base = 0x0001_4000_0000u64;

        let report = scan_module(
            &data,
            module_base,
            preferred_base,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_found, 1);

        // next_ip = module_base + 0x80 + 3 + 4 = module_base + 0x87
        // target  = module_base + 0x87 + (-0x50) = module_base + 0x37
        // preferred = preferred_base + 0x37
        let expected_preferred = preferred_base + 0x37;
        assert_eq!(report.results[0].resolved_preferred, expected_preferred);
    }

    #[test]
    fn pattern_not_found_records_failure() {
        let data = vec![0x00u8; 256];
        let entries = [entry("missing", "DE AD BE EF", ResolveMode::Direct)];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert_eq!(report.entries_failed, vec!["missing"]);
    }

    #[test]
    fn entries_for_wrong_module_are_skipped() {
        let mut data = vec![0x00u8; 256];
        data[0] = 0xAB;
        data[1] = 0xCD;

        let entries = [ScanEntry {
            name: "eqmainOnly",
            category: OffsetCategory::Function,
            module: ScanModule::EqMain,
            pattern: "AB CD",
            resolve: ResolveMode::Direct,
            expected_preferred: None,
        }];

        // Scan for EqGame — should skip the EqMain entry.
        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 0);
        assert_eq!(report.entries_found, 0);
    }

    #[test]
    fn validation_detects_match() {
        let mut data = vec![0x00u8; 256];
        data[0x30] = 0x55;
        data[0x31] = 0x48;

        let preferred_base = 0x0001_4000_0000u64;
        let expected = preferred_base + 0x30;

        let entries = [ScanEntry {
            name: "validated",
            category: OffsetCategory::Function,
            module: ScanModule::EqGame,
            pattern: "55 48",
            resolve: ResolveMode::Direct,
            expected_preferred: Some(expected),
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            preferred_base,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_validated, 1);
        assert!(report.results[0].validated);
        assert!(report.entries_moved.is_empty());
    }

    #[test]
    fn validation_detects_moved_offset() {
        let mut data = vec![0x00u8; 256];
        data[0x30] = 0x55;
        data[0x31] = 0x48;

        let preferred_base = 0x0001_4000_0000u64;
        // Expected at 0x40 but found at 0x30.
        let expected = preferred_base + 0x40;

        let entries = [ScanEntry {
            name: "moved",
            category: OffsetCategory::Function,
            module: ScanModule::EqGame,
            pattern: "55 48",
            resolve: ResolveMode::Direct,
            expected_preferred: Some(expected),
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            preferred_base,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_validated, 0);
        assert!(!report.results[0].validated);
        assert_eq!(report.entries_moved.len(), 1);
        assert_eq!(report.entries_moved[0].0, "moved");
        assert_eq!(report.entries_moved[0].1, expected);
        assert_eq!(report.entries_moved[0].2, preferred_base + 0x30);
    }

    #[test]
    fn apply_to_offset_db_merges_functions() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let original = db.get_function("castSpell").unwrap();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 0,
            entries_failed: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "castSpell".to_string(),
                category: OffsetCategory::Function,
                resolved_preferred: original + 0x100, // pretend it moved
                matched_at_offset: 0,
                validated: false,
            }],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db.get_function("castSpell"), Some(original + 0x100));
    }

    #[test]
    fn apply_to_offset_db_merges_globals() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let original = db.get_global("pinstLocalPlayer").unwrap();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 0,
            entries_failed: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "pinstLocalPlayer".to_string(),
                category: OffsetCategory::Global,
                resolved_preferred: original + 0x200,
                matched_at_offset: 0,
                validated: false,
            }],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db.get_global("pinstLocalPlayer"), Some(original + 0x200));
    }

    #[test]
    fn apply_preserves_unscanned_entries() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let original_use_skill = db.get_function("useSkill").unwrap();

        // Only scan castSpell — useSkill should be untouched.
        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 1,
            entries_failed: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "castSpell".to_string(),
                category: OffsetCategory::Function,
                resolved_preferred: 0xDEAD,
                matched_at_offset: 0,
                validated: true,
            }],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db.get_function("useSkill"), Some(original_use_skill));
    }

    #[test]
    fn empty_report_leaves_db_unchanged() {
        let db_before = OffsetDatabase::from_compiled_offsets();
        let mut db = db_before.clone();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 0,
            entries_found: 0,
            entries_validated: 0,
            entries_failed: vec![],
            entries_moved: vec![],
            results: vec![],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db, db_before);
    }
}
