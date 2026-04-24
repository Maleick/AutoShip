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
        LazyLock, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use textquest_common::ipc::{
    AutoAcceptAction, AutoAcceptRequestKind, AutoAcceptRule, AutoAcceptSettings,
    AutoAcceptTrustMode, AutoRezConfig,
};

/// Whether auto-accept is enabled. Disabled by default; toggled via IPC
/// `SetAutoAccept`.
static AUTO_ACCEPT_ENABLED: AtomicBool = AtomicBool::new(false);
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
const TRADE_ACCEPT_BUTTONS: &[&str] = &["TRDW_Trade_Button"];
const TASK_ACCEPT_BUTTONS: &[&str] = &["TASKSEL_AcceptButton", "TaskSelectAcceptButton"];
const CONFIRM_ACCEPT_BUTTONS: &[&str] = &["Yes_Button", "CD_Yes_Button"];
const CONFIRM_DECLINE_BUTTONS: &[&str] = &["No_Button", "CD_No_Button"];
const GENERIC_DIALOG_SPECS: &[DialogSpec] = &[
    DialogSpec {
        parent_sidl: "TRDW_TradeRequestWnd",
        default_kind: Some(AutoAcceptRequestKind::Trade),
        accept_buttons: TRADE_ACCEPT_BUTTONS,
        decline_buttons: &[],
    },
    DialogSpec {
        parent_sidl: "TradeWnd",
        default_kind: Some(AutoAcceptRequestKind::Trade),
        accept_buttons: TRADE_ACCEPT_BUTTONS,
        decline_buttons: &[],
    },
    DialogSpec {
        parent_sidl: "TaskSelectWnd",
        default_kind: Some(AutoAcceptRequestKind::TaskAdd),
        accept_buttons: TASK_ACCEPT_BUTTONS,
        decline_buttons: &[],
    },
    DialogSpec {
        parent_sidl: REZ_CONFIRM_DIALOG,
        default_kind: None,
        accept_buttons: CONFIRM_ACCEPT_BUTTONS,
        decline_buttons: CONFIRM_DECLINE_BUTTONS,
    },
];

#[derive(Debug, Clone, Copy)]
struct DialogSpec {
    parent_sidl: &'static str,
    default_kind: Option<AutoAcceptRequestKind>,
    accept_buttons: &'static [&'static str],
    decline_buttons: &'static [&'static str],
}

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
    AUTO_ACCEPT_SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enabled = enabled;
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
            .unwrap_or_else(PoisonError::into_inner);
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

/// Replace the current auto-accept settings snapshot.
pub fn set_settings(settings: AutoAcceptSettings) {
    AUTO_ACCEPT_ENABLED.store(settings.enabled, Ordering::Relaxed);
    *AUTO_ACCEPT_SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = settings;
    tracing::info!("Auto-accept settings updated");
}

fn current_rez_config() -> AutoRezConfig {
    AUTO_REZ_CONFIG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn clear_rez_runtime_state() {
    *PENDING_REZ_OFFER
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    *RECENT_REZ_CONTEXT
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
}

fn current_auto_accept_settings() -> AutoAcceptSettings {
    AUTO_ACCEPT_SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn generic_dialog_kind(
    parent_sidl: &str,
    dialog_text: Option<&str>,
) -> Option<AutoAcceptRequestKind> {
    match parent_sidl {
        "TRDW_TradeRequestWnd" | "TradeWnd" => Some(AutoAcceptRequestKind::Trade),
        "TaskSelectWnd" => Some(AutoAcceptRequestKind::TaskAdd),
        REZ_CONFIRM_DIALOG => dialog_text.and_then(classify_confirmation_dialog_text),
        _ => dialog_text.and_then(classify_confirmation_dialog_text),
    }
}

fn classify_confirmation_dialog_text(text: &str) -> Option<AutoAcceptRequestKind> {
    let lower = textquest_common::chat::strip_stml(text).to_ascii_lowercase();

    if is_rez_offer_text(&lower) {
        return Some(AutoAcceptRequestKind::Resurrection);
    }
    if lower.contains("translocate") || lower.contains("evacuate") {
        return Some(AutoAcceptRequestKind::Translocate);
    }
    if lower.contains("anchor") {
        return Some(AutoAcceptRequestKind::Anchor);
    }
    if lower.contains("fellowship") {
        return Some(AutoAcceptRequestKind::FellowshipInvite);
    }
    if lower.contains("raid") && lower.contains("invite") {
        return Some(AutoAcceptRequestKind::RaidInvite);
    }
    if lower.contains("group") && lower.contains("invite") {
        return Some(AutoAcceptRequestKind::GroupInvite);
    }
    if lower.contains("dynamic zone") || lower.contains("expedition") {
        return Some(AutoAcceptRequestKind::DzAdd);
    }
    if lower.contains("mission") {
        return Some(AutoAcceptRequestKind::MissionInvite);
    }
    if lower.contains("shared task") || lower.contains("task invite") {
        return Some(AutoAcceptRequestKind::TaskInvite);
    }
    if lower.contains("task") {
        return Some(AutoAcceptRequestKind::TaskAdd);
    }
    if lower.contains("quest") && (lower.contains("complete") || lower.contains("reward")) {
        return Some(AutoAcceptRequestKind::QuestCompletion);
    }
    if lower.contains("quest") {
        return Some(AutoAcceptRequestKind::QuestUpdate);
    }
    if lower.contains("trade") {
        return Some(AutoAcceptRequestKind::Trade);
    }

    None
}

fn sanitize_source_candidate(candidate: &str) -> Option<String> {
    let source = candidate
        .trim()
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' && ch != ' ')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if source.is_empty() {
        return None;
    }

    let lower = source.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "you" | "your" | "do" | "would" | "accept" | "the" | "a" | "an"
    ) {
        return None;
    }

    Some(source)
}

