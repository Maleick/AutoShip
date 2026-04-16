//! Auto-accept dialog handling.
//!
//! Scans a limited allowlist of EQ dialog windows (trade, task) and handles
//! resurrection confirmation popups with a dedicated trust-list, XP, and delay
//! policy.
//! Similar to `MQ2AutoAccept` plus `MQ2Rez`.
//!
//! Only active when in-world (local player != null) and enabled via IPC
//! command. Called from the game loop tick every 30 ticks (~1 second) to avoid
//! spam.

use std::{
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::sync::atomic::Ordering;

use textquest_common::{
    chat::strip_stml,
    ipc::{AutoAcceptRequestKind, AutoAcceptSettings, AutoAcceptTrustMode, AutoRezConfig},
};

/// In-memory auto-accept policy. Updated by IPC commands and read by the game
/// loop.
static AUTO_ACCEPT_SETTINGS: LazyLock<Mutex<AutoAcceptSettings>> =
    LazyLock::new(|| Mutex::new(AutoAcceptSettings::default()));

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoAcceptRequest {
    kind: AutoAcceptRequestKind,
    sender: Option<String>,
    text: String,
}

struct DetectedDialog {
    request: AutoAcceptRequest,
    parent_wnd: usize,
    button_wnd: usize,
    source: &'static str,
}

/// Enable or disable auto-accept.
pub fn set_enabled(enabled: bool) {
    set_settings_with(|settings| settings.enabled = enabled);
    tracing::info!(enabled, "Auto-accept dialog handling toggled");
}

/// Replace the entire auto-accept policy.
pub fn set_settings(settings: AutoAcceptSettings) {
    let enabled = settings.enabled;
    set_settings_with(|state| *state = settings);
    tracing::info!(enabled, "Auto-accept settings updated");
}

/// Check if auto-accept is enabled.
pub fn is_enabled() -> bool {
    settings().enabled
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
    let stripped = strip_stml(text);
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

/// Known dialog windows and their accept button SIDL names.
/// Format: (`parent_sidl_name`, `accept_button_sidl_name`)
///
/// These are stable SIDL names from EQ's UI XML definitions.
/// We intentionally exclude dangerous dialogs (delete character, etc.).
const DIALOG_ACCEPT_PAIRS: &[(&str, &str, AutoAcceptRequestKind)] = &[
    (
        "TradeWnd",
        "TRDW_Trade_Button",
        AutoAcceptRequestKind::Trade,
    ),
    (
        "TaskSelectWnd",
        "TaskSelectAcceptButton",
        AutoAcceptRequestKind::TaskAdd,
    ),
];

const LEGACY_RESPAWN_DIALOG: (&str, &str) = ("RespawnWnd", "RW_SelectButton");

const YES_NO_DIALOG_SIDL: &str = "yesnodialog";
const YES_NO_BUTTON_SIDL: &str = "YESNO_YesButton";

fn settings() -> AutoAcceptSettings {
    AUTO_ACCEPT_SETTINGS
        .lock()
        .expect("auto-accept settings lock poisoned")
        .clone()
}

fn set_settings_with(update: impl FnOnce(&mut AutoAcceptSettings)) {
    let mut settings = AUTO_ACCEPT_SETTINGS
        .lock()
        .expect("auto-accept settings lock poisoned");
    update(&mut settings);
}

fn normalize_player_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn should_accept_request(settings: &AutoAcceptSettings, request: &AutoAcceptRequest) -> bool {
    if !settings.enabled || !settings.is_kind_enabled(request.kind) {
        return false;
    }

    match settings.trust_mode {
        AutoAcceptTrustMode::Anyone => true,
        AutoAcceptTrustMode::TrustList => {
            let Some(sender) = request.sender.as_deref() else {
                return false;
            };
            let normalized_sender = normalize_player_name(sender);
            settings
                .trusted_players
                .iter()
                .map(|name| normalize_player_name(name))
                .any(|trusted| trusted == normalized_sender)
        }
    }
}

fn classify_request_from_text(
    primary_text: &str,
    candidates: &[&str],
) -> Option<AutoAcceptRequest> {
    let mut texts = Vec::new();
    push_candidate_text(&mut texts, primary_text);
    for candidate in candidates {
        push_candidate_text(&mut texts, candidate);
    }

    for text in &texts {
        let lower = text.to_ascii_lowercase();

        if lower.contains("anchor") {
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::Anchor,
                sender: extract_sender(
                    text,
                    &[
                        " would like to send you to their primary anchor",
                        " would like to send you to their secondary anchor",
                        " is attempting to teleport you to their primary anchor",
                        " is attempting to teleport you to their secondary anchor",
                        " would like to teleport you to their primary anchor",
                        " would like to teleport you to their secondary anchor",
                    ],
                ),
                text: text.clone(),
            });
        }

        if lower.contains("transloc")
            || lower.contains("teleport you to")
            || lower.contains("port you to")
        {
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::Translocate,
                sender: extract_sender(
                    text,
                    &[
                        " would like to translocate you",
                        " would like to teleport you to",
                        " would like to port you to",
                    ],
                ),
                text: text.clone(),
            });
        }

        if lower.contains("dynamic zone") || lower.contains(" expedition ") {
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::DzAdd,
                sender: extract_sender(
                    text,
                    &[
                        " has invited you to join the dynamic zone",
                        " has invited you to join the expedition",
                        " invites you to join the dynamic zone",
                        " invites you to join the expedition",
                    ],
                ),
                text: text.clone(),
            });
        }

        if lower.contains(" task ")
            || lower.contains("join the task")
            || lower.contains("join a task")
        {
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::TaskAdd,
                sender: extract_sender(
                    text,
                    &[
                        " has invited you to join the task",
                        " invites you to join the task",
                        " has invited you to join a task",
                        " invites you to join a task",
                    ],
                ),
                text: text.clone(),
            });
        }

        if lower.contains("join a group") || lower.contains("join the group") {
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::GroupInvite,
                sender: extract_sender(
                    text,
                    &[
                        " invites you to join a group",
                        " invites you to join the group",
                        " has invited you to join a group",
                        " has invited you to join the group",
                    ],
                ),
                text: text.clone(),
            });
        }

        if lower.contains("trade with ")
            || lower.contains("trading with ")
            || lower.contains("would like to trade")
            || lower.contains("wants to trade")
            || lower.contains("is trading with")
        {
            let sender = extract_sender(
                text,
                &[
                    " would like to trade with you",
                    " wants to trade with you",
                    " is trading with you",
                ],
            )
            .or_else(|| extract_sender_after_prefix(text, &["Trading with ", "Trade with "]));
            return Some(AutoAcceptRequest {
                kind: AutoAcceptRequestKind::Trade,
                sender,
                text: text.clone(),
            });
        }
    }

    None
}

