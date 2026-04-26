//! Scan engine for runtime offset auto-detection.
//!
//! Orchestrates byte-pattern scanning against loaded EQ modules, resolves
//! matched offsets into preferred-base addresses or field offsets, validates results against
//! compiled constants, and merges findings into an `OffsetDatabase`.
//!
//! See #746 (Auto Patch) for the roadmap.

use crate::{
    offset_db::OffsetDatabase,
    pattern_db::{OffsetCategory, ResolveMode, ScanEntry, ScanModule},
    scanner::Pattern,
};

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of scanning a single entry.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Symbolic name (matches `ScanEntry::name` and `OffsetDatabase` keys).
    pub name: String,
    /// Whether this is an address or struct field offset.
    pub category: OffsetCategory,
    /// Resolved address in preferred-base space, or a struct field offset.
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
    /// Names of entries that did not yield a usable resolved address.
    ///
    /// Includes both entries whose pattern was not found and entries whose
    /// pattern matched but address resolution failed (e.g. RIP displacement
    /// out of bounds or resolved target outside the module image).
    pub entries_failed: Vec<String>,
    /// Names of entries skipped because they use placeholder patterns.
    pub entries_skipped: Vec<String>,
    /// Entries where the scan result differs from the compiled constant:
    /// `(name, expected_preferred, found_preferred)`.
    pub entries_moved: Vec<(String, u64, u64)>,
    /// Successful scan results.
    pub results: Vec<ScanResult>,
}

/// Borrowed view of one module image to scan.
#[derive(Debug, Clone, Copy)]
pub struct ModuleImage<'a> {
    /// Module identifier.
    pub module: ScanModule,
    /// Runtime base address of the module image.
    pub module_base: u64,
    /// Compile-time preferred base address of the module image.
    pub preferred_base: u64,
    /// Full module bytes starting at offset 0.
    pub data: &'a [u8],
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Check if a pattern string is a placeholder stub (all `CC` bytes).
///
/// Placeholder patterns like `"CC CC CC CC CC CC CC CC"` would match the first
/// `int3` padding run in any module, producing identical bogus results for
/// every entry. The scan engine skips these to avoid misleading log output.
fn is_placeholder_pattern(pattern: &str) -> bool {
    let mut tokens = pattern.split_whitespace();
    match tokens.next() {
        Some(first) => {
            first.eq_ignore_ascii_case("CC") && tokens.all(|t| t.eq_ignore_ascii_case("CC"))
        }
        None => false,
    }
}

