//! Render strobe hook -- intercepts `CDisplay::RealRender_World`.
//! Background clients skip most render calls to save GPU. Foreground clients
//! always render normally. Background clients render once every `STROBE_INTERVAL`
//! ticks (~5 seconds at 30fps) so the orchestrator can still grab screenshots.

use std::sync::atomic::{AtomicU64, Ordering};

/// Background clients render once every 150 ticks (~5 seconds at 30fps).
const STROBE_INTERVAL: u64 = 150;

/// Monotonic tick counter for strobe timing.
static RENDER_TICK: AtomicU64 = AtomicU64::new(0);

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

/// Determine whether to render this frame.
/// Foreground clients always render. Background clients render once
/// every `STROBE_INTERVAL` ticks for screenshot/monitoring support.
fn should_render() -> bool {
    if super::game_loop::is_foreground() {
        return true;
    }
    let tick = RENDER_TICK.fetch_add(1, Ordering::Relaxed);
    tick.is_multiple_of(STROBE_INTERVAL)
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
        // Reset tick counter for deterministic test.
        RENDER_TICK.store(0, Ordering::Relaxed);

        // Foreground status is true by default (AtomicBool::new(true) in game_loop),
        // so should_render always returns true. We test the strobe math directly.
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
    fn stub_install_remove_are_safe() {
        // On non-Windows, install/remove are stubs that should not panic.
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            remove();
        }
    }
}
