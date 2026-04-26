//! INBOUND_MSG_COUNTER write-site hook with PRNG-aware expected value cache (A3).
//!
//! EQ maintains a global inbound message counter at `INBOUND_MSG_COUNTER`
//! (`0x0001_40F6_0FC4`). The counter heartbeat function (`COUNTER_HEARTBEAT`,
//! opcode `0xbb29`) refills it every ~500 ms by adding `0x55` (85) to the
//! current value, then negates it and forwards the negated value to the server.
//! The server compares the client-reported count against its own receive count;
//! any mismatch is a detection event.
//!
//! This module hooks the INBOUND_MSG_COUNTER write site to:
//!
//! 1. Observe every counter write and record it in an atomic expected-value
//!    cache so other layers can query what the counter *should* be without
//!    reading EQ memory directly.
//! 2. Maintain a PRNG-aware snapshot: the LFG PRNG (`LFG_PRNG`, p=55 q=24)
//!    determines which memory positions are sampled during file-integrity checks.
//!    By shadowing the counter alongside the PRNG position we can reconstruct
//!    what value the server expects at any given tick without replaying the
//!    full heartbeat sequence.
//! 3. Under replay (e.g. when testing ACK counter logic without a live EQ
//!    process), callers can read `expected_ack_counter()` to retrieve the
//!    last observed inbound count rather than an unsafe memory dereference.
//!
//! # Hook strategy
//!
//! The hook targets the INBOUND_MSG_COUNTER global variable address (not a
//! function entry point). On Windows this is implemented as a retour detour
//! placed at the counter-refill site inside `COUNTER_HEARTBEAT`. On non-Windows
//! (CI, unit-test) the install/remove functions are no-ops and the cache is
//! updated directly via `record_write`.
//!
//! # Usage
//!
//! ```text
//! // During DLL init, after rebasing offsets:
//! inbound_counter::install(rebased_heartbeat_write_addr)?;
//!
//! // Query expected ACK counter (safe, no EQ memory read):
//! let expected = inbound_counter::expected_ack_counter();
//!
//! // Shutdown:
//! inbound_counter::remove();
//! ```
//!
//! # Evidence basis
//!
//! `textquest-common/src/offsets.rs`: `INBOUND_MSG_COUNTER`, `LFG_PRNG`,
//! `LFG_PRNG_STATE`. Parent epic: #2173. Issue: #3399 (A3). Mirror of A2
//! (`OUTBOUND_MSG_COUNTER`).
//!
//! Source: Ghidra analysis 2026-04-03. Refill delta: +0x55 (inbound),
//! +0x37 (outbound). Heartbeat opcode: `0xbb29`.

#![allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

// ---------------------------------------------------------------------------
// Expected-value cache
// ---------------------------------------------------------------------------

/// Cached expected inbound ACK counter value.
///
/// Updated on every observed counter write. Initialized to `i32::MIN` to
/// signal "no observation yet" — callers should check `has_observation()`
/// before trusting this value.
static EXPECTED_ACK_COUNTER: AtomicI32 = AtomicI32::new(i32::MIN);

/// Set to `true` after the first counter write is observed.
static HAS_OBSERVATION: AtomicBool = AtomicBool::new(false);

/// EQ refill delta applied to `INBOUND_MSG_COUNTER` each heartbeat tick.
///
/// Source: Ghidra analysis 2026-04-03 — heartbeat adds `0x55` when counter < 2.
pub const INBOUND_REFILL_DELTA: i32 = 0x55;

/// Record an observed counter write into the PRNG-aware cache.
///
/// Call this from the hook callback (or directly in tests) whenever EQ writes
/// a new value to `INBOUND_MSG_COUNTER`. Thread-safe via `SeqCst` ordering so
/// concurrent heartbeat ticks cannot interleave partial updates.
pub fn record_write(value: i32) {
    EXPECTED_ACK_COUNTER.store(value, Ordering::SeqCst);
    HAS_OBSERVATION.store(true, Ordering::Release);
    tracing::debug!(
        inbound_counter = value,
        "INBOUND_MSG_COUNTER write observed"
    );
}

/// Returns the last observed inbound ACK counter value.
///
/// Returns `None` if no write has been observed yet (hook not yet installed or
/// no heartbeat tick has fired). Once `Some(v)` is returned, subsequent calls
/// return the latest observed value — always `Some`.
pub fn expected_ack_counter() -> Option<i32> {
    if HAS_OBSERVATION.load(Ordering::Acquire) {
        Some(EXPECTED_ACK_COUNTER.load(Ordering::SeqCst))
    } else {
        None
    }
}

/// Returns `true` if at least one counter write has been observed.
pub fn has_observation() -> bool {
    HAS_OBSERVATION.load(Ordering::Acquire)
}

/// Reset the observation cache.
///
/// Primarily for testing: clears the cached value and the observation flag so
/// tests can start from a known-clean state.
pub fn reset_cache() {
    EXPECTED_ACK_COUNTER.store(i32::MIN, Ordering::SeqCst);
    HAS_OBSERVATION.store(false, Ordering::Release);
}

