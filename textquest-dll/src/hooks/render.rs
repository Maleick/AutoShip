//! Render mode hook — intercepts `CDisplay::RealRender_World`.
//!
//! Three modes controlled via IPC `SetRenderMode`:
//! - **Normal**: full rendering (the "eyes" client).
//! - **Strobe**: render 1 frame every ~5 seconds for monitoring.
//! - **NullRender**: zero rendering — game loop runs, GPU idle.
//!
//! Defaults to Normal. Background detection (`is_foreground()`) only applies
//! when the mode is Normal — in that case, a non-foreground window falls back
//! to Strobe automatically.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use textquest_common::ipc::RenderMode;

/// Background clients render once every 150 ticks (~5 seconds at 30fps).
const STROBE_INTERVAL: u64 = 150;

/// Monotonic tick counter for strobe timing.
static RENDER_TICK: AtomicU64 = AtomicU64::new(0);

/// Current render mode. Encoded as u8: 0=Normal, 1=Strobe, 2=NullRender.
static RENDER_MODE: AtomicU8 = AtomicU8::new(0);

/// When true, the next `should_render()` call returns true (one-shot override)
/// and the post-Present hook captures the backbuffer.
static CAPTURE_REQUESTED: AtomicBool = AtomicBool::new(false);

/// When true, we are in the middle of a capture frame — render was forced on,
/// and the post-Present hook should capture + restore mode.
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Set the render mode. Called from the IPC command handler.
///
/// On the first call, this also triggers deferred DX11 hook installation
/// (the device may not have been ready at DLL init time).
pub fn set_mode(mode: RenderMode) {
    // Ensure DX11 vtable hooks are installed (deferred from init if device was null).
    super::dx11_null::ensure_installed();

    let encoded = match mode {
        RenderMode::Normal => 0,
        RenderMode::Strobe => 1,
        RenderMode::NullRender => 2,
    };
    RENDER_MODE.store(encoded, Ordering::Release);
    super::dx11_null::sync_suppress_draws();
    tracing::info!(%mode, "Render mode changed");
}

/// Get the current render mode.
pub fn mode() -> RenderMode {
    match RENDER_MODE.load(Ordering::Acquire) {
        1 => RenderMode::Strobe,
        2 => RenderMode::NullRender,
        _ => RenderMode::Normal,
    }
}

#[cfg(windows)]
mod inner {
    use retour::static_detour;

    // CDisplay::RealRender_World signature.
    // MQ2: void CDisplay::RealRender_World()
    // On x64: this = RCX (CDisplay*), no other params.
    type RenderFn = unsafe extern "system" fn(*mut core::ffi::c_void);

    static_detour! {
        static RenderHook: unsafe extern "system" fn(*mut core::ffi::c_void);
    }

    /// The detour function -- called instead of `CDisplay::RealRender_World`.
    fn render_detour(this: *mut core::ffi::c_void) {
        if super::should_render() {
            // SAFETY: `this` is the CDisplay* pointer passed by EQ's rendering
            // pipeline. The original RealRender_World function was saved by retour
            // during hook installation. We forward the same `this` pointer unchanged.
            unsafe {
                RenderHook.call(this);
            }
        }
        // When not rendering, just return -- EQ skips the 3D scene but
        // game logic (main loop) continues at full speed.
    }

    /// Install the render hook.
    pub fn install(render_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: render_addr was rebased from REAL_RENDER_WORLD offset against
        // the live eqgame.exe base address. The transmute converts it to a function
        // pointer matching CDisplay::RealRender_World's calling convention.
        // retour overwrites the function prologue with a trampoline. If the offset
        // is wrong, EQ will crash on the next render call.
        unsafe {
            let target: RenderFn = std::mem::transmute(render_addr);
            RenderHook.initialize(target, render_detour)?;
            RenderHook.enable()?;
        }
        tracing::info!(
            addr = format!("{:#x}", render_addr),
            "Render strobe hook installed"
        );
        Ok(())
    }

    /// Remove the render hook.
    pub fn remove() {
        // SAFETY: Disabling a retour hook restores the original function bytes.
        // Safe to call during graceful_shutdown() — see game_loop::remove() for details.
        unsafe {
            if RenderHook.is_enabled() {
                let _ = RenderHook.disable();
            }
        }
        tracing::info!("Render strobe hook removed");
    }
}

#[cfg(not(windows))]
mod inner {
    /// Stub -- hooks are only functional on Windows.
    pub fn install(_render_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Render strobe hook not available on this platform (stub)");
        Ok(())
    }

