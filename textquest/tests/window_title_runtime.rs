use std::{fs, path::PathBuf};

use tempfile::tempdir;
use textquest::window_title_runtime::{WindowTitleRuntime, default_config_path};
use textquest_common::{
    character_config::{
        CharacterConfig, ClassParams, RewardAutomationConfig, RotationEntry, TributePreferences,
        TributeStatus,
    },
    ipc::AutoRezConfig,
    window_title::default_window_title_format,
};

#[test]
fn default_path_targets_character_config_file() {
    assert_eq!(
        default_config_path(),
        PathBuf::from("config/character-configs.json")
    );
}

#[test]
fn tick_loads_window_title_formats_from_disk() {
    let temp = tempdir().expect("tempdir");
    let config_path = temp.path().join("character-configs.json");
    let mut configs = std::collections::HashMap::new();
    configs.insert(
        "Frostreaver".to_string(),
        CharacterConfig {
            character_name: "Frostreaver".into(),
            class: "Cleric".into(),
            role: "Healer".into(),
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: Vec::<RotationEntry>::new(),
            class_params: ClassParams::default(),
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Highclerk".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
            group_override: false,
            group_name: None,
            window_title_format: "[{server}] {character} ({level} {class_short})".into(),
            reward_automation: RewardAutomationConfig::default(),
            tribute_preferences: TributePreferences::default(),
            tribute_status: TributeStatus::default(),
            improve_auto_promote: Default::default(),
        },
    );
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&configs).expect("serialize character config"),
    )
    .expect("write character config");

    let mut runtime = WindowTitleRuntime::with_config_path(config_path);
    let configs = runtime.tick().expect("config reload");

    assert_eq!(
        configs
            .get("Frostreaver")
            .expect("Frostreaver config")
            .format,
        "[{server}] {character} ({level} {class_short})",
    );
}

#[test]
fn missing_character_falls_back_to_default_template() {
    let runtime = WindowTitleRuntime::with_config_path(PathBuf::from("config/missing.json"));
    let config = runtime.get_config("Missing");
    assert_eq!(config.format, default_window_title_format());
}