/// Scan all entries matching `module` against a memory region.
///
/// # Arguments
///
/// * `data` — byte slice of the **full** module image starting at
///   `module_base`. Both `Direct` and `RipRelative` resolution assume byte
///   offset 0 in `data` corresponds to `module_base` (i.e. the image's DOS
///   header). Passing a sub-section (e.g. `.text` only) will produce incorrect
///   preferred-base addresses for `Direct` and `RipRelative` entries.
/// * `module_base` — runtime virtual address of the module (e.g. the actual
///   base of eqgame.exe as returned by `GetModuleHandle`).
/// * `preferred_base` — the compile-time preferred base of the module (e.g.
///   `0x140000000` for eqgame.exe).
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
        entries_skipped: Vec::new(),
        entries_moved: Vec::new(),
        results: Vec::new(),
    };

    for entry in &relevant {
        // Skip placeholder patterns (all-CC stubs). These would match the first
        // 0xCC run in the module and produce identical, misleading results for
        // every entry. Real patterns from Ghidra export will replace them.
        if is_placeholder_pattern(entry.pattern.as_str()) {
            report.entries_skipped.push(entry.name.to_string());
            continue;
        }

        let pattern = Pattern::from_ida(entry.pattern.as_str());
        let match_offset = crate::scanner::scan_region(data, &pattern);

        let Some(offset) = match_offset else {
            report.entries_failed.push(entry.name.to_string());
            continue;
        };

        // Resolve the match offset into a preferred-base address or field offset.
        let resolved = match entry.resolve {
            ResolveMode::Direct => {
                // The match offset is the function RVA.
                Some(preferred_base + offset as u64)
            }
            ResolveMode::RipRelative { disp_offset } => {
                resolve_rip_relative(data, offset, disp_offset, module_base, preferred_base)
            }
            ResolveMode::ExtractDisplacement {
                disp_offset,
                disp_size,
            } => resolve_displacement(data, offset, disp_offset, disp_size),
        };

        let Some(resolved) = resolved else {
            report.entries_failed.push(entry.name.to_string());
            continue;
        };

        // Validate against compiled constant.
        if let Some(expected) = entry.expected_preferred
            && expected != resolved
        {
            report
                .entries_moved
                .push((entry.name.to_string(), expected, resolved));
        }

        let validated = entry.expected_preferred == Some(resolved);
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

/// Scan multiple module images using the same entry catalog.
#[must_use]
pub fn scan_modules(images: &[ModuleImage<'_>], entries: &[ScanEntry]) -> Vec<ScanReport> {
    images
        .iter()
        .map(|image| {
            scan_module(
                image.data,
                image.module_base,
                image.preferred_base,
                image.module,
                entries,
            )
        })
        .collect()
}

/// Scan multiple modules and merge the resolved offsets into a database.
///
/// Reports are returned in the same order as `images`.
pub fn scan_modules_into_offset_db(
    images: &[ModuleImage<'_>],
    entries: &[ScanEntry],
    db: &mut OffsetDatabase,
) -> Vec<ScanReport> {
    let reports = scan_modules(images, entries);
    for report in &reports {
        db.merge_scan_results(report);
    }
    reports
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
///
/// Returns `None` if the displacement cannot be read, the computed target
/// overflows, or the target falls outside the module image.
fn resolve_rip_relative(
    data: &[u8],
    match_off: usize,
    disp_offset: usize,
    module_base: u64,
    preferred_base: u64,
) -> Option<u64> {
    let disp_pos = match_off + disp_offset;
    if disp_pos + 4 > data.len() {
        return None;
    }

    let disp_bytes: [u8; 4] = data[disp_pos..disp_pos + 4].try_into().unwrap();
    let displacement = i32::from_le_bytes(disp_bytes) as i64;

    // next_ip is the address of the byte after the displacement field.
    let next_ip = module_base as i128 + match_off as i128 + disp_offset as i128 + 4;
    let target = next_ip + displacement as i128;

    // Validate: target must fit in u64 and lie within the module image.
    if target < 0 || target > u64::MAX as i128 {
        return None;
    }
    let target = target as u64;

    let module_end = module_base.checked_add(data.len() as u64)?;
    if target < module_base || target >= module_end {
        return None;
    }

    // Convert from runtime address to preferred-base address.
    let offset_within_module = target.checked_sub(module_base)?;
    preferred_base.checked_add(offset_within_module)
}

/// Extract a signed little-endian displacement from matched instruction bytes.
///
/// x86/x64 memory displacements are signed two's-complement values. This
/// function sign-extends the decoded 1/2/4-byte displacement to `i64` before
/// returning. Struct-field offsets are expected to be non-negative; a negative
/// displacement (e.g., `[rcx-0x10]`) is rejected with `None` so that
/// `apply_to_offset_db` never persists a bogus large positive value.
///
/// This is used for struct field offsets encoded directly in instructions such
/// as `mov eax, [rcx+0x3A0]`, where the displacement is the offset to apply to
/// the object pointer rather than a module address that needs rebasing.
fn resolve_displacement(
    data: &[u8],
    match_off: usize,
    disp_offset: usize,
    disp_size: usize,
) -> Option<u64> {
    let disp_pos = match_off.checked_add(disp_offset)?;
    let disp_end = disp_pos.checked_add(disp_size)?;
    let bytes = data.get(disp_pos..disp_end)?;

    let signed: i64 = match disp_size {
        1 => i64::from(bytes[0] as i8),
        2 => i64::from(i16::from_le_bytes(bytes.try_into().ok()?)),
        4 => i64::from(i32::from_le_bytes(bytes.try_into().ok()?)),
        8 => i64::from_le_bytes(bytes.try_into().ok()?),
        _ => return None,
    };

    // Struct-field offsets are non-negative; negative displacements are not
    // valid targets for offset-DB merging and would otherwise be stored as a
    // huge unsigned value after casting.
    if signed < 0 {
        return None;
    }
    Some(signed as u64)
}

// ---------------------------------------------------------------------------
// EQ version detection
// ---------------------------------------------------------------------------

/// Expected EQ client date that our compiled offsets target.
///
/// If the running client reports a different date, offsets are likely stale.
/// Delegates to the canonical constant in `offsets.rs`.
pub const EXPECTED_CLIENT_DATE: &str = crate::offsets::CLIENT_DATE;

/// Scan the module image for an EQ client date string.
///
/// EQ embeds `__ActualVersionDate` as an ASCII string in the form `"MonthName
/// DD YYYY"` (e.g., `"Mar 10 2026"`). This function scans for date-like
/// patterns and returns a normalized `YYYYMMDD` string if found.
///
/// Returns `None` if no date string is found.
#[must_use]
pub fn detect_client_date(data: &[u8]) -> Option<String> {
    // EQ uses abbreviated month names in __ActualVersionDate.
    // max_day is the maximum valid day for that month (Feb uses 29 to allow leap
    // years).
    const MONTHS: &[(&[u8], &str, u32)] = &[
        (b"Jan", "01", 31),
        (b"Feb", "02", 29),
        (b"Mar", "03", 31),
        (b"Apr", "04", 30),
        (b"May", "05", 31),
        (b"Jun", "06", 30),
        (b"Jul", "07", 31),
        (b"Aug", "08", 31),
        (b"Sep", "09", 30),
        (b"Oct", "10", 31),
        (b"Nov", "11", 30),
        (b"Dec", "12", 31),
    ];

    // Scan for patterns like "Mon DD YYYY" (11 bytes) or "Mon  D YYYY" (11 bytes).
    // We look for month abbreviations followed by a space, day digits, space,
    // 4-digit year. Minimum date string is 11 bytes (e.g., "Jan  5 2026"), so
    // iterate up to len-11.
    for window_start in 0..data.len().saturating_sub(10) {
        for &(month_bytes, month_num, max_day) in MONTHS {
            if data[window_start..window_start + 3] != *month_bytes {
                continue;
            }
            // Must be followed by a space
            if data[window_start + 3] != b' ' {
                continue;
            }
            // Parse day (1 or 2 digits) and year (4 digits)
            let rest = &data[window_start + 4..];
            if rest.len() < 7 {
                continue;
            }

            let (day_str, year_start) = if rest[0] == b' ' && rest[1].is_ascii_digit() {
                // "Mon  D YYYY" format (single-digit day with leading space)
                if rest[2] != b' ' {
                    continue;
                }
                let Ok(s) = std::str::from_utf8(&rest[1..2]) else {
                    continue;
                };
                (s, 3usize)
            } else if rest[0].is_ascii_digit() && rest[1].is_ascii_digit() {
                // "Mon DD YYYY" format
                if rest[2] != b' ' {
                    continue;
                }
                let Ok(s) = std::str::from_utf8(&rest[0..2]) else {
                    continue;
                };
                (s, 3usize)
            } else {
                continue;
            };

            if rest.len() < year_start + 4 {
                continue;
            }
            let year_bytes = &rest[year_start..year_start + 4];
            if !year_bytes.iter().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Ok(year) = std::str::from_utf8(year_bytes) else {
                continue;
            };

            // Validate year is reasonable (2020-2099)
            let Ok(year_num) = year.parse::<u32>() else {
                continue;
            };
            if !(2020..2100).contains(&year_num) {
                continue;
            }

            let Ok(day_num) = day_str.parse::<u32>() else {
                continue;
            };
            if !(1..=max_day).contains(&day_num) {
                continue;
            }

            return Some(format!("{}{}{:02}", year, month_num, day_num));
        }
    }
    None
}

/// Check whether the running EQ client matches our expected version.
///
/// Returns `(detected_date, matches_expected)`. If the date cannot be found,
/// returns `(None, false)`.
#[must_use]
pub fn check_version(data: &[u8]) -> (Option<String>, bool) {
    match detect_client_date(data) {
        Some(date) => {
            let matches = date == EXPECTED_CLIENT_DATE;
            (Some(date), matches)
        }
        None => (None, false),
    }
}

// ---------------------------------------------------------------------------
// Database merging
// ---------------------------------------------------------------------------

/// Merge scan results into an `OffsetDatabase`, overwriting matching keys in
/// the map matching the scanned module and offset category.
///
/// Only function/global results with a non-zero resolved address are applied —
/// zero indicates an address resolution failure. Field offsets may legitimately
/// be zero and are applied when they fit in `usize`.
pub fn apply_to_offset_db(report: &ScanReport, db: &mut OffsetDatabase) {
    for result in &report.results {
        match result.category {
            OffsetCategory::Function | OffsetCategory::Global => {
                if result.resolved_preferred == 0 {
                    continue;
                }
                let map = match (report.module, result.category) {
                    (ScanModule::EqGame, OffsetCategory::Function) => &mut db.functions,
                    (ScanModule::EqGame, OffsetCategory::Global) => &mut db.globals,
                    (ScanModule::EqMain, OffsetCategory::Function) => &mut db.eqmain_functions,
                    (ScanModule::EqMain, OffsetCategory::Global) => &mut db.eqmain_globals,
                    (ScanModule::EqGraphics, OffsetCategory::Function) => {
                        &mut db.eqgraphics_functions
                    }
                    (ScanModule::EqGraphics, OffsetCategory::Global) => &mut db.eqgraphics_globals,
                    _ => unreachable!(),
                };
                map.insert(result.name.clone(), result.resolved_preferred);
            }
            OffsetCategory::PlayerBaseField => {
                if let Ok(offset) = usize::try_from(result.resolved_preferred) {
                    db.player_base.insert(result.name.clone(), offset);
                }
            }
            OffsetCategory::PlayerZoneField => {
                if let Ok(offset) = usize::try_from(result.resolved_preferred) {
                    db.player_zone.insert(result.name.clone(), offset);
                }
            }
            OffsetCategory::SpawnManagerField => {
                if let Ok(offset) = usize::try_from(result.resolved_preferred) {
                    db.spawn_manager.insert(result.name.clone(), offset);
                }
            }
            OffsetCategory::ContextMenuManagerField => {
                if let Ok(offset) = usize::try_from(result.resolved_preferred) {
                    db.context_menu_manager.insert(result.name.clone(), offset);
                }
            }
            OffsetCategory::ContextMenuField => {
                if let Ok(offset) = usize::try_from(result.resolved_preferred) {
                    db.context_menu.insert(result.name.clone(), offset);
                }
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
    fn entry(name: &str, pattern: &str, resolve: ResolveMode) -> ScanEntry {
        ScanEntry {
            name: name.to_string(),
            category: if matches!(resolve, ResolveMode::Direct) {
                OffsetCategory::Function
            } else {
                OffsetCategory::Global
            },
            module: ScanModule::EqGame,
            pattern: pattern.to_string(),
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
        // Simulate: `mov rax, [rip+0x50]` at offset 0x10 in the module.
        //
        // Instruction encoding: 48 8B 05 <disp32>
        // disp32 = 0x00000050 (little-endian: 50 00 00 00)
        //
        // next_ip     = module_base + 0x10 + 3 + 4 = module_base + 0x17
        // target_addr = next_ip + 0x50
        // preferred   = preferred_base + (target_addr - module_base)
        //             = preferred_base + 0x17 + 0x50
        //             = preferred_base + 0x67

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
            name: "testGlobal".to_string(),
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??".to_string(),
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
            name: "testNeg".to_string(),
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??".to_string(),
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
    fn extract_displacement_reads_four_byte_field_offset() {
        // Simulate: `mov eax, [rcx+0x3A0]`.
        // Instruction encoding: 8B 81 <disp32>
        let mut data = vec![0x00u8; 256];
        data[0x20] = 0x8B;
        data[0x21] = 0x81;
        data[0x22] = 0xA0;
        data[0x23] = 0x03;
        data[0x24] = 0x00;
        data[0x25] = 0x00;

        let mut hp_current = entry(
            "player_zone::hpCurrent",
            "8B 81 ?? ?? 00 00",
            ResolveMode::ExtractDisplacement {
                disp_offset: 2,
                disp_size: 4,
            },
        );
        hp_current.category = OffsetCategory::PlayerZoneField;
        let entries = [hp_current];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_found, 1);
        assert_eq!(report.results[0].resolved_preferred, 0x3A0);
        assert_eq!(report.results[0].matched_at_offset, 0x20);
    }

    #[test]
    fn extract_displacement_reads_one_byte_field_offset() {
        // Simulate: `movzx eax, byte ptr [rcx+0x64]`.
        let mut data = vec![0x00u8; 256];
        data[0x10] = 0x0F;
        data[0x11] = 0xB6;
        data[0x12] = 0x41;
        data[0x13] = 0x64;

        let mut level = entry(
            "player_zone::level",
            "0F B6 41 ??",
            ResolveMode::ExtractDisplacement {
                disp_offset: 3,
                disp_size: 1,
            },
        );
        level.category = OffsetCategory::PlayerZoneField;
        let entries = [level];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_found, 1);
        assert_eq!(report.results[0].resolved_preferred, 0x64);
    }

    #[test]
    fn extract_displacement_out_of_bounds_records_failure() {
        let mut data = vec![0x00u8; 16];
        data[13] = 0x8B;
        data[14] = 0x81;
        data[15] = 0xA0;

        let entries = [entry(
            "player_zone::hpCurrent",
            "8B 81 A0",
            ResolveMode::ExtractDisplacement {
                disp_offset: 2,
                disp_size: 4,
            },
        )];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert!(report.results.is_empty());
        assert_eq!(report.entries_failed, vec!["player_zone::hpCurrent"]);
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
            name: "eqmainOnly".to_string(),
            category: OffsetCategory::Function,
            module: ScanModule::EqMain,
            pattern: "AB CD".to_string(),
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
            name: "validated".to_string(),
            category: OffsetCategory::Function,
            module: ScanModule::EqGame,
            pattern: "55 48".to_string(),
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
            name: "moved".to_string(),
            category: OffsetCategory::Function,
            module: ScanModule::EqGame,
            pattern: "55 48".to_string(),
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
            entries_skipped: vec![],
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
            entries_skipped: vec![],
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
    fn apply_to_offset_db_routes_results_by_module() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let report = ScanReport {
            module: ScanModule::EqMain,
            entries_scanned: 2,
            entries_found: 2,
            entries_validated: 0,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![
                ScanResult {
                    name: "cxwndManager".to_string(),
                    category: OffsetCategory::Global,
                    resolved_preferred: 0x0001_8038_24C8,
                    matched_at_offset: 0,
                    validated: false,
                },
                ScanResult {
                    name: "joinServer".to_string(),
                    category: OffsetCategory::Function,
                    resolved_preferred: 0x0001_8001_8060,
                    matched_at_offset: 0,
                    validated: false,
                },
            ],
        };

        apply_to_offset_db(&report, &mut db);

        assert_eq!(db.get_eqmain_global("cxwndManager"), Some(0x0001_8038_24C8));
        assert_eq!(db.get_eqmain_function("joinServer"), Some(0x0001_8001_8060));
        assert!(db.get_global("cxwndManager").is_none());
        assert!(db.get_function("joinServer").is_none());
    }

    #[test]
    fn apply_to_offset_db_keeps_eqgraphics_globals_module_scoped() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let report = ScanReport {
            module: ScanModule::EqGraphics,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 0,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "graphicsSingleton".to_string(),
                category: OffsetCategory::Global,
                resolved_preferred: 0x0001_40A0_9000,
                matched_at_offset: 0,
                validated: false,
            }],
        };

        apply_to_offset_db(&report, &mut db);

        assert_eq!(
            db.get_eqgraphics_global("graphicsSingleton"),
            Some(0x0001_40A0_9000)
        );
        assert!(db.get_global("graphicsSingleton").is_none());
    }

    #[test]
    fn scan_modules_into_offset_db_merges_each_module_snapshot() {
        let mut eqgame_data = vec![0x90_u8; 64];
        eqgame_data[0x10..0x15].copy_from_slice(&[0x48, 0x89, 0x5C, 0x24, 0x08]);

        let mut eqmain_data = vec![0x90_u8; 64];
        eqmain_data[0x18..0x1D].copy_from_slice(&[0x48, 0x89, 0x5C, 0x24, 0x08]);

        let entries = [
            ScanEntry {
                name: "castSpell".to_string(),
                category: OffsetCategory::Function,
                module: ScanModule::EqGame,
                pattern: "48 89 5C 24 08".to_string(),
                resolve: ResolveMode::Direct,
                expected_preferred: None,
            },
            ScanEntry {
                name: "joinServer".to_string(),
                category: OffsetCategory::Function,
                module: ScanModule::EqMain,
                pattern: "48 89 5C 24 08".to_string(),
                resolve: ResolveMode::Direct,
                expected_preferred: None,
            },
        ];

        let eqgame_image = ModuleImage {
            module: ScanModule::EqGame,
            module_base: 0x7FF6_0000_0000,
            preferred_base: 0x0001_4000_0000,
            data: &eqgame_data,
        };
        let eqmain_image = ModuleImage {
            module: ScanModule::EqMain,
            module_base: 0x7FF7_0000_0000,
            preferred_base: 0x0001_8000_0000,
            data: &eqmain_data,
        };

        let mut db = OffsetDatabase::from_compiled_offsets();
        let original_cast_spell = db.get_function("castSpell").unwrap();
        let original_join_server = db.get_eqmain_function("joinServer").unwrap();

        db.functions
            .insert("castSpell".to_string(), original_cast_spell + 0x200);
        db.eqmain_functions
            .insert("joinServer".to_string(), original_join_server + 0x200);

        let reports = scan_modules_into_offset_db(&[eqgame_image, eqmain_image], &entries, &mut db);

        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].module, ScanModule::EqGame);
        assert_eq!(reports[1].module, ScanModule::EqMain);
        assert_eq!(db.get_function("castSpell"), Some(0x0001_4000_0010));
        assert_eq!(db.get_eqmain_function("joinServer"), Some(0x0001_8000_0018));
    }

    #[test]
    fn apply_to_offset_db_merges_player_zone_field_offsets() {
        let mut db = OffsetDatabase::from_compiled_offsets();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 1,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "hpCurrent".to_string(),
                category: OffsetCategory::PlayerZoneField,
                resolved_preferred: 0x3A0,
                matched_at_offset: 0x20,
                validated: true,
            }],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db.get_player_zone_offset("hpCurrent"), Some(0x3A0));
    }

    #[test]
    fn apply_to_offset_db_merges_player_base_field_offsets() {
        let mut db = OffsetDatabase::from_compiled_offsets();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 1,
            entries_found: 1,
            entries_validated: 1,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![ScanResult {
                name: "spawnId".to_string(),
                category: OffsetCategory::PlayerBaseField,
                resolved_preferred: 0x158,
                matched_at_offset: 0x20,
                validated: true,
            }],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db.get_player_base_offset("spawnId"), Some(0x158));
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
            entries_skipped: vec![],
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
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![],
        };

        apply_to_offset_db(&report, &mut db);
        assert_eq!(db, db_before);
    }

    #[test]
    fn rip_relative_disp_read_out_of_bounds_records_failure() {
        // The pattern matches near the end of the buffer, but there aren't enough
        // bytes to read the 4-byte displacement field.
        let mut data = vec![0x00u8; 16];
        data[13] = 0x48;
        data[14] = 0x8B;
        data[15] = 0x05; // disp32 would need bytes [16..20], but buffer ends at 16

        let entries = [ScanEntry {
            name: "ripDispOob".to_string(),
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05".to_string(),
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: None,
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert!(report.results.is_empty());
        assert_eq!(report.entries_failed, vec!["ripDispOob"]);
    }

    #[test]
    fn rip_relative_target_outside_module_records_failure() {
        // Displacement resolves beyond the 256-byte module buffer.
        let mut data = vec![0x00u8; 256];
        data[0x80] = 0x48;
        data[0x81] = 0x8B;
        data[0x82] = 0x05;
        // disp32 = 0x100 → next_ip (module_base+0x87) + 0x100 = module_base+0x187
        // which is beyond the 256-byte module.
        data[0x83] = 0x00;
        data[0x84] = 0x01;
        data[0x85] = 0x00;
        data[0x86] = 0x00;

        let entries = [ScanEntry {
            name: "ripTargetOob".to_string(),
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??".to_string(),
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: None,
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert!(report.results.is_empty());
        assert_eq!(report.entries_failed, vec!["ripTargetOob"]);
    }

    #[test]
    fn rip_relative_target_before_module_records_failure() {
        // Negative displacement large enough to resolve before module_base.
        let mut data = vec![0x00u8; 256];
        data[0x10] = 0x48;
        data[0x11] = 0x8B;
        data[0x12] = 0x05;
        // disp32 = -0x1000 (0xFFFFF000) → next_ip (module_base+0x17) - 0x1000
        // = module_base - 0xFE9, which is before module_base.
        data[0x13] = 0x00;
        data[0x14] = 0xF0;
        data[0x15] = 0xFF;
        data[0x16] = 0xFF;

        let entries = [ScanEntry {
            name: "ripTargetBeforeBase".to_string(),
            category: OffsetCategory::Global,
            module: ScanModule::EqGame,
            pattern: "48 8B 05 ?? ?? ?? ??".to_string(),
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: None,
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert!(report.results.is_empty());
        assert_eq!(report.entries_failed, vec!["ripTargetBeforeBase"]);
    }

    // ─── Version detection tests ────────────────────────────────────

    #[test]
    fn detect_client_date_standard_format() {
        // "Mar 10 2026" embedded at offset 0x100 in a buffer
        let mut data = vec![0x00u8; 512];
        let date_str = b"Mar 10 2026";
        data[0x100..0x100 + date_str.len()].copy_from_slice(date_str);
        assert_eq!(detect_client_date(&data), Some("20260310".to_string()));
    }

    #[test]
    fn detect_client_date_single_digit_day() {
        let mut data = vec![0x00u8; 512];
        let date_str = b"Jan  5 2026";
        data[0x100..0x100 + date_str.len()].copy_from_slice(date_str);
        assert_eq!(detect_client_date(&data), Some("20260105".to_string()));
    }

    #[test]
    fn detect_client_date_not_found() {
        let data = vec![0x00u8; 512];
        assert_eq!(detect_client_date(&data), None);
    }

    #[test]
    fn detect_client_date_rejects_impossible_dates() {
        // Apr 31 doesn't exist — should not match.
        let mut data = vec![0x00u8; 512];
        data[0x100..0x100 + 11].copy_from_slice(b"Apr 31 2026");
        assert_eq!(detect_client_date(&data), None);

        // Feb 30 doesn't exist — should not match.
        data[0x100..0x100 + 11].copy_from_slice(b"Feb 30 2026");
        assert_eq!(detect_client_date(&data), None);

        // Jun 31 doesn't exist — should not match.
        data[0x100..0x100 + 11].copy_from_slice(b"Jun 31 2026");
        assert_eq!(detect_client_date(&data), None);
    }

    #[test]
    fn check_version_matches_expected() {
        let mut data = vec![0x00u8; 512];
        data[0x100..0x100 + 11].copy_from_slice(b"Apr 15 2026");
        let (date, matches) = check_version(&data);
        assert_eq!(date, Some("20260415".to_string()));
        assert!(matches);
    }

    #[test]
    fn check_version_detects_newer_client() {
        let mut data = vec![0x00u8; 512];
        data[0x100..0x100 + 11].copy_from_slice(b"Apr 14 2026");
        let (date, matches) = check_version(&data);
        assert_eq!(date, Some("20260414".to_string()));
        assert!(!matches);
    }

    #[test]
    fn check_version_no_date_found() {
        let data = vec![0x00u8; 512];
        let (date, matches) = check_version(&data);
        assert!(date.is_none());
        assert!(!matches);
    }

    #[test]
    fn detect_client_date_at_end_of_buffer() {
        // Date string placed at the very end of the buffer (edge case for off-by-one).
        let date_str = b"Mar 10 2026";
        let mut data = vec![0x00u8; date_str.len()];
        data[..date_str.len()].copy_from_slice(date_str);
        assert_eq!(detect_client_date(&data), Some("20260310".to_string()));
    }

    // ─── Placeholder detection tests ────────────────────────────────

    #[test]
    fn is_placeholder_detects_all_cc() {
        assert!(is_placeholder_pattern("CC CC CC CC CC CC CC CC"));
        assert!(is_placeholder_pattern("cc cc cc"));
    }

    #[test]
    fn is_placeholder_rejects_real_patterns() {
        assert!(!is_placeholder_pattern("48 89 5C 24 ?? 57 48 83 EC 30"));
        assert!(!is_placeholder_pattern("48 8B 05 ?? ?? ?? ??"));
        // A mix with one CC is not all-placeholder
        assert!(!is_placeholder_pattern("CC 48 89 5C"));
    }

    #[test]
    fn placeholder_entries_are_skipped_in_scan() {
        // A buffer full of 0xCC bytes — placeholder patterns would match everywhere.
        let data = vec![0xCCu8; 256];
        let entries = [ScanEntry {
            name: "placeholderFunc".to_string(),
            category: OffsetCategory::Function,
            module: ScanModule::EqGame,
            pattern: "CC CC CC CC CC CC CC CC".to_string(),
            resolve: ResolveMode::Direct,
            expected_preferred: Some(0x0001_4000_0030),
        }];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        // Placeholder should be tracked as skipped, not as a real scan failure.
        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert!(report.entries_failed.is_empty());
        assert_eq!(report.entries_skipped, vec!["placeholderFunc"]);
    }

    // ─── Additional is_placeholder_pattern edge cases ────────────────────

    #[test]
    fn is_placeholder_empty_string() {
        assert!(!is_placeholder_pattern(""));
    }

    #[test]
    fn is_placeholder_single_cc() {
        assert!(is_placeholder_pattern("CC"));
    }

    #[test]
    fn is_placeholder_mixed_case_cc() {
        assert!(is_placeholder_pattern("Cc cC CC"));
    }

    #[test]
    fn is_placeholder_non_cc_single_byte() {
        assert!(!is_placeholder_pattern("48"));
    }

    #[test]
    fn is_placeholder_cc_followed_by_non_cc() {
        assert!(!is_placeholder_pattern("CC CC 48"));
    }

    #[test]
    fn is_placeholder_whitespace_only() {
        // split_whitespace on all-whitespace yields None on first next()
        assert!(!is_placeholder_pattern("   "));
    }

    // ─── scan_module additional coverage ─────────────────────────────────

    #[test]
    fn scan_module_empty_data() {
        let entries = [entry("ghost", "48 89", ResolveMode::Direct)];
        let report = scan_module(&[], 0x1000, 0x1000, ScanModule::EqGame, &entries);
        assert_eq!(report.entries_scanned, 1);
        assert_eq!(report.entries_found, 0);
        assert_eq!(report.entries_failed, vec!["ghost"]);
    }

    #[test]
    fn scan_module_empty_entries() {
        let data = vec![0x48u8; 16];
        let report = scan_module(&data, 0x1000, 0x1000, ScanModule::EqGame, &[]);
        assert_eq!(report.entries_scanned, 0);
        assert_eq!(report.entries_found, 0);
        assert!(report.results.is_empty());
    }

    #[test]
    fn scan_module_multiple_entries_mixed_results() {
        let mut data = vec![0x00u8; 256];
        // Place "48 89" at offset 0x10
        data[0x10] = 0x48;
        data[0x11] = 0x89;

        let entries = [
            entry("found", "48 89", ResolveMode::Direct),
            entry("missing", "FF D0", ResolveMode::Direct),
        ];

        let report = scan_module(
            &data,
            0x7FF6_0000_0000,
            0x0001_4000_0000,
            ScanModule::EqGame,
            &entries,
        );

        assert_eq!(report.entries_scanned, 2);
        assert_eq!(report.entries_found, 1);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].name, "found");
        assert_eq!(report.entries_failed, vec!["missing"]);
    }

    #[test]
    fn scan_result_resolved_preferred_correct() {
        let mut data = vec![0x00u8; 256];
        data[0x50] = 0xAB;
        data[0x51] = 0xCD;

        let entries = [entry("precise", "AB CD", ResolveMode::Direct)];
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
        // resolved_preferred = preferred_base + match_offset
        assert_eq!(report.results[0].resolved_preferred, preferred_base + 0x50);
        assert_eq!(report.results[0].matched_at_offset, 0x50);
    }

    #[test]
    fn scan_report_has_correct_module() {
        let data = vec![0x00u8; 16];
        let report = scan_module(&data, 0x1000, 0x1000, ScanModule::EqMain, &[]);
        assert_eq!(report.module, ScanModule::EqMain);
    }
}
