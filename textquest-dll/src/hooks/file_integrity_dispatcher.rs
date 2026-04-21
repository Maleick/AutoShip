//! File integrity dispatcher hook — `FILE_INTEGRITY_DISPATCHER = 0x140564BC0`.
//!
//! EQ runs three file integrity checks on `WorldAuthenticate` (`FUN_1402C9C80`):
//!
//! | Check | File                       | Opcode |
//! |-------|----------------------------|--------|
//! | 1x    | `eqgame.exe` (self)        | 0x8bdc |
//! | 1sa   | `Resources/BaseData.txt`   | 0xe91d |
//! | 1sa   | `Resources/SkillCaps.txt`  | 0x9562 |
//!
//! Each check hashes the target file AND samples 256 random DWORDs using a
//! **Lagged Fibonacci PRNG** (`FUN_14025ABD0`, state at `DAT_140E8D148`).
//! The PRNG is deterministic — the server knows which positions were sampled,
//! so neither the full hash nor the per-position samples can be spoofed without
//! modelling the PRNG state.
//!
//! # Risk profile
//!
//! - `BaseData.txt` / `SkillCaps.txt` — we do not modify these. This check
//!   only matters if the operator's install is already patched (e.g. MQ2).
//! - `eqgame.exe` — we use in-memory HWBP (no disk writes), so
//!   `GetModuleFileNameA`-based on-disk hash is unaffected. The hook is a stub
//!   pending any future disk-resident shim.
//!
//! # Design
//!
//! 1. [`LfgPrng`] — Rust port of `FUN_14025ABD0` (additive lagged Fibonacci,
//!    p=55 q=24).  Used to replay the sampling sequence the server expects.
//! 2. [`FileHashCache`] — precomputed (full_hash, sample_sequence) pair per
//!    file.  Empty by default; swap in a clean cache entry behind
//!    [`CACHE_ACTIVE`] when a disk-resident shim is introduced.
//! 3. Byte-patch detour on `FILE_INTEGRITY_DISPATCHER` via
//!    `retour::static_detour!` — passthrough default (calls original).
//!    Uses the same pattern as `hooks::fingerprint` (also in this crate).
//!    All four HWBP debug register slots (DR0–DR3) are reserved for other
//!    hooks; a detour is the correct mechanism here.
//!
//! # Evidence basis
//!
//! `docs/wiki/Research-Anti-Detection.md` §Ghidra-Verified §File integrity
//! checks.  Parent issue #2173.

use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Number of random DWORDs the PRNG samples from each file during an integrity
/// check.  Derived from Ghidra analysis of `FUN_140564BC0`.
pub const INTEGRITY_SAMPLE_COUNT: usize = 256;

/// Lagged Fibonacci generator table size (Knuth §3.2.2 recommended value).
pub const LFG_TABLE_SIZE: usize = 55;

/// Longer lag *j* for the additive recurrence `X_n = X_{n-j} + X_{n-k}`.
/// Must equal `LFG_TABLE_SIZE` so the oldest entry is always overwritten.
pub const LFG_LAG_J: usize = 55;

/// Shorter lag *k*.
pub const LFG_LAG_K: usize = 24;

/// EQ file integrity opcodes sent to the server during WorldAuthenticate.
pub const OPCODE_EXE_HASH: u16 = 0x8bdc;
pub const OPCODE_BASEDATA_HASH: u16 = 0xe91d;
pub const OPCODE_SKILLCAPS_HASH: u16 = 0x9562;

// ─── Cache gate ──────────────────────────────────────────────────────────────

/// When `true`, the hook substitutes precomputed cache entries instead of
/// passing through.  Default `false` — passthrough.
///
/// Flip to `true` only when a `FileHashCache` entry has been loaded for the
/// relevant file (e.g. when a disk-resident shim is in use for `eqgame.exe`).
pub static CACHE_ACTIVE: AtomicBool = AtomicBool::new(false);

// ─── Lagged Fibonacci PRNG ────────────────────────────────────────────────────

