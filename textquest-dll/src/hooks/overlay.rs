//! DirectX 11 overlay hook — frame timing, state management, and resolution
//! tracking for the EQ in-process overlay.
//!
//! ## Architecture
//!
//! `overlay` is a thin state layer driven by `dx11_null::hooked_present`:
//! - `initialize(w, h)` — called once when the DX11 device is first available.
//! - `tick()` — called each Present; updates the frame-time ring buffer.
//! - `on_resize(w, h)` — called when the swap chain backbuffer is resized.
//! - `shutdown()` — releases resources and resets state.
//!
//! ## ImGui integration
//!
//! The Windows `inner` module stubs out ImGui DX11 init/render/shutdown.
//! Real integration is tracked in #1107 (window manager) and requires
//! `imgui` + `imgui-dx11` crates to be added as dependencies.

use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

/// Rolling frame window size for FPS / peak-time averaging.
const FRAME_WINDOW: usize = 64;

/// Overlay lifecycle state (encoded as u32).
/// 0 = Uninitialized, 1 = Initializing, 2 = Active, 3 = Suspended.
static OVERLAY_STATE: AtomicU32 = AtomicU32::new(0);

/// Last known backbuffer width.
static LAST_WIDTH: AtomicU32 = AtomicU32::new(0);

/// Last known backbuffer height.
static LAST_HEIGHT: AtomicU32 = AtomicU32::new(0);

/// Set when a resolution change is detected; cleared by `take_resolution_changed`.
static RESOLUTION_CHANGED: AtomicBool = AtomicBool::new(false);

/// Nanosecond timestamp of the last `tick()` call (wall-clock since UNIX_EPOCH).
static LAST_TICK_NS: AtomicU64 = AtomicU64::new(0);

/// Most recent frame delta in nanoseconds.
static LAST_FRAME_DELTA_NS: AtomicU64 = AtomicU64::new(0);

// ─── State machine ───────────────────────────────────────────────────────────

/// Overlay lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Uninitialized,
    Initializing,
    Active,
    Suspended,
}

impl OverlayState {
    fn encode(self) -> u32 {
        match self {
            Self::Uninitialized => 0,
            Self::Initializing => 1,
            Self::Active => 2,
            Self::Suspended => 3,
        }
    }

    fn decode(v: u32) -> Self {
        match v {
            1 => Self::Initializing,
            2 => Self::Active,
            3 => Self::Suspended,
            _ => Self::Uninitialized,
        }
    }
}

/// Return the current overlay state.
pub fn state() -> OverlayState {
    OverlayState::decode(OVERLAY_STATE.load(Ordering::Acquire))
}

fn set_state(new_state: OverlayState) {
    OVERLAY_STATE.store(new_state.encode(), Ordering::Release);
    tracing::debug!(state = ?new_state, "Overlay state transition");
}

// ─── Frame timing ring buffer ─────────────────────────────────────────────────

struct FrameRing {
    buf: [u64; FRAME_WINDOW],
    head: usize,
    count: usize,
}

impl FrameRing {
    const fn new() -> Self {
        Self {
            buf: [0u64; FRAME_WINDOW],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, delta_ns: u64) {
        self.buf[self.head] = delta_ns;
        self.head = (self.head + 1) % FRAME_WINDOW;
        if self.count < FRAME_WINDOW {
            self.count += 1;
        }
    }

    fn average_ns(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let sum: u64 = self.buf[..self.count].iter().sum();
        sum / self.count as u64
    }

    fn peak_ns(&self) -> u64 {
        self.buf[..self.count].iter().copied().max().unwrap_or(0)
    }

    fn fps(&self) -> f32 {
        let avg = self.average_ns();
        if avg == 0 {
            0.0
        } else {
            1_000_000_000.0 / avg as f32
        }
    }
}

static FRAME_TIMES: OnceLock<Mutex<FrameRing>> = OnceLock::new();

fn frame_ring() -> &'static Mutex<FrameRing> {
    FRAME_TIMES.get_or_init(|| Mutex::new(FrameRing::new()))
}

// ─── Performance snapshot ─────────────────────────────────────────────────────