fn infer_dialog_source(kind: AutoAcceptRequestKind, text: &str) -> Option<String> {
    if kind == AutoAcceptRequestKind::Resurrection {
        return parse_rez_offer_text(text).map(|offer| offer.caster_name);
    }

    let stripped = textquest_common::chat::strip_stml(text);
    let normalized = stripped.trim();
    if normalized.is_empty() {
        return None;
    }

    let lower = normalized.to_ascii_lowercase();
    for marker in [
        " invites you",
        " has invited you",
        " wants to trade",
        " wishes to",
        " would like",
        " offers ",
        " asks ",
        " is inviting",
    ] {
        if let Some(idx) = lower.find(marker) {
            return sanitize_source_candidate(&normalized[..idx]);
        }
    }

    normalized
        .split_whitespace()
        .next()
        .and_then(sanitize_source_candidate)
}

fn normalize_filter(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn contains_filter(haystack: Option<&str>, needle: &str) -> bool {
    let needle = normalize_filter(needle);
    if needle.is_empty() {
        return false;
    }

    haystack
        .map(|value| value.to_ascii_lowercase().contains(&needle))
        .unwrap_or(false)
}

fn source_matches_any(source: Option<&str>, filters: &[String]) -> bool {
    filters
        .iter()
        .any(|filter| contains_filter(source, filter.as_str()))
}

fn text_matches_any(text: Option<&str>, filters: &[String]) -> bool {
    filters
        .iter()
        .any(|filter| contains_filter(text, filter.as_str()))
}

fn rule_matches(
    rule: &AutoAcceptRule,
    kind: AutoAcceptRequestKind,
    source: Option<&str>,
    text: Option<&str>,
) -> bool {
    rule.kind == kind
        && rule
            .source_contains
            .as_deref()
            .map(|filter| contains_filter(source, filter))
            .unwrap_or(true)
        && rule
            .text_contains
            .as_deref()
            .map(|filter| contains_filter(text, filter))
            .unwrap_or(true)
}

fn source_allowed_by_trust_mode(settings: &AutoAcceptSettings, source: Option<&str>) -> bool {
    match settings.trust_mode {
        AutoAcceptTrustMode::Anyone => true,
        AutoAcceptTrustMode::TrustList => source_matches_any(source, &settings.trusted_players),
    }
}

fn decide_generic_dialog_action(
    settings: &AutoAcceptSettings,
    kind: AutoAcceptRequestKind,
    source: Option<&str>,
    text: Option<&str>,
) -> AutoAcceptAction {
    if !settings.enabled {
        return AutoAcceptAction::Ignore;
    }

    if source_matches_any(source, &settings.source_blocklist)
        || text_matches_any(text, &settings.text_blocklist)
    {
        return if settings.decline_blocked {
            AutoAcceptAction::Decline
        } else {
            AutoAcceptAction::Ignore
        };
    }

    for rule in &settings.rules {
        if rule_matches(rule, kind, source, text) {
            return rule.action;
        }
    }

    if !settings.source_allowlist.is_empty()
        && !source_matches_any(source, &settings.source_allowlist)
    {
        return AutoAcceptAction::Ignore;
    }

    if !settings.is_kind_enabled(kind) || !source_allowed_by_trust_mode(settings, source) {
        return AutoAcceptAction::Ignore;
    }

    AutoAcceptAction::Accept
}

fn mark_recent_rez_context(at: Instant) {
    *RECENT_REZ_CONTEXT
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(at);
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
/// Only scans when in-world and the relevant automation is enabled.
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

    let local_player =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)
            .map_or(0, |addr| *(addr as *const usize));
    if local_player == 0 {
        return;
    }

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

    let settings = current_auto_accept_settings();
    for spec in GENERIC_DIALOG_SPECS {
        // Find the parent dialog window by SIDL name (must be visible)
        let parent = crate::eq::widgets::find_visible_window_by_sidl_name(
            mgr,
            spec.parent_sidl,
            eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            eqg::CXWNDMGR_WINDOWS_ARRAY,
            eqg::CXWNDMGR_WINDOWS_COUNT,
        );

        let Some(parent_wnd) = parent else {
            continue;
        };

        let dialog_text = read_generic_dialog_text(parent_wnd);
        let Some(kind) = spec
            .default_kind
            .or_else(|| generic_dialog_kind(spec.parent_sidl, dialog_text.as_deref()))
        else {
            continue;
        };
        let source = dialog_text
            .as_deref()
            .and_then(|text| infer_dialog_source(kind, text));
        let action = decide_generic_dialog_action(
            &settings,
            kind,
            source.as_deref(),
            dialog_text.as_deref(),
        );
        if action == AutoAcceptAction::Ignore {
            continue;
        }

        let button_sidls = match action {
            AutoAcceptAction::Accept => spec.accept_buttons,
            AutoAcceptAction::Decline => spec.decline_buttons,
            AutoAcceptAction::Ignore => unreachable!(),
        };
        let Some((button_sidl, button_wnd)) =
            click_first_visible_child_by_sidl(parent_wnd, button_sidls)
        else {
            continue;
        };

        tracing::info!(
            parent = spec.parent_sidl,
            button = button_sidl,
            ?kind,
            ?action,
            source = source.as_deref().unwrap_or("unknown"),
            parent_ptr = format!("{:#x}", parent_wnd),
            button_ptr = format!("{:#x}", button_wnd),
            "Handling auto-accept dialog"
        );
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
            .unwrap_or_else(PoisonError::into_inner) = None;
        return false;
    };

    let Some(dialog_text) = read_rez_confirmation_text(dialog_wnd) else {
        return false;
    };
    let Some(offer) = parse_rez_offer_text(&dialog_text) else {
        return false;
    };

    let now = Instant::now();
    let elapsed_ms = {
        let mut guard = PENDING_REZ_OFFER
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
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
                    .unwrap_or_else(PoisonError::into_inner) = None;
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
                    .unwrap_or_else(PoisonError::into_inner) = None;
            }
            true
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_generic_dialog_text(dialog_wnd: usize) -> Option<String> {
    use textquest_common::offsets::eqmain as off;

    if let Some(text) = crate::eq::widgets::read_cxstr(dialog_wnd + off::CXWND_WINDOW_TEXT) {
        let stripped = textquest_common::chat::strip_stml(&text);
        if !stripped.trim().is_empty() {
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

    best_text.filter(|text| !text.trim().is_empty())
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn click_first_visible_child_by_sidl(
    parent_wnd: usize,
    sidl_names: &'static [&'static str],
) -> Option<(&'static str, usize)> {
    for sidl_name in sidl_names {
        if let Some(button_wnd) = crate::eq::widgets::find_child_by_sidl_text(parent_wnd, sidl_name)
            && crate::eq::widgets::is_visible(button_wnd)
        {
            crate::eq::widgets::click_button_via_vtable(button_wnd);
            return Some((sidl_name, button_wnd));
        }
    }

    None
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
            .unwrap_or_else(PoisonError::into_inner);
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
        .unwrap_or_else(PoisonError::into_inner) = None;
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
        }
    }

    #[test]
    fn dialog_specs_have_accept_buttons() {
        for spec in GENERIC_DIALOG_SPECS {
            assert!(
                !spec.parent_sidl.is_empty(),
                "parent SIDL name must not be empty"
            );
            assert!(
                !spec.accept_buttons.is_empty(),
                "accept buttons must not be empty"
            );
        }
    }

    #[test]
    fn dialog_specs_contain_trade_window() {
        let has_trade = GENERIC_DIALOG_SPECS
            .iter()
            .any(|spec| spec.parent_sidl == "TradeWnd");
        assert!(has_trade, "Must have TradeWnd pair");
    }

    #[test]
    fn generic_dialog_specs_contain_task_window() {
        let has_task = GENERIC_DIALOG_SPECS
            .iter()
            .any(|spec| spec.parent_sidl == "TaskSelectWnd");
        assert!(has_task, "Must have TaskSelectWnd pair");
    }

    #[test]
    fn respawn_window_handled_via_recent_rez_context() {
        let has_respawn = GENERIC_DIALOG_SPECS
            .iter()
            .any(|spec| spec.parent_sidl == "RespawnWnd");
        assert!(
            !has_respawn,
            "RespawnWnd must stay on the dedicated recent-rez handling path"
        );
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
            &AutoRezConfig {
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
            &AutoRezConfig {
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
            &AutoRezConfig {
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
            &AutoRezConfig {
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
            &AutoRezConfig {
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
    fn generic_dialog_kind_maps_known_dialogs() {
        assert_eq!(
            generic_dialog_kind("TradeWnd", None),
            Some(AutoAcceptRequestKind::Trade)
        );
        assert_eq!(
            generic_dialog_kind("TaskSelectWnd", None),
            Some(AutoAcceptRequestKind::TaskAdd)
        );
        assert_eq!(
            generic_dialog_kind(
                REZ_CONFIRM_DIALOG,
                Some("Leader invites you to join a group.")
            ),
            Some(AutoAcceptRequestKind::GroupInvite)
        );
        assert_eq!(generic_dialog_kind("UnknownWnd", None), None);
    }

    #[test]
    fn source_filter_policy_blocks_then_declines() {
        let mut settings = AutoAcceptSettings {
            enabled: true,
            source_blocklist: vec!["Badactor".into()],
            decline_blocked: true,
            ..AutoAcceptSettings::default()
        };

        let action = decide_generic_dialog_action(
            &settings,
            AutoAcceptRequestKind::GroupInvite,
            Some("BadActor"),
            Some("BadActor invites you to join a group."),
        );
        assert_eq!(action, AutoAcceptAction::Decline);

        settings.decline_blocked = false;
        let action = decide_generic_dialog_action(
            &settings,
            AutoAcceptRequestKind::GroupInvite,
            Some("BadActor"),
            Some("BadActor invites you to join a group."),
        );
        assert_eq!(action, AutoAcceptAction::Ignore);
    }

    #[test]
    fn trust_list_requires_inferred_source() {
        let settings = AutoAcceptSettings {
            enabled: true,
            trust_mode: AutoAcceptTrustMode::TrustList,
            trusted_players: vec!["Leader".into()],
            ..AutoAcceptSettings::default()
        };

        assert_eq!(
            decide_generic_dialog_action(
                &settings,
                AutoAcceptRequestKind::GroupInvite,
                Some("Leader"),
                Some("Leader invites you to join a group."),
            ),
            AutoAcceptAction::Accept
        );
        assert_eq!(
            decide_generic_dialog_action(
                &settings,
                AutoAcceptRequestKind::GroupInvite,
                Some("Stranger"),
                Some("Stranger invites you to join a group."),
            ),
            AutoAcceptAction::Ignore
        );
    }

    #[test]
    fn explicit_rule_can_accept_quest_completion() {
        let settings = AutoAcceptSettings {
            enabled: true,
            rules: vec![AutoAcceptRule {
                kind: AutoAcceptRequestKind::QuestCompletion,
                action: AutoAcceptAction::Accept,
                source_contains: Some("Priest".into()),
                text_contains: Some("reward".into()),
            }],
            ..AutoAcceptSettings::default()
        };

        let action = decide_generic_dialog_action(
            &settings,
            AutoAcceptRequestKind::QuestCompletion,
            Some("A Priest of Discord"),
            Some("A Priest of Discord offers a quest reward."),
        );
        assert_eq!(action, AutoAcceptAction::Accept);
    }

    #[test]
    fn infer_dialog_source_reads_sender_prefix() {
        assert_eq!(
            infer_dialog_source(
                AutoAcceptRequestKind::RaidInvite,
                "RaidLeader invites you to join a raid."
            ),
            Some("RaidLeader".into())
        );
    }
}
