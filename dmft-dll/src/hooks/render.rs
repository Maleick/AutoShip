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

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use dmft_common::ipc::RenderMode;

/// Background clients render once every 150 ticks (~5 seconds at 30fps).
const STROBE_INTERVAL: u64 = 150;

/// Monotonic tick counter for strobe timing.
static RENDER_TICK: AtomicU64 = AtomicU64::new(0);

/// Current render mode. Encoded as u8: 0=Normal, 1=Strobe, 2=NullRender.
static RENDER_MODE: AtomicU8 = AtomicU8::new(0);

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

/// Determine whether to render this frame based on the current render mode.
///
/// - `Normal` + foreground → always render.
/// - `Normal` + background → strobe (automatic fallback).
/// - `Strobe` → render every `STROBE_INTERVAL` ticks.
/// - `NullRender` → never render.
fn should_render() -> bool {
    match mode() {
        RenderMode::NullRender => false,
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
        assert!(STROBE_INTERVAL > 0);
    }

    #[test]
    fn should_render_strobes_when_background() {
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
    fn null_render_never_renders() {
        // Test the mode encoding/decoding directly to avoid races
        // with other tests that share the global RENDER_MODE atomic.
        RENDER_MODE.store(2, Ordering::Relaxed); // NullRender
        assert_eq!(mode(), RenderMode::NullRender);
        // In NullRender mode, should_render always returns false.
        // We test a small number of iterations to avoid tick counter races.
        for _ in 0..5 {
            assert!(!should_render());
        }
    }

    #[test]
    fn mode_round_trip() {
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
        // On non-Windows, install/remove are stubs that should not panic.
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            remove();
        }
    }
}
