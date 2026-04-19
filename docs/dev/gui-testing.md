# GUI Testing Guide

This document covers testing practices for GUI interactions in TextQuest, including EQ window management, box chat, and TUI state verification.

## What Is GUI in TextQuest

GUI testing in TextQuest refers to testing the interaction layer with EverQuest windows, not a traditional GUI framework. TextQuest uses Win32 APIs to interact with EQ game windows.

| Component | Purpose | Testing Focus |
|-----------|---------|---------------|
| **Window title** | Parse and render EQ window titles | Template parsing, token substitution |
| **Box chat** | Multi-box text communication | Route parsing, message formatting |
| **Window discovery** | Find EQ windows by title | Search, filtering, PID matching |
| **TUI state** | Track UI state per client | State transitions, event handling |

## Testing Window Title

Test window title parsing and rendering with various contexts:

```rust
// textquest-common/tests/window_title.rs
use textquest_common::window_title::{
    WindowTitleContext, class_long_name, class_short_name,
    render_window_title, default_window_title_format,
};

#[test]
fn renders_default_template_with_server_level_and_class() {
    let rendered = render_window_title(
        &default_window_title_format(),
        &WindowTitleContext {
            server: Some("Teek"),
            character: Some("Frostreaver"),
            level: Some(60),
            class_id: Some(2),
            zone_long_name: Some("Plane of Fire"),
            zone_short_name: Some("powfire"),
        },
    );

    assert_eq!(rendered, "[Teek] Frostreaver (60 CLR)");
}

#[test]
fn leaves_unknown_tokens_intact_and_blanks_missing_known_values() {
    let rendered = render_window_title(
        "{character}|{zone}|{class}|{missing}",
        &WindowTitleContext {
            server: None,
            character: Some("Aelrindel"),
            level: None,
            class_id: None,
            zone_long_name: None,
            zone_short_name: Some("nro"),
        },
    );

    assert_eq!(rendered, "Aelrindel|nro||{missing}");
}

#[test]
fn class_name_helpers_cover_known_and_unknown_ids() {
    assert_eq!(class_short_name(2), Some("CLR"));
    assert_eq!(class_long_name(2), Some("Cleric"));
    assert_eq!(class_short_name(250), None);
    assert_eq!(class_long_name(250), None);
}
```

## Testing Config-Driven Window Title Runtime

Test dynamic config loading and fallback behavior:

```rust
// textquest/tests/window_title_runtime.rs
use textquest::window_title_runtime::{WindowTitleRuntime, default_config_path};
use textquest_common::character_config::{CharacterConfig, ClassParams};
use textquest_common::window_title::default_window_title_format;

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
            // ... other fields
            window_title_format: "[{server}] {character} ({level} {class_short})".into(),
            ..Default::default()
        },
    );
    fs::write(&config_path, serde_json::to_string_pretty(&configs).expect("serialize"))
        .expect("write config");

    let mut runtime = WindowTitleRuntime::with_config_path(config_path);
    let configs = runtime.tick().expect("config reload");

    assert_eq!(
        configs.get("Frostreaver").expect("Frostreaver config").format,
        "[{server}] {character} ({level} {class_short})",
    );
}

#[test]
fn missing_character_falls_back_to_default_template() {
    let runtime = WindowTitleRuntime::with_config_path(PathBuf::from("config/missing.json"));
    let config = runtime.get_config("Missing");
    assert_eq!(config.format, default_window_title_format());
}
```

## Testing Box Chat

Test multi-box chat routing and parsing:

