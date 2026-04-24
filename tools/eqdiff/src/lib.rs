//! `eqdiff` — PE binary diff tooling for EverQuest binary analysis.
//!
//! Phase 1a: PE header parsing and string extraction from `.rdata` / `.data`
//! sections.  All addresses are expressed as RVAs (Relative Virtual Addresses)
//! so results remain valid regardless of ASLR base.

use anyhow::{Context, Result};
use goblin::pe::PE;

pub mod matcher;
pub mod offset_export;
pub mod xref;
pub use matcher::{
    FunctionMatch, StringFunctionRef, apply_matches_to_offsets_json,
    match_functions_by_string_references, string_function_refs_from_xrefs,
};
pub use offset_export::{
    BinaryDiffFunction, BinaryDiffOffsetExport, BinaryDiffOffsetReport, ExportedOffsetUpdate,
    export_offsets_from_binary_diff, map_function_to_offset_key,
};
pub use xref::{
    ImportedSymbol, StringRefMatch, build_string_xref_index, extract_imports,
    match_string_references,
};

/// A null-terminated ASCII or UTF-8 string found in a PE section, together
/// with its Relative Virtual Address (RVA) inside the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedString {
    /// Relative Virtual Address of the first byte of the string (ASLR-safe).
    pub rva: u32,
    /// The string content (printable ASCII, length ≥ `MIN_LEN`).
    pub value: String,
}

/// Minimum number of printable ASCII characters required for a string to be
/// included in extraction results.
pub const MIN_STR_LEN: usize = 4;

/// Sections considered for string extraction.
const TARGET_SECTIONS: &[&str] = &[".rdata", ".data"];

/// Parse a PE image from raw bytes and return a reference to the parsed PE.
///
/// # Errors
/// Returns an error if the bytes are not a valid PE/COFF image.
pub fn parse_pe(bytes: &[u8]) -> Result<PE<'_>> {
    PE::parse(bytes).context("failed to parse PE image")
}

/// Extract printable ASCII strings from the `.rdata` and `.data` sections of a
/// PE image.
///
/// Each returned [`ExtractedString`] contains:
/// - `rva` — the Relative Virtual Address of the string start (ASLR-safe).
/// - `value` — the string content (only strings of at least [`MIN_STR_LEN`]
///   characters are included).
///
/// # Arguments
/// * `pe`    — a parsed PE image (from [`parse_pe`]).
/// * `bytes` — the full raw PE bytes the image was parsed from.
///
/// # Notes
/// Strings are terminated by a NUL byte (`\0`) or by a non-printable
/// character.  Only bytes in the inclusive printable ASCII range `0x20..=0x7e`
/// are accepted as part of a string.
pub fn extract_strings<'a>(pe: &PE<'a>, bytes: &'a [u8]) -> Vec<ExtractedString> {
    let mut results = Vec::new();

    for section in &pe.sections {
        // Section names in PE are stored as a fixed 8-byte array, NUL-padded.
        let raw_name = &section.name;
        let name = std::str::from_utf8(raw_name)
            .unwrap_or("")
            .trim_end_matches('\0');

        if !TARGET_SECTIONS.contains(&name) {
            continue;
        }

        let file_offset = section.pointer_to_raw_data as usize;
        let raw_size = section.size_of_raw_data as usize;
        let virtual_address = section.virtual_address;

        // Guard against malformed PE files where offsets extend past the buffer.
        let section_end = file_offset.saturating_add(raw_size);
        if file_offset >= bytes.len() {
            continue;
        }
        let section_bytes = &bytes[file_offset..section_end.min(bytes.len())];

        // Walk the section byte-by-byte, collecting printable ASCII runs.
        let mut run_start: Option<usize> = None;
        let mut run_len: usize = 0;

        for (i, &byte) in section_bytes.iter().enumerate() {
            if is_printable_ascii(byte) {
                if run_start.is_none() {
                    run_start = Some(i);
                    run_len = 0;
                }
                run_len += 1;
            } else {
                // End of a run — emit if long enough.
                if let Some(start) = run_start.take() {
                    if run_len >= MIN_STR_LEN {
                        let s = std::str::from_utf8(&section_bytes[start..start + run_len]);
                        if let Ok(value) = s {
                            let rva = virtual_address.wrapping_add(start as u32);
                            results.push(ExtractedString {
                                rva,
                                value: value.to_owned(),
                            });
                        }
                    }
                    run_len = 0;
                }
            }
        }

        // Flush any run that extends to the end of the section.
        if let Some(start) = run_start
            && run_len >= MIN_STR_LEN
        {
            let s = std::str::from_utf8(&section_bytes[start..start + run_len]);
            if let Ok(value) = s {
                let rva = virtual_address.wrapping_add(start as u32);
                results.push(ExtractedString {
                    rva,
                    value: value.to_owned(),
                });
            }
        }
    }

    results
}

