use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{AppState, accounts, api};
use textquest::{alerts::AlertStore, config::AlertingConfig};

pub(crate) fn test_live_session_snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../data/runtime/{name}"))
}

pub(crate) fn demo_app_state() -> Arc<AppState> {
    demo_app_state_with_snapshot("test-live-sessions.json")
}

pub(crate) fn demo_app_state_with_snapshot(name: &str) -> Arc<AppState> {
    let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
    Arc::new(AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store: None,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
        auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
        character_config_path: test_live_session_snapshot_path("test-character-configs.json"),
        character_config_write_lock: tokio::sync::Mutex::new(()),
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
        timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
        timestamp_config_write_lock: tokio::sync::Mutex::new(()),
        kill_tracker_state: api::kill_tracker::KillTrackerState::new_empty(),
        alert_store: AlertStore::open_memory().expect("alert store"),
        alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
        alerting_config_path: std::env::temp_dir().join(format!(
            "textquest-test-alerting-{}.toml",
            uuid::Uuid::new_v4()
        )),
        api_token: None,
        live_session_snapshot_path: test_live_session_snapshot_path(name),
        xassist_configs: api::xassist::demo_xassist_configs(),
        chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
        say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
        auto_group_state: api::auto_group::AutoGroupState::new_demo(),
    })
}
