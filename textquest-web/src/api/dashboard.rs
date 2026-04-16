//! Integrated operator dashboard snapshot and action handlers.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub generated_at: String,
    pub environment: EnvironmentSummary,
    pub sessions: SessionSection,
    pub groups: GroupSection,
    pub navigation: NavigationSection,
    pub economy: EconomySection,
    pub combat: CombatSection,
    pub health: HealthSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSummary {
    pub cluster: String,
    pub shard: String,
    pub zone: String,
    pub websocket_connected: bool,
    pub alerts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSection {
    pub recovery_enabled: bool,
    pub profiles: Vec<String>,
    pub items: Vec<SessionCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCard {
    pub client_id: u32,
    pub character_name: String,
    pub profile: String,
    pub group_id: String,
    pub zone: String,
    pub level: u8,
    pub hp_pct: u8,
    pub mana_pct: u8,
    pub status: SessionStatus,
    pub recovery_state: RecoveryState,
    pub last_heartbeat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Online,
    Offline,
    Stuck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryState {
    Stable,
    Respawning,
    Waiting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSection {
    pub items: Vec<GroupCard>,
    pub command_log: Vec<GroupCommandLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCard {
    pub id: String,
    pub name: String,
    pub zone: String,
    pub formation: String,
    pub current_command: String,
    pub members: Vec<GroupMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    pub character_name: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCommandLogEntry {
    pub id: String,
    pub issued_at: String,
    pub group_name: String,
    pub command: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationSection {
    pub current_zone: String,
    pub active_route_id: String,
    pub stuck_clients: u32,
    pub routes: Vec<RouteCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteCard {
    pub id: String,
    pub name: String,
    pub zone: String,
    pub destination: String,
    pub progress_pct: u8,
    pub waypoints: Vec<Waypoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waypoint {
    pub id: String,
    pub x: u16,
    pub y: u16,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomySection {
    pub items_received: u32,
    pub last_vendor_run: String,
    pub total_profit: u32,
    pub profit_trend: Vec<TrendPoint>,
    pub recent_loot: Vec<LootRecord>,
    pub wishlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub label: String,
    pub value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LootRecord {
    pub id: String,
    pub item_name: String,
    pub recipient: String,
    pub source: String,
    pub distribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatSection {
    pub dps_series: Vec<DpsSeries>,
    pub spell_usage: Vec<SpellUsage>,
    pub death_log: Vec<DeathEntry>,
    pub rotations: Vec<RotationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpsSeries {
    pub character_name: String,
    pub color: String,
    pub samples: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellUsage {
    pub spell_name: String,
    pub casts: u32,
    pub efficiency: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeathEntry {
    pub id: String,
    pub character_name: String,
    pub reason: String,
    pub recovered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationSummary {
    pub character_name: String,
    pub efficiency: u8,
    pub drift_ms: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSection {
    pub clients: Vec<ClientHealth>,
    pub ipc_latency: LatencySummary,
    pub error_log: Vec<ErrorLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHealth {
    pub client_id: u32,
    pub character_name: String,
    pub memory_mb: u16,
    pub frame_rate: u8,
    pub status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencySummary {
    pub p50: u16,
    pub p95: u16,
    pub p99: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLogEntry {
    pub id: String,
    pub severity: ErrorSeverity,
    pub message: String,
    pub recovery_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DashboardActionRequest {
    CreateSession {
        profile: String,
        character_name: String,
    },
    TerminateSession {
        client_id: u32,
    },
    RecoverSession {
        client_id: u32,
    },
    CreateGroup {
        name: String,
        zone: String,
        formation: String,
    },
    UpdateGroup {
        group_id: String,
        formation: String,
        members: Vec<GroupMember>,
    },
    IssueGroupCommand {
        group_id: String,
        command: String,
    },
    CreateRoute {
        name: String,
        zone: String,
        destination: String,
        waypoints: Vec<Waypoint>,
    },
    SetActiveRoute {
        route_id: String,
    },
    UpdateWishlist {
        items: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    source: &'static str,
    snapshot: DashboardSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardActionError {
    error: String,
}

pub struct DashboardState {
    snapshot: RwLock<DashboardSnapshot>,
    tick_count: AtomicU64,
}

impl DashboardState {
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            snapshot: RwLock::new(demo_snapshot()),
            tick_count: AtomicU64::new(0),
        })
    }

    pub async fn current_snapshot(&self) -> DashboardSnapshot {
        self.snapshot.read().await.clone()
    }

    async fn apply_action(
        &self,
        action: DashboardActionRequest,
    ) -> Result<DashboardSnapshot, DashboardActionError> {
        let mut snapshot = self.snapshot.write().await;

        match action {
            DashboardActionRequest::CreateSession {
                profile,
                character_name,
            } => {
                let profile = profile.trim().to_string();
                let character_name = character_name.trim().to_string();
                if character_name.is_empty() {
                    return Err(DashboardActionError {
                        error: "Character name is required".to_string(),
                    });
                }
                if !snapshot
                    .sessions
                    .profiles
                    .iter()
                    .any(|item| item == &profile)
                {
                    return Err(DashboardActionError {
                        error: format!("Unknown session profile: {profile}"),
                    });
                }
                let current_zone = snapshot.environment.zone.clone();
                let next_client_id = snapshot
                    .sessions
                    .items
                    .iter()
                    .map(|session| session.client_id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let group_id = snapshot
                    .groups
                    .items
                    .first()
                    .map(|group| group.id.clone())
                    .unwrap_or_else(|| "grp-unassigned".to_string());
                snapshot.sessions.items.push(SessionCard {
                    client_id: next_client_id,
                    character_name: character_name.clone(),
                    profile,
                    group_id,
                    zone: current_zone,
                    level: 60,
                    hp_pct: 100,
                    mana_pct: 100,
                    status: SessionStatus::Online,
                    recovery_state: RecoveryState::Stable,
                    last_heartbeat: "0s ago".to_string(),
                });
                snapshot.health.clients.push(ClientHealth {
                    client_id: next_client_id,
                    character_name: character_name.clone(),
                    memory_mb: 676,
                    frame_rate: 60,
                    status: HealthStatus::Healthy,
                });
                snapshot.groups.command_log.insert(
                    0,
                    GroupCommandLogEntry {
                        id: format!("cmd-{}", Uuid::new_v4()),
                        issued_at: short_time_label(),
                        group_name: "Command Deck".to_string(),
                        command: format!("create session: {character_name}"),
                        status: "queued".to_string(),
                    },
                );
            }
            DashboardActionRequest::TerminateSession { client_id } => {
                if let Some(session) = snapshot
                    .sessions
                    .items
                    .iter_mut()
                    .find(|session| session.client_id == client_id)
                {
                    session.status = SessionStatus::Offline;
                    session.recovery_state = RecoveryState::Waiting;
                    session.last_heartbeat = "offline".to_string();
                }
                if let Some(client) = snapshot
                    .health
                    .clients
                    .iter_mut()
                    .find(|client| client.client_id == client_id)
                {
                    client.status = HealthStatus::Critical;
                    client.frame_rate = 0;
                }
            }
            DashboardActionRequest::RecoverSession { client_id } => {
                if let Some(session) = snapshot
                    .sessions
                    .items
                    .iter_mut()
                    .find(|session| session.client_id == client_id)
                {
                    session.status = SessionStatus::Online;
                    session.recovery_state = RecoveryState::Stable;
                    session.hp_pct = session.hp_pct.max(80);
                    session.mana_pct = session.mana_pct.max(60);
                    session.last_heartbeat = "1s ago".to_string();
                }
                if let Some(client) = snapshot
                    .health
                    .clients
                    .iter_mut()
                    .find(|client| client.client_id == client_id)
                {
                    client.status = HealthStatus::Healthy;
                    client.frame_rate = client.frame_rate.max(48);
                }
            }
            DashboardActionRequest::CreateGroup {
                name,
                zone,
                formation,
            } => {
                snapshot.groups.items.push(GroupCard {
                    id: format!("grp-{}", Uuid::new_v4()),
                    name: name.clone(),
                    zone,
                    formation,
                    current_command: "camp".to_string(),
                    members: Vec::new(),
                });
                snapshot.groups.command_log.insert(
                    0,
                    GroupCommandLogEntry {
                        id: format!("cmd-{}", Uuid::new_v4()),
                        issued_at: short_time_label(),
                        group_name: name,
                        command: "group created".to_string(),
                        status: "applied".to_string(),
                    },
                );
            }
            DashboardActionRequest::UpdateGroup {
                group_id,
                formation,
                members,
            } => {
                if let Some(group) = snapshot
                    .groups
                    .items
                    .iter_mut()
                    .find(|group| group.id == group_id)
                {
                    group.formation = formation;
                    group.members = members;
                }
            }
            DashboardActionRequest::IssueGroupCommand { group_id, command } => {
                if let Some(group_index) = snapshot
                    .groups
                    .items
                    .iter()
                    .position(|group| group.id == group_id)
                {
                    let group_name = {
                        let group = &mut snapshot.groups.items[group_index];
                        group.current_command = command.clone();
                        group.name.clone()
                    };
                    snapshot.groups.command_log.insert(
                        0,
                        GroupCommandLogEntry {
                            id: format!("cmd-{}", Uuid::new_v4()),
                            issued_at: short_time_label(),
                            group_name,
                            command,
                            status: "applied".to_string(),
                        },
                    );
                }
            }
            DashboardActionRequest::CreateRoute {
                name,
                zone,
                destination,
                waypoints,
            } => {
                let route_id = format!("route-{}", Uuid::new_v4());
                snapshot.navigation.active_route_id = route_id.clone();
                snapshot.navigation.routes.push(RouteCard {
                    id: route_id,
                    name,
                    zone,
                    destination,
                    progress_pct: 0,
                    waypoints,
                });
            }
            DashboardActionRequest::SetActiveRoute { route_id } => {
                if snapshot
                    .navigation
                    .routes
                    .iter()
                    .any(|route| route.id == route_id)
                {
                    snapshot.navigation.active_route_id = route_id;
                }
            }
            DashboardActionRequest::UpdateWishlist { items } => {
                snapshot.economy.wishlist = items;
            }
        }

        refresh_snapshot(&mut snapshot);
        Ok(snapshot.clone())
    }

    pub async fn tick(&self) -> DashboardSnapshot {
        let step = self.tick_count.fetch_add(1, Ordering::Relaxed) + 1;
        let mut snapshot = self.snapshot.write().await;

        for (index, session) in snapshot.sessions.items.iter_mut().enumerate() {
            let offset = ((step + index as u64) % 5) as u8;
            match session.status {
                SessionStatus::Online => {
                    session.hp_pct = session.hp_pct.saturating_sub(offset).max(68);
                    session.mana_pct = session.mana_pct.saturating_sub(offset / 2).max(32);
                    session.last_heartbeat = format!("{}s ago", (step + index as u64) % 7);
                }
                SessionStatus::Stuck => {
                    session.last_heartbeat = format!("{}s ago", 10 + (step % 5));
                }
                SessionStatus::Offline => {}
            }
        }

        for (index, route) in snapshot.navigation.routes.iter_mut().enumerate() {
            let delta = ((step + index as u64) % 6) as u8;
            route.progress_pct = (route.progress_pct.saturating_add(delta)).min(100);
        }

        snapshot.economy.total_profit = snapshot
            .economy
            .total_profit
            .saturating_add(57 + (step % 3) as u32 * 11);
        let total_profit = snapshot.economy.total_profit;
        if let Some(last) = snapshot.economy.profit_trend.last_mut() {
            last.value = total_profit;
        }

        for (index, series) in snapshot.combat.dps_series.iter_mut().enumerate() {
            if !series.samples.is_empty() {
                series.samples.rotate_left(1);
                if let Some(last) = series.samples.last_mut() {
                    *last = last.saturating_add(((step + index as u64) % 250) as u32);
                }
            }
        }

        for (index, spell) in snapshot.combat.spell_usage.iter_mut().enumerate() {
            spell.casts = spell.casts.saturating_add((step as u32 + index as u32) % 2);
            spell.efficiency = spell.efficiency.saturating_sub((step % 2) as u8).max(82);
        }

        for (index, client) in snapshot.health.clients.iter_mut().enumerate() {
            client.memory_mb = client
                .memory_mb
                .saturating_add(((step + index as u64) % 4) as u16);
            if client.status != HealthStatus::Critical {
                client.frame_rate = client
                    .frame_rate
                    .saturating_sub(((step + index as u64) % 3) as u8)
                    .max(38);
            }
        }

        snapshot.health.ipc_latency = LatencySummary {
            p50: 8 + (step % 6) as u16,
            p95: 20 + (step % 11) as u16,
            p99: 34 + (step % 17) as u16,
        };

        refresh_snapshot(&mut snapshot);
        snapshot.clone()
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_dashboard))
        .route("/action", post(apply_dashboard_action))
}

async fn get_dashboard(State(state): State<Arc<AppState>>) -> Json<DashboardSnapshot> {
    let mut snapshot = state.dashboard_state.current_snapshot().await;
    hydrate_runtime_state(state.as_ref(), &mut snapshot);
    Json(snapshot)
}

async fn apply_dashboard_action(
    State(state): State<Arc<AppState>>,
    Json(action): Json<DashboardActionRequest>,
) -> Result<Json<DashboardSnapshot>, (StatusCode, Json<DashboardActionError>)> {
    let mut snapshot = state
        .dashboard_state
        .apply_action(action)
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(error)))?;
    hydrate_runtime_state(state.as_ref(), &mut snapshot);
    broadcast_snapshot(state.as_ref(), snapshot.clone(), "action");
    Ok(Json(snapshot))
}

pub fn spawn_dashboard_tick_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let snapshot = state.dashboard_state.tick().await;
            broadcast_snapshot(state.as_ref(), snapshot, "tick");
        }
    });
}

fn broadcast_snapshot(state: &AppState, snapshot: DashboardSnapshot, source: &'static str) {
    let mut snapshot = snapshot;
    hydrate_runtime_state(state, &mut snapshot);
    if let Ok(event) = serde_json::to_string(&DashboardEvent {
        kind: "dashboard.snapshot",
        source,
        snapshot,
    }) {
        let _ = state.event_tx.send(event);
    }
}

fn demo_snapshot() -> DashboardSnapshot {
    let mut snapshot = DashboardSnapshot {
        generated_at: iso_timestamp(),
        environment: EnvironmentSummary {
            cluster: "Teek".to_string(),
            shard: "operator-1".to_string(),
            zone: "Plane of Fire".to_string(),
            websocket_connected: true,
            alerts: 0,
        },
        sessions: SessionSection {
            recovery_enabled: true,
            profiles: vec![
                "Cleric Anchor".to_string(),
                "Pull Squad".to_string(),
                "Loot Crew".to_string(),
            ],
            items: vec![
                SessionCard {
                    client_id: 1,
                    character_name: "Frostreaver".to_string(),
                    profile: "Cleric Anchor".to_string(),
                    group_id: "grp-1".to_string(),
                    zone: "Plane of Fire".to_string(),
                    level: 60,
                    hp_pct: 98,
                    mana_pct: 87,
                    status: SessionStatus::Online,
                    recovery_state: RecoveryState::Stable,
                    last_heartbeat: "2s ago".to_string(),
                },
                SessionCard {
                    client_id: 2,
                    character_name: "Noxus".to_string(),
                    profile: "Pull Squad".to_string(),
                    group_id: "grp-1".to_string(),
                    zone: "Plane of Fire".to_string(),
                    level: 60,
                    hp_pct: 72,
                    mana_pct: 24,
                    status: SessionStatus::Stuck,
                    recovery_state: RecoveryState::Respawning,
                    last_heartbeat: "12s ago".to_string(),
                },
            ],
        },
        groups: GroupSection {
            items: vec![
                GroupCard {
                    id: "grp-1".to_string(),
                    name: "Fire Core".to_string(),
                    zone: "Plane of Fire".to_string(),
                    formation: "Tight Camp".to_string(),
                    current_command: "camp".to_string(),
                    members: vec![
                        GroupMember {
                            character_name: "Noxus".to_string(),
                            role: "tank".to_string(),
                            status: "engaged".to_string(),
                        },
                        GroupMember {
                            character_name: "Frostreaver".to_string(),
                            role: "healer".to_string(),
                            status: "support".to_string(),
                        },
                        GroupMember {
                            character_name: "Aelrindel".to_string(),
                            role: "dps".to_string(),
                            status: "engaged".to_string(),
                        },
                    ],
                },
                GroupCard {
                    id: "grp-2".to_string(),
                    name: "Backline Sweep".to_string(),
                    zone: "Plane of Fire".to_string(),
                    formation: "Loose Arc".to_string(),
                    current_command: "navigate".to_string(),
                    members: vec![
                        GroupMember {
                            character_name: "Valerius".to_string(),
                            role: "dps".to_string(),
                            status: "rotating".to_string(),
                        },
                        GroupMember {
                            character_name: "Mystik".to_string(),
                            role: "puller".to_string(),
                            status: "pathing".to_string(),
                        },
                    ],
                },
            ],
            command_log: vec![
                GroupCommandLogEntry {
                    id: "cmd-1".to_string(),
                    issued_at: "08:00".to_string(),
                    group_name: "Fire Core".to_string(),
                    command: "camp".to_string(),
                    status: "applied".to_string(),
                },
                GroupCommandLogEntry {
                    id: "cmd-2".to_string(),
                    issued_at: "08:02".to_string(),
                    group_name: "Backline Sweep".to_string(),
                    command: "navigate".to_string(),
                    status: "queued".to_string(),
                },
            ],
        },
        navigation: NavigationSection {
            current_zone: "Plane of Fire".to_string(),
            active_route_id: "route-1".to_string(),
            stuck_clients: 1,
            routes: vec![
                RouteCard {
                    id: "route-1".to_string(),
                    name: "Fire Ring Sweep".to_string(),
                    zone: "Plane of Fire".to_string(),
                    destination: "Magi Ring".to_string(),
                    progress_pct: 62,
                    waypoints: vec![
                        Waypoint {
                            id: "wp-1".to_string(),
                            x: 10,
                            y: 20,
                            label: "Camp".to_string(),
                        },
                        Waypoint {
                            id: "wp-2".to_string(),
                            x: 64,
                            y: 48,
                            label: "Ridge".to_string(),
                        },
                        Waypoint {
                            id: "wp-3".to_string(),
                            x: 88,
                            y: 20,
                            label: "Ring".to_string(),
                        },
                    ],
                },
                RouteCard {
                    id: "route-2".to_string(),
                    name: "Vendor Return".to_string(),
                    zone: "Plane of Knowledge".to_string(),
                    destination: "Smith Rondo".to_string(),
                    progress_pct: 18,
                    waypoints: vec![
                        Waypoint {
                            id: "wp-4".to_string(),
                            x: 18,
                            y: 70,
                            label: "Bank".to_string(),
                        },
                        Waypoint {
                            id: "wp-5".to_string(),
                            x: 42,
                            y: 58,
                            label: "Tunnel".to_string(),
                        },
                    ],
                },
            ],
        },
        economy: EconomySection {
            items_received: 17,
            last_vendor_run: "14m ago".to_string(),
            total_profit: 18_423,
            profit_trend: vec![
                TrendPoint {
                    label: "Mon".to_string(),
                    value: 2_100,
                },
                TrendPoint {
                    label: "Tue".to_string(),
                    value: 2_800,
                },
                TrendPoint {
                    label: "Wed".to_string(),
                    value: 3_100,
                },
                TrendPoint {
                    label: "Thu".to_string(),
                    value: 2_700,
                },
                TrendPoint {
                    label: "Fri".to_string(),
                    value: 4_100,
                },
            ],
            recent_loot: vec![
                LootRecord {
                    id: "loot-1".to_string(),
                    item_name: "Mace of Fiery Might".to_string(),
                    recipient: "Noxus".to_string(),
                    source: "Fennin".to_string(),
                    distribution: "main assist".to_string(),
                },
                LootRecord {
                    id: "loot-2".to_string(),
                    item_name: "Spell: Ancient Flame".to_string(),
                    recipient: "Frostreaver".to_string(),
                    source: "Tables".to_string(),
                    distribution: "wishlist".to_string(),
                },
            ],
            wishlist: vec![
                "Spell: Ancient Flame".to_string(),
                "Earring of the Forge".to_string(),
            ],
        },
        combat: CombatSection {
            dps_series: vec![
                DpsSeries {
                    character_name: "Aelrindel".to_string(),
                    color: "#60a5fa".to_string(),
                    samples: vec![1_200, 1_800, 2_400, 2_100],
                },
                DpsSeries {
                    character_name: "Noxus".to_string(),
                    color: "#f97316".to_string(),
                    samples: vec![900, 1_100, 950, 1_050],
                },
            ],
            spell_usage: vec![
                SpellUsage {
                    spell_name: "Ice Comet".to_string(),
                    casts: 24,
                    efficiency: 91,
                },
                SpellUsage {
                    spell_name: "Complete Heal".to_string(),
                    casts: 12,
                    efficiency: 97,
                },
            ],
            death_log: vec![DeathEntry {
                id: "death-1".to_string(),
                character_name: "Valerius".to_string(),
                reason: "Lava pathing".to_string(),
                recovered_at: "08:05".to_string(),
            }],
            rotations: vec![
                RotationSummary {
                    character_name: "Aelrindel".to_string(),
                    efficiency: 88,
                    drift_ms: 420,
                },
                RotationSummary {
                    character_name: "Frostreaver".to_string(),
                    efficiency: 95,
                    drift_ms: 160,
                },
            ],
        },
        health: HealthSection {
            clients: vec![
                ClientHealth {
                    client_id: 1,
                    character_name: "Frostreaver".to_string(),
                    memory_mb: 684,
                    frame_rate: 58,
                    status: HealthStatus::Healthy,
                },
                ClientHealth {
                    client_id: 2,
                    character_name: "Noxus".to_string(),
                    memory_mb: 742,
                    frame_rate: 41,
                    status: HealthStatus::Warning,
                },
            ],
            ipc_latency: LatencySummary {
                p50: 9,
                p95: 22,
                p99: 37,
            },
            error_log: vec![ErrorLogEntry {
                id: "err-1".to_string(),
                severity: ErrorSeverity::Warning,
                message: "Noxus exceeded stuck threshold".to_string(),
                recovery_action: "Respawn requested".to_string(),
            }],
        },
    };

    refresh_snapshot(&mut snapshot);
    snapshot
}

fn refresh_snapshot(snapshot: &mut DashboardSnapshot) {
    snapshot.generated_at = iso_timestamp();
    snapshot.navigation.stuck_clients = snapshot
        .sessions
        .items
        .iter()
        .filter(|session| session.status == SessionStatus::Stuck)
        .count() as u32;
    snapshot.environment.zone = snapshot.navigation.current_zone.clone();
    snapshot.environment.alerts = snapshot
        .health
        .error_log
        .iter()
        .filter(|entry| entry.severity != ErrorSeverity::Info)
        .count() as u32
        + snapshot.navigation.stuck_clients;

    for client in &mut snapshot.health.clients {
        if let Some(session) = snapshot
            .sessions
            .items
            .iter()
            .find(|session| session.client_id == client.client_id)
        {
            client.character_name = session.character_name.clone();
            client.status = match session.status {
                SessionStatus::Online => {
                    if client.frame_rate < 45 {
                        HealthStatus::Warning
                    } else {
                        HealthStatus::Healthy
                    }
                }
                SessionStatus::Stuck => HealthStatus::Warning,
                SessionStatus::Offline => HealthStatus::Critical,
            };
        }
    }
}

fn hydrate_runtime_state(state: &AppState, snapshot: &mut DashboardSnapshot) {
    snapshot.environment.websocket_connected = state.event_tx.receiver_count() > 0;
}

fn iso_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn short_time_label() -> String {
    Utc::now().format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde_json::Value;

    use crate::{AppState, accounts, api};

    fn test_state() -> Arc<AppState> {
        let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            discord_state: api::discord::DiscordState::new_demo(),
            api_token: None,
        })
    }

    #[tokio::test]
    async fn get_dashboard_returns_snapshot_with_all_sections() {
        let state = test_state();
        let _receiver = state.event_tx.subscribe();
        let response = get_dashboard(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let json: Value = serde_json::from_slice(&body).expect("dashboard json");

        for field in [
            "environment",
            "sessions",
            "groups",
            "navigation",
            "economy",
            "combat",
            "health",
        ] {
            assert!(
                json.get(field).is_some(),
                "expected dashboard snapshot to include `{field}`"
            );
        }
    }

    #[tokio::test]
    async fn post_dashboard_action_mutates_snapshot_and_broadcasts_event() {
        let state = test_state();
        let mut receiver = state.event_tx.subscribe();

        let response = apply_dashboard_action(
            State(state.clone()),
            axum::Json(DashboardActionRequest::CreateSession {
                profile: "Loot Crew".to_string(),
                character_name: "Newpuller".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let json: Value = serde_json::from_slice(&body).expect("dashboard action response");

        let sessions = json["sessions"]["items"]
            .as_array()
            .expect("sessions array");
        assert!(
            sessions
                .iter()
                .any(|session| session["characterName"] == "Newpuller"),
            "new session should be present in the updated snapshot"
        );

        let event = receiver.recv().await.expect("dashboard event");
        let event_json: Value = serde_json::from_str(&event).expect("dashboard event json");
        assert_eq!(event_json["type"], "dashboard.snapshot");
        assert_eq!(event_json["source"], "action");
        assert_eq!(
            event_json["snapshot"]["environment"]["websocketConnected"],
            true
        );
    }

    #[tokio::test]
    async fn post_dashboard_action_rejects_invalid_session_requests() {
        let state = test_state();
        let response = apply_dashboard_action(
            State(state),
            axum::Json(DashboardActionRequest::CreateSession {
                profile: "Unknown".to_string(),
                character_name: "   ".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let json: Value = serde_json::from_slice(&body).expect("dashboard error response");
        assert_eq!(json["error"], "Character name is required");
    }
}
