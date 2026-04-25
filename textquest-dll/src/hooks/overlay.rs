//! Direct3D overlay hook state — frame timing, render-target tracking, and
//! backend-aware Present integration for the EQ in-process overlay.
//!
//! ## Architecture
//!
//! `overlay` is a thin state layer driven by Direct3D Present hooks:
//! - `initialize_for_backend(...)` — called once when a D3D device is available.
//! - `render_present(...)` — called each Present; updates frame/render timing.
//! - `on_device_reset(...)` — called when the swap chain/backbuffer is recreated.
//! - `shutdown()` — releases resources and resets state.
//!
//! ## Renderer integration
//!
//! The Windows `inner` module is the platform seam for ImGui/custom-widget
//! drawing. It is intentionally dependency-light here; backend crates can be
//! added behind this API without changing the Present hook contract.

use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

use crate::overlay::{manager::WindowManager, theme::Theme, window::Window};

/// Rolling frame window size for FPS / peak-time averaging.
const FRAME_WINDOW: usize = 64;
const DEFAULT_HUD_WINDOW_ID: &str = "textquest_hud";

/// Overlay render budget: keep CPU-side overlay work under 1ms per Present.
pub const OVERLAY_RENDER_BUDGET_NS: u64 = 1_000_000;

/// Overlay lifecycle state (encoded as u32).
/// 0 = Uninitialized, 1 = Initializing, 2 = Active, 3 = Suspended.
static OVERLAY_STATE: AtomicU32 = AtomicU32::new(0);

/// Active Direct3D backend (encoded as u32; 0 = unknown, 11 = DX11, 12 = DX12).
static ACTIVE_BACKEND: AtomicU32 = AtomicU32::new(0);

/// Last known backbuffer width.
static LAST_WIDTH: AtomicU32 = AtomicU32::new(0);

/// Last known backbuffer height.
static LAST_HEIGHT: AtomicU32 = AtomicU32::new(0);

/// Last known number of overlay render targets.
static LAST_RENDER_TARGET_COUNT: AtomicU32 = AtomicU32::new(0);

/// Set when a resolution change is detected; cleared by `take_resolution_changed`.
static RESOLUTION_CHANGED: AtomicBool = AtomicBool::new(false);

/// Nanosecond timestamp of the last `tick()` call (wall-clock since UNIX_EPOCH).
static LAST_TICK_NS: AtomicU64 = AtomicU64::new(0);

/// Most recent frame delta in nanoseconds.
static LAST_FRAME_DELTA_NS: AtomicU64 = AtomicU64::new(0);

/// Most recent CPU-side overlay render time in nanoseconds.
static LAST_RENDER_NS: AtomicU64 = AtomicU64::new(0);

/// Count of frames that exceeded [`OVERLAY_RENDER_BUDGET_NS`].
static OVER_BUDGET_FRAMES: AtomicU64 = AtomicU64::new(0);

// ─── Frame context and result types ──────────────────────────────────────────

/// Input context for a single overlay frame (one Direct3D Present call).
#[derive(Debug, Clone, Copy)]
pub struct OverlayFrameContext {
    /// Which Direct3D backend is active.
    pub backend: Direct3DBackend,
    /// Current backbuffer width in pixels.
    pub width: u32,
    /// Current backbuffer height in pixels.
    pub height: u32,
    /// Number of active render targets tracked by the overlay.
    pub render_target_count: u32,
    /// Whether the swap chain is currently in exclusive fullscreen mode.
    pub fullscreen: bool,
}

impl OverlayFrameContext {
    /// Construct an `OverlayFrameContext` for a DX11 Present with the given
    /// backbuffer dimensions and a single render target.
    pub fn dx11(width: u32, height: u32) -> Self {
        Self {
            backend: Direct3DBackend::Dx11,
            width,
            height,
            render_target_count: 1,
            fullscreen: false,
        }
    }

    /// Override the render-target count (used by swap chains with > 1 buffer).
    pub fn with_render_targets(mut self, count: u32) -> Self {
        self.render_target_count = count;
        self
    }
}

/// Result returned from [`render_present`] after one overlay frame.
#[derive(Debug, Clone, Copy)]
pub struct OverlayFrameResult {
    /// Backend that was active for this frame.
    pub backend: Direct3DBackend,
    /// CPU-side time spent on overlay rendering, in nanoseconds.
    pub render_time_ns: u64,
    /// Number of render targets active during this frame.
    pub render_target_count: u32,
    /// `true` if the overlay CPU work exceeded [`OVERLAY_RENDER_BUDGET_NS`].
    pub over_budget: bool,
}