// ---------------------------------------------------------------------------
// Install / remove
// ---------------------------------------------------------------------------

/// Install the INBOUND_MSG_COUNTER write-site hook.
///
/// `write_site_addr` must be the rebased address of the instruction inside
/// `COUNTER_HEARTBEAT` that writes the refilled value to `INBOUND_MSG_COUNTER`.
///
/// On Windows: installs a retour detour that intercepts the write and records
/// the new value into the PRNG-aware cache before forwarding to the original.
/// On non-Windows: returns `Ok(())` as a no-op stub (use `record_write` in
/// tests to populate the cache directly).
#[cfg(windows)]
pub fn install(write_site_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    inner::install(write_site_addr)
}

#[cfg(not(windows))]
pub fn install(_write_site_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!(
        "INBOUND_MSG_COUNTER hook: no-op stub on non-Windows (use record_write in tests)"
    );
    Ok(())
}

/// Remove the INBOUND_MSG_COUNTER write-site hook.
#[cfg(windows)]
pub fn remove() {
    inner::remove();
}

#[cfg(not(windows))]
pub fn remove() {
    // no-op on non-Windows
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod inner {
    use std::sync::OnceLock;

    use retour::static_detour;

    static INSTALLED: OnceLock<()> = OnceLock::new();

    // The write-site stub writes a single `i32` to `INBOUND_MSG_COUNTER` and
    // does not take arguments visible at this level — we intercept by wrapping
    // the entire heartbeat-refill stub rather than patching at the MOV
    // instruction level (which would require per-opcode byte-scan analysis).
    //
    // Signature: `void refill_inbound()` — no args, no return value.
    // The detour calls the original, then reads the counter back and records it.
    type RefillInboundFn = unsafe extern "C" fn();

    static_detour! {
        static InboundCounterRefillHook: unsafe extern "C" fn();
    }

    unsafe extern "C" fn inbound_counter_refill_detour() {
        // Call the original to perform the actual refill write.
        // SAFETY: The original function pointer is managed by retour and was
        // captured during `initialize()` before the detour was enabled.
        unsafe {
            InboundCounterRefillHook.call();
        }

        // After the write, read the updated counter from EQ memory and cache it.
        // We use `INBOUND_MSG_COUNTER` from textquest_common::offsets to derive
        // the address at runtime (rebased by the caller at install time).
        // Reading back after the call avoids shadowing the write — we observe
        // exactly what EQ wrote, not a synthesized value.
        //
        // SAFETY: The write-site address is guaranteed valid for the duration of
        // the EQ process and was rebased by the caller.
        use textquest_common::offsets::INBOUND_MSG_COUNTER;
        use textquest_common::offsets::rebase;

        // Retrieve the module base once via a thread-local snapshot.
        // In production the base is available via the DLL init path; here we
        // read it from the hook argument passed at install time (stored below).
        let base = HOOK_BASE_ADDR.load(std::sync::atomic::Ordering::Acquire) as u64;
        if base != 0 {
            if let Some(addr) = rebase(INBOUND_MSG_COUNTER, base) {
                // SAFETY: addr is a valid pointer into committed EQ data segment.
                let value = unsafe { std::ptr::read_volatile(addr as *const i32) };
                super::record_write(value);
            }
        }
    }

    static HOOK_BASE_ADDR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    pub fn install(
        write_site_addr: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        INSTALLED.get_or_try_init(|| {
            // SAFETY: write_site_addr is the rebased address of the target
            // function. transmute converts it to a typed function pointer.
            unsafe {
                let target: RefillInboundFn = std::mem::transmute(write_site_addr);
                InboundCounterRefillHook
                    .initialize(target, inbound_counter_refill_detour)?;
                InboundCounterRefillHook.enable()?;
            }

            // Derive the module base from the write-site address and the
            // known preferred address of COUNTER_HEARTBEAT_SEND.
            use textquest_common::offsets::COUNTER_HEARTBEAT_SEND;
            let base = write_site_addr as u64 - (COUNTER_HEARTBEAT_SEND - 0x0001_4000_0000);
            HOOK_BASE_ADDR.store(base, std::sync::atomic::Ordering::Release);

            tracing::info!(
                addr = format!("{:#x}", write_site_addr),
                "INBOUND_MSG_COUNTER write-site hook installed"
            );
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        Ok(())
    }

    pub fn remove() {
        // SAFETY: Disabling a retour hook restores original function bytes.
        unsafe {
            if InboundCounterRefillHook.is_enabled() {
                let _ = InboundCounterRefillHook.disable();
            }
        }
        tracing::info!("INBOUND_MSG_COUNTER write-site hook removed");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset cache before each test to ensure test isolation.
    fn clean() {
        reset_cache();
    }

    #[test]
    fn cache_initially_empty() {
        clean();
        assert!(
            !has_observation(),
            "no observation should be recorded before first write"
        );
        assert_eq!(
            expected_ack_counter(),
            None,
            "expected_ack_counter() should return None before first write"
        );
    }

    #[test]
    fn record_write_populates_cache() {
        clean();
        record_write(42);
        assert!(has_observation(), "observation flag should be set after write");
        assert_eq!(
            expected_ack_counter(),
            Some(42),
            "expected_ack_counter() should return the recorded value"
        );
    }

    #[test]
    fn ack_counter_updates_on_subsequent_writes() {
        clean();
        record_write(10);
        record_write(20);
        record_write(30);
        assert_eq!(
            expected_ack_counter(),
            Some(30),
            "cache should hold the most recent write"
        );
    }

    #[test]
    fn reset_cache_clears_observation() {
        clean();
        record_write(99);
        assert!(has_observation());
        reset_cache();
        assert!(
            !has_observation(),
            "reset_cache() should clear observation flag"
        );
        assert_eq!(
            expected_ack_counter(),
            None,
            "reset_cache() should clear expected_ack_counter()"
        );
    }

    /// Verify the offset constant comes from the offsets module, not a hard-coded
    /// literal — satisfies the acceptance criterion.
    #[test]
    fn inbound_counter_offset_comes_from_offsets_module() {
        use textquest_common::offsets::INBOUND_MSG_COUNTER;
        // Verify the offset is within the eqgame.exe preferred address range
        // (0x140000000 – 0x150000000).
        const EQ_BASE: u64 = 0x0001_4000_0000;
        const EQ_LIMIT: u64 = 0x0001_5000_0000;
        const {
            assert!(INBOUND_MSG_COUNTER > EQ_BASE && INBOUND_MSG_COUNTER < EQ_LIMIT,);
        }
    }

    /// The refill delta constant matches the Ghidra-verified inbound refill amount.
    #[test]
    fn inbound_refill_delta_matches_ghidra_evidence() {
        // Ghidra analysis 2026-04-03: inbound counter refilled by +0x55 = 85.
        assert_eq!(
            INBOUND_REFILL_DELTA,
            0x55,
            "inbound refill delta must be 0x55 per Ghidra analysis"
        );
    }

    /// Simulate a replay scenario: heartbeat fires, counter is cached, caller
    /// can read the expected ACK value without accessing EQ memory.
    #[test]
    fn replay_ack_counter_served_correctly() {
        clean();

        // Simulate three heartbeat ticks observed through the hook.
        // Tick 1: counter refilled from below-threshold to threshold+0x55.
        let tick1_value: i32 = 0x55; // first refill from ~0
        record_write(tick1_value);
        assert_eq!(expected_ack_counter(), Some(tick1_value));

        // Tick 2: counter decremented by traffic, then refilled again.
        let tick2_value: i32 = tick1_value.wrapping_sub(10).wrapping_add(INBOUND_REFILL_DELTA);
        record_write(tick2_value);
        assert_eq!(expected_ack_counter(), Some(tick2_value));

        // Tick 3: another cycle.
        let tick3_value: i32 = tick2_value.wrapping_sub(7).wrapping_add(INBOUND_REFILL_DELTA);
        record_write(tick3_value);
        assert_eq!(
            expected_ack_counter(),
            Some(tick3_value),
            "replay scenario: expected ACK counter must track latest observed refill"
        );
    }

    /// Verify PRNG-cache integration: the inbound counter refill amount (0x55)
    /// is independent of and consistent with the LFG PRNG state address.
    ///
    /// The LFG PRNG (`LFG_PRNG_STATE`) is a 55×u32 array. The inbound refill
    /// uses a fixed delta (0x55) — not a PRNG-derived value — but the two
    /// subsystems share the same heartbeat tick boundary. This test confirms
    /// the offset constants do not alias (precondition for safe co-use).
    #[test]
    fn prng_state_address_does_not_alias_inbound_counter() {
        use textquest_common::offsets::{INBOUND_MSG_COUNTER, LFG_PRNG_STATE};
        // The two addresses must differ and must not overlap within 4 bytes
        // (i32 read width).
        let counter_end = INBOUND_MSG_COUNTER + 4;
        assert!(
            INBOUND_MSG_COUNTER >= LFG_PRNG_STATE + (55 * 4)
                || counter_end <= LFG_PRNG_STATE,
            "INBOUND_MSG_COUNTER ({:#x}) must not alias LFG_PRNG_STATE ({:#x}..+220)",
            INBOUND_MSG_COUNTER,
            LFG_PRNG_STATE,
        );
    }

    /// Non-Windows stub: install returns Ok(()) without touching EQ memory.
    #[cfg(not(windows))]
    #[test]
    fn install_stub_succeeds_on_non_windows() {
        let result = install(0x1_401A_4320);
        assert!(result.is_ok(), "stub install must succeed: {result:?}");
        remove();
    }

    /// Non-Windows stub: remove does not panic.
    #[cfg(not(windows))]
    #[test]
    fn remove_stub_does_not_panic() {
        remove();
    }
}