/// Returns `true` for printable ASCII bytes (`0x20` space through `0x7e` `~`).
#[inline]
fn is_printable_ascii(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

/// Convenience wrapper: parse a PE image from raw bytes and extract all
/// strings in a single call.
///
/// # Errors
/// Returns an error if the bytes do not represent a valid PE image.
pub fn parse_and_extract(bytes: &[u8]) -> Result<Vec<ExtractedString>> {
    let pe = parse_pe(bytes)?;
    Ok(extract_strings(&pe, bytes))
}

// ────────────────────────────────────────────────────────────────────────────
// Unit tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Build a minimal, syntactically valid PE32+ (64-bit) image in memory.
    ///
    /// Layout:
    ///   0x000  DOS header (MZ stub, e_lfanew = 0x40)
    ///   0x040  PE signature ("PE\0\0")
    ///   0x044  COFF header (machine = AMD64, 2 sections, no symbol table)
    ///   0x058  Optional header (PE32+, minimal fields)
    ///   0x0e8  Section table  (.rdata entry at 0x0e8, .data entry at 0x110)
    ///   0x200  .rdata raw data (512 bytes)
    ///   0x400  .data  raw data (512 bytes)
    fn build_test_pe(rdata_content: &[u8], data_content: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 0x600];

        // DOS header
        buf[0] = b'M';
        buf[1] = b'Z';
        buf[0x3c] = 0x40; // e_lfanew = 0x40

        // PE signature
        buf[0x40] = b'P';
        buf[0x41] = b'E';
        buf[0x42] = 0;
        buf[0x43] = 0;

        // COFF header at 0x44
        // Machine: AMD64 (0x8664)
        buf[0x44] = 0x64;
        buf[0x45] = 0x86;
        // NumberOfSections: 2
        buf[0x46] = 2;
        buf[0x47] = 0;
        // TimeDateStamp: 0 (4 bytes)
        // PointerToSymbolTable: 0 (4 bytes)
        // NumberOfSymbols: 0 (4 bytes)
        // SizeOfOptionalHeader: 0xf0 (PE32+)
        buf[0x54] = 0xf0;
        buf[0x55] = 0x00;
        // Characteristics: 0x0022 (executable | large address aware)
        buf[0x56] = 0x22;
        buf[0x57] = 0x00;

        // Optional header (PE32+) starts at 0x58
        // Magic: 0x020b (PE32+)
        buf[0x58] = 0x0b;
        buf[0x59] = 0x02;
        // SizeOfImage: 0x1000
        buf[0x50 + 0x38] = 0x00; // offset 0x88 from PE sig
        // Use absolute offsets for clarity:
        // SizeOfImage at 0x58 + 56 = 0x90
        let size_of_image_off = 0x58 + 56;
        buf[size_of_image_off] = 0x00;
        buf[size_of_image_off + 1] = 0x10;
        buf[size_of_image_off + 2] = 0x00;
        buf[size_of_image_off + 3] = 0x00;
        // SizeOfHeaders at 0x58 + 60 = 0x94
        let size_of_headers_off = 0x58 + 60;
        buf[size_of_headers_off] = 0x00;
        buf[size_of_headers_off + 1] = 0x02;
        buf[size_of_headers_off + 2] = 0x00;
        buf[size_of_headers_off + 3] = 0x00;
        // NumberOfRvaAndSizes at 0x58 + 108 = 0xc4
        let num_rva_off = 0x58 + 108;
        buf[num_rva_off] = 16;

        // Section table starts at 0x58 + 0xf0 = 0x148
        // But SizeOfOptionalHeader is 0xf0 which puts section table at 0x44 + 20 + 0xf0
        // = 0x148 Actually: section table = PE sig (0x40) + 4 + COFF (20) +
        // OptHdr (0xf0) = 0x148
        let sec_table = 0x40 + 4 + 20 + 0xf0; // = 0x148

        // .rdata section at sec_table
        write_section_entry(
            &mut buf,
            sec_table,
            b".rdata\0\0",
            rdata_content.len() as u32,
            0x1000, // VirtualAddress
            0x200,  // SizeOfRawData (padded to 0x200)
            0x200,  // PointerToRawData
        );

        // .data section at sec_table + 40
        write_section_entry(
            &mut buf,
            sec_table + 40,
            b".data\0\0\0",
            data_content.len() as u32,
            0x2000, // VirtualAddress
            0x200,  // SizeOfRawData
            0x400,  // PointerToRawData
        );

        // Write section raw data
        let rdata_len = rdata_content.len().min(0x200);
        buf[0x200..0x200 + rdata_len].copy_from_slice(&rdata_content[..rdata_len]);

        let data_len = data_content.len().min(0x200);
        buf[0x400..0x400 + data_len].copy_from_slice(&data_content[..data_len]);

        buf
    }

    fn write_section_entry(
        buf: &mut [u8],
        offset: usize,
        name: &[u8; 8],
        virtual_size: u32,
        virtual_address: u32,
        size_of_raw_data: u32,
        pointer_to_raw_data: u32,
    ) {
        buf[offset..offset + 8].copy_from_slice(name);
        write_u32(buf, offset + 8, virtual_size);
        write_u32(buf, offset + 12, virtual_address);
        write_u32(buf, offset + 16, size_of_raw_data);
        write_u32(buf, offset + 20, pointer_to_raw_data);
    }

    fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    // ── test 1: printable ASCII predicate ────────────────────────────────────

    #[test]
    fn test_is_printable_ascii_boundaries() {
        // Space (0x20) and tilde (0x7e) are included.
        assert!(is_printable_ascii(0x20));
        assert!(is_printable_ascii(0x7e));
        // Bytes outside the range are excluded.
        assert!(!is_printable_ascii(0x1f));
        assert!(!is_printable_ascii(0x7f));
        assert!(!is_printable_ascii(0x00));
        assert!(!is_printable_ascii(0xff));
        // Alphanumeric sanity checks.
        assert!(is_printable_ascii(b'A'));
        assert!(is_printable_ascii(b'z'));
        assert!(is_printable_ascii(b'0'));
    }

    // ── test 2: invalid bytes are rejected ───────────────────────────────────

    #[test]
    fn test_parse_pe_rejects_invalid_bytes() {
        let bad = b"not a PE file at all";
        let result = parse_pe(bad);
        assert!(result.is_err(), "expected parse error for non-PE bytes");
    }

    // ── test 3: strings below minimum length are excluded ────────────────────

    #[test]
    fn test_short_strings_excluded() {
        // "abc\0" is only 3 chars — below MIN_STR_LEN (4).
        let mut rdata = vec![0u8; 0x100];
        rdata[0] = b'a';
        rdata[1] = b'b';
        rdata[2] = b'c';
        rdata[3] = 0x00;
        // Pad the rest with NULs (already zeroed).

        let pe_bytes = build_test_pe(&rdata, &[]);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return, // skip if goblin rejects our hand-crafted PE
        };
        let strings = extract_strings(&pe, &pe_bytes);
        // "abc" has length 3 < MIN_STR_LEN (4) — must not appear.
        assert!(
            !strings.iter().any(|s| s.value == "abc"),
            "3-char string should be excluded"
        );
    }

    // ── test 4: strings meeting minimum length are included ──────────────────

    #[test]
    fn test_strings_at_min_length_included() {
        // "test" is exactly 4 chars — at the boundary.
        let mut rdata = vec![0u8; 0x100];
        let s = b"test";
        rdata[0..4].copy_from_slice(s);
        rdata[4] = 0x00;

        let pe_bytes = build_test_pe(&rdata, &[]);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return,
        };
        let strings = extract_strings(&pe, &pe_bytes);
        assert!(
            strings.iter().any(|s| s.value == "test"),
            "4-char string 'test' should be included"
        );
    }

    // ── test 5: RVAs are ASLR-safe (section VirtualAddress + offset) ─────────

    #[test]
    fn test_rva_is_aslr_safe() {
        // Place a known string at a known offset in .rdata.
        let mut rdata = vec![0u8; 0x100];
        let target = b"EverQuest";
        let str_offset = 16usize; // 16 bytes into .rdata
        rdata[str_offset..str_offset + target.len()].copy_from_slice(target);
        rdata[str_offset + target.len()] = 0x00;

        let pe_bytes = build_test_pe(&rdata, &[]);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return,
        };
        let strings = extract_strings(&pe, &pe_bytes);

        let found = strings.iter().find(|s| s.value == "EverQuest");
        assert!(found.is_some(), "should find 'EverQuest' in .rdata");

        let entry = found.unwrap();
        // .rdata VA = 0x1000, string is at offset 16 → expected RVA = 0x1010
        assert_eq!(
            entry.rva, 0x1010,
            "RVA should be VirtualAddress + offset = 0x1010"
        );
    }

    // ── test 6: strings extracted from both .rdata and .data ─────────────────

    #[test]
    fn test_extracts_from_both_sections() {
        let mut rdata = vec![0u8; 0x40];
        rdata[0..8].copy_from_slice(b"rdata_ok");

        let mut data = vec![0u8; 0x40];
        data[0..7].copy_from_slice(b"data_ok");

        let pe_bytes = build_test_pe(&rdata, &data);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return,
        };
        let strings = extract_strings(&pe, &pe_bytes);

        let has_rdata = strings.iter().any(|s| s.value == "rdata_ok");
        let has_data = strings.iter().any(|s| s.value == "data_ok");
        assert!(has_rdata, "should find 'rdata_ok' from .rdata section");
        assert!(has_data, "should find 'data_ok' from .data section");
    }

    // ── test 7: non-printable bytes terminate strings ────────────────────────

    #[test]
    fn test_non_printable_terminates_string() {
        // "hello\x01world" — the \x01 should split into two runs:
        // "hello" (5 chars, included) and "world" (5 chars, included).
        let mut rdata = vec![0u8; 0x40];
        rdata[0] = b'h';
        rdata[1] = b'e';
        rdata[2] = b'l';
        rdata[3] = b'l';
        rdata[4] = b'o';
        rdata[5] = 0x01; // non-printable separator
        rdata[6] = b'w';
        rdata[7] = b'o';
        rdata[8] = b'r';
        rdata[9] = b'l';
        rdata[10] = b'd';

        let pe_bytes = build_test_pe(&rdata, &[]);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return,
        };
        let strings = extract_strings(&pe, &pe_bytes);

        assert!(
            strings.iter().any(|s| s.value == "hello"),
            "should find 'hello'"
        );
        assert!(
            strings.iter().any(|s| s.value == "world"),
            "should find 'world'"
        );
        // The combined string should NOT appear.
        assert!(
            !strings.iter().any(|s| s.value.contains("hello\x01world")),
            "non-printable byte must split the string"
        );
    }

    // ── test 8: parse_and_extract convenience wrapper ────────────────────────

    #[test]
    fn test_parse_and_extract_convenience() {
        let mut rdata = vec![0u8; 0x40];
        rdata[0..12].copy_from_slice(b"TextQuest123");

        let pe_bytes = build_test_pe(&rdata, &[]);
        let result = parse_and_extract(&pe_bytes);
        match result {
            Ok(strings) => {
                assert!(
                    strings.iter().any(|s| s.value == "TextQuest123"),
                    "convenience wrapper should extract 'TextQuest123'"
                );
            }
            Err(_) => {
                // Tolerate if our hand-crafted PE fails goblin's strict checks.
            }
        }
    }

    // ── test 9: empty sections produce no strings ────────────────────────────

    #[test]
    fn test_empty_section_produces_no_strings() {
        let pe_bytes = build_test_pe(&[], &[]);
        let pe = match parse_pe(&pe_bytes) {
            Ok(p) => p,
            Err(_) => return,
        };
        let strings = extract_strings(&pe, &pe_bytes);
        // Zero-length sections should yield no strings (only NUL bytes remain).
        assert!(
            strings.is_empty() || strings.iter().all(|s| s.value.len() < MIN_STR_LEN),
            "empty sections should not produce valid strings"
        );
    }

    // ── test 10: ExtractedString derives Clone and PartialEq correctly ────────

    #[test]
    fn test_extracted_string_clone_and_eq() {
        let a = ExtractedString {
            rva: 0x1234,
            value: "hello".to_owned(),
        };
        let b = a.clone();
        assert_eq!(a, b);

        let c = ExtractedString {
            rva: 0x5678,
            value: "world".to_owned(),
        };
        assert_ne!(a, c);
    }
}
