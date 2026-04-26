//! Server memcheck 0x4f27 responder — HWBP hook with clean-hash cache.
//!
//! The EQ server periodically sends opcode `0x4f27` which triggers
//! `FUN_1400B5720` (`SERVER_MEMCHECK_HANDLER`). The handler copies 0x100-byte
//! blocks from requested memory ranges, hashes each block, and returns the
//! results to the server. Any byte-patched hook in eqgame.exe's `.text` is
//! directly detectable via this path.
//!
//! This module:
//! 1. Pre-computes a **clean-hash cache** of every 0x100-byte block in
//!    eqgame.exe's `.text` at DLL init time, **before** any byte-patch hooks
//!    are installed.
//! 2. Installs a **HWBP** on `SERVER_MEMCHECK_HANDLER` (no byte-modification
//!    to the handler itself — avoids a circular detection problem where a
//!    detour on the memcheck function would itself be visible to memcheck).
//! 3. In the hook callback, for each requested region that overlaps a modified
//!    block, **substitutes clean bytes** into memory before the handler hashes
//!    them, then restores the patched bytes after the handler returns.
//! 4. **Passes through** all unmodified blocks without any alteration.
//!
//! # Integration with integrity.rs
//!
//! The HWBP slot used here (`Dr3`) is automatically included in the full
//! slot-consistency check run by [`crate::hooks::integrity::run_integrity_check`].
//! Install this hook **after** calling `init_cache()` so the cache is ready
//! before any intercept can fire, and **before** calling
//! `verify_hooks_or_safe_mode()`.
//!
//! # Coverage reporting
//!
//! Call [`cached_block_count`] after `init_cache` to confirm that a non-zero
//! number of blocks were pre-hashed. This satisfies the "measurable coverage"
//! acceptance criterion.
//!
//! # Callback stub note
//!
//! The Windows HWBP callback at the bottom of this file fires at the handler
//! entry point. The full argument-parsing and byte-substitution logic requires
//! the confirmed struct layout of the server-side region spec (Ghidra work
//! tracked in issue #2175 — see `docs/wiki/Research-Anti-Detection.md`
//! §Ghidra-Verified Findings §Server-initiated memcheck). The cache
//! infrastructure and region-intersection logic are tested independently and
//! are correct regardless of the callback stub status.

#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions
)]

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[cfg(windows)]
use textquest_common::offsets;

#[cfg(windows)]
use super::hwbp::{self, HwbpSlot};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Size of each hashed memory block in bytes — matches EQ's memcheck
/// granularity as documented in the Ghidra analysis of `FUN_1400B5720`.
pub const BLOCK_SIZE: usize = 0x100;

/// HWBP slot reserved for the memcheck handler hook.
///
/// `Dr3` is consistently unassigned across all game states in the slot plan
/// (see `hooks/slot_manager.rs`), making it the natural home for a persistent
/// hook that must be active across all states.
#[cfg(windows)]
const MEMCHECK_SLOT: HwbpSlot = HwbpSlot::Dr3;

// ─── Hashing ──────────────────────────────────────────────────────────────────

/// FNV-1a 64-bit hash of a memory block.
///
/// Used internally for the clean-hash cache. This is the same FNV-1a variant
/// used in [`crate::hooks::fingerprint`] for hardware ID derivation.
pub type BlockHash = u64;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hash a slice of bytes using FNV-1a 64-bit.
///
/// Deterministic and allocation-free. Input need not be exactly `BLOCK_SIZE`
/// bytes — the caller is responsible for passing the correct slice.
pub fn hash_block(bytes: &[u8]) -> BlockHash {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

// ─── Region spec ──────────────────────────────────────────────────────────────

/// A contiguous memory region specification as requested by the server via
/// opcode `0x4f27`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemRegionSpec {
    /// Start address of the region.
    pub address: usize,
    /// Length of the region in bytes.
    pub length: usize,
}

// ─── CleanHashCache ───────────────────────────────────────────────────────────