```rust
// textquest/tests/box_chat.rs
use textquest_common::box_chat::{BoxChatConfig, OutboundRoute, parse_slash_route};

#[test]
fn app_config_defaults_box_chat_to_disabled() {
    let cfg = AppConfig::default_config();

    assert!(!cfg.box_chat.enabled);
    assert_eq!(cfg.box_chat.port, 2112);
}

#[test]
fn parse_bc_route_accepts_slash_or_mq2_double_slash_payloads() {
    assert_eq!(
        parse_slash_route("/bc /assist MainTank"),
        Some(Ok(OutboundRoute::Broadcast {
            command: "/assist MainTank".to_string(),
        }))
    );

    assert_eq!(
        parse_slash_route("/bca //follow MainTank"),
        Some(Ok(OutboundRoute::Broadcast {
            command: "/follow MainTank".to_string(),
        }))
    );
}

#[test]
fn parse_bct_route_targets_specific_character() {
    assert_eq!(
        parse_slash_route("/bct Cleric01 //cast 1"),
        Some(Ok(OutboundRoute::Target {
            character: "Cleric01".to_string(),
            command: "/cast 1".to_string(),
        }))
    );
}

#[test]
fn app_config_parses_box_chat_section() {
    let cfg: AppConfig = toml::from_str(
        r#"
[box_chat]
enabled = true
host = "192.168.1.25"
port = 3002
auto_connect = true
"#,
    )
    .expect("box chat config should parse");

    assert!(cfg.box_chat.enabled);
    assert_eq!(cfg.box_chat.port, 3002);
}
```

## Testing Window Discovery

Test finding EQ windows by title substring:

```rust
// textquest/src/process/window.rs
use textquest::process::window::{find_windows_by_title, WindowHandle};

#[cfg(windows)]
#[test]
fn find_windows_by_title_returns_matching_windows() {
    let windows = find_windows_by_title("eqgame").expect("search should succeed");

    // Verification depends on running EQ clients
    // Tests may be empty on systems without EQ running
}

#[cfg(not(windows))]
#[test]
fn find_windows_by_title_stub_returns_empty_results() {
    let windows = find_windows_by_title("eqgame").expect("stub search should succeed");
    assert!(
        windows.is_empty(),
        "non-Windows stub should not report windows"
    );
}
```

## Testing TUI State

Test terminal UI state per client:

```rust
// Integration test pattern
use textquest::tui::state::AppState;

#[test]
fn alert_system_batches_warnings_within_window() {
    let mut app = AppState::default();

    // Queue multiple warnings
    app.add_warning(client_id, "Warning 1".into());
    app.add_warning(client_id, "Warning 2".into());

    // Within batch window, warnings are queued
    assert_eq!(app.warnings_for_client(client_id), 2);

    // After window expires, batch is flushed
    app.advance_time(Duration::from_secs(61));
    assert!(app.warnings_for_client(client_id).is_empty());
}

#[test]
fn dashboard_xp_rate_windowed_calculates_correctly() {
    let mut db = DashboardState::new();

    // Add XP events across time window
    db.record_xp_gain(1000);
    db.advance_time(Duration::from_secs(300));
    db.record_xp_gain(2000);

    let rate = db.xp_rate_windowed(Duration::from_secs(900));
    assert!(rate > 0.0);
}
```

## Test Data Patterns

### WindowTitleContext Builder

```rust
fn window_title_context() -> WindowTitleContext {
    WindowTitleContext {
        server: Some("Teek".into()),
        character: Some("Frostreaver".into()),
        level: Some(60),
        class_id: Some(2),
        zone_long_name: Some("Plane of Fire".into()),
        zone_short_name: Some("powfire".into()),
    }
}
```

### CharacterConfig with Window Title

```rust
fn char_config_with_title(format: &str) -> CharacterConfig {
    CharacterConfig {
        character_name: "TestChar".into(),
        class: "Cleric".into(),
        window_title_format: format.into(),
        ..Default::default()
    }
}
```

## Coverage Expectations

| Component | Coverage Target |
|-----------|-----------------|
| Window title parsing | 80%+ |
| Box chat routing | 80%+ |
| Window discovery stubs | 70%+ |
| TUI state logic | 70%+ |

## Running GUI Tests

```bash
# Window title tests
cargo test -p textquest-common window_title
cargo test -p textquest window_title

# Box chat tests
cargo test box_chat

# Window discovery
cargo test window::find

# TUI state tests
cargo test tui::state
cargo test alert_system

# All GUI-related tests
cargo test gui
```

## Common Patterns Summary

| Pattern | Example |
|---------|---------|
| Template parsing | `render_window_title(format, context)` |
| Token substitution | `{character}`, `{level}` |
| Config fallback | `missing_character_falls_back_to_default_template` |
| Route parsing | `parse_slash_route("/bc /command")` |
| State batching | `add_warning` + batch window flush |