fn push_candidate_text(texts: &mut Vec<String>, value: &str) {
    let stripped = strip_stml(value).trim().to_string();
    if stripped.is_empty() {
        return;
    }

    if texts
        .iter()
        .all(|existing| !existing.eq_ignore_ascii_case(&stripped))
    {
        texts.push(stripped);
    }
}

fn extract_sender(text: &str, markers: &[&str]) -> Option<String> {
    let normalized = strip_stml(text);
    let lower = normalized.to_ascii_lowercase();

    for marker in markers {
        if let Some(index) = lower.find(marker) {
            let sender = normalized[..index].trim();
            if !sender.is_empty() {
                return Some(sender.to_string());
            }
        }
    }

    None
}

fn extract_sender_after_prefix(text: &str, prefixes: &[&str]) -> Option<String> {
    let normalized = strip_stml(text);
    let lower = normalized.to_ascii_lowercase();

    for prefix in prefixes {
        let prefix_lower = prefix.to_ascii_lowercase();
        if lower.starts_with(&prefix_lower) {
            let sender = normalized[prefix.len()..].trim();
            if !sender.is_empty() {
                return Some(sender.to_string());
            }
        }
    }

    None
}

#[cfg(windows)]
unsafe fn detect_dialog(mgr: usize) -> Option<DetectedDialog> {
    if let Some(dialog) = unsafe { detect_yesno_dialog(mgr) } {
        return Some(dialog);
    }

    unsafe { detect_fixed_dialog(mgr) }
}