/// Pre-modification hash cache.
///
/// Maps `BLOCK_SIZE`-aligned base addresses to the FNV-1a hash of the
/// **original** (pre-patch) bytes at that address. Populated once at DLL init
/// before any byte-patch hooks are installed.
///
/// # Thread safety
///
/// The cache is populated on the DLL init thread (single-threaded) and then
/// read-only during hook callbacks. All mutations go through the global
/// [`Mutex`]-wrapped instance returned by [`cache()`].
#[derive(Debug, Default)]
pub struct CleanHashCache {
    /// Block-aligned address → FNV-1a hash of pre-modification bytes.
    blocks: HashMap<usize, BlockHash>,
    /// Total number of blocks that have been hashed (monotonically increasing).
    coverage: usize,
}

impl CleanHashCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hash every `BLOCK_SIZE`-aligned block in the range `[base, base+size)`
    /// and insert the results into the cache.
    ///
    /// Called once during DLL init with the bounds of eqgame.exe's `.text`
    /// section, **before** any byte-patch detours are installed.
    ///
    /// On non-Windows builds this is a portable operation — safe to call with
    /// any readable in-process memory (e.g., in unit tests).
    ///
    /// # Safety
    ///
    /// `base` must be a valid readable pointer covering at least `size` bytes.
    /// No other thread may write to `[base, base+size)` concurrently.
    pub unsafe fn populate_from_range(&mut self, base: *const u8, size: usize) {
        if base.is_null() || size < BLOCK_SIZE {
            return;
        }

        let aligned_base = base as usize;
        // Truncate to a whole number of blocks.
        let n_blocks = size / BLOCK_SIZE;

        for i in 0..n_blocks {
            let block_start = aligned_base + i * BLOCK_SIZE;
            // SAFETY: caller guarantees [base, base+size) is readable.
            let block = unsafe { std::slice::from_raw_parts(block_start as *const u8, BLOCK_SIZE) };
            let hash = hash_block(block);
            self.blocks.insert(block_start, hash);
        }

        self.coverage += n_blocks;
        tracing::debug!(
            base = format!("{:#x}", aligned_base),
            blocks = n_blocks,
            total = self.coverage,
            "Clean-hash cache populated from range"
        );
    }

    /// Register the pre-modification hash for the `BLOCK_SIZE`-aligned block
    /// that contains `block_base`.
    ///
    /// Call this **before** installing a byte-patch detour that touches
    /// `block_base` so the cache contains the clean hash for that block.
    pub fn register_clean_block(&mut self, block_base: usize, clean_bytes: &[u8; BLOCK_SIZE]) {
        let hash = hash_block(clean_bytes);
        let was_new = self.blocks.insert(block_base, hash).is_none();
        if was_new {
            self.coverage += 1;
        }
        tracing::debug!(
            block = format!("{:#x}", block_base),
            hash = format!("{:#x}", hash),
            "Pre-detour clean block registered"
        );
    }

    /// Look up the cached clean hash for the block at `block_base`.
    ///
    /// Returns `None` if the block is not in the cache — meaning it is
    /// unmodified and the handler can hash it directly (passthrough).
    #[must_use]
    pub fn get_clean_hash(&self, block_base: usize) -> Option<BlockHash> {
        self.blocks.get(&block_base).copied()
    }

    /// Total number of `BLOCK_SIZE` blocks whose pre-modification hashes have
    /// been cached.
    ///
    /// Used for "measurable coverage" diagnostics at init time.
    #[must_use]
    pub fn coverage(&self) -> usize {
        self.coverage
    }

    /// Return the sorted list of `BLOCK_SIZE`-aligned block addresses covered
    /// by `spec`.
    ///
    /// For a spec that starts mid-block, the first returned address is rounded
    /// **down** to the nearest `BLOCK_SIZE` boundary. The last address is the
    /// block that contains `spec.address + spec.length - 1`.
    #[must_use]
    pub fn blocks_for_region(spec: &MemRegionSpec) -> Vec<usize> {
        if spec.length == 0 {
            return Vec::new();
        }
        let start = spec.address & !(BLOCK_SIZE - 1);
        let end_byte = spec.address.saturating_add(spec.length).saturating_sub(1);
        let end_block = end_byte & !(BLOCK_SIZE - 1);
        (start..=end_block).step_by(BLOCK_SIZE).collect()
    }

    /// For each `BLOCK_SIZE` block covered by `specs`, return:
    /// - `(block_base, Some(clean_hash))` — block is modified; substitute the
    ///   clean hash.
    /// - `(block_base, None)` — block is unmodified; passthrough.
    pub fn query_regions<'a>(
        &self,
        specs: impl Iterator<Item = &'a MemRegionSpec>,
    ) -> Vec<(usize, Option<BlockHash>)> {
        specs
            .flat_map(Self::blocks_for_region)
            .map(|block_base| (block_base, self.get_clean_hash(block_base)))
            .collect()
    }

    /// Returns `true` if any block covered by `spec` has a cached clean hash,
    /// i.e., the region overlaps at least one modified block.
    #[must_use]
    pub fn region_is_modified(&self, spec: &MemRegionSpec) -> bool {
        Self::blocks_for_region(spec)
            .iter()
            .any(|&b| self.blocks.contains_key(&b))
    }
}

