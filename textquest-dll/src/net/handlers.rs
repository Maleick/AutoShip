//! Inbound opcode handlers — server-initiated memcheck and integrity requests.
//!
//! # Opcode `0x4f27` — Server memcheck
//!
//! The EQ server periodically sends opcode `0x4f27`, which triggers
//! `FUN_1400B5720` (`SERVER_MEMCHECK_HANDLER`). The native handler hashes
//! requested memory regions and returns the results. Any byte-patch hook in
//! `.text` is directly visible to this path.
//!
//! This handler uses the **PRNG-aware clean-hash cache** maintained by
//! [`crate::hooks::memcheck`]:
//!
//! 1. Parse `(address, length)` region specs from the inbound payload.
//! 2. For each block that intersects a cached (pre-modification) entry, return
//!    the pre-modification `BlockHash` instead of the live bytes.
//! 3. For unmodified blocks return `None` — the caller can fall through to the
//!    original handler or hash them directly.
//!
//! No unsafe memory reads are performed outside the `hooks::memcheck` init
//! path (which runs before any patch hooks are installed).
//!
//! # Payload format
//!
//! Provisional layout (Ghidra `FUN_1400B5720`, confirmed struct pending #2175):
//!
//! ```text
//! bytes [0..2]    — opcode (u16 LE) = 0x4f27
//! bytes [2..4]    — region_count (u16 LE)
//! bytes [4..N]    — region_count × { addr: u64 LE, len: u32 LE } = 12 bytes each
//! ```
//!
//! # PRNG-aware cache note
//!
//! The Lagged Fibonacci PRNG (`FUN_14025ABD0`) seeds the file integrity checks
//! (`FILE_INTEGRITY_DISPATCHER`, opcodes `0x8bdc`/`0xe91d`/`0x9562`), not
//! the memcheck opcode directly. The "PRNG-aware" label in this issue refers
//! to the fact that the clean-hash cache stores pre-modification bytes **before**
//! any hook alters the `.text` section, which is exactly what the PRNG-sampled
//! integrity checks and the memcheck hashes both need.
//!
//! Evidence: `docs/wiki/Research-Anti-Detection.md` §Server-initiated memcheck.
//! Parent issue #2173. Sub-issue #3397 (A1).

#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions
)]

use crate::hooks::memcheck::{self, BlockHash, MemRegionSpec};

// ─── Opcode constants ─────────────────────────────────────────────────────────

/// Server-initiated memcheck opcode.
///
/// Sent by the EQ login/world server to probe specific memory regions.
/// Corresponds to `FUN_1400B5720` (`SERVER_MEMCHECK_HANDLER`) in Ghidra.
pub const OPCODE_SERVER_MEMCHECK: u16 = 0x4f27;

// ─── Handler result ───────────────────────────────────────────────────────────

/// Result for a single `BLOCK_SIZE` block returned by the memcheck handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockResult {
    /// `BLOCK_SIZE`-aligned base address of the block.
    pub block_base: usize,
    /// Pre-modification hash from the clean-hash cache, or `None` if the block
    /// is unmodified (passthrough — hash it live).
    pub clean_hash: Option<BlockHash>,
}

/// Response from [`handle_server_memcheck`].
#[derive(Debug, Clone)]
pub struct MemcheckResponse {
    /// Per-block results, in region-scan order.
    pub blocks: Vec<BlockResult>,
    /// `true` if at least one block had a cached (spoofed) hash.
    pub any_spoofed: bool,
}

// ─── Dispatch ─────────────────────────────────────────────────────────────────

/// Dispatch an inbound EQ packet to the appropriate handler.
///
/// Returns `Some(response_bytes)` if the opcode is recognised and handled, or
/// `None` for pass-through (unknown opcodes).
///
/// The `payload` slice must start at byte 0 of the EQ packet (including the
/// 2-byte opcode field). Packets shorter than 2 bytes are silently ignored
/// (returns `None`).
pub fn dispatch(payload: &[u8]) -> Option<Vec<u8>> {
    if payload.len() < 2 {
        return None;
    }
    let opcode = u16::from_le_bytes([payload[0], payload[1]]);
    match opcode {
        OPCODE_SERVER_MEMCHECK => {
            let response = handle_server_memcheck(payload);
            Some(encode_memcheck_response(&response))
        }
        _ => None,
    }
}