#[cfg(windows)]
unsafe fn detect_yesno_dialog(mgr: usize) -> Option<DetectedDialog> {
    use textquest_common::offsets::eqgame as eqg;

    let parent_wnd = unsafe {
        crate::eq::widgets::find_visible_window_by_sidl_name(
            mgr,
            YES_NO_DIALOG_SIDL,
            eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            eqg::CXWNDMGR_WINDOWS_ARRAY,
            eqg::CXWNDMGR_WINDOWS_COUNT,
        )
    }?;
    let button_wnd = unsafe {
        crate::eq::widgets::find_child_by_sidl_text(parent_wnd, YES_NO_BUTTON_SIDL)
            .or_else(|| crate::eq::widgets::find_child_button_by_text(parent_wnd, "Yes"))
    }?;
    if !unsafe { crate::eq::widgets::is_visible(button_wnd) } {
        return None;
    }

    let dialog_text = crate::login::widgets::read_yesno_dialog_text(parent_wnd).unwrap_or_default();
    let candidates = collect_window_text_candidates(parent_wnd);
    let candidate_refs = candidates.iter().map(String::as_str).collect::<Vec<_>>();
    let request = classify_request_from_text(&dialog_text, &candidate_refs)?;

    Some(DetectedDialog {
        request,
        parent_wnd,
        button_wnd,
        source: YES_NO_DIALOG_SIDL,
    })
}

#[cfg(windows)]
unsafe fn detect_fixed_dialog(mgr: usize) -> Option<DetectedDialog> {
    use textquest_common::offsets::eqgame as eqg;

    for &(parent_sidl, button_sidl, fallback_kind) in DIALOG_ACCEPT_PAIRS {
        let Some(parent_wnd) = (unsafe {
            crate::eq::widgets::find_visible_window_by_sidl_name(
                mgr,
                parent_sidl,
                eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
                eqg::CXWNDMGR_WINDOWS_ARRAY,
                eqg::CXWNDMGR_WINDOWS_COUNT,
            )
        }) else {
            continue;
        };

        let Some(button_wnd) =
            (unsafe { crate::eq::widgets::find_child_by_sidl_text(parent_wnd, button_sidl) })
        else {
            continue;
        };

        if !unsafe { crate::eq::widgets::is_visible(button_wnd) } {
            continue;
        }

        let candidates = collect_window_text_candidates(parent_wnd);
        let candidate_refs = candidates.iter().map(String::as_str).collect::<Vec<_>>();
        let mut request =
            classify_request_from_text("", &candidate_refs).unwrap_or_else(|| AutoAcceptRequest {
                kind: fallback_kind,
                sender: extract_sender_after_prefix(
                    &candidates.join(" | "),
                    &["Trading with ", "Trade with "],
                ),
                text: candidates.join(" | "),
            });

        if parent_sidl.eq_ignore_ascii_case("TaskSelectWnd")
            && request.kind == AutoAcceptRequestKind::TaskAdd
        {
            let lower = request.text.to_ascii_lowercase();
            if lower.contains("dynamic zone") || lower.contains(" expedition ") {
                request.kind = AutoAcceptRequestKind::DzAdd;
            }
        }

        return Some(DetectedDialog {
            request,
            parent_wnd,
            button_wnd,
            source: parent_sidl,
        });
    }

    None
}