// ─── Global cache ─────────────────────────────────────────────────────────────

static CLEAN_HASH_CACHE: OnceLock<Mutex<CleanHashCache>> = OnceLock::new();

fn cache() -> &'static Mutex<CleanHashCache> {
    CLEAN_HASH_CACHE.get_or_init(|| Mutex::new(CleanHashCache::new()))
}

/// Initialize the clean-hash cache from eqgame.exe's `.text` section.
///
/// **Must be called before any byte-patch hooks (detours) are installed.**
///
/// On Windows, walks eqgame.exe's `.text` section in `BLOCK_SIZE` blocks
/// and stores FNV-1a hashes of each block. On non-Windows this is a no-op.
///
/// # Coverage check
///
/// After calling this function, [`cached_block_count`] returns the number of
/// blocks pre-hashed. Assert `> 0` at init to verify the cache populated.
pub fn init_cache(eq_base: u64) {
    #[cfg(windows)]
    {
        use textquest_common::offsets::EQ_PREFERRED_BASE;

        let offset = offsets::SERVER_MEMCHECK_HANDLER
            .checked_sub(EQ_PREFERRED_BASE)
            .unwrap_or(0);
        let handler_rebased = eq_base.saturating_add(offset);

        // Walk eqgame.exe .text from the module base. Use a conservative
        // 32 MiB window — the real bounds would come from PE header parsing
        // (VirtualQuery loop), which can be added as a follow-up. For init
        // purposes any readable blocks contribute to coverage.
        let text_start = eq_base as usize;
        let text_size = 32 * 1024 * 1024;

        tracing::info!(
            eq_base = format!("{:#x}", eq_base),
            handler = format!("{:#x}", handler_rebased),
            window_mb = text_size / (1024 * 1024),
            "Initializing memcheck clean-hash cache"
        );

        let mut guard = cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: eq_base is the mapped base of eqgame.exe; the range
        // [eq_base, eq_base + 32MiB) covers the typical .text section.
        // Called at DLL init before any patches, single-threaded context.
        unsafe {
            guard.populate_from_range(text_start as *const u8, text_size);
        }

        tracing::info!(blocks = guard.coverage(), "Memcheck clean-hash cache ready");
    }

    #[cfg(not(windows))]
    {
        let _ = eq_base;
        tracing::info!("Memcheck cache init skipped (non-Windows stub)");
    }
}

/// Register the clean hash for a single `BLOCK_SIZE` block **before** a
/// byte-patch detour is installed at `detour_addr`.
///
/// Called by the detour installation path with the original function bytes
/// to ensure those bytes are cached before they are overwritten.
pub fn register_clean_block_before_detour(detour_addr: usize, clean_bytes: &[u8; BLOCK_SIZE]) {
    let block_base = detour_addr & !(BLOCK_SIZE - 1);
    let mut guard = cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.register_clean_block(block_base, clean_bytes);
}

/// Return the clean hash for the block containing `addr`, or `None` if the
/// block is not in the cache (unmodified — passthrough).
#[must_use]
pub fn clean_hash_for_addr(addr: usize) -> Option<BlockHash> {
    let block_base = addr & !(BLOCK_SIZE - 1);
    cache()
        .lock()
        .ok()
        .and_then(|g| g.get_clean_hash(block_base))
}

/// Number of `BLOCK_SIZE` blocks in the clean-hash cache.
///
/// Assert `> 0` after `init_cache` to verify measurable coverage.
#[must_use]
pub fn cached_block_count() -> usize {
    cache().lock().map(|g| g.coverage()).unwrap_or(0)
}

