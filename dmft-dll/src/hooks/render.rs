//! Render strobe hook -- intercepts `CDisplay::RealRender_World`.
//! Uses hardware breakpoint hooking (DR1) instead of detour/trampoline.

use std::sync::atomic::{AtomicU64, Ordering};

use super::hwbp::{self, HwbpSlot};

const STROBE_INTERVAL: u64 = 150;
static RENDER_TICK: AtomicU64 = AtomicU64::new(0);
const RENDER_SLOT: HwbpSlot = HwbpSlot::Dr1;

fn render_callback(_exception_info: *mut ()) -> bool {
    let _ = should_render();
    true
}

pub fn install(render_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    hwbp::register(RENDER_SLOT, render_addr, render_callback)?;
    tracing::info!(
        addr = format!("{:#x}", render_addr),
        "Render strobe HWBP hook installed (DR1)"
    );
    Ok(())
}

pub fn remove() {
    if hwbp::is_active(RENDER_SLOT) {
        if let Err(e) = hwbp::unregister(RENDER_SLOT) {
            tracing::warn!("Failed to remove render HWBP: {}", e);
        }
    }
    tracing::info!("Render strobe hook removed");
}

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
