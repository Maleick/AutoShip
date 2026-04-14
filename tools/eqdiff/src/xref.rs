//! Cross-reference index for PE binaries.
//!
//! Maps extracted strings and imported symbols to the code locations that
//! reference them.

use crate::ExtractedString;
use anyhow::Result;
use goblin::pe::PE;
use iced_x86::{Decoder, DecoderOptions, Instruction, OpKind, Register};
use std::collections::HashMap;

/// A pair of an extracted string and the list of code RVAs that reference it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringRefMatch {
    /// The string being referenced.
    pub string: ExtractedString,
    /// List of RVAs in the code (usually `.text`) that reference the string's
    /// RVA.
    pub referencing_rvas: Vec<u32>,
}

/// An imported symbol with its RVA in the Import Address Table (IAT).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSymbol {
    /// The Relative Virtual Address of this symbol's entry in the IAT.
    pub rva: u32,
    /// The name of the symbol (e.g., "GetProcAddress").
    pub name: String,
    /// The name of the DLL it is imported from (e.g., "KERNEL32.dll").
    pub dll: String,
}

/// Extract imported symbols and their IAT RVAs from a PE image.
pub fn extract_imports(pe: &PE) -> Result<Vec<ImportedSymbol>> {
    let mut results = Vec::new();
    for import in &pe.imports {
        results.push(ImportedSymbol {
            rva: import.rva as u32,
            name: import.name.to_string(),
            dll: import.dll.to_string(),
        });
    }
    Ok(results)
}