// ─── Render pipeline ──────────────────────────────────────────────────────────

/// Minimal pipeline contract: accept a frame context and produce a result.
trait RenderPipeline: Send {
    fn sync_targets(&mut self, frame: &OverlayFrameContext);
    fn render(&mut self, frame: &OverlayFrameContext) -> OverlayFrameResult;
}

struct NullPipeline;

impl RenderPipeline for NullPipeline {
    fn sync_targets(&mut self, _frame: &OverlayFrameContext) {}
    fn render(&mut self, frame: &OverlayFrameContext) -> OverlayFrameResult {
        let start = std::time::Instant::now();
        let render_time_ns = start.elapsed().as_nanos() as u64;
        OverlayFrameResult {
            backend: frame.backend,
            render_time_ns,
            render_target_count: frame.render_target_count.max(1),
            over_budget: false,
        }
    }
}

static PIPELINE: OnceLock<Mutex<Box<dyn RenderPipeline>>> = OnceLock::new();

fn pipeline() -> &'static Mutex<Box<dyn RenderPipeline>> {
    PIPELINE.get_or_init(|| Mutex::new(Box::new(NullPipeline)))
}

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

/// Direct3D runtime that supplied the Present callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direct3DBackend {
    Dx11,
    Dx12,
}

impl Direct3DBackend {
    fn encode(self) -> u32 {
        match self {
            Self::Dx11 => 11,
            Self::Dx12 => 12,
        }
    }

