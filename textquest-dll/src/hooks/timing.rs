//! Timing normalization for anti-debug evasion checks.
//!
//! Win32 timing API hooks (`GetTickCount` / `QueryPerformanceCounter`) can
//! distort execution measurements when hook callbacks add measurable latency.
//! TextQuest can optionally subtract accumulated hook overhead from timing
//! values to avoid false-positive anti-debug detections.

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

const NS_PER_MS: u64 = 1_000_000;

#[cfg(windows)]
use windows::Win32::Foundation::BOOL;

static TIMING_CORRECTION_ENABLED: AtomicBool = AtomicBool::new(false);
static GAME_LOOP_OVERHEAD_NS: AtomicU64 = AtomicU64::new(0);
pub(super) static QPC_FREQUENCY_HZ: AtomicU64 = AtomicU64::new(10_000_000); // 10 MHz fallback

/// Record hook execution overhead observed on the process game loop.
///
/// This value is shared with the timing hooks and used to normalize timing API
/// returns. Stores the most-recent frame sample (not accumulated).
pub(crate) fn record_game_loop_hook_overhead(duration: Duration) {
    let nanos = clamp_duration_nanos_u64(duration);
    GAME_LOOP_OVERHEAD_NS.store(nanos, Ordering::Release);
}

pub(crate) fn set_enabled(enabled: bool) {
    TIMING_CORRECTION_ENABLED.store(enabled, Ordering::Release);
}

#[allow(clippy::cast_sign_loss)]
pub(crate) fn adjust_tick_count(raw: u32) -> u32 {
    if !TIMING_CORRECTION_ENABLED.load(Ordering::Acquire) {
        return raw;
    }

    let overhead_ms = overhead_ms();
    raw.saturating_sub(overhead_ms as u32)
}

#[allow(clippy::cast_sign_loss)]
pub(crate) fn adjust_qpc(raw: i64) -> i64 {
    if !TIMING_CORRECTION_ENABLED.load(Ordering::Acquire) {
        return raw;
    }

    if raw <= 0 {
        return 0;
    }

    let overhead_ticks = overhead_as_qpc_ticks();
    let raw_u128 = raw as u128;
    (raw_u128.saturating_sub(overhead_ticks as u128)) as i64
}

fn overhead_ms() -> u64 {
    let ns = GAME_LOOP_OVERHEAD_NS.load(Ordering::Acquire);
    ns / NS_PER_MS
}

fn overhead_as_qpc_ticks() -> u64 {
    let freq = QPC_FREQUENCY_HZ.load(Ordering::Acquire);
    let ns = GAME_LOOP_OVERHEAD_NS.load(Ordering::Acquire);
    // Convert nanoseconds to QPC ticks: ticks = ns * freq / 1_000_000_000
    ((ns as u128).saturating_mul(freq as u128) / 1_000_000_000) as u64
}

fn clamp_duration_nanos_u64(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(windows)]
mod inner {
    use super::*;
    use retour::static_detour;

    type GetTickCountFn = unsafe extern "system" fn() -> u32;
    type QueryPerformanceCounterFn = unsafe extern "system" fn(*mut i64) -> BOOL;

    static_detour! {
        static GetTickCountHook: unsafe extern "system" fn() -> u32;
    }

    static_detour! {
        static QueryPerformanceCounterHook: unsafe extern "system" fn(*mut i64) -> BOOL;
    }

    fn get_tick_count_detour() -> u32 {
        // SAFETY: `GetTickCountHook` stores a valid signature-proven function pointer.
        let current = unsafe { GetTickCountHook.call() };
        super::adjust_tick_count(current)
    }

    fn query_performance_counter_detour(perf_counter: *mut i64) -> BOOL {
        // SAFETY: `QueryPerformanceCounterHook` stores a valid signature-proven
        // function pointer; the nullable pointer contract comes from the Win32 API.
        let result = unsafe { QueryPerformanceCounterHook.call(perf_counter) };
        if result.as_bool() && !perf_counter.is_null() {
            // SAFETY: perf_counter was validated non-null above.
            unsafe {
                let current = *perf_counter;
                *perf_counter = super::adjust_qpc(current);
            }
        }
        result
    }

