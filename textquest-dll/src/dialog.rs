//! Auto-accept dialog handling.
//!
//! Scans a limited allowlist of EQ dialog windows (trade, task) and handles
//! resurrection confirmation popups with a dedicated trust-list and XP policy.
//! Similar to `MQ2AutoAccept` plus `MQ2Rez`.
//!
//! Only active when in-world (local player != null) and enabled via IPC
//! command. Called from the game loop tick every 30 ticks (~1 second) to avoid
//! spam.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex,
    },
    time::{Duration, Instant},
};

use textquest_common::ipc::AutoRezConfig;
/// Whether auto-accept is enabled. Disabled by default; toggled via IPC
/// `SetAutoAccept`.
static AUTO_ACCEPT_ENABLED: AtomicBool = AtomicBool::new(false);
static AUTO_REZ_CONFIG: LazyLock<Mutex<AutoRezConfig>> =
    LazyLock::new(|| Mutex::new(AutoRezConfig::default()));
static PENDING_REZ_OFFER: LazyLock<Mutex<Option<PendingRezOffer>>> =
    LazyLock::new(|| Mutex::new(None));
static RECENT_REZ_CONTEXT: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

const RECENT_REZ_RESPAWN_WINDOW: Duration = Duration::from_secs(10);
const REZ_CONFIRM_DIALOG: &str = "ConfirmationDialogBox";
const REZ_CONFIRM_TEXT_CHILD: &str = "cd_textoutput";
const REZ_YES_BUTTONS: &[&str] = &["Yes_Button", "CD_Yes_Button"];
const REZ_NO_BUTTONS: &[&str] = &["No_Button", "CD_No_Button"];
const GENERIC_DIALOG_ACCEPT_PAIRS: &[(&str, &str)] = &[
    ("TradeWnd", "TRDW_Trade_Button"),
    ("TaskSelectWnd", "TaskSelectAcceptButton"),
    ("RespawnWnd", "RW_SelectButton"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct RezOffer {
    caster_name: String,
    xp_pct: u8,
}

#[derive(Debug, Clone)]
struct PendingRezOffer {
    offer: RezOffer,
    first_seen_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RezDecision {
    Wait,
    Accept,
    Decline,
    Ignore,
}

/// Enable or disable auto-accept.
pub fn set_enabled(enabled: bool) {
    AUTO_ACCEPT_ENABLED.store(enabled, Ordering::Relaxed);
    tracing::info!(enabled, "Auto-accept dialog handling toggled");
}

/// Check if auto-accept is enabled.
pub fn is_enabled() -> bool {
    AUTO_ACCEPT_ENABLED.load(Ordering::Relaxed)
}

/// Replace the current auto-rez configuration.
pub fn set_rez_config(config: AutoRezConfig) {
    let enabled = config.enabled;
    let min_xp_pct = config.min_xp_pct;
    let trust_count = config.trusted_casters.len();
    let delay_ms = config.delay_ms;
    let decline_if_untrusted = config.decline_if_untrusted;

    {
        let mut guard = AUTO_REZ_CONFIG
            .lock()
            .expect("auto rez config lock poisoned");
        *guard = config;
    }

    if !enabled {
        clear_rez_runtime_state();
    }

    tracing::info!(
        enabled,
        min_xp_pct,
        trust_count,
        delay_ms,
        decline_if_untrusted,
        "Auto-rez configuration updated"
    );
}

fn current_rez_config() -> AutoRezConfig {
    AUTO_REZ_CONFIG
        .lock()
        .expect("auto rez config lock poisoned")
        .clone()
}

fn clear_rez_runtime_state() {
    *PENDING_REZ_OFFER
        .lock()
        .expect("pending rez offer lock poisoned") = None;
    *RECENT_REZ_CONTEXT
        .lock()
        .expect("recent rez context lock poisoned") = None;
}

fn mark_recent_rez_context(at: Instant) {
    *RECENT_REZ_CONTEXT
        .lock()
        .expect("recent rez context lock poisoned") = Some(at);
}

fn rez_offer_matches_policy(config: &AutoRezConfig, offer: &RezOffer) -> bool {
    config.enabled
        && offer.xp_pct >= config.min_xp_pct
        && config
            .trusted_casters
            .iter()
            .any(|trusted| trusted.trim().eq_ignore_ascii_case(&offer.caster_name))
}

fn is_rez_offer_text(lower: &str) -> bool {
    lower.contains("resurrect")
        || lower.contains("resurrection")
        || lower.contains("return you to your corpse")
}

fn parse_rez_offer_text(text: &str) -> Option<RezOffer> {
    let stripped = textquest_common::chat::strip_stml(text);
    let normalized = stripped.trim();
    if normalized.is_empty() {
        return None;
    }

    let caster_name = normalized
        .split_whitespace()
        .next()
        .map(|name| {
            name.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        })?
        .to_string();
    if caster_name.is_empty() {
        return None;
    }

    let lower = normalized.to_ascii_lowercase();
    if !is_rez_offer_text(&lower) {
        return None;
    }

    let xp_pct = if lower.contains("return you to your corpse") {
        100
    } else {
        let pct_start = normalized.find('(')?;
        let digits: String = normalized[pct_start + 1..]
            .chars()
            .skip_while(|ch| !ch.is_ascii_digit())
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        let pct = digits.parse::<u8>().ok()?;
        if pct > 100 {
            return None;
        }
        pct
    };

    Some(RezOffer {
        caster_name,
        xp_pct,
    })
}

fn decide_rez_offer(config: &AutoRezConfig, offer: &RezOffer, elapsed_ms: u64) -> RezDecision {
    if !config.enabled {
        return RezDecision::Ignore;
    }

    if elapsed_ms < u64::from(config.delay_ms) {
        return RezDecision::Wait;
    }

    if rez_offer_matches_policy(config, offer) {
        RezDecision::Accept
    } else if config.decline_if_untrusted {
        RezDecision::Decline
    } else {
        RezDecision::Ignore
    }
}

/// Scan for visible dialogs and auto-click the accept button.
///
/// Must be called from the game loop thread (button clicks use vtable calls).
/// Only scans when in-world and auto-accept is enabled.
///
/// # Safety
/// Requires valid eqgame `CXWndManager` pointer. Must be called from game loop
/// thread.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn check_dialogs() {
    let auto_accept_enabled = AUTO_ACCEPT_ENABLED.load(Ordering::Relaxed);
    let rez_config = current_rez_config();
    if !auto_accept_enabled && !rez_config.enabled {
        return;
    }

    let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
    if eq_base == 0 {
        return;
    }

    // Only scan when in-world (local player exists)
    let local_player =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)
            .map_or(0, |addr| *(addr as *const usize));

    if local_player == 0 {
        return;
    }

    // Get eqgame CXWndManager
    let Some(mgr_ptr_addr) =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_CXWND_MANAGER, eq_base)
    else {
        return;
    };

    let mgr = *(mgr_ptr_addr as *const usize);
    if mgr == 0 {
        return;
    }

    if rez_config.enabled {
        if handle_rez_confirmation_dialog(mgr, &rez_config) {
            return;
        }

        if handle_recent_rez_respawn(mgr) {
            return;
        }
    }

    // Use eqgame offsets for window scanning
    use textquest_common::offsets::eqgame as eqg;

    if !auto_accept_enabled {
        return;
    }

    for &(parent_sidl, button_sidl) in GENERIC_DIALOG_ACCEPT_PAIRS {
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

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn handle_rez_confirmation_dialog(mgr: usize, config: &AutoRezConfig) -> bool {
    use textquest_common::offsets::eqgame as eqg;

    let Some(dialog_wnd) = crate::eq::widgets::find_visible_window_by_sidl_name(
        mgr,
        REZ_CONFIRM_DIALOG,
        eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
        eqg::CXWNDMGR_WINDOWS_ARRAY,
        eqg::CXWNDMGR_WINDOWS_COUNT,
    ) else {
        *PENDING_REZ_OFFER
            .lock()
            .expect("pending rez offer lock poisoned") = None;
        return false;
    };

    let Some(dialog_text) = read_rez_confirmation_text(dialog_wnd) else {
        return true;
    };
    let Some(offer) = parse_rez_offer_text(&dialog_text) else {
        return true;
    };

    let now = Instant::now();
    let elapsed_ms = {
        let mut guard = PENDING_REZ_OFFER
            .lock()
            .expect("pending rez offer lock poisoned");
        match guard.as_mut() {
            Some(pending) if pending.offer == offer => {
                pending.first_seen_at.elapsed().as_millis() as u64
            }
            _ => {
                *guard = Some(PendingRezOffer {
                    offer: offer.clone(),
                    first_seen_at: now,
                });
                0
            }
        }
    };

    if rez_offer_matches_policy(config, &offer) {
        mark_recent_rez_context(now);
    }

    match decide_rez_offer(config, &offer, elapsed_ms) {
        RezDecision::Wait | RezDecision::Ignore => true,
        RezDecision::Accept => {
            if click_rez_confirmation_button(dialog_wnd, REZ_YES_BUTTONS) {
                tracing::info!(
                    caster = %offer.caster_name,
                    xp_pct = offer.xp_pct,
                    elapsed_ms,
                    "Accepted resurrection offer"
                );
                mark_recent_rez_context(now);
                *PENDING_REZ_OFFER
                    .lock()
                    .expect("pending rez offer lock poisoned") = None;
            }
            true
        }
        RezDecision::Decline => {
            if click_rez_confirmation_button(dialog_wnd, REZ_NO_BUTTONS) {
                tracing::info!(
                    caster = %offer.caster_name,
                    xp_pct = offer.xp_pct,
                    elapsed_ms,
                    "Declined resurrection offer"
                );
                *PENDING_REZ_OFFER
                    .lock()
                    .expect("pending rez offer lock poisoned") = None;
            }
            true
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_rez_confirmation_text(dialog_wnd: usize) -> Option<String> {
    use textquest_common::offsets::eqmain as off;

    if let Some(text_child) =
        crate::eq::widgets::find_child_by_sidl_text(dialog_wnd, REZ_CONFIRM_TEXT_CHILD)
        && let Some(text) = crate::eq::widgets::read_cxstr(text_child + off::CXWND_WINDOW_TEXT)
    {
        let stripped = textquest_common::chat::strip_stml(&text);
        if !stripped.is_empty() {
            return Some(stripped);
        }
    }

    let mut best_text: Option<String> = None;
    let mut child = *((dialog_wnd + off::CXWND_FIRST_NODE) as *const usize);
    let mut count = 0u32;
    while child != 0 && count < 200 {
        count += 1;
        if let Some(text) = crate::eq::widgets::read_cxstr(child + off::CXWND_WINDOW_TEXT) {
            let stripped = textquest_common::chat::strip_stml(&text);
            if stripped.len() > best_text.as_ref().map_or(0, String::len) {
                best_text = Some(stripped);
            }
        }
        child = *((child + off::CXWND_NEXT) as *const usize);
    }

    best_text.filter(|text| !text.is_empty())
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn click_rez_confirmation_button(dialog_wnd: usize, sidl_names: &[&str]) -> bool {
    for sidl_name in sidl_names {
        if let Some(button_wnd) = crate::eq::widgets::find_child_by_sidl_text(dialog_wnd, sidl_name)
            && crate::eq::widgets::is_visible(button_wnd)
        {
            crate::eq::widgets::click_button_via_vtable(button_wnd);
            return true;
        }
    }

    let button_text = if std::ptr::eq(sidl_names, REZ_YES_BUTTONS) {
        "Yes"
    } else {
        "No"
    };
    if let Some(button_wnd) = crate::eq::widgets::find_child_button_by_text(dialog_wnd, button_text)
    {
        crate::eq::widgets::click_button_via_vtable(button_wnd);
        return true;
    }

    false
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn handle_recent_rez_respawn(mgr: usize) -> bool {
    use textquest_common::offsets::eqgame as eqg;

    {
        let mut guard = RECENT_REZ_CONTEXT
            .lock()
            .expect("recent rez context lock poisoned");
        let Some(context_at) = *guard else {
            return false;
        };
        if context_at.elapsed() > RECENT_REZ_RESPAWN_WINDOW {
            *guard = None;
            return false;
        }
    }

    let Some(respawn_wnd) = crate::eq::widgets::find_visible_window_by_sidl_name(
        mgr,
        "RespawnWnd",
        eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
        eqg::CXWNDMGR_WINDOWS_ARRAY,
        eqg::CXWNDMGR_WINDOWS_COUNT,
    ) else {
        return false;
    };

    let Some(button_wnd) =
        crate::eq::widgets::find_child_by_sidl_text(respawn_wnd, "RW_SelectButton")
    else {
        return false;
    };
    if !crate::eq::widgets::is_visible(button_wnd) {
        return false;
    }

    tracing::info!("Completing resurrection via RespawnWnd");
    crate::eq::widgets::click_button_via_vtable(button_wnd);
    *RECENT_REZ_CONTEXT
        .lock()
        .expect("recent rez context lock poisoned") = None;
    true
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

    #[cfg(not(windows))]
    #[test]
    fn check_dialogs_noop_on_non_windows() {
        unsafe {
            check_dialogs();
        } // should not panic
    }

    #[test]
    fn dialog_pairs_have_two_elements_each() {
        for (parent, button) in GENERIC_DIALOG_ACCEPT_PAIRS {
            assert!(!parent.is_empty(), "parent SIDL name must not be empty");
            assert!(!button.is_empty(), "button SIDL name must not be empty");
        }
    }

    #[test]
    fn dialog_pairs_contain_trade_window() {
        let has_trade = GENERIC_DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "TradeWnd");
        assert!(has_trade, "Must have TradeWnd pair");
    }

    #[test]
    fn generic_dialog_pairs_contain_task_window() {
        let has_task = GENERIC_DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "TaskSelectWnd");
        assert!(has_task, "Must have TaskSelectWnd pair");
    }

    #[test]
    fn generic_dialog_pairs_contain_respawn_window() {
        let has_respawn = GENERIC_DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _)| *parent == "RespawnWnd");
        assert!(has_respawn, "Must have RespawnWnd pair");
    }

    #[test]
    fn auto_accept_starts_disabled() {
        let _guard = auto_accept_test_lock();
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn parse_rez_offer_extracts_caster_and_pct() {
        let offer =
            parse_rez_offer_text("Clericbob tells you, 'Resurrection incoming' (96 percent)")
                .expect("expected rez offer");

        assert_eq!(
            offer,
            RezOffer {
                caster_name: "Clericbob".into(),
                xp_pct: 96,
            }
        );
    }

    #[test]
    fn parse_rez_offer_treats_corpse_return_as_full_rez() {
        let offer = parse_rez_offer_text("Clericbob will return you to your corpse.")
            .expect("expected corpse-return offer");

        assert_eq!(
            offer,
            RezOffer {
                caster_name: "Clericbob".into(),
                xp_pct: 100,
            }
        );
    }

    #[test]
    fn parse_rez_offer_rejects_unrelated_confirmation_text() {
        assert!(
            parse_rez_offer_text("Clericbob invites you to join the raid (96 members).").is_none()
        );
    }

    #[test]
    fn rez_policy_waits_until_delay_expires() {
        let action = decide_rez_offer(
            &textquest_common::ipc::AutoRezConfig {
                enabled: true,
                min_xp_pct: 90,
                trusted_casters: vec!["Clericbob".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
            &RezOffer {
                caster_name: "Clericbob".into(),
                xp_pct: 96,
            },
            5_000,
        );

        assert_eq!(action, RezDecision::Wait);
    }

    #[test]
    fn rez_policy_accepts_trusted_offer_once_delay_elapsed() {
        let action = decide_rez_offer(
            &textquest_common::ipc::AutoRezConfig {
                enabled: true,
                min_xp_pct: 90,
                trusted_casters: vec!["clericbob".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
            &RezOffer {
                caster_name: "ClericBob".into(),
                xp_pct: 96,
            },
            5_100,
        );

        assert_eq!(action, RezDecision::Accept);
    }

    #[test]
    fn rez_policy_declines_untrusted_offer_when_configured() {
        let action = decide_rez_offer(
            &textquest_common::ipc::AutoRezConfig {
                enabled: true,
                min_xp_pct: 90,
                trusted_casters: vec!["Clericbob".into()],
                decline_if_untrusted: true,
                delay_ms: 0,
            },
            &RezOffer {
                caster_name: "Randomcleric".into(),
                xp_pct: 96,
            },
            0,
        );

        assert_eq!(action, RezDecision::Decline);
    }

    #[test]
    fn rez_policy_declines_low_xp_offer_when_configured() {
        let action = decide_rez_offer(
            &textquest_common::ipc::AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Clericbob".into()],
                decline_if_untrusted: true,
                delay_ms: 0,
            },
            &RezOffer {
                caster_name: "Clericbob".into(),
                xp_pct: 90,
            },
            0,
        );

        assert_eq!(action, RezDecision::Decline);
    }

    #[test]
    fn rez_policy_ignores_untrusted_offer_when_decline_disabled() {
        let action = decide_rez_offer(
            &textquest_common::ipc::AutoRezConfig {
                enabled: true,
                min_xp_pct: 90,
                trusted_casters: vec!["Clericbob".into()],
                decline_if_untrusted: false,
                delay_ms: 0,
            },
            &RezOffer {
                caster_name: "Randomcleric".into(),
                xp_pct: 96,
            },
            0,
        );

        assert_eq!(action, RezDecision::Ignore);
    }
}
