//! Call graph construction and match propagation for binary diffing.

use goblin::pe::PE;
use std::collections::{BTreeSet, HashMap, VecDeque};

/// A discovered function body in a PE image, expressed as RVA + byte length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BinaryFunction {
    /// Function start RVA.
    pub rva: u32,
    /// Function body size in bytes.
    pub size: u32,
}

/// A call instruction found inside a known function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    /// Caller function start RVA.
    pub caller_rva: u32,
    /// Call instruction RVA.
    pub call_site_rva: u32,
    /// Offset of the call instruction from the caller start.
    pub call_site_offset: u32,
    /// Direct target function RVA, if the call resolves to a known function.
    pub target_rva: Option<u32>,
    /// True for memory-indirect calls such as `FF 15 rel32`.
    pub is_indirect: bool,
}

/// Adjacency lists for function call relationships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraph {
    /// Known functions indexed by start RVA.
    pub functions: Vec<BinaryFunction>,
    /// Outgoing calls for each caller function RVA.
    pub outgoing: HashMap<u32, Vec<CallEdge>>,
}

/// Confidence assigned to a propagated function match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchConfidence {
    /// Confidence score in the range `0.0..=1.0`.
    pub score: f32,
    /// Number of propagation hops from the original seed.
    pub hop: u32,
}

/// Matched function pair between two binaries.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionMatch {
    /// Function RVA in the old/source binary.
    pub old_rva: u32,
    /// Function RVA in the new/target binary.
    pub new_rva: u32,
    /// Match confidence.
    pub confidence: MatchConfidence,
}

/// Function match report, including unmatched functions in both binaries.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchReport {
    /// Matched function pairs and confidence scores.
    pub matches: Vec<FunctionMatch>,
    /// Old/source functions not matched to any new function.
    pub unmatched_old: Vec<u32>,
    /// New/target functions not matched from any old function.
    pub unmatched_new: Vec<u32>,
}

/// Build a call graph by scanning known function bodies for `CALL` opcodes.
pub fn build_call_graph(pe: &PE, bytes: &[u8], functions: &[BinaryFunction]) -> CallGraph {
    let function_by_rva: BTreeSet<u32> = functions.iter().map(|function| function.rva).collect();
    let mut outgoing: HashMap<u32, Vec<CallEdge>> = HashMap::new();

    for function in functions {
        let Some(function_bytes) = function_body_bytes(pe, bytes, *function) else {
            continue;
        };

        let mut offset = 0usize;
        while offset < function_bytes.len() {
            let opcode = function_bytes[offset];
            if opcode == 0xe8 && offset + 5 <= function_bytes.len() {
                let rel =
                    i32::from_le_bytes(function_bytes[offset + 1..offset + 5].try_into().unwrap());
                let call_site_rva = function.rva.wrapping_add(offset as u32);
                let next_rva = call_site_rva.wrapping_add(5);
                let target_rva = next_rva.wrapping_add_signed(rel);
                outgoing.entry(function.rva).or_default().push(CallEdge {
                    caller_rva: function.rva,
                    call_site_rva,
                    call_site_offset: offset as u32,
                    target_rva: function_by_rva.contains(&target_rva).then_some(target_rva),
                    is_indirect: false,
                });
                offset += 5;
            } else if opcode == 0xff
                && offset + 6 <= function_bytes.len()
                && function_bytes[offset + 1] == 0x15
            {
                let rel =
                    i32::from_le_bytes(function_bytes[offset + 2..offset + 6].try_into().unwrap());
                let call_site_rva = function.rva.wrapping_add(offset as u32);
                let next_rva = call_site_rva.wrapping_add(6);
                let memory_target_rva = next_rva.wrapping_add_signed(rel);
                outgoing.entry(function.rva).or_default().push(CallEdge {
                    caller_rva: function.rva,
                    call_site_rva,
                    call_site_offset: offset as u32,
                    target_rva: function_by_rva
                        .contains(&memory_target_rva)
                        .then_some(memory_target_rva),
                    is_indirect: true,
                });
                offset += 6;
            } else {
                offset += 1;
            }
        }
    }

    CallGraph {
        functions: functions.to_vec(),
        outgoing,
    }
}

