//! Byte-level function similarity scoring for binary diff candidate ranking.

use iced_x86::{Decoder, DecoderOptions, Instruction};
use std::cmp::Ordering;

/// Raw function bytes anchored by the function RVA they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBytes {
    /// Function start RVA.
    pub rva: u32,
    /// Raw bytes beginning at the function prologue.
    pub bytes: Vec<u8>,
}

impl FunctionBytes {
    /// Construct function bytes for similarity scoring.
    pub fn new(rva: u32, bytes: Vec<u8>) -> Self {
        Self { rva, bytes }
    }
}

/// Candidate score produced by byte-similarity ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct ByteSimilarityMatch {
    /// Candidate function RVA.
    pub function_rva: u32,
    /// Similarity in the inclusive range `0.0..=1.0`.
    pub score: f64,
}

/// Compare function prologue bytes while wildcarding displacement/immediate bytes.
///
/// The score is the percentage of matching bytes among positions that are not
/// wildcarded in either function. Only the first `prologue_len` bytes are used.
pub fn byte_similarity(old: &FunctionBytes, new: &FunctionBytes, prologue_len: usize) -> f64 {
    let old_len = old.bytes.len().min(prologue_len);
    let new_len = new.bytes.len().min(prologue_len);
    let compare_len = old_len.min(new_len);
    if compare_len == 0 {
        return 0.0;
    }

    let old_mask = wildcard_mask(&old.bytes[..old_len]);
    let new_mask = wildcard_mask(&new.bytes[..new_len]);

    let mut comparable = 0usize;
    let mut matching = 0usize;

    for i in 0..compare_len {
        if old_mask[i] || new_mask[i] {
            continue;
        }
        comparable += 1;
        if old.bytes[i] == new.bytes[i] {
            matching += 1;
        }
    }

    if comparable == 0 {
        0.0
    } else {
        matching as f64 / comparable as f64
    }
}

/// Rank candidates by byte similarity, descending, for use as a tiebreaker.
pub fn rank_candidates_by_byte_similarity(
    old: &FunctionBytes,
    candidates: &[FunctionBytes],
    prologue_len: usize,
) -> Vec<ByteSimilarityMatch> {
    let mut ranked: Vec<_> = candidates
        .iter()
        .map(|candidate| ByteSimilarityMatch {
            function_rva: candidate.rva,
            score: byte_similarity(old, candidate, prologue_len),
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.function_rva.cmp(&b.function_rva))
    });
    ranked
}

fn wildcard_mask(bytes: &[u8]) -> Vec<bool> {
    let mut mask = vec![false; bytes.len()];
    let mut decoder = Decoder::with_ip(64, bytes, 0, DecoderOptions::NONE);
    let mut instruction = Instruction::default();

    while decoder.can_decode() {
        decoder.decode_out(&mut instruction);
        let offsets = decoder.get_constant_offsets(&instruction);
        let instruction_start = instruction.ip() as usize;
        mark_range(
            &mut mask,
            instruction_start + offsets.displacement_offset(),
            offsets.displacement_size(),
        );
        mark_range(
            &mut mask,
            instruction_start + offsets.immediate_offset(),
            offsets.immediate_size(),
        );
        mark_range(
            &mut mask,
            instruction_start + offsets.immediate_offset2(),
            offsets.immediate_size2(),
        );
    }

    mask
}

fn mark_range(mask: &mut [bool], offset: usize, size: usize) {
    let start = offset;
    let end = start.saturating_add(size).min(mask.len());
    for slot in &mut mask[start..end] {
        *slot = true;
    }
}