/// Overlay performance snapshot — safe to clone and send over IPC.
#[derive(Debug, Clone)]
pub struct OverlayPerfSnapshot {
    /// Most recent frame delta in nanoseconds.
    pub last_frame_ns: u64,
    /// Frames-per-second derived from the rolling window average.
    pub fps: f32,
    /// Peak frame time in the rolling window (nanoseconds).
    pub peak_frame_ns: u64,
    /// Current backbuffer width.
    pub width: u32,
    /// Current backbuffer height.
    pub height: u32,
}

/// Snapshot current performance metrics.
pub fn perf_snapshot() -> OverlayPerfSnapshot {
    let ring = frame_ring().lock().unwrap_or_else(|e| e.into_inner());
    OverlayPerfSnapshot {
        last_frame_ns: LAST_FRAME_DELTA_NS.load(Ordering::Acquire),
        fps: ring.fps(),
        peak_frame_ns: ring.peak_ns(),
        width: LAST_WIDTH.load(Ordering::Acquire),
        height: LAST_HEIGHT.load(Ordering::Acquire),
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialize the overlay with the swap chain's current backbuffer dimensions.
///
/// Called from `dx11_null::hook_device_from_swap_chain` on the first Present.
pub fn initialize(width: u32, height: u32) {
    set_state(OverlayState::Initializing);

    LAST_WIDTH.store(width, Ordering::Release);
    LAST_HEIGHT.store(height, Ordering::Release);

    #[cfg(windows)]
    inner::init_render_context(width, height);

    set_state(OverlayState::Active);
    tracing::info!(width, height, "Overlay initialized");
}

/// Tick the overlay — call once per Present.
///
/// Updates the frame-time ring buffer and drives rendering when `Active`.
pub fn tick() {
    tick_frame_time();

    #[cfg(windows)]
    if state() == OverlayState::Active {
        inner::render_frame();
    }
}

/// Notify the overlay that the swap chain backbuffer was resized.
pub fn on_resize(width: u32, height: u32) {
    let prev_w = LAST_WIDTH.swap(width, Ordering::AcqRel);
    let prev_h = LAST_HEIGHT.swap(height, Ordering::AcqRel);

    if prev_w != width || prev_h != height {
        RESOLUTION_CHANGED.store(true, Ordering::Release);
        tracing::info!(
            from_width = prev_w,
            from_height = prev_h,
            to_width = width,
            to_height = height,
            "Overlay: backbuffer resized"
        );

        #[cfg(windows)]
        if state() == OverlayState::Active {
            inner::handle_resize(width, height);
        }
    }
}

/// Shut down the overlay and release render resources.
pub fn shutdown() {
    #[cfg(windows)]
    inner::destroy_render_context();

    set_state(OverlayState::Uninitialized);
    tracing::info!("Overlay shut down");
}

/// Suspend overlay rendering (e.g., during zone transitions).
pub fn suspend() {
    if state() == OverlayState::Active {
        set_state(OverlayState::Suspended);
        tracing::debug!("Overlay suspended");
    }
}

/// Resume overlay rendering after a suspension.
pub fn resume() {
    if state() == OverlayState::Suspended {
        set_state(OverlayState::Active);
        tracing::debug!("Overlay resumed");
    }
}

/// Check and clear the resolution-changed flag.
pub fn take_resolution_changed() -> bool {
    RESOLUTION_CHANGED.swap(false, Ordering::AcqRel)
}

/// Return the current backbuffer dimensions (last seen).
pub fn current_resolution() -> (u32, u32) {
    (
        LAST_WIDTH.load(Ordering::Acquire),
        LAST_HEIGHT.load(Ordering::Acquire),
    )
}

/// Return the most recent frame delta in nanoseconds.
pub fn last_frame_delta_ns() -> u64 {
    LAST_FRAME_DELTA_NS.load(Ordering::Acquire)
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn tick_frame_time() {
    let now_ns = wall_clock_ns();
    let prev = LAST_TICK_NS.swap(now_ns, Ordering::AcqRel);
    if prev > 0 && now_ns > prev {
        let delta = now_ns - prev;
        LAST_FRAME_DELTA_NS.store(delta, Ordering::Release);
        if let Ok(mut ring) = frame_ring().lock() {
            ring.push(delta);
        }
    }
}

fn wall_clock_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

// ─── Windows render context stubs ─────────────────────────────────────────────

#[cfg(windows)]
mod inner {
    /// Initialize ImGui DX11 render context.
    ///
    /// Stub — real initialization requires `imgui` + `imgui-dx11` crates
    /// (tracked in #1107).
    pub fn init_render_context(width: u32, height: u32) {
        tracing::info!(
            width,
            height,
            "Overlay render context: ImGui DX11 init deferred (see #1107)"
        );
    }

    /// Render one overlay frame.
    ///
    /// Stub — will call `imgui_dx11::render()` once #1107 is implemented.
    pub fn render_frame() {}

    /// Handle a backbuffer resize — tear down and recreate render targets.
    ///
    /// Stub — will invalidate ImGui font/render target on resize.
    pub fn handle_resize(width: u32, height: u32) {
        tracing::info!(
            width,
            height,
            "Overlay: resize stub — recreate targets here"
        );
    }

    /// Destroy ImGui DX11 render context.
    ///
    /// Stub — will call `imgui_dx11::shutdown()` once #1107 is implemented.
    pub fn destroy_render_context() {
        tracing::info!("Overlay render context: shutdown stub");
    }
}

// ─── Test helpers ─────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) fn reset_test_state() {
    OVERLAY_STATE.store(0, Ordering::Relaxed);
    LAST_WIDTH.store(0, Ordering::Relaxed);
    LAST_HEIGHT.store(0, Ordering::Relaxed);
    RESOLUTION_CHANGED.store(false, Ordering::Relaxed);
    LAST_TICK_NS.store(0, Ordering::Relaxed);
    LAST_FRAME_DELTA_NS.store(0, Ordering::Relaxed);
    if let Ok(mut ring) = frame_ring().lock() {
        ring.buf = [0u64; FRAME_WINDOW];
        ring.head = 0;
        ring.count = 0;
    }
}

#[cfg(test)]
pub(crate) fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("overlay test state lock poisoned")
}

#[cfg(test)]
pub(crate) fn inject_frame_deltas(deltas_ns: &[u64]) {
    if let Ok(mut ring) = frame_ring().lock() {
        for &d in deltas_ns {
            ring.push(d);
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_uninitialized() {
        let _guard = test_state_lock();
        reset_test_state();
        assert_eq!(state(), OverlayState::Uninitialized);
    }

    #[test]
    fn state_encode_decode_round_trip() {
        let _guard = test_state_lock();
        reset_test_state();
        for s in [
            OverlayState::Uninitialized,
            OverlayState::Initializing,
            OverlayState::Active,
            OverlayState::Suspended,
        ] {
            set_state(s);
            assert_eq!(state(), s);
        }
    }

    #[test]
    fn initialize_transitions_to_active() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize(1920, 1080);
        assert_eq!(state(), OverlayState::Active);
        assert_eq!(current_resolution(), (1920, 1080));
    }

    #[test]
    fn shutdown_resets_to_uninitialized() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize(1920, 1080);
        shutdown();
        assert_eq!(state(), OverlayState::Uninitialized);
    }

    #[test]
    fn suspend_resume_cycle() {
        let _guard = test_state_lock();
        reset_test_state();
        set_state(OverlayState::Active);
        suspend();
        assert_eq!(state(), OverlayState::Suspended);
        resume();
        assert_eq!(state(), OverlayState::Active);
    }

    #[test]
    fn suspend_noop_when_not_active() {
        let _guard = test_state_lock();
        reset_test_state();
        set_state(OverlayState::Uninitialized);
        suspend();
        assert_eq!(state(), OverlayState::Uninitialized);
    }

    #[test]
    fn resume_noop_when_not_suspended() {
        let _guard = test_state_lock();
        reset_test_state();
        set_state(OverlayState::Active);
        resume();
        assert_eq!(state(), OverlayState::Active);
    }

    #[test]
    fn on_resize_sets_resolution_changed_flag() {
        let _guard = test_state_lock();
        reset_test_state();
        LAST_WIDTH.store(1920, Ordering::Relaxed);
        LAST_HEIGHT.store(1080, Ordering::Relaxed);
        on_resize(2560, 1440);
        assert!(take_resolution_changed());
        assert_eq!(current_resolution(), (2560, 1440));
    }

    #[test]
    fn on_resize_noop_when_same_dimensions() {
        let _guard = test_state_lock();
        reset_test_state();
        LAST_WIDTH.store(1920, Ordering::Relaxed);
        LAST_HEIGHT.store(1080, Ordering::Relaxed);
        on_resize(1920, 1080);
        assert!(!take_resolution_changed());
    }

    #[test]
    fn take_resolution_changed_clears_flag() {
        let _guard = test_state_lock();
        reset_test_state();
        RESOLUTION_CHANGED.store(true, Ordering::Relaxed);
        assert!(take_resolution_changed());
        assert!(!take_resolution_changed());
    }

    #[test]
    fn fps_computed_correctly_from_injected_frames() {
        let _guard = test_state_lock();
        reset_test_state();
        // 60 fps = ~16.667ms per frame.
        let frame_ns = 16_666_667u64;
        inject_frame_deltas(&[frame_ns; 32]);
        let snap = perf_snapshot();
        assert!(
            snap.fps > 57.0 && snap.fps < 63.0,
            "expected ~60fps, got {}",
            snap.fps
        );
        assert_eq!(snap.peak_frame_ns, frame_ns);
    }

    #[test]
    fn fps_zero_when_no_frames_recorded() {
        let _guard = test_state_lock();
        reset_test_state();
        let snap = perf_snapshot();
        assert_eq!(snap.fps, 0.0);
        assert_eq!(snap.peak_frame_ns, 0);
    }

    #[test]
    fn perf_snapshot_peak_reflects_worst_frame() {
        let _guard = test_state_lock();
        reset_test_state();
        inject_frame_deltas(&[8_000_000, 16_000_000, 33_000_000, 12_000_000]);
        let snap = perf_snapshot();
        assert_eq!(snap.peak_frame_ns, 33_000_000);
    }

    #[test]
    fn frame_ring_wraps_on_overflow() {
        let _guard = test_state_lock();
        reset_test_state();
        // Push more than FRAME_WINDOW entries; count should saturate at FRAME_WINDOW.
        let deltas: Vec<u64> = (0..FRAME_WINDOW + 10)
            .map(|i| (i as u64 + 1) * 1_000_000)
            .collect();
        inject_frame_deltas(&deltas);
        let ring = frame_ring().lock().unwrap();
        assert_eq!(ring.count, FRAME_WINDOW);
    }

    #[test]
    fn tick_updates_frame_delta_on_second_call() {
        let _guard = test_state_lock();
        reset_test_state();
        // First tick — no previous timestamp, delta stays 0.
        tick();
        assert_eq!(last_frame_delta_ns(), 0);
        // Second tick — delta should be non-zero (wall clock advanced).
        std::thread::sleep(std::time::Duration::from_micros(100));
        tick();
        assert!(
            last_frame_delta_ns() > 0,
            "frame delta should be positive after two ticks"
        );
    }

    #[test]
    fn perf_snapshot_resolution_matches_stored() {
        let _guard = test_state_lock();
        reset_test_state();
        LAST_WIDTH.store(3840, Ordering::Relaxed);
        LAST_HEIGHT.store(2160, Ordering::Relaxed);
        let snap = perf_snapshot();
        assert_eq!(snap.width, 3840);
        assert_eq!(snap.height, 2160);
    }

    #[test]
    fn state_constants_are_distinct() {
        assert_ne!(
            OverlayState::Uninitialized.encode(),
            OverlayState::Initializing.encode()
        );
        assert_ne!(
            OverlayState::Active.encode(),
            OverlayState::Suspended.encode()
        );
        assert_ne!(
            OverlayState::Uninitialized.encode(),
            OverlayState::Active.encode()
        );
    }
}