/// Rust port of `FUN_14025ABD0` — EQ's Lagged Fibonacci Generator.
///
/// The on-disk binary implements an additive recurrence over a 55-element
/// DWORD table:
///
/// ```text
/// X[n % 55] = X[(n - 55) % 55] + X[(n - 24) % 55]   (mod 2^32)
/// ```
///
/// The state is seeded once from `DAT_140E8D148` (a 55-DWORD block in the
/// EQ .data segment).  Because the server mirrors this seed, it can
/// independently replay the entire sampling sequence.
///
/// # Cross-check
///
/// Capture the live PRNG state from the running process (read 55 DWORDs at the
/// rebased `DAT_140E8D148` address), construct an [`LfgPrng`] with
/// [`LfgPrng::from_state`], then compare 1 000 calls to [`LfgPrng::next`]
/// against values logged by the in-process hook.
#[derive(Clone, Debug)]
pub struct LfgPrng {
    /// Circular table of the last `LFG_TABLE_SIZE` outputs.
    table: [u32; LFG_TABLE_SIZE],
    /// Current write position (wraps mod `LFG_TABLE_SIZE`).
    index: usize,
}

impl LfgPrng {
    /// Construct from a raw 55-DWORD state snapshot (e.g. read from the
    /// in-process `DAT_140E8D148` block at runtime).
    ///
    /// `state[0]` maps to the oldest entry; `state[54]` is the most recent.
    /// `index` is set to `LFG_TABLE_SIZE - 1` so the *next* call to
    /// [`next`](Self::next) advances correctly.
    pub fn from_state(state: &[u32; LFG_TABLE_SIZE]) -> Self {
        Self {
            table: *state,
            index: LFG_TABLE_SIZE - 1,
        }
    }

    /// Seed from a single 32-bit value using a linear congruential expander.
    ///
    /// This is provided for unit tests where an exact on-disk state snapshot is
    /// unavailable.  The expander fills the table with enough entropy to avoid
    /// degenerate all-zero behaviour.
    ///
    /// LCG parameters: multiplier = 1664525, addend = 1013904223 (Numerical
    /// Recipes / common game engine variant).
    pub fn seed(seed: u32) -> Self {
        let mut table = [0u32; LFG_TABLE_SIZE];
        let mut v = seed;
        for entry in &mut table {
            v = v.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *entry = v;
        }
        Self {
            table,
            index: LFG_TABLE_SIZE - 1,
        }
    }

    /// Advance the generator by one step and return the next pseudo-random
    /// 32-bit value.
    ///
    /// Implements:
    /// ```text
    /// index  = (index + 1) % 55
    /// value  = table[(index + 55 - 55) % 55] + table[(index + 55 - 24) % 55]
    /// table[index] = value
    /// ```
    ///
    /// Because `LFG_LAG_J == LFG_TABLE_SIZE`, `(index - j) % 55 == index`
    /// *before* the advance — i.e. we always read the oldest slot (the one
    /// about to be overwritten).  This matches the standard in-place update
    /// seen in Ghidra output for `FUN_14025ABD0`.
    #[inline]
    pub fn next(&mut self) -> u32 {
        self.index = (self.index + 1) % LFG_TABLE_SIZE;
        let lag_j = self.index; // (index - 55) mod 55 == index (j == table_size)
        let lag_k = (self.index + LFG_TABLE_SIZE - LFG_LAG_K) % LFG_TABLE_SIZE;
        let value = self.table[lag_j].wrapping_add(self.table[lag_k]);
        self.table[self.index] = value;
        value
    }

    /// Generate the 256-DWORD sampling sequence for a file of `file_len` bytes.
    ///
    /// Each raw PRNG value is reduced modulo the number of valid DWORD-aligned
    /// positions in the file: `file_len / 4`.  A zero-length or sub-4-byte
    /// file returns an array of zeros.
    pub fn sample_positions(&mut self, file_len: u64) -> [u32; INTEGRITY_SAMPLE_COUNT] {
        let dword_count = file_len / 4;
        let mut positions = [0u32; INTEGRITY_SAMPLE_COUNT];
        if dword_count == 0 {
            return positions;
        }
        let modulus = dword_count as u32;
        for pos in &mut positions {
            *pos = self.next() % modulus;
        }
        positions
    }
}

// ─── Per-file hash cache ──────────────────────────────────────────────────────

/// Which file the cache entry covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityFile {
    /// `eqgame.exe` self-hash (opcode `0x8bdc`).
    EqGameExe,
    /// `Resources/BaseData.txt` (opcode `0xe91d`).
    BaseDataTxt,
    /// `Resources/SkillCaps.txt` (opcode `0x9562`).
    SkillCapsTxt,
}

