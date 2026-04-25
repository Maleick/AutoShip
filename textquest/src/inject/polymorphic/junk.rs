//! Sub-issue #738 — junk code insertion engine.
//!
//! Interleaves dead-code instructions between real ones to break signature
//! matching while preserving program semantics. Junk must be:
//!  - flag-neutral (or used in a context where flags are dead),
//!  - register-neutral (clobbers only scratch regs we reserve),
//!  - control-flow-neutral (no jumps, no calls).
//!
//! ## Today (foundation)
//! Inserts random NOP-equivalent fillers at a configurable density. The
//! taxonomy of higher-quality junk (xchg eax,eax; lea r,[r]; push/pop;
//! mov r,r; test r,r) lands in #738.

use super::PolymorphicError;
use rand::RngCore;

/// Insert junk every `density` real bytes. `density` of 0 is a passthrough.
pub fn interleave(stub: &[u8], density: u8) -> Result<Vec<u8>, PolymorphicError> {
    if density == 0 {
        return Ok(stub.to_vec());
    }
    let mut out = Vec::with_capacity(stub.len() + stub.len() / density as usize);
    for (i, b) in stub.iter().enumerate() {
        out.push(*b);
        if (i + 1) % density as usize == 0 {
            for _ in 0..pick_junk_run_len() {
                out.push(pick_junk_byte());
            }
        }
    }
    Ok(out)
}

fn pick_junk_run_len() -> u8 {
    let mut b = [0u8; 1];
    rand::rng().fill_bytes(&mut b);
    1 + (b[0] % 3)
}

fn pick_junk_byte() -> u8 {
    // Single-byte filler pool — single NOP and operand-size prefix.
    // #738 will swap this for full multi-byte junk instructions.
    const POOL: &[u8] = &[0x90, 0x66, 0x90];
    let mut b = [0u8; 1];
    rand::rng().fill_bytes(&mut b);
    POOL[(b[0] as usize) % POOL.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_zero_is_passthrough() {
        let stub = vec![1, 2, 3, 4, 5];
        assert_eq!(interleave(&stub, 0).unwrap(), stub);
    }

    #[test]
    fn interleave_grows_stub() {
        let stub = vec![0xAAu8; 64];
        let out = interleave(&stub, 4).unwrap();
        assert!(out.len() > stub.len());
        // Original bytes still appear in order.
        let mut idx = 0;
        for &b in &out {
            if idx < stub.len() && b == stub[idx] {
                idx += 1;
            }
        }
        assert_eq!(idx, stub.len(), "all original bytes must survive in order");
    }
}