// ─── 0x4f27 — server memcheck ─────────────────────────────────────────────────

/// Handle opcode `0x4f27` — server-initiated memcheck.
///
/// Parses region specs from `payload` and queries the PRNG-aware clean-hash
/// cache ([`crate::hooks::memcheck`]) for each covered block.  Returns a
/// [`MemcheckResponse`] containing per-block results.
///
/// No unsafe memory reads are performed here.  All byte access is through the
/// pre-populated cache, which was built before any hook patches were applied.
///
/// # Payload layout (provisional — Ghidra `FUN_1400B5720`, struct pending #2175)
///
/// ```text
/// [0..2]   opcode       u16 LE = 0x4f27
/// [2..4]   region_count u16 LE
/// [4..]    region_count × { addr: u64 LE, len: u32 LE }  (12 bytes each)
/// ```
pub fn handle_server_memcheck(payload: &[u8]) -> MemcheckResponse {
    let specs = parse_region_specs(payload);

    let raw = memcheck::query_regions_from_cache(&specs);

    let any_spoofed = raw.iter().any(|(_, h)| h.is_some());
    let blocks = raw
        .into_iter()
        .map(|(block_base, clean_hash)| BlockResult {
            block_base,
            clean_hash,
        })
        .collect();

    MemcheckResponse {
        blocks,
        any_spoofed,
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Parse `MemRegionSpec` list from a raw `0x4f27` payload.
///
/// Returns an empty `Vec` if the payload is too short or `region_count` is
/// zero. Unknown trailing bytes are silently ignored.
fn parse_region_specs(payload: &[u8]) -> Vec<MemRegionSpec> {
    // Minimum: 2-byte opcode + 2-byte count.
    if payload.len() < 4 {
        return Vec::new();
    }
    let region_count = u16::from_le_bytes([payload[2], payload[3]]) as usize;
    if region_count == 0 {
        return Vec::new();
    }

    // Each region spec: addr (8 bytes) + len (4 bytes) = 12 bytes.
    const SPEC_SIZE: usize = 12;
    let specs_payload = &payload[4..];
    let available = specs_payload.len() / SPEC_SIZE;
    let count = region_count.min(available);

    (0..count)
        .map(|i| {
            let base = i * SPEC_SIZE;
            let addr = u64::from_le_bytes(
                specs_payload[base..base + 8]
                    .try_into()
                    .unwrap_or([0u8; 8]),
            ) as usize;
            let len = u32::from_le_bytes(
                specs_payload[base + 8..base + 12]
                    .try_into()
                    .unwrap_or([0u8; 4]),
            ) as usize;
            MemRegionSpec {
                address: addr,
                length: len,
            }
        })
        .collect()
}

/// Encode a [`MemcheckResponse`] to bytes for forwarding.
///
/// Format: for each block — `block_base: u64 LE` + `hash_present: u8` +
/// `hash: u64 LE` (only written when `hash_present == 1`).
///
/// This is an internal TextQuest wire format, not the EQ server protocol.
fn encode_memcheck_response(response: &MemcheckResponse) -> Vec<u8> {
    let mut out = Vec::with_capacity(response.blocks.len() * 17);
    for block in &response.blocks {
        out.extend_from_slice(&(block.block_base as u64).to_le_bytes());
        match block.clean_hash {
            Some(hash) => {
                out.push(1u8);
                out.extend_from_slice(&hash.to_le_bytes());
            }
            None => {
                out.push(0u8);
            }
        }
    }
    out
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::memcheck::{self, BLOCK_SIZE};

    // ── Helper: build a synthetic 0x4f27 payload ──────────────────────────────

    fn make_memcheck_payload(specs: &[(u64, u32)]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&OPCODE_SERVER_MEMCHECK.to_le_bytes());
        payload.extend_from_slice(&(specs.len() as u16).to_le_bytes());
        for &(addr, len) in specs {
            payload.extend_from_slice(&addr.to_le_bytes());
            payload.extend_from_slice(&len.to_le_bytes());
        }
        payload
    }

    // ── dispatch: unknown opcode returns None ─────────────────────────────────

    #[test]
    fn dispatch_unknown_opcode_returns_none() {
        let payload = [0x01u8, 0x02, 0x00, 0x00];
        assert!(dispatch(&payload).is_none());
    }

    #[test]
    fn dispatch_short_payload_returns_none() {
        assert!(dispatch(&[]).is_none());
        assert!(dispatch(&[0x27]).is_none());
    }

    // ── dispatch: memcheck opcode returns Some ────────────────────────────────

    #[test]
    fn dispatch_memcheck_opcode_returns_some() {
        // Zero-region payload — still dispatched.
        let payload = make_memcheck_payload(&[]);
        assert!(dispatch(&payload).is_some());
    }

    // ── parse_region_specs ────────────────────────────────────────────────────

    #[test]
    fn parse_region_specs_too_short_returns_empty() {
        assert!(parse_region_specs(&[0x27, 0x4f]).is_empty());
        assert!(parse_region_specs(&[]).is_empty());
    }

    #[test]
    fn parse_region_specs_zero_count_returns_empty() {
        // count = 0
        let payload = [0x27u8, 0x4f, 0x00, 0x00];
        assert!(parse_region_specs(&payload).is_empty());
    }

    #[test]
    fn parse_region_specs_single_region() {
        let payload = make_memcheck_payload(&[(0x0001_4000_0000u64, 0x100u32)]);
        let specs = parse_region_specs(&payload);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].address, 0x0001_4000_0000usize);
        assert_eq!(specs[0].length, 0x100);
    }

    #[test]
    fn parse_region_specs_truncates_when_payload_short() {
        // Claim 3 regions but only provide bytes for 1.
        let mut payload = make_memcheck_payload(&[(0x1000u64, 0x100u32)]);
        // Patch the count to say 3.
        payload[2] = 3;
        payload[3] = 0;
        let specs = parse_region_specs(&payload);
        // Only 1 fully parseable region.
        assert_eq!(specs.len(), 1);
    }

    // ── handle_server_memcheck: handler returns spoofed bytes ─────────────────

    /// Core acceptance test: handler returns the pre-modification clean hash
    /// (from the PRNG-aware cache) and does NOT read live `.text` section bytes.
    ///
    /// We simulate a patched block by registering a clean entry in the cache
    /// before the "patch", then ask the handler to respond for that range.
    /// The expected clean hash is compared to the one in `BlockResult`.
    #[test]
    fn handler_returns_spoofed_hash_for_cached_block() {
        // Use an address range that is clearly not live .text on any platform.
        const TEST_ADDR: usize = 0x0001_400B_5700; // block-aligned

        // Simulate pre-patch registration.
        let clean_bytes = [0x90u8; BLOCK_SIZE]; // NOPs — pretend original bytes
        memcheck::register_clean_block_before_detour(TEST_ADDR, &clean_bytes);

        let expected_hash = memcheck::hash_block(&clean_bytes);

        // Build a 0x4f27 payload covering the cached block.
        let payload = make_memcheck_payload(&[(TEST_ADDR as u64, BLOCK_SIZE as u32)]);
        let response = handle_server_memcheck(&payload);

        assert!(
            response.any_spoofed,
            "at least one block should have been spoofed"
        );
        assert_eq!(response.blocks.len(), 1);
        assert_eq!(
            response.blocks[0].clean_hash,
            Some(expected_hash),
            "handler must return pre-modification hash, not live bytes"
        );
    }

    /// Blocks not in the cache should pass through (clean_hash == None).
    #[test]
    fn handler_returns_none_for_unmodified_block() {
        // An address that was never registered.
        const UNMODIFIED_ADDR: usize = 0xDEAD_0000;

        let payload = make_memcheck_payload(&[(UNMODIFIED_ADDR as u64, BLOCK_SIZE as u32)]);
        let response = handle_server_memcheck(&payload);

        assert!(
            !response.any_spoofed,
            "unmodified block should not be spoofed"
        );
        assert_eq!(response.blocks.len(), 1);
        assert_eq!(
            response.blocks[0].clean_hash, None,
            "unmodified block should return None (passthrough)"
        );
    }

    /// Handler with an empty payload (no regions) should return an empty response.
    #[test]
    fn handler_empty_payload_returns_empty_response() {
        let payload = make_memcheck_payload(&[]);
        let response = handle_server_memcheck(&payload);
        assert!(response.blocks.is_empty());
        assert!(!response.any_spoofed);
    }
}