/// Propagate seeded function matches through matching caller/callee call sites.
pub fn propagate_function_matches(
    old_graph: &CallGraph,
    new_graph: &CallGraph,
    seed_matches: &[(u32, u32)],
) -> MatchReport {
    let old_functions: BTreeSet<u32> = old_graph
        .functions
        .iter()
        .map(|function| function.rva)
        .collect();
    let new_functions: BTreeSet<u32> = new_graph
        .functions
        .iter()
        .map(|function| function.rva)
        .collect();
    let mut matched_old_to_new = HashMap::new();
    let mut matched_new = BTreeSet::new();
    let mut confidences = HashMap::new();
    let mut queue = VecDeque::new();

    for &(old_rva, new_rva) in seed_matches {
        if old_functions.contains(&old_rva) && new_functions.contains(&new_rva) {
            matched_old_to_new.insert(old_rva, new_rva);
            matched_new.insert(new_rva);
            confidences.insert(old_rva, MatchConfidence { score: 1.0, hop: 0 });
            queue.push_back((old_rva, new_rva, 0));
        }
    }

    while let Some((old_rva, new_rva, hop)) = queue.pop_front() {
        let Some(old_edges) = old_graph.outgoing.get(&old_rva) else {
            continue;
        };
        let Some(new_edges) = new_graph.outgoing.get(&new_rva) else {
            continue;
        };

        for old_edge in old_edges.iter().filter(|edge| edge.target_rva.is_some()) {
            let matching_new_edges: Vec<&CallEdge> = new_edges
                .iter()
                .filter(|edge| {
                    edge.call_site_offset == old_edge.call_site_offset && edge.target_rva.is_some()
                })
                .collect();
            if matching_new_edges.len() != 1 {
                continue;
            }

            let old_target = old_edge.target_rva.unwrap();
            let new_target = matching_new_edges[0].target_rva.unwrap();
            if matched_old_to_new.contains_key(&old_target) || matched_new.contains(&new_target) {
                continue;
            }

            let next_hop = hop + 1;
            matched_old_to_new.insert(old_target, new_target);
            matched_new.insert(new_target);
            confidences.insert(
                old_target,
                MatchConfidence {
                    score: propagation_confidence(next_hop),
                    hop: next_hop,
                },
            );
            queue.push_back((old_target, new_target, next_hop));
        }
    }

    let mut matches: Vec<FunctionMatch> = matched_old_to_new
        .into_iter()
        .map(|(old_rva, new_rva)| FunctionMatch {
            old_rva,
            new_rva,
            confidence: confidences[&old_rva],
        })
        .collect();
    matches.sort_by_key(|match_entry| match_entry.old_rva);

    let matched_old: BTreeSet<u32> = matches
        .iter()
        .map(|match_entry| match_entry.old_rva)
        .collect();
    let matched_new: BTreeSet<u32> = matches
        .iter()
        .map(|match_entry| match_entry.new_rva)
        .collect();

    MatchReport {
        matches,
        unmatched_old: old_functions.difference(&matched_old).copied().collect(),
        unmatched_new: new_functions.difference(&matched_new).copied().collect(),
    }
}

fn propagation_confidence(hop: u32) -> f32 {
    if hop == 1 {
        0.9
    } else {
        0.9 * 0.85_f32.powi((hop - 1) as i32)
    }
}