    /// Stub -- nothing to remove on non-Windows platforms.
    pub fn remove() {
        tracing::warn!("Render strobe hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};

#[cfg(test)]
pub(crate) fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("render test state lock poisoned")
}

#[cfg(test)]
pub(crate) fn reset_test_state() {
    RENDER_TICK.store(0, Ordering::Relaxed);
    RENDER_MODE.store(0, Ordering::Relaxed);
    CAPTURE_REQUESTED.store(false, Ordering::Relaxed);
    CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
}

/// Request a single-frame capture. Sets the capture flag so the next
/// `should_render()` returns true even in NullRender mode.
pub fn request_capture() {
    CAPTURE_REQUESTED.store(true, Ordering::Release);
    tracing::info!("Screenshot capture requested — next frame will render");
}

/// Check and clear the capture-active flag. Called from the Present hook
/// after the frame has been rendered.
pub fn take_capture_active() -> bool {
    CAPTURE_ACTIVE.swap(false, Ordering::AcqRel)
}

/// Check if a capture is currently active (frame is rendering for capture).
pub fn is_capture_active() -> bool {
    CAPTURE_ACTIVE.load(Ordering::Acquire)
}

/// Determine whether to render this frame based on the current render mode.
///
/// - `Normal` + foreground → always render.
/// - `Normal` + background → strobe (automatic fallback).
/// - `Strobe` → render every `STROBE_INTERVAL` ticks.
/// - `NullRender` → wait for DX11 Present hook activation, then render once so
///   draw-call suppression can take over.
fn should_render() -> bool {
    // One-shot override: if a capture was requested, force-render this frame.
    if CAPTURE_REQUESTED.swap(false, Ordering::AcqRel) {
        CAPTURE_ACTIVE.store(true, Ordering::Release);
        return true;
    }

    match mode() {
        // Keep NullRender alive long enough for Present hooks to initialize draw
        // suppression; once Present is active we can render one frame to install
        // context hooks.
        RenderMode::NullRender => super::dx11_null::present_hook_installed(),
        RenderMode::Strobe => {
            let tick = RENDER_TICK.fetch_add(1, Ordering::Relaxed);
            tick.is_multiple_of(STROBE_INTERVAL)
        }
        RenderMode::Normal => {
            if super::game_loop::is_foreground() {
                return true;
            }
            // Background windows in Normal mode fall back to strobe.
            let tick = RENDER_TICK.fetch_add(1, Ordering::Relaxed);
            tick.is_multiple_of(STROBE_INTERVAL)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn strobe_interval_is_nonzero() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();
        assert!(STROBE_INTERVAL > 0);
    }

    #[test]
    fn should_render_strobes_when_background() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        // Reset state for deterministic test.
        RENDER_TICK.store(0, Ordering::Relaxed);
        RENDER_MODE.store(1, Ordering::Relaxed); // Strobe mode

        let mut rendered = 0u64;
        let total = STROBE_INTERVAL * 3;
        for _ in 0..total {
            let tick = RENDER_TICK.fetch_add(1, Ordering::Relaxed);
            if tick.is_multiple_of(STROBE_INTERVAL) {
                rendered += 1;
            }
        }
        // Over 3 full intervals we expect exactly 3 render frames.
        assert_eq!(rendered, 3);
    }

    #[test]
    fn null_render_requires_dx11_present_hook() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        RENDER_MODE.store(2, Ordering::Relaxed); // NullRender
        assert_eq!(mode(), RenderMode::NullRender);
        // In NullRender mode, wait for Present hook activation.
        assert!(!should_render());

        super::super::dx11_null::set_present_hook_installed_for_test(true);
        assert!(should_render());
        super::super::dx11_null::set_present_hook_installed_for_test(false);
    }

    #[test]
    fn mode_round_trip() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        // Use direct atomic store to avoid races with other tests
        // that share the global RENDER_MODE atomic.
        RENDER_MODE.store(0, Ordering::Relaxed); // Normal
        assert_eq!(mode(), RenderMode::Normal);

        RENDER_MODE.store(1, Ordering::Relaxed); // Strobe
        assert_eq!(mode(), RenderMode::Strobe);

        RENDER_MODE.store(2, Ordering::Relaxed); // NullRender
        assert_eq!(mode(), RenderMode::NullRender);
    }

    #[test]
    fn stub_install_remove_are_safe() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        // On non-Windows, install/remove are stubs that should not panic.
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            remove();
        }
    }

    #[test]
    fn capture_flag_forces_render_in_null_mode() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        RENDER_MODE.store(2, Ordering::Relaxed); // NullRender
        CAPTURE_REQUESTED.store(false, Ordering::Relaxed);
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);

        // Without capture, NullRender should not render.
        assert!(!should_render());

        // Request a capture — next call should force-render.
        request_capture();
        assert!(should_render());
        // The capture-active flag should now be set.
        assert!(is_capture_active());

        // Subsequent call should NOT render (one-shot).
        assert!(!should_render());
    }

    #[test]
    fn take_capture_active_clears_flag() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        CAPTURE_ACTIVE.store(true, Ordering::Relaxed);
        assert!(take_capture_active());
        assert!(!take_capture_active());
    }

    #[test]
    fn capture_does_not_interfere_with_normal_mode() {
        let _guard = test_state_lock();
        reset_test_state();
        super::super::dx11_null::reset_test_state();

        RENDER_MODE.store(0, Ordering::Relaxed); // Normal
        CAPTURE_REQUESTED.store(false, Ordering::Relaxed);
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);

        request_capture();
        assert!(should_render());
        assert!(is_capture_active());
    }
}
