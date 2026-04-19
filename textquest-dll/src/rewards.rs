//! Mission/task reward automation.
//!
//! Detects the reward window, switches to the configured reward tab, and claims
//! the selected reward once the preferred tab is active.

use std::sync::{Mutex, OnceLock};

use textquest_common::character_config::{RewardAutomationConfig, resolve_reward_index};

fn config_store() -> &'static Mutex<Option<RewardAutomationConfig>> {
    static CONFIG: OnceLock<Mutex<Option<RewardAutomationConfig>>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(None))
}

pub fn set_config(config: RewardAutomationConfig) {
    let rule_count = config.rules.len();
    if let Ok(mut slot) = config_store().lock() {
        *slot = Some(config);
    }
    tracing::info!(rule_count, "Reward automation config updated");
}

fn current_config() -> Option<RewardAutomationConfig> {
    config_store().lock().ok().and_then(|guard| guard.clone())
}

#[derive(Clone, Copy)]
#[repr(C)]
struct EqRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl EqRect {
    fn width(self) -> i32 {
        self.right.saturating_sub(self.left)
    }

    fn height(self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }
}

fn planned_reward_index(
    task_name: &str,
    reward_names: &[String],
    config: &RewardAutomationConfig,
) -> Option<usize> {
    resolve_reward_index(task_name, reward_names, config)
}

#[cfg(windows)]
unsafe fn read_rect(ptr: usize) -> EqRect {
    *(ptr as *const EqRect)
}

#[cfg(windows)]
unsafe fn is_enabled(wnd: usize) -> bool {
    *((wnd + textquest_common::offsets::eqgame::CXWND_ENABLED) as *const bool)
}

#[cfg(windows)]
unsafe fn read_page_ptr(tab_wnd: usize, index: usize) -> Option<usize> {
    use textquest_common::offsets::eqgame as eqg;

    let count = *((tab_wnd + eqg::CTABWND_PAGE_COUNT) as *const i32);
    if count <= 0 || index >= count as usize {
        return None;
    }

    let array_ptr = *((tab_wnd + eqg::CTABWND_PAGE_ARRAY) as *const usize);
    if array_ptr == 0 {
        return None;
    }

    let page_ptr = *((array_ptr + index * core::mem::size_of::<usize>()) as *const usize);
    (page_ptr != 0).then_some(page_ptr)
}

#[cfg(windows)]
unsafe fn read_reward_names(tab_wnd: usize) -> Vec<String> {
    use textquest_common::offsets::eqgame as eqg;

    let count = *((tab_wnd + eqg::CTABWND_PAGE_COUNT) as *const i32);
    if count <= 0 {
        return Vec::new();
    }

    (0..count as usize)
        .map(|index| {
            read_page_ptr(tab_wnd, index)
                .and_then(|page_ptr| {
                    crate::eq::widgets::read_cxstr(page_ptr + eqg::CPAGEWND_TAB_TEXT)
                })
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(windows)]
unsafe fn click_tab(tab_wnd: usize, tab_count: usize, target_index: usize) {
    use textquest_common::offsets::eqgame as eqg;

    let client_rect = read_rect(tab_wnd + eqg::CXWND_CLIENT_RECT);
    let fallback_rect = read_rect(tab_wnd + eqg::CXWND_LOCATION);
    let rect = if client_rect.width() > 0 && client_rect.height() > 0 {
        client_rect
    } else {
        fallback_rect
    };

    let configured_tab_width = *((tab_wnd + eqg::CTABWND_TAB_WIDTH) as *const i32);
    let tab_width = if configured_tab_width > 0 {
        configured_tab_width
    } else {
        (rect.width() / (tab_count.max(1) as i32)).max(1)
    };
    let tab_height = (*((tab_wnd + eqg::CTABWND_TAB_HEIGHT) as *const i32)).max(1);
    let x = rect.left + (target_index as i32 * tab_width) + (tab_width / 2);
    let y = rect.top + (tab_height.min(rect.height().max(1)) / 2);

    crate::eq::widgets::click_window_point_via_vtable(tab_wnd, x, y);
}

/// Detect the reward window and apply the currently configured preference.
///
/// Runs on the game loop thread only.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn check_reward_window() {
    let Some(config) = current_config() else {
        return;
    };

    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
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

    use textquest_common::offsets::{eqgame as eqg, eqmain as off};

    let Some(reward_wnd) = crate::eq::widgets::find_visible_window_by_sidl_name(
        mgr,
        "RewardSelectionWnd",
        eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
        eqg::CXWNDMGR_WINDOWS_ARRAY,
        eqg::CXWNDMGR_WINDOWS_COUNT,
    ) else {
        return;
    };

    let task_name =
        crate::eq::widgets::read_cxstr(reward_wnd + off::CXWND_WINDOW_TEXT).unwrap_or_default();

    let tab_wnd = *((reward_wnd + off::CXWND_FIRST_NODE) as *const usize);
    if tab_wnd == 0 {
        return;
    }

    let reward_names = read_reward_names(tab_wnd);
    let Some(target_index) = planned_reward_index(&task_name, &reward_names, &config) else {
        return;
    };

    let current_index = (*((tab_wnd + eqg::CTABWND_CUR_TAB_INDEX) as *const i32)).max(0) as usize;
    if current_index != target_index {
        click_tab(tab_wnd, reward_names.len(), target_index);
        tracing::info!(
            task = task_name,
            target_index,
            reward = reward_names.get(target_index).cloned().unwrap_or_default(),
            "Reward automation switched reward tab"
        );
        return;
    }

    let Some(page_ptr) = read_page_ptr(tab_wnd, target_index) else {
        return;
    };
    let Some(choose_button) =
        crate::eq::widgets::find_child_by_sidl_text(page_ptr, "RewardSelectionChooseButton")
    else {
        return;
    };
    if !crate::eq::widgets::is_visible(choose_button) || !is_enabled(choose_button) {
        return;
    }

    tracing::info!(
        task = task_name,
        target_index,
        reward = reward_names.get(target_index).cloned().unwrap_or_default(),
        "Reward automation claiming reward"
    );
    crate::eq::widgets::click_button_via_vtable(choose_button);
}

#[cfg(not(windows))]
pub unsafe fn check_reward_window() {}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::character_config::{RewardPreference, TaskRewardPreference};

    #[test]
    fn planned_reward_index_uses_default_rule() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 2 },
            }],
        };
        let rewards = vec!["Potion".to_string(), "Gem".to_string()];

        assert_eq!(
            planned_reward_index("Unknown Task", &rewards, &config),
            Some(1)
        );
    }

    #[test]
    fn set_config_replaces_previous_config() {
        set_config(RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "Mission".into(),
                preference: RewardPreference::ByName {
                    reward_name: "Gem".into(),
                },
            }],
        });

        let snapshot = current_config().expect("config should be set");
        assert_eq!(snapshot.rules.len(), 1);
        assert_eq!(snapshot.rules[0].task_matcher, "Mission");
    }

    #[cfg(not(windows))]
    #[test]
    fn check_reward_window_is_noop_on_non_windows() {
        unsafe {
            check_reward_window();
        }
    }
}