fn function_body_bytes<'a>(pe: &PE, bytes: &'a [u8], function: BinaryFunction) -> Option<&'a [u8]> {
    for section in &pe.sections {
        let section_start = section.virtual_address;
        let section_end = section_start.checked_add(section.size_of_raw_data)?;
        let function_end = function.rva.checked_add(function.size)?;
        if function.rva < section_start || function_end > section_end {
            continue;
        }

        let section_file_offset = section.pointer_to_raw_data as usize;
        let function_offset = function.rva.checked_sub(section_start)? as usize;
        let file_start = section_file_offset.checked_add(function_offset)?;
        let file_end = file_start.checked_add(function.size as usize)?;
        return bytes.get(file_start..file_end);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_pe_x64(text_content: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 0x600];

        buf[0] = b'M';
        buf[1] = b'Z';
        buf[0x3c] = 0x80;
        buf[0x80] = b'P';
        buf[0x81] = b'E';
        buf[0x84] = 0x64;
        buf[0x85] = 0x86;
        buf[0x86] = 1;
        buf[0x94] = 0xf0;
        buf[0x96] = 0x22;
        buf[0x98] = 0x0b;
        buf[0x99] = 0x02;
        buf[0x98 + 24] = 0x00;
        buf[0x98 + 25] = 0x00;
        buf[0x98 + 26] = 0x40;
        buf[0x98 + 56] = 0x00;
        buf[0x98 + 57] = 0x20;
        buf[0x98 + 60] = 0x00;
        buf[0x98 + 61] = 0x02;
        buf[0x98 + 108] = 16;

        let sec_table = 0x80 + 4 + 20 + 0xf0;
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
        buf[0x200..0x200 + text_content.len()].copy_from_slice(text_content);
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
    fn call_graph_records_direct_relative_calls_to_known_functions() {
        let mut text = vec![0x90; 0x80];
        text[0..5].copy_from_slice(&[0xe8, 0x1b, 0x00, 0x00, 0x00]);
        let pe_bytes = build_test_pe_x64(&text);
        let pe = PE::parse(&pe_bytes).unwrap();
        let functions = [
            BinaryFunction {
                rva: 0x1000,
                size: 0x20,
            },
            BinaryFunction {
                rva: 0x1020,
                size: 0x20,
            },
        ];

        let graph = build_call_graph(&pe, &pe_bytes, &functions);

        let edges = graph.outgoing.get(&0x1000).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].call_site_offset, 0);
        assert_eq!(edges[0].target_rva, Some(0x1020));
        assert!(!edges[0].is_indirect);
    }

    #[test]
    fn call_graph_records_ff15_indirect_calls() {
        let mut text = vec![0x90; 0x80];
        text[4..10].copy_from_slice(&[0xff, 0x15, 0x16, 0x00, 0x00, 0x00]);
        let pe_bytes = build_test_pe_x64(&text);
        let pe = PE::parse(&pe_bytes).unwrap();
        let functions = [
            BinaryFunction {
                rva: 0x1000,
                size: 0x20,
            },
            BinaryFunction {
                rva: 0x1020,
                size: 0x20,
            },
        ];

        let graph = build_call_graph(&pe, &pe_bytes, &functions);

        let edge = &graph.outgoing.get(&0x1000).unwrap()[0];
        assert_eq!(edge.call_site_rva, 0x1004);
        assert_eq!(edge.call_site_offset, 4);
        assert_eq!(edge.target_rva, Some(0x1020));
        assert!(edge.is_indirect);
    }

    #[test]
    fn propagation_matches_unmatched_callees_at_same_call_site() {
        let old_graph = graph_with_edges(&[(0x1000, 0, 0x1100), (0x1100, 8, 0x1200)]);
        let new_graph = graph_with_edges(&[(0x2000, 0, 0x2100), (0x2100, 8, 0x2200)]);

        let report = propagate_function_matches(&old_graph, &new_graph, &[(0x1000, 0x2000)]);

        assert_eq!(
            report.matches,
            vec![
                FunctionMatch {
                    old_rva: 0x1000,
                    new_rva: 0x2000,
                    confidence: MatchConfidence { score: 1.0, hop: 0 },
                },
                FunctionMatch {
                    old_rva: 0x1100,
                    new_rva: 0x2100,
                    confidence: MatchConfidence { score: 0.9, hop: 1 },
                },
                FunctionMatch {
                    old_rva: 0x1200,
                    new_rva: 0x2200,
                    confidence: MatchConfidence {
                        score: 0.765,
                        hop: 2
                    },
                },
            ]
        );
        assert!(report.unmatched_old.is_empty());
        assert!(report.unmatched_new.is_empty());
    }

    #[test]
    fn propagation_reports_unmatched_functions() {
        let old_graph = graph_with_edges(&[(0x1000, 0, 0x1100)]);
        let mut new_graph = graph_with_edges(&[(0x2000, 4, 0x2100)]);
        new_graph.functions.push(BinaryFunction {
            rva: 0x2200,
            size: 0x10,
        });

        let report = propagate_function_matches(&old_graph, &new_graph, &[(0x1000, 0x2000)]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.unmatched_old, vec![0x1100]);
        assert_eq!(report.unmatched_new, vec![0x2100, 0x2200]);
    }

    fn graph_with_edges(edges: &[(u32, u32, u32)]) -> CallGraph {
        let mut function_rvas = BTreeSet::new();
        let mut outgoing: HashMap<u32, Vec<CallEdge>> = HashMap::new();
        for &(caller_rva, offset, target_rva) in edges {
            function_rvas.insert(caller_rva);
            function_rvas.insert(target_rva);
            outgoing.entry(caller_rva).or_default().push(CallEdge {
                caller_rva,
                call_site_rva: caller_rva + offset,
                call_site_offset: offset,
                target_rva: Some(target_rva),
                is_indirect: false,
            });
        }
        CallGraph {
            functions: function_rvas
                .into_iter()
                .map(|rva| BinaryFunction { rva, size: 0x10 })
                .collect(),
            outgoing,
        }
    }
}