/// Build an index mapping string RVAs to the RVAs of instructions that
/// reference them.
///
/// This walks all executable sections (e.g., `.text`) and uses a disassembler
/// to find instructions that reference addresses in the provided `strings`
/// list.
pub fn build_string_xref_index(
    pe: &PE,
    bytes: &[u8],
    strings: &[ExtractedString],
) -> HashMap<u32, Vec<u32>> {
    let mut index: HashMap<u32, Vec<u32>> = HashMap::new();

    // Create a set of string RVAs for fast lookup.
    let string_rvas: HashMap<u32, ()> = strings.iter().map(|s| (s.rva, ())).collect();

    let image_base = pe.image_base as u64;

    for section in &pe.sections {
        // Only process executable sections (IMAGE_SCN_MEM_EXECUTE = 0x20000000).
        if section.characteristics & 0x20000000 == 0 {
            continue;
        }

        let file_offset = section.pointer_to_raw_data as usize;
        let raw_size = section.size_of_raw_data as usize;
        let virtual_address = section.virtual_address;

        let section_end = file_offset.saturating_add(raw_size);
        if file_offset >= bytes.len() {
            continue;
        }
        let section_bytes = &bytes[file_offset..section_end.min(bytes.len())];

        let bitness = if pe.is_64 { 64 } else { 32 };

        // We set the IP to the VirtualAddress (RVA) so that RIP-relative addressing
        // automatically yields RVAs.
        let mut decoder = Decoder::with_ip(
            bitness,
            section_bytes,
            virtual_address as u64,
            DecoderOptions::NONE,
        );
        let mut instruction = Instruction::default();

        while decoder.can_decode() {
            decoder.decode_out(&mut instruction);
            let instr_rva = instruction.ip() as u32;

            // Check each operand for memory references or immediate values.
            for op_idx in 0..instruction.op_count() {
                let op_kind = instruction.op_kind(op_idx);

                match op_kind {
                    OpKind::Memory => {
                        let addr = if instruction.is_ip_rel_memory_operand() {
                            Some(instruction.ip_rel_memory_address())
                        } else if instruction.memory_base() == Register::None
                            && instruction.memory_index() == Register::None
                        {
                            Some(instruction.memory_displacement64())
                        } else {
                            None
                        };

                        if let Some(addr) = addr {
                            // 1. Check if it's already an RVA (e.g. RIP-relative in x64)
                            if string_rvas.contains_key(&(addr as u32)) {
                                index.entry(addr as u32).or_default().push(instr_rva);
                            }
                            // 2. Check if it's an absolute address (e.g. in x86)
                            else if addr >= image_base {
                                let rva = (addr - image_base) as u32;
                                if string_rvas.contains_key(&rva) {
                                    index.entry(rva).or_default().push(instr_rva);
                                }
                            }
                        }
                    }
                    OpKind::Immediate32 => {
                        let imm = instruction.immediate32();
                        // Immediate could be an RVA or an absolute address.
                        if string_rvas.contains_key(&imm) {
                            index.entry(imm).or_default().push(instr_rva);
                        } else if imm as u64 >= image_base {
                            let rva = (imm as u64 - image_base) as u32;
                            if string_rvas.contains_key(&rva) {
                                index.entry(rva).or_default().push(instr_rva);
                            }
                        }
                    }
                    OpKind::Immediate64 => {
                        let imm = instruction.immediate64();
                        if string_rvas.contains_key(&(imm as u32)) {
                            index.entry(imm as u32).or_default().push(instr_rva);
                        } else if imm >= image_base {
                            let rva = (imm - image_base) as u32;
                            if string_rvas.contains_key(&rva) {
                                index.entry(rva).or_default().push(instr_rva);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    index
}

/// Convenience function to match extracted strings with their code references.
pub fn match_string_references(
    pe: &PE,
    bytes: &[u8],
    strings: Vec<ExtractedString>,
) -> Vec<StringRefMatch> {
    let index = build_string_xref_index(pe, bytes, &strings);
    strings
        .into_iter()
        .map(|s| {
            let rva = s.rva;
            let referencing_rvas = index.get(&rva).cloned().unwrap_or_default();
            StringRefMatch {
                string: s,
                referencing_rvas,
            }
        })
        .collect()
}

// ────────────────────────────────────────────────────────────────────────────
// Unit tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract_strings;

    /// Build a minimal PE32+ (64-bit) image with .text and .rdata sections.
    fn build_test_pe_x64(text_content: &[u8], rdata_content: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 0x800];

        // DOS header
        buf[0] = b'M';
        buf[1] = b'Z';
        buf[0x3c] = 0x80; // e_lfanew

        // PE signature at 0x80
        buf[0x80] = b'P';
        buf[0x81] = b'E';

        // COFF header at 0x84
        buf[0x84] = 0x64;
        buf[0x85] = 0x86; // AMD64
        buf[0x86] = 2; // 2 sections
        buf[0x94] = 0xf0; // SizeOfOptionalHeader
        buf[0x96] = 0x22; // Characteristics

        // Optional header (PE32+) at 0x98
        buf[0x98] = 0x0b;
        buf[0x99] = 0x02; // Magic
        buf[0x98 + 24] = 0x00;
        buf[0x98 + 25] = 0x00;
        buf[0x98 + 26] = 0x40; // ImageBase = 0x400000
        buf[0x98 + 56] = 0x00;
        buf[0x98 + 57] = 0x20; // SizeOfImage = 0x2000
        buf[0x98 + 60] = 0x00;
        buf[0x98 + 61] = 0x02; // SizeOfHeaders = 0x200
        buf[0x98 + 108] = 16; // NumberOfRvaAndSizes

        let sec_table = 0x80 + 4 + 20 + 0xf0; // 0x188

        // .text section
        write_section_entry(
            &mut buf,
            sec_table,
            b".text\0\0\0",
            text_content.len() as u32,
            0x1000,
            0x200,
            0x200,
            0x60000020,
        );
        // .rdata section
        write_section_entry(
            &mut buf,
            sec_table + 40,
            b".rdata\0\0",
            rdata_content.len() as u32,
            0x2000,
            0x200,
            0x400,
            0x40000040,
        );

        buf[0x200..0x200 + text_content.len()].copy_from_slice(text_content);
        buf[0x400..0x400 + rdata_content.len()].copy_from_slice(rdata_content);

        buf
    }

    #[allow(clippy::too_many_arguments)]
    fn write_section_entry(
        buf: &mut [u8],
        offset: usize,
        name: &[u8; 8],
        v_size: u32,
        v_addr: u32,
        r_size: u32,
        r_ptr: u32,
        char: u32,
    ) {
        buf[offset..offset + 8].copy_from_slice(name);
        buf[offset + 8..offset + 12].copy_from_slice(&v_size.to_le_bytes());
        buf[offset + 12..offset + 16].copy_from_slice(&v_addr.to_le_bytes());
        buf[offset + 16..offset + 20].copy_from_slice(&r_size.to_le_bytes());
        buf[offset + 20..offset + 24].copy_from_slice(&r_ptr.to_le_bytes());
        buf[offset + 36..offset + 40].copy_from_slice(&char.to_le_bytes());
    }

    #[test]
    fn test_xref_64bit_rip_relative() {
        // String "Hello" at RVA 0x2000 (.rdata)
        let rdata = b"Hello\0";

        // LEA RAX, [RIP + 0xff9]
        // Instruction at RVA 0x1000. Length is 7 bytes.
        // Next IP is 0x1007. 0x1007 + 0xff9 = 0x2000.
        // Opcode: 48 8d 05 f9 0f 00 00
        let mut text = vec![0u8; 0x100];
        text[0..7].copy_from_slice(&[0x48, 0x8d, 0x05, 0xf9, 0x0f, 0x00, 0x00]);

        let pe_bytes = build_test_pe_x64(&text, rdata);
        let pe = goblin::pe::PE::parse(&pe_bytes).unwrap();
        let strings = extract_strings(&pe, &pe_bytes);

        let target_string = strings
            .iter()
            .find(|s| s.value == "Hello")
            .expect("found Hello");
        assert_eq!(target_string.rva, 0x2000);

        let xrefs = build_string_xref_index(&pe, &pe_bytes, &strings);
        let refs = xrefs.get(&0x2000).expect("should have ref for Hello");
        assert!(refs.contains(&0x1000));
    }

    #[test]
    fn test_xref_immediate_rva() {
        // String "World" at RVA 0x2000
        let rdata = b"World\0";

        // MOV EAX, 0x2000 (Immediate 32-bit RVA)
        // Opcode: b8 00 20 00 00
        let mut text = vec![0u8; 0x100];
        text[0..5].copy_from_slice(&[0xb8, 0x00, 0x20, 0x00, 0x00]);

        let pe_bytes = build_test_pe_x64(&text, rdata);
        let pe = goblin::pe::PE::parse(&pe_bytes).unwrap();
        let strings = extract_strings(&pe, &pe_bytes);

        let xrefs = build_string_xref_index(&pe, &pe_bytes, &strings);
        let refs = xrefs.get(&0x2000).expect("should have ref for World");
        assert!(refs.contains(&0x1000));
    }

    #[test]
    fn test_extract_imports_empty() {
        let pe_bytes = build_test_pe_x64(&[], &[]);
        let pe = goblin::pe::PE::parse(&pe_bytes).unwrap();
        let imports = extract_imports(&pe).unwrap();
        assert!(imports.is_empty());
    }
}
