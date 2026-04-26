//! Integrated operator dashboard snapshot and action handlers.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use textquest_common::nav::{
    RelocationOptionState, RelocationSourceKind, build_relocation_destination_statuses,
    relocation_catalog,
};
use textquest_common::spawn_finder::LiveSpawnSnapshot;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub generated_at: String,
    pub environment: EnvironmentSummary,
    pub sessions: SessionSection,
    pub spawn_finder: SpawnFinderSection,
    pub groups: GroupSection,
    pub navigation: NavigationSection,
    pub relocation: RelocationSection,
    pub economy: EconomySection,
    pub combat: CombatSection,
    pub health: HealthSection,
    pub affinity: Option<AffinitySection>,
    pub camera: Option<CameraSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffinitySection {
    pub enabled: bool,
    pub focused_client_id: Option<u32>,
    pub assignments: Vec<AffinityAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffinityAssignment {
    pub client_id: u32,
    pub cpu_mask: u64,
    pub priority: String,
    pub is_focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSection {
    pub presets: Vec<CameraPresetCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPresetCard {
    pub name: String,
    pub hotkey: Option<String>,
    pub distance: Option<f32>,
    pub is_default: bool,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnFinderSection {
    pub observers: Vec<SpawnObserverSummary>,
    pub items: Vec<SpawnFinderItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnObserverSummary {
    pub client_id: u32,
    pub character_name: String,
    pub zone: String,
    pub total_spawns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnFinderItem {
    pub observer_client_id: u32,
    pub observer_name: String,
    pub observer_zone: String,
    pub spawn_id: u32,
    pub name: String,
    pub spawn_type: String,
    pub level: u8,
    pub class_name: String,
    pub race_name: String,
    pub distance: u16,
    pub hp_pct: u8,
    pub is_current_target: bool,
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
pub struct CampOverlay {
    pub camp_center: [f32; 2],
    pub pull_point: [f32; 2],
    pub camp_radius: f32,
    pub pull_radius: f32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationSection {
    pub current_zone: String,
    pub active_route_id: String,
    pub stuck_clients: u32,
    pub routes: Vec<RouteCard>,
    pub camp_overlay: Option<CampOverlay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationSection {
    pub ready_destinations: u32,
    pub cooling_down_count: u32,
    pub destinations: Vec<RelocationDestinationCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationDestinationCard {
    pub zone: String,
    pub label: String,
    pub preferred_option: Option<String>,
    pub preferred_source: Option<RelocationSourceKind>,
    pub options: Vec<RelocationOptionCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationOptionCard {
    pub id: String,
    pub name: String,
    pub source: RelocationSourceKind,
    pub owned: bool,
    pub ready: bool,
    pub cooldown_remaining_secs: Option<u32>,
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
    TargetSpawn {
        client_id: u32,
        spawn_id: u32,
    },
    UpdateWishlist {
        items: Vec<String>,
    },
    SetAffinityEnabled {
        enabled: bool,
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
pub struct DashboardActionError {
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
            DashboardActionRequest::TargetSpawn { .. } => {}
            DashboardActionRequest::UpdateWishlist { items } => {
                snapshot.economy.wishlist = items;
            }
            DashboardActionRequest::SetAffinityEnabled { enabled } => {
                if let Some(ref mut affinity) = snapshot.affinity {
                    affinity.enabled = enabled;
                } else {
                    snapshot.affinity = Some(AffinitySection {
                        enabled,
                        focused_client_id: None,
                        assignments: Vec::new(),
                    });
                }
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

pub async fn get_dashboard(State(state): State<Arc<AppState>>) -> Json<DashboardSnapshot> {
    let mut snapshot = state.dashboard_state.current_snapshot().await;
    hydrate_runtime_state(state.as_ref(), &mut snapshot);
    Json(snapshot)
}

pub async fn apply_dashboard_action(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(action): Json<DashboardActionRequest>,
) -> Result<Json<DashboardSnapshot>, (StatusCode, Json<DashboardActionError>)> {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(DashboardActionError {
                error: "Untrusted origin".to_string(),
            }),
        ));
    }

    let mut snapshot = match action {
        DashboardActionRequest::TargetSpawn {
            client_id,
            spawn_id,
        } => {
            send_target_command(client_id, spawn_id).map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(DashboardActionError {
                        error: error.to_string(),
                    }),
                )
            })?;
            state.dashboard_state.current_snapshot().await
        }
        action => state
            .dashboard_state
            .apply_action(action)
            .await
            .map_err(|error| (StatusCode::BAD_REQUEST, Json(error)))?,
    };
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
        spawn_finder: SpawnFinderSection {
            observers: vec![
                SpawnObserverSummary {
                    client_id: 1,
                    character_name: "Frostreaver".to_string(),
                    zone: "Plane of Fire".to_string(),
                    total_spawns: 2,
                },
                SpawnObserverSummary {
                    client_id: 2,
                    character_name: "Noxus".to_string(),
                    zone: "Plane of Fire".to_string(),
                    total_spawns: 2,
                },
            ],
            items: vec![
                SpawnFinderItem {
                    observer_client_id: 1,
                    observer_name: "Frostreaver".to_string(),
                    observer_zone: "Plane of Fire".to_string(),
                    spawn_id: 9901,
                    name: "a fire giant".to_string(),
                    spawn_type: "NPC".to_string(),
                    level: 61,
                    class_name: "WAR".to_string(),
                    race_name: "Ogre".to_string(),
                    distance: 34,
                    hp_pct: 82,
                    is_current_target: true,
                },
                SpawnFinderItem {
                    observer_client_id: 1,
                    observer_name: "Frostreaver".to_string(),
                    observer_zone: "Plane of Fire".to_string(),
                    spawn_id: 9902,
                    name: "a lava walker".to_string(),
                    spawn_type: "NPC".to_string(),
                    level: 60,
                    class_name: "MNK".to_string(),
                    race_name: "Elemental".to_string(),
                    distance: 62,
                    hp_pct: 100,
                    is_current_target: false,
                },
                SpawnFinderItem {
                    observer_client_id: 2,
                    observer_name: "Noxus".to_string(),
                    observer_zone: "Plane of Fire".to_string(),
                    spawn_id: 9911,
                    name: "Frostreaver".to_string(),
                    spawn_type: "PC".to_string(),
                    level: 60,
                    class_name: "CLR".to_string(),
                    race_name: "Human".to_string(),
                    distance: 18,
                    hp_pct: 98,
                    is_current_target: false,
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
            camp_overlay: Some(CampOverlay {
                camp_center: [420.0, 680.0],
                pull_point: [510.0, 720.0],
                camp_radius: 80.0,
                pull_radius: 40.0,
                name: "Fire Core Camp".to_string(),
            }),
        },
        relocation: demo_relocation_section(),
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
        affinity: Some(AffinitySection {
            enabled: true,
            focused_client_id: Some(1),
            assignments: vec![
                AffinityAssignment {
                    client_id: 1,
                    cpu_mask: 2,
                    priority: "Normal".to_string(),
                    is_focused: true,
                },
                AffinityAssignment {
                    client_id: 2,
                    cpu_mask: 4,
                    priority: "BelowNormal".to_string(),
                    is_focused: false,
                },
            ],
        }),
        camera: Some(CameraSection {
            presets: vec![
                CameraPresetCard {
                    name: "First Person".to_string(),
                    hotkey: Some("F5".to_string()),
                    distance: None,
                    is_default: true,
                },
                CameraPresetCard {
                    name: "Close".to_string(),
                    hotkey: Some("F6".to_string()),
                    distance: Some(15.0),
                    is_default: false,
                },
                CameraPresetCard {
                    name: "Far".to_string(),
                    hotkey: Some("F7".to_string()),
                    distance: Some(100.0),
                    is_default: false,
                },
                CameraPresetCard {
                    name: "Overhead".to_string(),
                    hotkey: Some("F8".to_string()),
                    distance: None,
                    is_default: false,
                },
            ],
        }),
    };

    refresh_snapshot(&mut snapshot);
    snapshot
}

fn demo_relocation_section() -> RelocationSection {
    fn option_state(id: &str, cooldown_remaining_secs: Option<u32>) -> RelocationOptionState {
        let option = relocation_catalog()
            .into_iter()
            .find(|option| option.id == id)
            .expect("relocation catalog option");
        RelocationOptionState::new(option, true, cooldown_remaining_secs)
    }

    let options = vec![
        option_state("throne_of_heroes", None),
        option_state("origin", None),
        option_state("primary_anchor", Some(900)),
        option_state("secondary_anchor", Some(480)),
        option_state("nexus_gate_talisman", None),
    ];
    let destinations = build_relocation_destination_statuses(&options)
        .into_iter()
        .map(|status| RelocationDestinationCard {
            zone: status.zone_name,
            label: status.destination_label,
            preferred_option: status.preferred_option_name,
            preferred_source: status.preferred_source,
            options: status
                .options
                .into_iter()
                .map(|option| RelocationOptionCard {
                    id: option.option.id,
                    name: option.option.name,
                    source: option.option.source,
                    owned: option.owned,
                    ready: option.ready,
                    cooldown_remaining_secs: option.cooldown_remaining_secs,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    RelocationSection {
        ready_destinations: destinations
            .iter()
            .filter(|destination| destination.options.iter().any(|option| option.ready))
            .count() as u32,
        cooling_down_count: destinations
            .iter()
            .flat_map(|destination| destination.options.iter())
            .filter(|option| !option.ready && option.cooldown_remaining_secs.is_some())
            .count() as u32,
        destinations,
    }
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
    match read_live_spawn_snapshot(&spawn_snapshot_path(state)) {
        Ok(Some(live_snapshot)) => {
            snapshot.spawn_finder = build_spawn_finder_section(&live_snapshot);
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "Failed to read live spawn snapshot");
        }
    }
}

fn iso_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn short_time_label() -> String {
    Utc::now().format("%H:%M").to_string()
}

fn spawn_snapshot_path(state: &AppState) -> std::path::PathBuf {
    state
        .live_session_snapshot_path
        .with_file_name("live_spawns.json")
}

fn read_live_spawn_snapshot(path: &std::path::Path) -> anyhow::Result<Option<LiveSpawnSnapshot>> {
    if !path.exists() {
        return Ok(None);
    }

    let payload = std::fs::read(path)?;
    let snapshot = serde_json::from_slice::<LiveSpawnSnapshot>(&payload)?;
    Ok(Some(snapshot))
}

fn build_spawn_finder_section(snapshot: &LiveSpawnSnapshot) -> SpawnFinderSection {
    let observers = snapshot
        .observers
        .iter()
        .map(|observer| SpawnObserverSummary {
            client_id: observer.client_id,
            character_name: observer.character_name.clone(),
            zone: observer_zone_label(observer),
            total_spawns: observer.nearby_spawns.len(),
        })
        .collect();

    let items = snapshot
        .observers
        .iter()
        .flat_map(|observer| {
            let zone = observer_zone_label(observer);
            observer
                .nearby_spawns
                .iter()
                .filter(|spawn| spawn.spawn_id != 0 && !spawn.displayed_name.trim().is_empty())
                .map(move |spawn| {
                    let distance = ((spawn.x - observer.local_player.x).powi(2)
                        + (spawn.y - observer.local_player.y).powi(2))
                    .sqrt()
                    .round()
                    .clamp(0.0, u16::MAX as f32) as u16;
                    SpawnFinderItem {
                        observer_client_id: observer.client_id,
                        observer_name: observer.character_name.clone(),
                        observer_zone: zone.clone(),
                        spawn_id: spawn.spawn_id,
                        name: spawn.displayed_name.clone(),
                        spawn_type: spawn_type_label(spawn.spawn_type).to_string(),
                        level: spawn.level,
                        class_name: spawn.class_str(),
                        race_name: spawn.race_name(),
                        distance,
                        hp_pct: spawn.hp_pct().round().clamp(0.0, 255.0) as u8,
                        is_current_target: observer.target_spawn_id == Some(spawn.spawn_id),
                    }
                })
        })
        .collect();

    SpawnFinderSection { observers, items }
}

fn observer_zone_label(observer: &textquest_common::spawn_finder::LiveSpawnObserver) -> String {
    if observer.zone_long_name.trim().is_empty() {
        observer.zone_short_name.clone()
    } else {
        observer.zone_long_name.clone()
    }
}

fn spawn_type_label(spawn_type: u8) -> &'static str {
    match spawn_type {
        0 => "PC",
        1 => "NPC",
        2 | 3 => "Corpse",
        _ => "Unknown",
    }
}

#[cfg(windows)]
fn send_target_command(client_id: u32, spawn_id: u32) -> anyhow::Result<()> {
    use anyhow::Context;

    let token = textquest::ipc::load_session_token(client_id).with_context(|| {
        format!("missing session token for PID {client_id}; inject the DLL before targeting")
    })?;
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let pipe = textquest::ipc::pipe::CommandPipe::connect(client_id, session_id)?;
    pipe.send_raw_token(&token)?;
    pipe.send_async(&textquest_common::ipc::Command::SetTarget { spawn_id })?;
    Ok(())
}

#[cfg(not(windows))]
fn send_target_command(client_id: u32, spawn_id: u32) -> anyhow::Result<()> {
    let _ = (client_id, spawn_id);
    anyhow::bail!("Live spawn targeting is only available on Windows builds")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use serde_json::Value;

    use crate::AppState;

    fn test_state() -> Arc<AppState> {
        crate::test_support::demo_app_state()
    }

    /// Build a HeaderMap with a trusted local-dev Origin for mutation tests.
    fn trusted_headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::ORIGIN,
            crate::api::loot::TRUSTED_ORIGINS[0].parse().unwrap(),
        );
        h
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
            "spawnFinder",
            "groups",
            "navigation",
            "relocation",
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
            trusted_headers(),
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
            trusted_headers(),
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

    #[tokio::test]
    async fn get_dashboard_prefers_live_spawn_snapshot_when_present() {
        let state = test_state();
        let spawn_path = spawn_snapshot_path(state.as_ref());
        if let Some(parent) = spawn_path.parent() {
            std::fs::create_dir_all(parent).expect("spawn snapshot dir");
        }
        let live_snapshot = LiveSpawnSnapshot {
            observers: vec![textquest_common::spawn_finder::LiveSpawnObserver {
                client_id: 77,
                character_name: "Scout".into(),
                zone_short_name: "soldungc".into(),
                zone_long_name: "Solusek's Lair".into(),
                local_player: textquest_common::types::SpawnData {
                    spawn_id: 1,
                    name: "Scout".into(),
                    displayed_name: "Scout".into(),
                    spawn_type: 0,
                    level: 60,
                    class_id: 9,
                    race_id: 1,
                    x: 10.0,
                    y: 10.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 900,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 0,
                    endurance_max: 0,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
            combat_target_id: None,
                },
                target_spawn_id: Some(9001),
                nearby_spawns: vec![textquest_common::types::SpawnData {
                    spawn_id: 9001,
                    name: "a goblin raider".into(),
                    displayed_name: "a goblin raider".into(),
                    spawn_type: 1,
                    level: 22,
                    class_id: 1,
                    race_id: 9,
                    x: 22.0,
                    y: 10.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 470,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 0,
                    endurance_max: 0,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
            combat_target_id: None,
                }],
            }],
        };
        std::fs::write(
            &spawn_path,
            serde_json::to_vec(&live_snapshot).expect("serialize live snapshot"),
        )
        .expect("write live snapshot");

        let response = get_dashboard(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let json: Value = serde_json::from_slice(&body).expect("dashboard json");

        assert_eq!(
            json["spawnFinder"]["observers"][0]["characterName"],
            "Scout"
        );
        assert_eq!(json["spawnFinder"]["items"][0]["spawnId"], 9001);
        assert_eq!(json["spawnFinder"]["items"][0]["distance"], 12);
        assert_eq!(json["spawnFinder"]["items"][0]["className"], "WAR");
        assert_eq!(json["spawnFinder"]["items"][0]["raceName"], "Troll");
        assert_eq!(json["spawnFinder"]["items"][0]["isCurrentTarget"], true);
    }

    #[tokio::test]
    async fn get_dashboard_uses_empty_live_spawn_snapshot_when_present() {
        let state = test_state();
        let spawn_path = spawn_snapshot_path(state.as_ref());
        if let Some(parent) = spawn_path.parent() {
            std::fs::create_dir_all(parent).expect("spawn snapshot dir");
        }
        std::fs::write(
            &spawn_path,
            serde_json::to_vec(&LiveSpawnSnapshot::default()).expect("serialize live snapshot"),
        )
        .expect("write live snapshot");

        let response = get_dashboard(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let json: Value = serde_json::from_slice(&body).expect("dashboard json");

        assert_eq!(
            json["spawnFinder"]["observers"]
                .as_array()
                .expect("observers array")
                .len(),
            0
        );
        assert_eq!(
            json["spawnFinder"]["items"]
                .as_array()
                .expect("items array")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn post_dashboard_action_rejects_untrusted_origin() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::ORIGIN,
            "https://evil.example".parse().expect("origin header"),
        );

        let response = apply_dashboard_action(
            State(state.clone()),
            headers,
            axum::Json(DashboardActionRequest::CreateSession {
                profile: "Loot Crew".to_string(),
                character_name: "Blocked".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let Json(snapshot) = get_dashboard(State(state)).await;
        assert!(
            !snapshot
                .sessions
                .items
                .iter()
                .any(|session| session.character_name == "Blocked")
        );
    }
}