    fn decode(v: u32) -> Option<Self> {
        match v {
            11 => Some(Self::Dx11),
            12 => Some(Self::Dx12),
            _ => None,
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

/// Return the active Direct3D backend, if one has been initialized.
pub fn active_backend() -> Option<Direct3DBackend> {
    Direct3DBackend::decode(ACTIVE_BACKEND.load(Ordering::Acquire))
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
    /// Current Direct3D backend, if known.
    pub backend: Option<Direct3DBackend>,
    /// Number of active render targets tracked by the overlay.
    pub render_target_count: u32,
    /// Most recent CPU-side overlay render time.
    pub last_render_ns: u64,
    /// Frames whose CPU-side overlay work exceeded 1ms.
    pub over_budget_frames: u64,
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
        backend: active_backend(),
        render_target_count: LAST_RENDER_TARGET_COUNT.load(Ordering::Acquire),
        last_render_ns: LAST_RENDER_NS.load(Ordering::Acquire),
        over_budget_frames: OVER_BUDGET_FRAMES.load(Ordering::Acquire),
    }
}

// ─── HUD window runtime ──────────────────────────────────────────────────────

/// Minimal backend contract used by the DX11 hook until the ImGui renderer is
/// linked in. Production builds use a no-op backend; the hook still owns window
/// state, frame dispatch, and persistence-ready layout data.
pub trait HudRenderer: Send {
    fn render_hud(&mut self, windows: &WindowManager, perf: &OverlayPerfSnapshot);
}

#[derive(Default)]
struct NullHudRenderer;

impl HudRenderer for NullHudRenderer {
    fn render_hud(&mut self, _windows: &WindowManager, _perf: &OverlayPerfSnapshot) {}
}

struct HudRuntime {
    windows: WindowManager,
    renderer: Box<dyn HudRenderer>,
    rendered_frames: u64,
}

impl Default for HudRuntime {
    fn default() -> Self {
        Self {
            windows: WindowManager::new(Theme::Dark),
            renderer: Box::<NullHudRenderer>::default(),
            rendered_frames: 0,
        }
    }
}

impl HudRuntime {
    fn ensure_default_window(&mut self) {
        if self.windows.get_window(DEFAULT_HUD_WINDOW_ID).is_none() {
            self.windows.add_window(default_hud_window());
        }
    }

    fn render_frame(&mut self, perf: &OverlayPerfSnapshot) {
        self.ensure_default_window();
        self.rendered_frames = self.rendered_frames.saturating_add(1);
        self.renderer.render_hud(&self.windows, perf);
    }

    fn snapshot(&self) -> HudRuntimeSnapshot {
        HudRuntimeSnapshot {
            window_count: self.windows.windows().len(),
            theme: self.windows.theme,
            rendered_frames: self.rendered_frames,
            has_default_hud_window: self.windows.get_window(DEFAULT_HUD_WINDOW_ID).is_some(),
        }
    }
}

static HUD_RUNTIME: OnceLock<Mutex<HudRuntime>> = OnceLock::new();

fn hud_runtime() -> &'static Mutex<HudRuntime> {
    HUD_RUNTIME.get_or_init(|| Mutex::new(HudRuntime::default()))
}

fn default_hud_window() -> Window {
    let mut window = Window::new(DEFAULT_HUD_WINDOW_ID, "TextQuest HUD");
    window.x = 24.0;
    window.y = 24.0;
    window.width = 320.0;
    window.height = 180.0;
    window.closeable = false;
    window
}

fn render_hud_frame() {
    let perf = perf_snapshot();
    let mut runtime = hud_runtime().lock().unwrap_or_else(|e| e.into_inner());
    runtime.render_frame(&perf);
}

/// Snapshot the current HUD runtime wiring for diagnostics and tests.
#[derive(Debug, Clone, PartialEq)]
pub struct HudRuntimeSnapshot {
    pub window_count: usize,
    pub theme: Theme,
    pub rendered_frames: u64,
    pub has_default_hud_window: bool,
}

pub fn hud_snapshot() -> HudRuntimeSnapshot {
    let runtime = hud_runtime().lock().unwrap_or_else(|e| e.into_inner());
    runtime.snapshot()
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialize the overlay with the swap chain's current backbuffer dimensions.
///
/// Called from `dx11_null::hook_device_from_swap_chain` on the first Present.
pub fn initialize(width: u32, height: u32) {
    initialize_for_backend(Direct3DBackend::Dx11, width, height, 1, false);
}

/// Initialize the overlay for a specific Direct3D backend.
pub fn initialize_for_backend(
    backend: Direct3DBackend,
    width: u32,
    height: u32,
    render_target_count: u32,
    fullscreen: bool,
) {
    set_state(OverlayState::Initializing);

    ACTIVE_BACKEND.store(backend.encode(), Ordering::Release);
    LAST_WIDTH.store(width, Ordering::Release);
    LAST_HEIGHT.store(height, Ordering::Release);
    LAST_RENDER_TARGET_COUNT.store(render_target_count.max(1), Ordering::Release);

    let frame = OverlayFrameContext {
        backend,
        width,
        height,
        render_target_count: render_target_count.max(1),
        fullscreen,
    };
    if let Ok(mut guard) = pipeline().lock() {
        guard.sync_targets(&frame);
    }

    hud_runtime()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .ensure_default_window();

    #[cfg(windows)]
    inner::init_render_context(
        backend,
        width,
        height,
        frame.render_target_count,
        fullscreen,
    );

    set_state(OverlayState::Active);
    tracing::info!(
        ?backend,
        width,
        height,
        render_targets = frame.render_target_count,
        fullscreen,
        "Overlay initialized"
    );
}

/// Tick the overlay — call once per Present.
///
/// Legacy entry point: updates frame timing and drives rendering for callers
/// that cannot pass a full [`OverlayFrameContext`].
pub fn tick() {
    tick_frame_time();

    if state() == OverlayState::Active {
        render_hud_frame();

        #[cfg(windows)]
        inner::render_frame();
    }
}

/// Render one overlay frame from a Direct3D Present callback.
pub fn render_present(frame: OverlayFrameContext) -> OverlayFrameResult {
    tick_frame_time();

    if state() == OverlayState::Uninitialized {
        initialize_for_backend(
            frame.backend,
            frame.width,
            frame.height,
            frame.render_target_count,
            frame.fullscreen,
        );
    } else {
        ACTIVE_BACKEND.store(frame.backend.encode(), Ordering::Release);
        LAST_RENDER_TARGET_COUNT.store(frame.render_target_count.max(1), Ordering::Release);
        if current_resolution() != (frame.width, frame.height) {
            on_resize(frame.width, frame.height);
        }
    }

    let result = if state() == OverlayState::Active {
        let mut guard = pipeline()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.render(&frame)
    } else {
        OverlayFrameResult {
            backend: frame.backend,
            render_time_ns: 0,
            render_target_count: frame.render_target_count.max(1),
            over_budget: false,
        }
    };

    LAST_RENDER_NS.store(result.render_time_ns, Ordering::Release);
    if result.over_budget {
        OVER_BUDGET_FRAMES.fetch_add(1, Ordering::AcqRel);
        tracing::warn!(
            render_time_ns = result.render_time_ns,
            budget_ns = OVERLAY_RENDER_BUDGET_NS,
            "Overlay render exceeded budget"
        );
    }

    result
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

/// Notify the overlay that the Direct3D device or swap-chain targets were reset.
pub fn on_device_reset(
    backend: Direct3DBackend,
    width: u32,
    height: u32,
    render_target_count: u32,
    fullscreen: bool,
) {
    tracing::info!(
        ?backend,
        width,
        height,
        render_targets = render_target_count.max(1),
        fullscreen,
        "Overlay device reset"
    );
    shutdown();
    initialize_for_backend(backend, width, height, render_target_count, fullscreen);
}

/// Shut down the overlay and release render resources.
pub fn shutdown() {
    #[cfg(windows)]
    inner::destroy_render_context();

    set_state(OverlayState::Uninitialized);
    ACTIVE_BACKEND.store(0, Ordering::Release);
    LAST_RENDER_TARGET_COUNT.store(0, Ordering::Release);
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
    use super::Direct3DBackend;

    /// Initialize ImGui DX11 render context.
    ///
    /// Stub — real initialization requires backend-specific ImGui/custom widget
    /// renderer crates.
    pub fn init_render_context(
        backend: Direct3DBackend,
        width: u32,
        height: u32,
        render_target_count: u32,
        fullscreen: bool,
    ) {
        tracing::info!(
            ?backend,
            width,
            height,
            render_targets = render_target_count,
            fullscreen,
            "Overlay render context init deferred"
        );
    }

    /// Render one overlay frame.
    ///
    /// Stub — will call the backend renderer once the widget draw backend lands.
    pub fn render_frame(_backend: Direct3DBackend) {}

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
    ACTIVE_BACKEND.store(0, Ordering::Relaxed);
    LAST_WIDTH.store(0, Ordering::Relaxed);
    LAST_HEIGHT.store(0, Ordering::Relaxed);
    LAST_RENDER_TARGET_COUNT.store(0, Ordering::Relaxed);
    RESOLUTION_CHANGED.store(false, Ordering::Relaxed);
    LAST_TICK_NS.store(0, Ordering::Relaxed);
    LAST_FRAME_DELTA_NS.store(0, Ordering::Relaxed);
    LAST_RENDER_NS.store(0, Ordering::Relaxed);
    OVER_BUDGET_FRAMES.store(0, Ordering::Relaxed);
    if let Ok(mut ring) = frame_ring().lock() {
        ring.buf = [0u64; FRAME_WINDOW];
        ring.head = 0;
        ring.count = 0;
    }
    if let Some(runtime) = HUD_RUNTIME.get() {
        *runtime.lock().unwrap_or_else(|e| e.into_inner()) = HudRuntime::default();
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
    fn initialize_seeds_default_hud_window() {
        let _guard = test_state_lock();
        reset_test_state();
        assert_eq!(hud_snapshot().window_count, 0);

        initialize(1280, 720);
        let snapshot = hud_snapshot();

        assert_eq!(snapshot.window_count, 1);
        assert_eq!(snapshot.theme, Theme::Dark);
        assert!(snapshot.has_default_hud_window);
    }

    #[test]
    fn active_tick_renders_hud_frame() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize(1280, 720);

        tick();

        assert_eq!(hud_snapshot().rendered_frames, 1);
    }

    #[test]
    fn suspended_tick_does_not_render_hud_frame() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize(1280, 720);
        suspend();

        tick();

        assert_eq!(hud_snapshot().rendered_frames, 0);
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
    fn initialize_for_backend_tracks_targets() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize_for_backend(Direct3DBackend::Dx12, 2560, 1440, 3, true);
        let snap = perf_snapshot();
        assert_eq!(state(), OverlayState::Active);
        assert_eq!(snap.backend, Some(Direct3DBackend::Dx12));
        assert_eq!(snap.render_target_count, 3);
    }

    #[test]
    fn render_present_records_cpu_budget() {
        let _guard = test_state_lock();
        reset_test_state();
        let result = render_present(OverlayFrameContext::dx11(1920, 1080).with_render_targets(2));
        let snap = perf_snapshot();
        assert_eq!(result.render_target_count, 2);
        assert_eq!(snap.render_target_count, 2);
        assert!(snap.last_render_ns <= OVERLAY_RENDER_BUDGET_NS);
        assert_eq!(snap.over_budget_frames, 0);
    }

    #[test]
    fn device_reset_reinitializes_overlay_targets() {
        let _guard = test_state_lock();
        reset_test_state();
        initialize_for_backend(Direct3DBackend::Dx11, 1280, 720, 1, false);
        on_device_reset(Direct3DBackend::Dx12, 3840, 2160, 4, true);
        let snap = perf_snapshot();
        assert_eq!(current_resolution(), (3840, 2160));
        assert_eq!(snap.backend, Some(Direct3DBackend::Dx12));
        assert_eq!(snap.render_target_count, 4);
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