    pub fn install() -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            // Cache QPC frequency for accurate tick conversion.
            let mut freq_value = 0i64;
            if windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq_value)
                .is_ok()
                && freq_value > 0
            {
                super::QPC_FREQUENCY_HZ.store(freq_value as u64, Ordering::Release);
            }

            let get_tick_count_addr: GetTickCountFn = std::mem::transmute(
                windows::Win32::System::SystemInformation::GetTickCount as usize,
            );
            GetTickCountHook.initialize(get_tick_count_addr, get_tick_count_detour)?;
            GetTickCountHook.enable()?;

            let qpc_addr: QueryPerformanceCounterFn = std::mem::transmute(
                windows::Win32::System::Performance::QueryPerformanceCounter as usize,
            );
            if let Err(e) =
                QueryPerformanceCounterHook.initialize(qpc_addr, query_performance_counter_detour)
            {
                let _ = GetTickCountHook.disable();
                return Err(e.into());
            }
            if let Err(e) = QueryPerformanceCounterHook.enable() {
                let _ = GetTickCountHook.disable();
                return Err(e.into());
            }
        }

        tracing::info!("Timing correction hooks installed (GetTickCount/QPC)");
        Ok(())
    }

    pub fn remove() {
        unsafe {
            if GetTickCountHook.is_enabled() {
                let _ = GetTickCountHook.disable();
            }
            if QueryPerformanceCounterHook.is_enabled() {
                let _ = QueryPerformanceCounterHook.disable();
            }
        }
        tracing::info!("Timing correction hooks removed");
    }
}

#[cfg(not(windows))]
mod inner {
    pub fn install() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn remove() {
        // no-op on non-Windows
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};

#[cfg(test)]
pub(crate) fn reset_test_state() {
    TIMING_CORRECTION_ENABLED.store(false, Ordering::Release);
    GAME_LOOP_OVERHEAD_NS.store(0, Ordering::Release);
    QPC_FREQUENCY_HZ.store(10_000_000, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjusts_tick_count_without_panic() {
        reset_test_state();
        set_enabled(true);
        let result = adjust_tick_count(150);
        assert!(result <= 150);
    }

    #[test]
    fn adjusts_qpc_without_panic() {
        reset_test_state();
        set_enabled(true);
        GAME_LOOP_OVERHEAD_NS.store(100, Ordering::Release);
        // 100 ns * 10_000_000 Hz / 1_000_000_000 = 1 tick; 1000 - 1 = 999
        let adjusted = adjust_qpc(1_000);
        assert_eq!(adjusted, 999);
    }

    #[test]
    fn does_not_adjust_when_disabled() {
        reset_test_state();
        let tick = adjust_tick_count(1234);
        let qpc = adjust_qpc(1234);
        assert_eq!(tick, 1234);
        assert_eq!(qpc, 1234);
    }

    #[test]
    fn adjust_qpc_saturates_at_zero() {
        reset_test_state();
        set_enabled(true);
        GAME_LOOP_OVERHEAD_NS.store(10_000_000_000, Ordering::Release);
        let adjusted = adjust_qpc(5_000);
        assert_eq!(adjusted, 0);
    }

    #[test]
    fn adjust_qpc_zero_or_negative_returns_zero() {
        reset_test_state();
        set_enabled(true);
        GAME_LOOP_OVERHEAD_NS.store(1_000_000, Ordering::Release);
        assert_eq!(adjust_qpc(0), 0);
        assert_eq!(adjust_qpc(-1), 0);
        assert_eq!(adjust_qpc(-100), 0);
    }

    #[test]
    fn record_game_loop_hook_overhead_affects_adjustment() {
        reset_test_state();
        set_enabled(true);

        // Record 10ms = 10_000_000 ns overhead
        record_game_loop_hook_overhead(std::time::Duration::from_millis(10));

        let raw: u32 = 1000;
        let adjusted = adjust_tick_count(raw);
        // overhead_ms = 10_000_000 / 1_000_000 = 10 ms
        assert_eq!(adjusted, 990);
    }

    #[test]
    fn record_game_loop_hook_overhead_zero_duration() {
        reset_test_state();
        set_enabled(true);
        record_game_loop_hook_overhead(std::time::Duration::ZERO);
        // Zero overhead → no adjustment
        assert_eq!(adjust_tick_count(500), 500);
    }

    #[test]
    fn adjust_tick_count_saturates_at_zero() {
        reset_test_state();
        set_enabled(true);
        // Large overhead → should saturate at 0
        GAME_LOOP_OVERHEAD_NS.store(u64::MAX, Ordering::Release);
        let adjusted = adjust_tick_count(100);
        assert_eq!(adjusted, 0);
    }

    #[test]
    fn overhead_as_qpc_ticks_uses_frequency() {
        reset_test_state();
        set_enabled(true);
        // freq = 1 Hz, overhead = 1_000_000_000 ns → 1 tick
        QPC_FREQUENCY_HZ.store(1, Ordering::Release);
        GAME_LOOP_OVERHEAD_NS.store(1_000_000_000, Ordering::Release);
        let adjusted = adjust_qpc(10);
        assert_eq!(adjusted, 9);
    }
}