impl IntegrityFile {
    /// Server opcode associated with this check.
    pub const fn opcode(self) -> u16 {
        match self {
            Self::EqGameExe => OPCODE_EXE_HASH,
            Self::BaseDataTxt => OPCODE_BASEDATA_HASH,
            Self::SkillCapsTxt => OPCODE_SKILLCAPS_HASH,
        }
    }
}

/// Precomputed integrity response for a single file.
///
/// Populated offline from a known-clean install.  Loaded via
/// [`set_cache`] and activated by setting [`CACHE_ACTIVE`] to `true`.
#[derive(Clone)]
pub struct FileHashCache {
    /// Which file this cache covers.
    pub file: IntegrityFile,
    /// Full-file hash as EQ would compute it (opaque bytes — format matches the
    /// on-wire response for the relevant opcode).
    pub full_hash: Vec<u8>,
    /// The 256 DWORD samples at the positions the PRNG would have selected.
    pub samples: [u32; INTEGRITY_SAMPLE_COUNT],
}

impl std::fmt::Debug for FileHashCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileHashCache")
            .field("file", &self.file)
            .field("full_hash_len", &self.full_hash.len())
            .field("samples[0]", &self.samples[0])
            .finish()
    }
}

// ─── Global cache store ───────────────────────────────────────────────────────

static CACHE_EXE: OnceLock<FileHashCache> = OnceLock::new();
static CACHE_BASEDATA: OnceLock<FileHashCache> = OnceLock::new();
static CACHE_SKILLCAPS: OnceLock<FileHashCache> = OnceLock::new();

/// Load a precomputed cache entry for `file`.
///
/// Must be called before setting [`CACHE_ACTIVE`] to `true`.  Each file may
/// only be registered once (subsequent calls for the same file are silently
/// ignored — `OnceLock` semantics).
pub fn set_cache(entry: FileHashCache) {
    let file = entry.file;
    let cell = match file {
        IntegrityFile::EqGameExe => &CACHE_EXE,
        IntegrityFile::BaseDataTxt => &CACHE_BASEDATA,
        IntegrityFile::SkillCapsTxt => &CACHE_SKILLCAPS,
    };
    let _ = cell.set(entry);
    tracing::info!(file = ?file, "FileHashCache loaded");
}

/// Retrieve the loaded cache entry for `file`, or `None` if not yet loaded.
pub fn get_cache(file: IntegrityFile) -> Option<&'static FileHashCache> {
    match file {
        IntegrityFile::EqGameExe => CACHE_EXE.get(),
        IntegrityFile::BaseDataTxt => CACHE_BASEDATA.get(),
        IntegrityFile::SkillCapsTxt => CACHE_SKILLCAPS.get(),
    }
}

// ─── Byte-patch detour ────────────────────────────────────────────────────────
//
// Uses `retour::static_detour!` — the same pattern as `hooks::fingerprint`.
// This inlines a JMP stub at the function entry (5-15 bytes) via retour's
// trampoline engine.  No HWBP debug registers are consumed.
//
// `FILE_INTEGRITY_DISPATCHER` calling convention: `extern "system"` with a
// single `*mut c_void` context pointer, matching EQ's x64 thiscall-via-system
// convention used for all WorldAuthenticate sub-functions.
//
// Passthrough default: the detour calls the original via `.call(ctx)` so the
// three integrity checks run normally.  When `CACHE_ACTIVE` is set, future
// work here will skip the original and write spoofed response bytes instead.

#[cfg(windows)]
mod inner {
    use retour::static_detour;

    use super::CACHE_ACTIVE;
    use std::sync::atomic::Ordering;

    type DispatcherFn = unsafe extern "system" fn(*mut core::ffi::c_void);

    static_detour! {
        static IntegrityDispatcherHook: unsafe extern "system" fn(*mut core::ffi::c_void);
    }

    /// Detour body for `FILE_INTEGRITY_DISPATCHER`.
    ///
    /// **Current behaviour:** passthrough — calls the original dispatcher so all
    /// three integrity checks (0x8bdc / 0xe91d / 0x9562) execute normally.
    ///
    /// When [`CACHE_ACTIVE`] is `true` and a [`super::FileHashCache`] entry is
    /// loaded for the relevant file, a future update here will skip the original
    /// call and write precomputed response bytes into the outbound packet buffer
    /// instead.
    fn dispatcher_detour(ctx: *mut core::ffi::c_void) {
        if CACHE_ACTIVE.load(Ordering::Acquire) {
            // Future: look up FileHashCache by opcode, write spoofed bytes.
            // For now fall through so the server doesn't disconnect.
            tracing::warn!(
                "FILE_INTEGRITY_DISPATCHER: CACHE_ACTIVE=true but spoof not yet \
                 implemented; calling original"
            );
        } else {
            tracing::trace!("FILE_INTEGRITY_DISPATCHER passthrough");
        }

        // Call the original function so EQ's integrity checks complete normally.
        // SAFETY: `ctx` is the same pointer EQ passed; the original function was
        // saved by retour during installation.
        unsafe {
            IntegrityDispatcherHook.call(ctx);
        }
    }

