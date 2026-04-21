use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use textquest::{alerts::AlertStore, config::AlertingConfig};
use textquest_common::auto_group::AutoGroupSettings;

use crate::{AppState, accounts, api};

pub(crate) fn test_live_session_snapshot_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("textquest-web-test-runtime/{name}/{name}"))
}

pub(crate) fn test_admin_session_snapshot_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("textquest-web-test-runtime/{name}/{name}"))
}

pub(crate) fn demo_app_state() -> Arc<AppState> {
    let live_name = format!("test-live-sessions-{}.json", uuid::Uuid::new_v4());
    demo_app_state_with_snapshot(&live_name)
}

pub(crate) fn demo_app_state_with_snapshot(name: &str) -> Arc<AppState> {
    let admin_name = format!("test-admin-sessions-{name}");
    let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
    Arc::new(AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store: None,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
        auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
        tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
        character_config_path: test_live_session_snapshot_path(&format!(
            "test-character-configs-{name}"
        )),
        character_config_write_lock: tokio::sync::Mutex::new(()),
        auto_group_settings: tokio::sync::RwLock::new(AutoGroupSettings::default()),
        loot_state: api::loot::LootState::new_demo(),
        economy_state: api::economy::EconomyState::new_demo(),
        dashboard_state: api::dashboard::DashboardState::new_demo(),
        soul_audit: api::soul::SoulAuditState::new_demo(),
        discord_state: api::discord::DiscordState::new_demo(),
        player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
        player_watch_write_lock: tokio::sync::Mutex::new(()),
        gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
        spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
        vendor_watch_state: api::vendor_watch::VendorWatchState::new_demo(),
        inventory_utility_parity: tokio::sync::RwLock::new(
            textquest_common::inventory_utility::InventoryUtilityConfig::default(),
        ),
        inventory_utility_parity_path: test_live_session_snapshot_path(&format!(
            "test-inventory-utility-parity-{name}"
        )),
        inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
        timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
        timestamp_config_write_lock: tokio::sync::Mutex::new(()),
        kill_tracker_state: api::kill_tracker::KillTrackerState::new_empty(),
        alert_store: AlertStore::open_memory().expect("alert store"),
        alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
        alerting_config_path: std::env::temp_dir().join(format!(
            "textquest-test-alerting-{}.toml",
            uuid::Uuid::new_v4()
        )),
        auto_group_config_path: std::env::temp_dir().join(format!(
            "textquest-test-auto-group-{}.toml",
            uuid::Uuid::new_v4()
        )),
        api_token: None,
        live_session_snapshot_path: test_live_session_snapshot_path(name),
        admin_session_snapshot_path: test_admin_session_snapshot_path(&admin_name),
        xassist_configs: api::xassist::demo_xassist_configs(),
        chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
        say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
        session_controls: tokio::sync::RwLock::new(HashMap::new()),
        auto_group_state: api::auto_group::AutoGroupState::new_demo(),
        extension_catalog_state: api::extensions::ExtensionCatalogState::load(
            std::env::temp_dir().join(format!(
                "textquest-test-extension-catalog-{}.json",
                uuid::Uuid::new_v4()
            )),
        ),
        session_control_state: api::session_control::SessionControlState::new(),
        session_logs: tokio::sync::RwLock::new(HashMap::new()),
    })
}
