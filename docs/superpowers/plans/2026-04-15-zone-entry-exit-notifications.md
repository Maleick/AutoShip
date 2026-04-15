# Zone Entry Exit Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add player zone entry and exit notifications with friend-aware filtering, a dedicated spawn/despawn event stream panel in the TUI, an optional sound alert on PC zone-in, and web configuration for the new behavior.

**Architecture:** Extend the existing spawn-delta IPC path so spawn lifecycle events carry enough metadata to identify player characters. Keep the TUI event stream on the existing `SpawnAlertFeed`, add a small player-notification policy layer in the TUI app state, and expose the configurable filter/sound/friend-list settings through the config model plus a focused web API and dashboard panel.

**Tech Stack:** Rust workspace crates (`textquest`, `textquest-common`, `textquest-dll`, `textquest-web`), Axum, React, TypeScript, Vitest, ratatui.

---

### Task 1: Model Player Notification Settings And Spawn Event Metadata

**Files:**
- Modify: `textquest/src/config.rs`
- Modify: `textquest-common/src/ipc.rs`
- Modify: `textquest-common/src/types.rs`
- Modify: `textquest-dll/src/hooks/game_loop.rs`

- [ ] **Step 1: Write the failing Rust tests for config defaults and spawn-delta metadata**

```rust
#[test]
fn spawn_watch_defaults_include_player_notifications() {
    let cfg = AppConfig::default_config();
    assert_eq!(cfg.spawn_watch.player_filter_mode, PlayerFilterMode::All);
    assert!(!cfg.spawn_watch.sound_on_player_zone_in);
    assert!(cfg.spawn_watch.friends.is_empty());
}

#[test]
fn spawn_delta_events_preserve_spawn_type_for_created_and_destroyed_entries() {
    let previous: HashMap<u32, SpawnSnapshot> = [(
        7,
        SpawnSnapshot::new("RangerOne", 0),
    )]
    .into_iter()
    .collect();

    let current = vec![SpawnData {
        spawn_id: 11,
        displayed_name: "ClericTwo".into(),
        spawn_type: 0,
        ..SpawnData::default()
    }];

    let (_next, events) =
        compute_spawn_delta_events(&previous, &current, "greatdivide".into(), 12345);

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].spawn_type, 0);
    assert_eq!(events[1].spawn_type, 0);
}
```

- [ ] **Step 2: Run the targeted Rust tests and verify they fail for the right reason**

Run: `cargo test -p textquest spawn_watch_defaults_include_player_notifications spawn_delta_events_preserve_spawn_type_for_created_and_destroyed_entries -- --nocapture`

Expected: FAIL because the new config fields and spawn-event metadata do not exist yet.