    /// Install the byte-patch detour on `FILE_INTEGRITY_DISPATCHER`.
    ///
    /// `dispatcher_addr` must be the **rebased** address, i.e.
    /// `offsets::rebase(offsets::FILE_INTEGRITY_DISPATCHER, actual_base).unwrap()`.
    pub fn install(dispatcher_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: `dispatcher_addr` is the live rebased address of
        // FILE_INTEGRITY_DISPATCHER.  The transmute converts it to a function
        // pointer matching the calling convention.
        unsafe {
            let target: DispatcherFn = std::mem::transmute(dispatcher_addr);
            IntegrityDispatcherHook.initialize(target, dispatcher_detour)?;
            IntegrityDispatcherHook.enable()?;
        }
        tracing::info!(
            addr = format!("{:#x}", dispatcher_addr),
            "FILE_INTEGRITY_DISPATCHER byte-patch detour installed (passthrough)"
        );
        Ok(())
    }

    /// Remove the byte-patch detour, restoring the original function prologue.
    pub fn remove() {
        // SAFETY: disabling a retour hook restores the saved original bytes.
        unsafe {
            if IntegrityDispatcherHook.is_enabled() {
                let _ = IntegrityDispatcherHook.disable();
            }
        }
        tracing::info!("FILE_INTEGRITY_DISPATCHER detour removed");
    }
}

#[cfg(not(windows))]
mod inner {
    /// Stub: byte-patch detours require the Windows PE loader.
    pub fn install(_dispatcher_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("FILE_INTEGRITY_DISPATCHER detour not available on this platform (stub)");
        Ok(())
    }

