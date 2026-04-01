//! Auto-accept dialog handling.
//!
//! Scans for common EQ dialog windows (group invite, trade, task, resurrect, etc.)
//! and automatically clicks the accept/yes button. Similar to `MQ2AutoAccept`.
//!
//! Only active when in-world (local player != null) and enabled via IPC command.
//! Called from the game loop tick every 30 ticks (~1 second) to avoid spam.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether auto-accept is enabled. Toggled via IPC `SetAutoAccept` command.
static AUTO_ACCEPT_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable auto-accept.
pub fn set_enabled(enabled: bool) {
    AUTO_ACCEPT_ENABLED.store(enabled, Ordering::Relaxed);
    tracing::info!(enabled, "Auto-accept dialog handling toggled");
}

/// Check if auto-accept is enabled.
pub fn is_enabled() -> bool {
    AUTO_ACCEPT_ENABLED.load(Ordering::Relaxed)
}

/// Known dialog windows and their accept button SIDL names.
/// Format: (`parent_sidl_name`, `accept_button_sidl_name`)
///
/// These are stable SIDL names from EQ's UI XML definitions.
/// We intentionally exclude dangerous dialogs (delete character, etc.).
const DIALOG_ACCEPT_PAIRS: &[(&str, &str)] = &[
    // Generic yes/no confirmation — used for group invites, raid invites, etc.
    ("ConfirmationDialogBox", "CD_Yes_Button"),
    ("ConfirmationDialogBox", "Yes"),
    // Large dialog variant
    ("LargeDialogWindow", "LDW_YesButton"),
    // Trade window — accept trade
    ("TradeWnd", "TRDW_Trade_Button"),
    // Task/expedition confirmation
    ("TaskSelectWnd", "TaskSelectAcceptButton"),
    // Resurrect/respawn
    ("RespawnWnd", "RW_SelectButton"),
    // Loot confirmation (rolling on items)
    ("ConfirmationDialogBox", "CD_Yes_Button"),
];

/// Scan for visible dialogs and auto-click the accept button.
///
/// Must be called from the game loop thread (button clicks use vtable calls).
/// Only scans when in-world and auto-accept is enabled.
///
/// # Safety
/// Requires valid eqgame `CXWndManager` pointer. Must be called from game loop thread.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn check_dialogs() {
    if !AUTO_ACCEPT_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
    if eq_base == 0 {
        return;
    }

    // Only scan when in-world (local player exists)
    let local_player =
        dmft_common::offsets::rebase(dmft_common::offsets::PINST_LOCAL_PLAYER, eq_base)
            .map_or(0, |addr| *(addr as *const usize));

    if local_player == 0 {
        return;
    }

    // Get eqgame CXWndManager
    let mgr_ptr_addr =
        match dmft_common::offsets::rebase(dmft_common::offsets::PINST_CXWND_MANAGER, eq_base) {
            Some(addr) => addr,
            None => return,
        };

    let mgr = *(mgr_ptr_addr as *const usize);
    if mgr == 0 {
        return;
    }

    // Use eqgame offsets for window scanning
    use dmft_common::offsets::eqgame as eqg;

    for &(parent_sidl, button_sidl) in DIALOG_ACCEPT_PAIRS {
        // Find the parent dialog window by SIDL name (must be visible)
        let parent = crate::eq::widgets::find_visible_window_by_sidl_name(
            mgr,
            parent_sidl,
            eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            eqg::CXWNDMGR_WINDOWS_ARRAY,
            eqg::CXWNDMGR_WINDOWS_COUNT,
        );

        let Some(parent_wnd) = parent else {
            continue;
        };

        // Find the accept button child by SIDL name
        let button = crate::eq::widgets::find_child_by_sidl_text(parent_wnd, button_sidl);

        let Some(button_wnd) = button else {
            continue;
        };

        // Verify button is visible before clicking
        if !crate::eq::widgets::is_visible(button_wnd) {
            continue;
        }

        tracing::info!(
            parent = parent_sidl,
            button = button_sidl,
            parent_ptr = format!("{:#x}", parent_wnd),
            button_ptr = format!("{:#x}", button_wnd),
            "Auto-accepting dialog"
        );

        crate::eq::widgets::click_button_via_vtable(button_wnd);

        // Only accept one dialog per tick to avoid race conditions
        return;
    }
}

#[cfg(not(windows))]
pub unsafe fn check_dialogs() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    fn auto_accept_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("dialog test lock poisoned")
    }

    #[test]
    fn auto_accept_toggle() {
        let _guard = auto_accept_test_lock();
        set_enabled(true);
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    #[test]
    fn dialog_pairs_not_empty() {
        assert!(!DIALOG_ACCEPT_PAIRS.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn check_dialogs_noop_on_non_windows() {
        unsafe {
            check_dialogs();
        } // should not panic
    }

    #[test]
    fn dialog_pairs_have_two_elements_each() {
        for (parent, button) in DIALOG_ACCEPT_PAIRS {
            assert!(!parent.is_empty(), "parent SIDL name must not be empty");
            assert!(!button.is_empty(), "button SIDL name must not be empty");
        }
    }

    #[test]
    fn dialog_pairs_contain_confirmation_dialog() {
        let has_confirm = DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "ConfirmationDialogBox");
        assert!(has_confirm, "Must have ConfirmationDialogBox pair");
    }

    #[test]
    fn dialog_pairs_contain_trade_window() {
        let has_trade = DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "TradeWnd");
        assert!(has_trade, "Must have TradeWnd pair");
    }

    #[test]
    fn dialog_pairs_contain_respawn_window() {
        let has_respawn = DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "RespawnWnd");
        assert!(has_respawn, "Must have RespawnWnd pair");
    }

    #[test]
    fn auto_accept_starts_enabled() {
        let _guard = auto_accept_test_lock();
        set_enabled(true);
        assert!(is_enabled());
    }
}