#[cfg(windows)]
unsafe fn collect_window_text_candidates(parent_wnd: usize) -> Vec<String> {
    use textquest_common::offsets::{eqgame as eqg, eqmain as off};

    let mut texts = Vec::new();
    push_candidate_text(
        &mut texts,
        &unsafe { crate::eq::widgets::read_cxstr(parent_wnd + off::CXWND_WINDOW_TEXT) }
            .unwrap_or_default(),
    );
    push_candidate_text(
        &mut texts,
        &unsafe { crate::eq::widgets::read_cxstr(parent_wnd + eqg::CSIDL_SCREEN_WND_SIDL_TEXT) }
            .unwrap_or_default(),
    );

    let mut child = unsafe { *((parent_wnd + off::CXWND_FIRST_NODE) as *const usize) };
    let mut count = 0u32;
    while child != 0 && count < 200 {
        count += 1;
        push_candidate_text(
            &mut texts,
            &unsafe { crate::eq::widgets::read_cxstr(child + off::CXWND_WINDOW_TEXT) }
                .unwrap_or_default(),
        );
        child = unsafe { *((child + off::CXWND_NEXT) as *const usize) };
    }

    texts
}

#[cfg(windows)]
unsafe fn detect_legacy_respawn_dialog(mgr: usize) -> Option<(usize, usize)> {
    use textquest_common::offsets::eqgame as eqg;

    let (parent_sidl, button_sidl) = LEGACY_RESPAWN_DIALOG;
    let parent_wnd = unsafe {
        crate::eq::widgets::find_visible_window_by_sidl_name(
            mgr,
            parent_sidl,
            eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            eqg::CXWNDMGR_WINDOWS_ARRAY,
            eqg::CXWNDMGR_WINDOWS_COUNT,
        )
    }?;
    let button_wnd =
        unsafe { crate::eq::widgets::find_child_by_sidl_text(parent_wnd, button_sidl) }?;
    if !unsafe { crate::eq::widgets::is_visible(button_wnd) } {
        return None;
    }
    Some((parent_wnd, button_wnd))
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
    let settings = settings();
    let rez_config = current_rez_config();
    if !settings.enabled && !rez_config.enabled {
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

    if !settings.enabled {
        return;
    }

    let Some(dialog) = (unsafe { detect_dialog(mgr) }) else {
        if let Some((parent_wnd, button_wnd)) = unsafe { detect_legacy_respawn_dialog(mgr) } {
            tracing::info!(
                parent_ptr = format!("{:#x}", parent_wnd),
                button_ptr = format!("{:#x}", button_wnd),
                "Auto-accepting legacy respawn dialog"
            );
            unsafe { crate::eq::widgets::click_button_via_vtable(button_wnd) };
        }
        return;
    };

    if !should_accept_request(&settings, &dialog.request) {
        tracing::debug!(
            kind = ?dialog.request.kind,
            sender = dialog.request.sender.as_deref(),
            source = dialog.source,
            "Auto-accept dialog detected but rejected by policy"
        );
        return;
    }

    tracing::info!(
        kind = ?dialog.request.kind,
        sender = dialog.request.sender.as_deref(),
        source = dialog.source,
        parent_ptr = format!("{:#x}", dialog.parent_wnd),
        button_ptr = format!("{:#x}", dialog.button_wnd),
        "Auto-accepting dialog"
    );

    unsafe { crate::eq::widgets::click_button_via_vtable(dialog.button_wnd) };
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
    use textquest_common::ipc::{AutoAcceptSettings, AutoAcceptTrustMode};

    fn auto_accept_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("dialog test lock poisoned")
    }

    fn enabled_settings() -> AutoAcceptSettings {
        AutoAcceptSettings {
            enabled: true,
            ..AutoAcceptSettings::default()
        }
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
        for &(parent, button, _) in DIALOG_ACCEPT_PAIRS {
            assert!(!parent.is_empty(), "parent SIDL name must not be empty");
            assert!(!button.is_empty(), "button SIDL name must not be empty");
        }
    }

    #[test]
    fn dialog_pairs_contain_trade_window() {
        let has_trade = DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _, _)| *parent == "TradeWnd");
        assert!(has_trade, "Must have TradeWnd pair");
    }

    #[test]
    fn dialog_pairs_contain_task_window() {
        let has_task = DIALOG_ACCEPT_PAIRS
            .iter()
            .any(|(parent, _, _)| *parent == "TaskSelectWnd");
        assert!(has_task, "Must have TaskSelectWnd pair");
    }

    #[test]
    fn legacy_respawn_pair_is_preserved() {
        assert_eq!(LEGACY_RESPAWN_DIALOG.0, "RespawnWnd");
        assert_eq!(LEGACY_RESPAWN_DIALOG.1, "RW_SelectButton");
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

    #[test]
    fn classify_yesno_group_invite_extracts_sender() {
        let request = classify_request_from_text(
            "Vorash invites you to join a group. Do you wish to accept?",
            &[],
        )
        .expect("group invite should classify");

        assert_eq!(request.kind, AutoAcceptRequestKind::GroupInvite);
        assert_eq!(request.sender.as_deref(), Some("Vorash"));
    }

    #[test]
    fn classify_trade_candidates_extracts_sender_from_window_text() {
        let request = classify_request_from_text(
            "",
            &["Trading with Merchantbuddy", "TradeWnd", "Ready to trade"],
        )
        .expect("trade request should classify");

        assert_eq!(request.kind, AutoAcceptRequestKind::Trade);
        assert_eq!(request.sender.as_deref(), Some("Merchantbuddy"));
    }

    #[test]
    fn classify_anchor_request_distinguishes_anchor_from_translocate() {
        let request = classify_request_from_text(
            "Alyra would like to send you to their primary anchor. Do you wish to go?",
            &[],
        )
        .expect("anchor request should classify");

        assert_eq!(request.kind, AutoAcceptRequestKind::Anchor);
        assert_eq!(request.sender.as_deref(), Some("Alyra"));
    }

    #[test]
    fn trust_list_mode_rejects_unknown_or_missing_sender() {
        let settings = AutoAcceptSettings {
            enabled: true,
            trust_mode: AutoAcceptTrustMode::TrustList,
            trusted_players: vec!["Trustedcleric".into()],
            ..AutoAcceptSettings::default()
        };
        let trusted = AutoAcceptRequest {
            kind: AutoAcceptRequestKind::GroupInvite,
            sender: Some("TrustedCleric".into()),
            text: "TrustedCleric invites you to join a group.".into(),
        };
        let unknown = AutoAcceptRequest {
            kind: AutoAcceptRequestKind::GroupInvite,
            sender: Some("Randomwizard".into()),
            text: "Randomwizard invites you to join a group.".into(),
        };
        let missing = AutoAcceptRequest {
            kind: AutoAcceptRequestKind::Trade,
            sender: None,
            text: "Trading with ???".into(),
        };

        assert!(should_accept_request(&settings, &trusted));
        assert!(!should_accept_request(&settings, &unknown));
        assert!(!should_accept_request(&settings, &missing));
    }

    #[test]
    fn per_type_toggle_blocks_disabled_request_kind() {
        let settings = AutoAcceptSettings {
            enabled: true,
            accept_group_invites: false,
            ..AutoAcceptSettings::default()
        };
        let request = AutoAcceptRequest {
            kind: AutoAcceptRequestKind::GroupInvite,
            sender: Some("Trustedcleric".into()),
            text: "Trustedcleric invites you to join a group.".into(),
        };

        assert!(!should_accept_request(&settings, &request));
    }

    #[test]
    fn legacy_enabled_state_turns_on_all_request_types() {
        let settings = AutoAcceptSettings::legacy(true);
        let kinds = [
            AutoAcceptRequestKind::GroupInvite,
            AutoAcceptRequestKind::Trade,
            AutoAcceptRequestKind::TaskAdd,
            AutoAcceptRequestKind::DzAdd,
            AutoAcceptRequestKind::Translocate,
            AutoAcceptRequestKind::Anchor,
        ];

        assert!(settings.enabled);
        assert!(kinds.iter().all(|kind| settings.is_kind_enabled(*kind)));
    }

    #[test]
    fn allow_all_mode_accepts_requests_without_sender() {
        let settings = enabled_settings();
        let request = AutoAcceptRequest {
            kind: AutoAcceptRequestKind::TaskAdd,
            sender: None,
            text: "You have been invited to join the task Epic Recovery.".into(),
        };

        assert!(should_accept_request(&settings, &request));
    }
}