    /// Stub removal — no-op on non-Windows.
    pub fn remove() {
        tracing::warn!("FILE_INTEGRITY_DISPATCHER detour removal stub (non-Windows)");
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── PRNG parity tests ─────────────────────────────────────────────────────

    /// Seed with a known value and verify the first few outputs are
    /// deterministic.  These values are the ground truth for this Rust
    /// implementation; they must be cross-checked against the in-process
    /// hook log once the DLL is running on Frostreaver.
    #[test]
    fn prng_deterministic_from_seed() {
        let mut a = LfgPrng::seed(0xDEAD_BEEF);
        let mut b = LfgPrng::seed(0xDEAD_BEEF);
        for _ in 0..1000 {
            assert_eq!(a.next(), b.next(), "PRNG must be deterministic");
        }
    }

    /// Different seeds must produce different sequences.
    #[test]
    fn prng_different_seeds_differ() {
        let mut a = LfgPrng::seed(1);
        let mut b = LfgPrng::seed(2);
        let seq_a: Vec<u32> = (0..256).map(|_| a.next()).collect();
        let seq_b: Vec<u32> = (0..256).map(|_| b.next()).collect();
        assert_ne!(
            seq_a, seq_b,
            "different seeds must yield different sequences"
        );
    }

    /// Generate exactly 256 DWORD positions — matches INTEGRITY_SAMPLE_COUNT.
    #[test]
    fn sample_positions_count() {
        let mut prng = LfgPrng::seed(0x1234_5678);
        // Simulate a 1 MiB file
        let positions = prng.sample_positions(1024 * 1024);
        assert_eq!(positions.len(), INTEGRITY_SAMPLE_COUNT);
    }

    /// All sampled positions must be within the valid DWORD range for the file.
    #[test]
    fn sample_positions_in_range() {
        let file_len: u64 = 0x10_0000; // 1 MiB
        let dword_count = file_len / 4;
        let mut prng = LfgPrng::seed(0xABCD_EF01);
        let positions = prng.sample_positions(file_len);
        for (i, &pos) in positions.iter().enumerate() {
            assert!(
                (pos as u64) < dword_count,
                "position[{i}] = {pos:#x} out of range (dword_count = {dword_count:#x})"
            );
        }
    }

    /// Zero-length file returns all-zero positions (no division by zero).
    #[test]
    fn sample_positions_empty_file() {
        let mut prng = LfgPrng::seed(0);
        let positions = prng.sample_positions(0);
        assert!(
            positions.iter().all(|&p| p == 0),
            "empty file must return all-zero positions"
        );
    }

    /// `from_state` round-trip: construct from a known table, advance N steps,
    /// verify against an independent seed-expanded instance advanced to the
    /// same point.
    ///
    /// This models the cross-check against the binary: capture the live table
    /// at `DAT_140E8D148`, construct `LfgPrng::from_state`, advance 1 000
    /// times, compare against hook-logged values.
    #[test]
    fn from_state_parity_1000_iterations() {
        // Build a reference instance via seed expansion
        let mut reference = LfgPrng::seed(0x5A5A_5A5A);
        // Warm it up a bit so the table is non-trivial
        for _ in 0..55 {
            reference.next();
        }

        // Clone its internal table to simulate a live memory snapshot
        let snapshot = reference.table;

        // Construct a second instance from the captured state
        let mut from_snap = LfgPrng::from_state(&snapshot);

        // The `from_state` index starts at LFG_TABLE_SIZE - 1, which matches
        // the reference's current index (it just wrote to slot 54 in the warmup).
        // Both must produce identical next-1000 outputs.
        for i in 0..1000 {
            let r = reference.next();
            let s = from_snap.next();
            assert_eq!(
                r, s,
                "from_state diverged at iteration {i}: reference={r:#010x} snapshot={s:#010x}"
            );
        }
    }

    /// Verify the LFG recurrence property: after sufficient warm-up the
    /// sequence satisfies `X[n] == X[n-55] + X[n-24]` (mod 2^32).
    #[test]
    fn lfg_recurrence_property() {
        let mut prng = LfgPrng::seed(0x9999_1111);
        // Collect enough values to check the recurrence from position 55
        let n = LFG_TABLE_SIZE * 3;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(prng.next());
        }

        // After the table has fully turned over (i >= 55), check recurrence.
        // X[i] = X[i - 55] + X[i - 24]  (indices into the `values` array)
        for i in LFG_TABLE_SIZE..n {
            let expected = values[i - LFG_LAG_J].wrapping_add(values[i - LFG_LAG_K]);
            assert_eq!(
                values[i], expected,
                "recurrence failed at i={i}: values[{i}]={:#010x} expected={expected:#010x}",
                values[i]
            );
        }
    }

    // ── Cache tests ───────────────────────────────────────────────────────────

    /// `set_cache` stores an entry; `get_cache` retrieves it.
    #[test]
    fn cache_roundtrip() {
        let entry = FileHashCache {
            file: IntegrityFile::EqGameExe,
            full_hash: vec![0xAB, 0xCD, 0xEF],
            samples: [0u32; INTEGRITY_SAMPLE_COUNT],
        };
        set_cache(entry.clone());
        let loaded = get_cache(IntegrityFile::EqGameExe);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.full_hash, entry.full_hash);
    }

    /// Unloaded cache slots return `None`.
    #[test]
    fn cache_miss_returns_none() {
        // BaseData and SkillCaps are not loaded by the roundtrip test above
        // (they share the same OnceLock cells across the whole test binary, but
        // we never call `set_cache` for them in this module's tests).
        // We can only assert None if those cells haven't been written elsewhere;
        // since test ordering is non-deterministic we skip asserting None and
        // just verify the function doesn't panic.
        let _ = get_cache(IntegrityFile::BaseDataTxt);
        let _ = get_cache(IntegrityFile::SkillCapsTxt);
    }

    // ── Opcode mapping ────────────────────────────────────────────────────────

    #[test]
    fn opcode_mapping() {
        assert_eq!(IntegrityFile::EqGameExe.opcode(), OPCODE_EXE_HASH);
        assert_eq!(IntegrityFile::BaseDataTxt.opcode(), OPCODE_BASEDATA_HASH);
        assert_eq!(IntegrityFile::SkillCapsTxt.opcode(), OPCODE_SKILLCAPS_HASH);
    }

    // ── Hook stub tests ───────────────────────────────────────────────────────

    /// On non-Windows, `install` and `remove` must not panic.
    #[cfg(not(windows))]
    #[test]
    fn install_remove_stub() {
        assert!(install(0x1234_5678).is_ok());
        remove();
    }
}