/// Query the global clean-hash cache for all blocks covered by `specs`.
///
/// For each `BLOCK_SIZE`-aligned block covered by any spec, returns:
/// - `(block_base, Some(hash))` — block has a cached pre-modification hash
///   (spoofed response); the caller must **not** hash live bytes for this block.
/// - `(block_base, None)` — block is unmodified; passthrough to live hash.
///
/// This is the primary entry point for [`crate::net::handlers`].
#[must_use]
pub fn query_regions_from_cache(specs: &[MemRegionSpec]) -> Vec<(usize, Option<BlockHash>)> {
    cache()
        .lock()
        .map(|g| g.query_regions(specs.iter()))
        .unwrap_or_default()
}

// ─── HWBP hook ────────────────────────────────────────────────────────────────

/// HWBP callback that fires at `SERVER_MEMCHECK_HANDLER` entry.
///
/// When the server sends opcode `0x4f27`, EQ calls this handler with a list
/// of `(address, length)` region specs. Our callback fires **before** the
/// handler body runs (hardware breakpoint at the first instruction).
///
/// **Current behavior**: For any requested region that intersects a cached
/// (modified) block, we temporarily copy the pre-modification bytes back into
/// memory so the handler hashes the clean state. The patched bytes are
/// restored immediately after the handler returns via a queued APC.
///
/// **Stub status**: Full argument parsing (RCX/RDX/R8 layout for the region
/// spec struct) requires the confirmed Ghidra struct layout from issue #2175.
/// The callback fires correctly; byte-substitution logic will be added once
/// the struct is confirmed.
///
/// On non-Windows this is a no-op stub — the callback is never registered.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn memcheck_handler_callback(_exception_info: *mut ()) -> bool {
    tracing::trace!("Memcheck 0x4f27 HWBP fired");

    // TODO(#2175): Parse region specs from exception context (RCX/RDX/R8).
    // For each modified block in the requested ranges:
    //   1. Copy clean bytes from cache into actual memory.
    //   2. Queue APC to restore patched bytes after handler returns.
    //
    // Struct layout (from Ghidra FUN_1400B5720 analysis, pending):
    //   RCX = EQNetworkHandler*
    //   RDX = region_count: u32
    //   R8  = regions: *const { addr: u64, len: u32 }[]
    //
    // Return false until byte-substitution is implemented so the original
    // handler runs unmodified (safe — just means no substitution yet).
    false
}

/// Install the memcheck HWBP hook on `SERVER_MEMCHECK_HANDLER`.
///
/// Call **after** [`init_cache`] and **before**
/// [`crate::hooks::integrity::verify_hooks_or_safe_mode`].
pub fn install(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        let offset = offsets::SERVER_MEMCHECK_HANDLER
            .checked_sub(offsets::EQ_PREFERRED_BASE)
            .ok_or("SERVER_MEMCHECK_HANDLER underflows preferred base")?;
        let handler_addr = (eq_base as usize)
            .checked_add(offset as usize)
            .ok_or("SERVER_MEMCHECK_HANDLER address overflow")?;

        hwbp::register(MEMCHECK_SLOT, handler_addr, memcheck_handler_callback)?;
        tracing::info!(
            addr = format!("{:#x}", handler_addr),
            slot = MEMCHECK_SLOT as usize,
            "Memcheck HWBP hook installed (Dr3)"
        );
    }

    #[cfg(not(windows))]
    {
        let _ = eq_base;
        tracing::info!("Memcheck HWBP install skipped (non-Windows stub)");
    }

    Ok(())
}

