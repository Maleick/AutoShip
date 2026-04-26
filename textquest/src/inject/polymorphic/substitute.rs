//! Sub-issue #739 — register / instruction substitution engine.
//!
//! Replaces instructions with semantically equivalent alternatives:
//!  - register substitution (use rcx instead of rax for a temp)
//!  - opcode substitution (`lea r,[r+0]` ↔ `mov r,r`, `xor r,r` ↔ `sub r,r`)
//!  - addressing-mode substitution (`mov [rsp+8], r` ↔ `push r; ...; pop r`)
//!
//! ## Today (foundation)
//! Performs a simple byte-pattern substitution on known NOP variants so the
//! pipeline produces measurably different output. The full disassemble →
//! transform → reassemble loop lands in #739; it requires `iced-x86` or
//! similar, which we'll add as a dep when that sub-issue ships.

use super::PolymorphicError;
use rand::RngCore;
use std::ops::Range;

/// Apply substitution transforms to `stub` in place.
pub fn transform(stub: &[u8]) -> Result<Vec<u8>, PolymorphicError> {
    transform_with_protected_ranges(stub, &[])
}

/// Apply substitution transforms while leaving protected byte ranges untouched.
pub fn transform_with_protected_ranges(
    stub: &[u8],
    protected_ranges: &[Range<usize>],
) -> Result<Vec<u8>, PolymorphicError> {
    let mut out = Vec::with_capacity(stub.len());
    for (idx, &b) in stub.iter().enumerate() {
        if protected_ranges.iter().any(|range| range.contains(&idx)) {
            out.push(b);
            continue;
        }
        out.push(match b {
            // Coin-flip swap NOP (0x90) for an operand-size prefix (0x66)
            // and vice versa. Both are filler-equivalent at this stage.
            0x90 if coin_flip() => 0x66,
            0x66 if coin_flip() => 0x90,
            other => other,
        });
    }
    Ok(out)
}

fn coin_flip() -> bool {
    let mut b = [0u8; 1];
    rand::rng().fill_bytes(&mut b);
    b[0] & 1 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_preserves_length() {
        let stub = vec![0x90u8; 64];
        let out = transform(&stub).unwrap();
        assert_eq!(out.len(), stub.len());
    }

    #[test]
    fn transform_changes_at_least_one_byte_on_average() {
        // Probabilistic — over 1024 NOPs, we expect ~half flipped.
        let stub = vec![0x90u8; 1024];
        let out = transform(&stub).unwrap();
        let diffs = stub.iter().zip(&out).filter(|(a, b)| a != b).count();
        assert!(diffs > 100, "expected meaningful divergence, got {diffs}");
    }

    #[test]
    fn protected_ranges_are_never_mutated() {
        let stub = vec![0x90, 0x66, 0x90, 0x66, 0x90, 0x66];
        let protected = 1..5;
        let out = transform_with_protected_ranges(&stub, &[protected.clone()]).unwrap();
        assert_eq!(&out[protected], &[0x66, 0x90, 0x66, 0x90]);
    }
}