- [ ] **Step 3: Add the minimal config and IPC model changes**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFilterMode {
    All,
    StrangersOnly,
    FriendsOnly,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SpawnWatchConfig {
    pub enabled: bool,
    pub watch_names: Vec<String>,
    pub alert_named: bool,
    pub max_feed_entries: usize,
    pub player_filter_mode: PlayerFilterMode,
    pub sound_on_player_zone_in: bool,
    pub friends: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpawnEvent {
    pub client_id: ClientId,
    pub zone: String,
    pub spawn_name: String,
    pub spawn_type: u8,
    pub kind: SpawnEventKind,
    pub timestamp_ms: u64,
}
```

- [ ] **Step 4: Update spawn-delta snapshotting in the DLL to populate the new metadata**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct SpawnSnapshot {
    name: String,
    spawn_type: u8,
}

impl SpawnSnapshot {
    fn new(name: impl Into<String>, spawn_type: u8) -> Self {
        Self {
            name: name.into(),
            spawn_type,
        }
    }
}

for spawn in current {
    next.insert(
        spawn.spawn_id,
        SpawnSnapshot::new(spawn.displayed_name.clone(), spawn.spawn_type),
    );
}
```

- [ ] **Step 5: Re-run the targeted Rust tests and verify they pass**

Run: `cargo test -p textquest spawn_watch_defaults_include_player_notifications spawn_delta_events_preserve_spawn_type_for_created_and_destroyed_entries -- --nocapture`

Expected: PASS

### Task 2: Add Player-Aware TUI Notifications, Event Stream Panel, And Sound Trigger

**Files:**
- Modify: `textquest/src/cli.rs`
- Modify: `textquest/src/tui/app.rs`
- Modify: `textquest/src/tui/state.rs`
- Modify: `textquest/src/tui/ui/map.rs`
- Modify: `textquest/src/tui/run.rs`

- [ ] **Step 1: Write the failing Rust tests for player filtering, friend matching, and sound trigger requests**

```rust
#[test]
fn player_zone_notifications_respect_friend_filter_modes() {
    let mut app = App::new();
    app.player_notification_filter = PlayerFilterMode::StrangersOnly;
    app.set_player_notification_friends(["FriendOne"]);

    app.apply_spawn_events(vec![SpawnEvent {
        client_id: 7,
        zone: "greatdivide".into(),
        spawn_name: "FriendOne".into(),
        spawn_type: 0,
        kind: SpawnEventKind::Created,
        timestamp_ms: 1,
    }]);

    assert!(!app.status_message.contains("FriendOne"));
}

#[test]
fn player_zone_in_with_sound_enabled_requests_terminal_bell() {
    let mut app = App::new();
    app.sound_on_player_zone_in = true;

    app.apply_spawn_events(vec![SpawnEvent {
        client_id: 7,
        zone: "greatdivide".into(),
        spawn_name: "Visitor".into(),
        spawn_type: 0,
        kind: SpawnEventKind::Created,
        timestamp_ms: 1,
    }]);

    assert_eq!(app.pending_terminal_bells(), 1);
}
```

- [ ] **Step 2: Run the targeted TUI tests and verify they fail**

Run: `cargo test -p textquest player_zone_notifications_respect_friend_filter_modes player_zone_in_with_sound_enabled_requests_terminal_bell -- --nocapture`

Expected: FAIL because the player-notification policy state and bell queue do not exist yet.

- [ ] **Step 3: Add minimal app state and helper methods for player notification policy**

```rust
pub struct App {
    pub player_notification_filter: PlayerFilterMode,
    pub sound_on_player_zone_in: bool,
    player_notification_friends: HashSet<String>,
    pending_terminal_bells: u8,
}

fn should_announce_player(&self, name: &str) -> bool {
    let is_friend = self.player_notification_friends.contains(&name.to_ascii_lowercase());
    match self.player_notification_filter {
        PlayerFilterMode::All => true,
        PlayerFilterMode::StrangersOnly => !is_friend,
        PlayerFilterMode::FriendsOnly => is_friend,
    }
}
```

- [ ] **Step 4: Wire the new policy into `apply_spawn_events` while keeping the full event feed**

```rust
let is_player = event.spawn_type == 0;
if is_player && self.should_announce_player(&event.spawn_name) {
    let verb = if is_up { "entered" } else { "left" };
    self.set_feedback(
        if is_up { ToastLevel::Warning } else { ToastLevel::Info },
        format!("[Player] {} {verb} {}", event.spawn_name, event.zone),
        true,
    );
    if is_up && self.sound_on_player_zone_in {
        self.pending_terminal_bells = self.pending_terminal_bells.saturating_add(1);
    }
}
```

- [ ] **Step 5: Render a dedicated tactical event stream panel from `spawn_alert_feed`**

```rust
enum TacticalSectionKind {
    Named,
    Events,
    Navigation,
}

fn draw_spawn_event_panel(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let events = app.spawn_alert_feed.events();
    let rows: Vec<Row> = events
        .iter()
        .rev()
        .take(6)
        .map(|event| {
            Row::new(vec![
                Cell::from(if event.is_up { "IN" } else { "OUT" }),
                Cell::from(app.redact_name(&event.spawn_name).into_owned()),
                Cell::from(event.zone.clone()),
            ])
        })
        .collect();
    // render as its own panel block
}
```

- [ ] **Step 6: Drain pending bell requests from the TUI run loop**

Run: add a small hook in `textquest/src/tui/run.rs` that emits `\x07` once per queued bell after `apply_spawn_events`.

Expected behavior: one terminal bell per matching player zone-in event when enabled.

- [ ] **Step 7: Re-run the targeted TUI tests and the tactical UI tests**

Run: `cargo test -p textquest player_zone_notifications_respect_friend_filter_modes player_zone_in_with_sound_enabled_requests_terminal_bell -- --nocapture`

Expected: PASS

### Task 3: Expose Player Notification Settings Through The Web API

**Files:**
- Modify: `textquest-web/Cargo.toml`
- Modify: `textquest-web/src/api.rs`
- Modify: `textquest-web/src/main.rs`

- [ ] **Step 1: Write the failing Axum tests for the new config route**

```rust
#[tokio::test]
async fn player_watch_config_roundtrips() {
    let app = build_app(build_state());

    let (status, body) = json_response(
        app.clone(),
        Request::builder()
            .uri("/api/config/player-watch")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["filter_mode"], "all");
}
```

- [ ] **Step 2: Run the web-backend tests and verify they fail**

Run: `cargo test -p textquest-web player_watch_config_roundtrips -- --nocapture`

Expected: FAIL because the route and state do not exist yet.

- [ ] **Step 3: Add a focused API model plus in-memory state seeded from config defaults**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerWatchConfig {
    pub filter_mode: String,
    pub sound_on_zone_in: bool,
    pub friends: Vec<String>,
}

pub async fn get_player_watch_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let cfg = state.player_watch_config.read().await.clone();
    (StatusCode::OK, Json(cfg))
}
```

- [ ] **Step 4: Mount the route and add PUT support**

```rust
.route(
    "/config/player-watch",
    get(api::get_player_watch_config).put(api::put_player_watch_config),
)
```

- [ ] **Step 5: Re-run the web-backend tests and verify they pass**

Run: `cargo test -p textquest-web player_watch_config_roundtrips -- --nocapture`

Expected: PASS

### Task 4: Add The Web Dashboard Configuration Panel

**Files:**
- Create: `web/src/hooks/usePlayerWatchConfig.ts`
- Create: `web/src/hooks/usePlayerWatchConfig.test.tsx`
- Create: `web/src/components/PlayerWatchPanel.tsx`
- Modify: `web/src/types.ts`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/LeftSidebar.tsx`

- [ ] **Step 1: Write the failing hook test for fetch and save behavior**

```tsx
it("loads and saves player watch config", async () => {
  const fetchMock = vi.mocked(fetch);
  fetchMock
    .mockResolvedValueOnce(
      jsonResponse({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: ["FriendOne"],
      }),
    )
    .mockResolvedValueOnce(jsonResponse({ updated: true }))
    .mockResolvedValueOnce(
      jsonResponse({
        filter_mode: "friends_only",
        sound_on_zone_in: true,
        friends: ["FriendOne", "FriendTwo"],
      }),
    );

  const { result } = renderHook(() => usePlayerWatchConfig());
  await waitFor(() => expect(result.current.loading).toBe(false));
  await act(async () => {
    await result.current.saveConfig({
      filter_mode: "friends_only",
      sound_on_zone_in: true,
      friends: ["FriendOne", "FriendTwo"],
    });
  });
  expect(fetchMock).toHaveBeenNthCalledWith(
    2,
    "/api/config/player-watch",
    expect.objectContaining({ method: "PUT" }),
  );
});
```

- [ ] **Step 2: Run the web hook test and verify it fails**

Run: `npm --prefix web test -- usePlayerWatchConfig`

Expected: FAIL because the hook and type do not exist yet.

- [ ] **Step 3: Add the minimal hook and type definitions**

```tsx
export interface PlayerWatchConfig {
  filter_mode: "all" | "strangers_only" | "friends_only";
  sound_on_zone_in: boolean;
  friends: string[];
}
```

- [ ] **Step 4: Add a dedicated security/player-watch panel and route the sidebar to it**

```tsx
{activeView === "security" ? (
  <PlayerWatchPanel />
) : activeView === "formations" ? (
  <GroupBuilder />
) : /* existing branches */ null}
```

- [ ] **Step 5: Re-run the web hook test and the frontend test suite slice**

Run: `npm --prefix web test -- usePlayerWatchConfig`

Expected: PASS

### Task 5: Update Docs And Run Final Verification

**Files:**
- Modify: `docs/wiki/Configuration.md`
- Modify: `docs/wiki/Operator-Guide.md`

- [ ] **Step 1: Document the new config block and operator-facing TUI behavior**

```toml
[spawn_watch]
player_filter_mode = "strangers_only"
sound_on_player_zone_in = true
friends = ["Camrene", "Zisdarenu"]
```

- [ ] **Step 2: Run focused Rust verification**

Run: `cargo test -p textquest spawn_ -- --nocapture`

Expected: PASS for the touched spawn/player-notification tests.

- [ ] **Step 3: Run focused web-backend verification**

Run: `cargo test -p textquest-web player_watch_config_roundtrips -- --nocapture`

Expected: PASS

- [ ] **Step 4: Run focused frontend verification**

Run: `npm --prefix web test -- usePlayerWatchConfig`

Expected: PASS

- [ ] **Step 5: Run formatting on the touched crates**

Run: `cargo fmt --all`

Expected: no diff after formatting.