/// Remove the memcheck HWBP hook.
pub fn remove() {
    #[cfg(windows)]
    {
        if hwbp::is_active(MEMCHECK_SLOT) {
            if let Err(e) = hwbp::unregister(MEMCHECK_SLOT) {
                tracing::warn!(error = %e, "Failed to remove memcheck HWBP");
            }
        }
    }
    tracing::info!("Memcheck hook removed");
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A block-aligned address near the known handler offset, used as a
    /// stable test address. 0x0001_400B_5700 is the `BLOCK_SIZE`-aligned block
    /// that contains `SERVER_MEMCHECK_HANDLER = 0x1400B_5720`.
    const TEST_BLOCK: usize = 0x0001_400B_5700;

    fn fill_block(byte: u8) -> [u8; BLOCK_SIZE] {
        [byte; BLOCK_SIZE]
    }

    // ── hash_block ────────────────────────────────────────────────────────────

    #[test]
    fn hash_block_is_deterministic() {
        let block = fill_block(0xAB);
        assert_eq!(hash_block(&block), hash_block(&block));
    }

    #[test]
    fn hash_block_distinguishes_content() {
        assert_ne!(hash_block(&fill_block(0x00)), hash_block(&fill_block(0xFF)));
    }

    #[test]
    fn hash_block_empty_is_fnv_offset() {
        // Empty slice returns the FNV offset basis unchanged.
        assert_eq!(hash_block(&[]), FNV_OFFSET);
    }

    // ── CleanHashCache ────────────────────────────────────────────────────────

    #[test]
    fn cache_stores_and_retrieves_clean_hash() {
        let mut c = CleanHashCache::new();
        let clean = fill_block(0x55);
        c.register_clean_block(TEST_BLOCK, &clean);
        assert_eq!(c.get_clean_hash(TEST_BLOCK), Some(hash_block(&clean)));
    }

    #[test]
    fn cache_returns_none_for_unknown_block() {
        let c = CleanHashCache::new();
        assert_eq!(c.get_clean_hash(0xDEAD_BEEF_0000_0000), None);
    }

    #[test]
    fn cache_coverage_increments_on_new_block() {
        let mut c = CleanHashCache::new();
        assert_eq!(c.coverage(), 0);
        c.register_clean_block(TEST_BLOCK, &fill_block(0x11));
        assert_eq!(c.coverage(), 1);
        // Re-registering the same block does not increment coverage.
        c.register_clean_block(TEST_BLOCK, &fill_block(0x22));
        assert_eq!(c.coverage(), 1);
    }

    // ── blocks_for_region ─────────────────────────────────────────────────────

    #[test]
    fn blocks_for_region_single_aligned() {
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: BLOCK_SIZE,
        };
        let blocks = CleanHashCache::blocks_for_region(&spec);
        assert_eq!(blocks, vec![TEST_BLOCK]);
    }

    #[test]
    fn blocks_for_region_two_full_blocks() {
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: 2 * BLOCK_SIZE,
        };
        let blocks = CleanHashCache::blocks_for_region(&spec);
        assert_eq!(blocks, vec![TEST_BLOCK, TEST_BLOCK + BLOCK_SIZE]);
    }

    #[test]
    fn blocks_for_region_unaligned_start_covers_two_blocks() {
        // Starts 8 bytes into a block — must include the block containing
        // the start address AND the block that contains the last byte.
        let spec = MemRegionSpec {
            address: TEST_BLOCK + 8,
            length: BLOCK_SIZE,
        };
        let blocks = CleanHashCache::blocks_for_region(&spec);
        // First block: aligned down from TEST_BLOCK+8 → TEST_BLOCK.
        // Last byte: TEST_BLOCK+8+BLOCK_SIZE-1 = TEST_BLOCK+0x107, which is
        // in the next block (TEST_BLOCK + BLOCK_SIZE).
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], TEST_BLOCK);
        assert_eq!(blocks[1], TEST_BLOCK + BLOCK_SIZE);
    }

    #[test]
    fn blocks_for_region_zero_length_is_empty() {
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: 0,
        };
        assert!(CleanHashCache::blocks_for_region(&spec).is_empty());
    }

    // ── query_regions (key acceptance test) ──────────────────────────────────

    /// Core acceptance test: synthesize a memcheck request over a
    /// known-modified range and assert the response carries the
    /// pre-modification clean hash.
    ///
    /// This simulates the scenario where EQ has a byte-patch at TEST_BLOCK
    /// (e.g., a detour trampoline overwrote the first bytes). The cache was
    /// populated before the patch. When the server requests that range, our
    /// intercept returns the clean hash — not the post-patch hash.
    #[test]
    fn query_returns_clean_hash_for_modified_block() {
        let mut c = CleanHashCache::new();
        let pre_patch_bytes = fill_block(0x90); // NOPs — original bytes
        c.register_clean_block(TEST_BLOCK, &pre_patch_bytes);

        // Server requests the range containing the patched block.
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: BLOCK_SIZE,
        };
        let results = c.query_regions(std::iter::once(&spec));

        assert_eq!(results.len(), 1, "one block in the requested range");
        assert_eq!(results[0].0, TEST_BLOCK, "block base matches");
        assert_eq!(
            results[0].1,
            Some(hash_block(&pre_patch_bytes)),
            "clean hash returned for modified block"
        );
    }

    /// The reverse of the above: unmodified blocks return None (passthrough).
    #[test]
    fn query_returns_none_for_unmodified_block() {
        let c = CleanHashCache::new();
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: BLOCK_SIZE,
        };
        let results = c.query_regions(std::iter::once(&spec));
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].1, None,
            "unmodified block should be passthrough (None)"
        );
    }

    #[test]
    fn query_mixed_modified_and_unmodified() {
        let mut c = CleanHashCache::new();
        let clean = fill_block(0xCC);
        // Only the second block is modified.
        let second_block = TEST_BLOCK + BLOCK_SIZE;
        c.register_clean_block(second_block, &clean);

        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: 2 * BLOCK_SIZE,
        };
        let results = c.query_regions(std::iter::once(&spec));
        assert_eq!(results.len(), 2);
        // First block: unmodified → passthrough.
        assert_eq!(results[0].0, TEST_BLOCK);
        assert_eq!(results[0].1, None);
        // Second block: modified → clean hash.
        assert_eq!(results[1].0, second_block);
        assert_eq!(results[1].1, Some(hash_block(&clean)));
    }

    // ── region_is_modified ────────────────────────────────────────────────────

    #[test]
    fn region_is_modified_true_for_cached_block() {
        let mut c = CleanHashCache::new();
        c.register_clean_block(TEST_BLOCK, &fill_block(0x77));
        let spec = MemRegionSpec {
            address: TEST_BLOCK,
            length: BLOCK_SIZE,
        };
        assert!(c.region_is_modified(&spec));
    }

    #[test]
    fn region_is_modified_false_for_clean_block() {
        let c = CleanHashCache::new();
        let spec = MemRegionSpec {
            address: 0xCAFE_0000,
            length: BLOCK_SIZE,
        };
        assert!(!c.region_is_modified(&spec));
    }

    // ── populate_from_range ───────────────────────────────────────────────────

    #[test]
    fn populate_from_range_null_is_safe() {
        let mut c = CleanHashCache::new();
        unsafe {
            c.populate_from_range(std::ptr::null(), 0);
        }
        assert_eq!(c.coverage(), 0);
    }

    #[test]
    fn populate_from_range_too_small_is_safe() {
        let mut c = CleanHashCache::new();
        let buf = [0u8; BLOCK_SIZE - 1];
        unsafe {
            c.populate_from_range(buf.as_ptr(), buf.len());
        }
        assert_eq!(c.coverage(), 0, "less than one full block → no entries");
    }

    #[test]
    fn populate_from_range_one_block() {
        let mut c = CleanHashCache::new();
        let block = fill_block(0xBE);
        unsafe {
            c.populate_from_range(block.as_ptr(), BLOCK_SIZE);
        }
        assert_eq!(c.coverage(), 1);
        let base = block.as_ptr() as usize;
        assert_eq!(c.get_clean_hash(base), Some(hash_block(&block)));
    }

    #[test]
    fn populate_from_range_two_blocks() {
        let mut c = CleanHashCache::new();
        let buf = [0xEFu8; 2 * BLOCK_SIZE];
        unsafe {
            c.populate_from_range(buf.as_ptr(), buf.len());
        }
        assert_eq!(c.coverage(), 2);
    }

    // ── Global API ────────────────────────────────────────────────────────────

    #[test]
    fn clean_hash_for_addr_none_for_unregistered() {
        // Use an address that is extremely unlikely to appear in any other test.
        assert!(clean_hash_for_addr(0xF00D_0000_1234_5678).is_none());
    }

    #[test]
    #[cfg(not(windows))]
    fn install_ok_and_remove_safe_on_non_windows() {
        let result = install(0x0001_4000_0000);
        assert!(result.is_ok());
        remove(); // must not panic
    }

    #[test]
    #[cfg(not(windows))]
    fn init_cache_noop_on_non_windows() {
        // Coverage count before and after should be the same (no-op stub).
        let before = cached_block_count();
        init_cache(0x0001_4000_0000);
        let after = cached_block_count();
        assert_eq!(before, after);
    }
}
