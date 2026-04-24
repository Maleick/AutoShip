use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use textquest_common::{nav::NavStatus, types::GameState};

use crate::metrics::baseline_scorecard::{
    CombatMetrics, EconomyMetrics, GroupCoordinationMetrics, MovementMetrics,
};

const WINDOW_SAMPLE_CAPACITY: usize = 3_600;
const EVENT_QUEUE_CAPACITY: usize = 4_096;
const LOOT_EVENT_CAPACITY: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    OneMin,
    FiveMin,
    SixtyMin,
}

impl TimeWindow {
    fn duration(&self) -> Duration {
        match self {
            Self::OneMin => Duration::from_secs(60),
            Self::FiveMin => Duration::from_secs(300),
            Self::SixtyMin => Duration::from_secs(3_600),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateMetrics {
    pub dps: f64,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub kills: u32,
    pub deaths: u32,
    pub items_looted: u32,
    pub plat_earned: u64,
    pub movement_distance: f64,
    pub stuck_events: u32,
    pub spells_cast: u32,
    pub healing_done: u32,
    pub ipc_messages: u64,
    pub ipc_latency_ms: u64,
    pub system_memory_bytes: Option<u64>,
    pub timestamp_count: u32,
}

impl AggregateMetrics {
    fn add(&mut self, other: &AggregateMetrics) {
        self.dps += other.dps;
        self.damage_dealt += other.damage_dealt;
        self.damage_taken += other.damage_taken;
        self.kills += other.kills;
        self.deaths += other.deaths;
        self.items_looted += other.items_looted;
        self.plat_earned += other.plat_earned;
        self.movement_distance += other.movement_distance;
        self.stuck_events += other.stuck_events;
        self.spells_cast += other.spells_cast;
        self.healing_done += other.healing_done;
        self.ipc_messages += other.ipc_messages;
        self.ipc_latency_ms += other.ipc_latency_ms;
        self.system_memory_bytes = other.system_memory_bytes.or(self.system_memory_bytes);
        self.timestamp_count += other.timestamp_count;
    }

    fn finalize_for_window(&mut self, window: TimeWindow) {
        let seconds = window.duration().as_secs_f64();
        self.dps = if seconds > 0.0 {
            self.damage_dealt as f64 / seconds
        } else {
            0.0
        };
    }

    pub fn average_dps(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.dps / self.timestamp_count as f64
    }

    pub fn kill_rate(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.kills as f64 / self.timestamp_count as f64
    }

    pub fn loot_rate(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.items_looted as f64 / self.timestamp_count as f64
    }

    pub fn average_ipc_latency_ms(&self) -> f64 {
        if self.ipc_messages == 0 {
            return 0.0;
        }
        self.ipc_latency_ms as f64 / self.ipc_messages as f64
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterMetrics {
    pub name: String,
    pub combat: CharacterCombatMetrics,
    pub movement: CharacterMovementMetrics,
    pub economy: CharacterEconomyMetrics,
    pub system: CharacterSystemMetrics,
    pub session_start: i64,
}

impl CharacterMetrics {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            combat: CharacterCombatMetrics::new(),
            movement: CharacterMovementMetrics::new(),
            economy: CharacterEconomyMetrics::new(),
            system: CharacterSystemMetrics::default(),
            session_start: unix_timestamp(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterCombatMetrics {
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub total_kills: u32,
    pub total_deaths: u32,
    pub total_spells_cast: u32,
    pub total_healing_done: u32,
    pub active_combat_time_secs: u64,
    pub combat_start_timestamp: Option<i64>,
    pub dps_samples: Vec<f64>,
}

impl CharacterCombatMetrics {
    fn new() -> Self {
        Self {
            dps_samples: Vec::with_capacity(WINDOW_SAMPLE_CAPACITY),
            ..Default::default()
        }
    }

    fn record_damage(&mut self, damage: u64) {
        self.total_damage_dealt = self.total_damage_dealt.saturating_add(damage);
    }

    fn record_dps_sample(&mut self, dps: f64) {
        push_bounded(&mut self.dps_samples, dps, WINDOW_SAMPLE_CAPACITY);
    }

    fn average_dps(&self) -> f64 {
        average(&self.dps_samples)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterMovementMetrics {
    pub total_distance: f64,
    pub stuck_events: u32,
    pub stuck_start_timestamp: Option<i64>,
    pub navigation_failures: u32,
    pub route_efficiencies: Vec<f64>,
}

impl CharacterMovementMetrics {
    fn new() -> Self {
        Self {
            route_efficiencies: Vec::with_capacity(WINDOW_SAMPLE_CAPACITY),
            ..Default::default()
        }
    }

    fn record_distance(&mut self, distance: f64) {
        self.total_distance += distance;
    }

    fn record_route_efficiency(&mut self, efficiency: f64) {
        push_bounded(
            &mut self.route_efficiencies,
            efficiency,
            WINDOW_SAMPLE_CAPACITY,
        );
    }

    fn average_route_efficiency(&self) -> f64 {
        average(&self.route_efficiencies)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterEconomyMetrics {
    pub total_items_looted: u32,
    pub total_plat_earned: u64,
    pub total_plat_spent: u64,
    pub loot_events: Vec<LootEvent>,
}

impl CharacterEconomyMetrics {
    fn new() -> Self {
        Self {
            loot_events: Vec::with_capacity(LOOT_EVENT_CAPACITY),
            ..Default::default()
        }
    }

    fn record_loot(&mut self, item_name: &str, value: u64) {
        self.total_items_looted = self.total_items_looted.saturating_add(1);
        self.total_plat_earned = self.total_plat_earned.saturating_add(value);
        if self.loot_events.len() >= LOOT_EVENT_CAPACITY {
            self.loot_events.remove(0);
        }
        self.loot_events.push(LootEvent {
            item_name: item_name.to_owned(),
            value,
            timestamp: unix_timestamp(),
        });
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterSystemMetrics {
    pub ipc_messages: u64,
    pub total_ipc_latency_ms: u64,
    pub last_ipc_latency_ms: Option<u64>,
    pub memory_bytes: Option<u64>,
}

impl CharacterSystemMetrics {
    fn record_ipc_latency(&mut self, latency_ms: u64) {
        self.ipc_messages = self.ipc_messages.saturating_add(1);
        self.total_ipc_latency_ms = self.total_ipc_latency_ms.saturating_add(latency_ms);
        self.last_ipc_latency_ms = Some(latency_ms);
    }

    fn record_memory(&mut self, memory_bytes: u64) {
        self.memory_bytes = Some(memory_bytes);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootEvent {
    pub item_name: String,
    pub value: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetMetrics {
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub total_kills: u32,
    pub total_deaths: u32,
    pub total_items_looted: u32,
    pub total_plat_earned: u64,
    pub total_plat_spent: u64,
    pub active_characters: usize,
    pub fleet_dps: f64,
    pub fleet_dps_samples: Vec<f64>,
    pub movement_distance: f64,
    pub stuck_events: u32,
    pub ipc_messages: u64,
}

impl FleetMetrics {
    fn new() -> Self {
        Self {
            fleet_dps_samples: Vec::with_capacity(WINDOW_SAMPLE_CAPACITY),
            ..Default::default()
        }
    }

    fn record_dps_sample(&mut self, dps: f64) {
        push_bounded(&mut self.fleet_dps_samples, dps, WINDOW_SAMPLE_CAPACITY);
        self.recalculate_fleet_dps();
    }

    fn recalculate_fleet_dps(&mut self) {
        self.fleet_dps = average(&self.fleet_dps_samples);
    }
}

#[derive(Debug, Clone)]
pub enum MetricsEvent {
    CombatDamage {
        character_name: String,
        damage: u64,
        at: Instant,
    },
    CombatKill {
        character_name: String,
        at: Instant,
    },
    Movement {
        character_name: String,
        distance: f64,
        stuck_events: u32,
        route_efficiency: Option<f64>,
        at: Instant,
    },
    Loot {
        character_name: String,
        item_name: String,
        value: u64,
        at: Instant,
    },
    System {
        character_name: String,
        memory_bytes: Option<u64>,
        ipc_latency_ms: Option<u64>,
        at: Instant,
    },
}

#[derive(Debug, Clone, Copy)]
struct PositionSample {
    x: f32,
    y: f32,
    z: f32,
    timestamp_ms: u64,
    stuck: bool,
}

#[derive(Debug, Clone)]
struct TimeWindowBuffer {
    entries: VecDeque<(Instant, AggregateMetrics)>,
    capacity: usize,
}

impl TimeWindowBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, timestamp: Instant, metrics: AggregateMetrics) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((timestamp, metrics));
    }

    fn aggregate_at(&self, window: TimeWindow, now: Instant) -> AggregateMetrics {
        let cutoff = now.checked_sub(window.duration()).unwrap_or(now);
        let mut agg = AggregateMetrics::default();
        for (ts, metrics) in &self.entries {
            if *ts >= cutoff {
                agg.add(metrics);
            }
        }
        agg.finalize_for_window(window);
        agg
    }
}

#[derive(Debug, Clone)]
pub struct TimeWindowMetrics {
    per_character: HashMap<String, TimeWindowBuffer>,
    fleet: TimeWindowBuffer,
    capacity: usize,
}

impl TimeWindowMetrics {
    fn new() -> Self {
        Self {
            per_character: HashMap::new(),
            fleet: TimeWindowBuffer::new(WINDOW_SAMPLE_CAPACITY),
            capacity: WINDOW_SAMPLE_CAPACITY,
        }
    }

    fn record(&mut self, character_name: &str, at: Instant, metrics: AggregateMetrics) {
        self.per_character
            .entry(character_name.to_owned())
            .or_insert_with(|| TimeWindowBuffer::new(self.capacity))
            .push(at, metrics.clone());
        self.fleet.push(at, metrics);
    }

    fn get(&self, window: TimeWindow) -> HashMap<String, AggregateMetrics> {
        let now = Instant::now();
        self.per_character
            .iter()
            .map(|(name, buffer)| (name.clone(), buffer.aggregate_at(window, now)))
            .collect()
    }

    fn character_aggregate_at(
        &self,
        character_name: &str,
        window: TimeWindow,
        now: Instant,
    ) -> AggregateMetrics {
        self.per_character
            .get(character_name)
            .map(|buffer| buffer.aggregate_at(window, now))
            .unwrap_or_default()
    }

    fn fleet_aggregate_at(&self, window: TimeWindow, now: Instant) -> AggregateMetrics {
        self.fleet.aggregate_at(window, now)
    }
}

impl Default for TimeWindowMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MetricsCollector {
    per_character: HashMap<String, CharacterMetrics>,
    fleet_aggregates: FleetMetrics,
    time_windows: TimeWindowMetrics,
    last_positions: HashMap<String, PositionSample>,
    event_tx: SyncSender<MetricsEvent>,
    event_rx: Receiver<MetricsEvent>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let (event_tx, event_rx) = sync_channel(EVENT_QUEUE_CAPACITY);
        Self {
            per_character: HashMap::new(),
            fleet_aggregates: FleetMetrics::new(),
            time_windows: TimeWindowMetrics::new(),
            last_positions: HashMap::new(),
            event_tx,
            event_rx,
        }
    }

    pub fn event_sender(&self) -> SyncSender<MetricsEvent> {
        self.event_tx.clone()
    }

    pub fn enqueue_event(&self, event: MetricsEvent) -> bool {
        match self.event_tx.try_send(event) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        }
    }

    pub fn drain_events(&mut self) -> usize {
        let mut count = 0;
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => {
                    self.apply_event(event);
                    count += 1;
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return count,
            }
        }
    }

    pub fn collect_game_state_snapshots(
        &mut self,
        client_names: &HashMap<u32, String>,
        game_states: &HashMap<u32, GameState>,
    ) -> usize {
        let mut events = Vec::new();
        for (pid, state) in game_states {
            let Some(player) = state.local_player.as_ref() else {
                continue;
            };
            let character_name = metrics_character_name(*pid, client_names, state);
            self.ensure_character(&character_name);

            let stuck = matches!(state.nav_status, NavStatus::Stuck { .. });
            let current = PositionSample {
                x: player.x,
                y: player.y,
                z: player.z,
                timestamp_ms: state.timestamp_ms,
                stuck,
            };
            let previous = self.last_positions.insert(character_name.clone(), current);
            let distance = previous
                .filter(|sample| state.timestamp_ms >= sample.timestamp_ms)
                .map(|sample| distance_3d(sample, current))
                .unwrap_or_default();
            let new_stuck_event = stuck && previous.is_none_or(|sample| !sample.stuck);

            if distance > 0.0 || new_stuck_event {
                events.push(MetricsEvent::Movement {
                    character_name,
                    distance,
                    stuck_events: u32::from(new_stuck_event),
                    route_efficiency: None,
                    at: Instant::now(),
                });
            }
        }

        for event in events {
            self.enqueue_event(event);
        }
        self.drain_events()
    }

    pub fn enqueue_combat_damage(&self, character_name: &str, damage: u64) -> bool {
        self.enqueue_event(MetricsEvent::CombatDamage {
            character_name: character_name.to_owned(),
            damage,
            at: Instant::now(),
        })
    }

    pub fn record_combat_damage_at(&mut self, character_name: &str, damage: u64, at: Instant) {
        self.apply_event(MetricsEvent::CombatDamage {
            character_name: character_name.to_owned(),
            damage,
            at,
        });
    }

    pub fn record_system_sample(
        &mut self,
        character_name: &str,
        memory_bytes: Option<u64>,
        ipc_latency_ms: Option<u64>,
    ) {
        self.apply_event(MetricsEvent::System {
            character_name: character_name.to_owned(),
            memory_bytes,
            ipc_latency_ms,
            at: Instant::now(),
        });
    }

    pub fn update_combat(&mut self, char_name: &str, metrics: CombatMetrics) {
        let damage = (metrics.dps * metrics.avg_pull_to_kill_secs * f64::from(metrics.total_kills))
            .max(0.0)
            .round() as u64;
        if damage > 0 {
            self.record_combat_damage_at(char_name, damage, Instant::now());
        } else {
            self.ensure_character(char_name);
        }

        if let Some(char_metrics) = self.per_character.get_mut(char_name) {
            char_metrics.combat.total_kills = char_metrics
                .combat
                .total_kills
                .saturating_add(metrics.total_kills);
            char_metrics.combat.active_combat_time_secs =
                char_metrics.combat.active_combat_time_secs.saturating_add(
                    (metrics.avg_pull_to_kill_secs * f64::from(metrics.total_kills)) as u64,
                );
        }
        if metrics.total_kills > 0 {
            self.time_windows.record(
                char_name,
                Instant::now(),
                AggregateMetrics {
                    kills: metrics.total_kills,
                    timestamp_count: 1,
                    ..Default::default()
                },
            );
        }
        self.recalculate_fleet_totals();
    }

    pub fn update_movement(&mut self, char_name: &str, metrics: MovementMetrics) {
        self.apply_movement_delta(
            char_name,
            metrics.total_distance,
            metrics.stuck_event_count,
            Some(metrics.route_efficiency),
            Instant::now(),
        );
    }

    pub fn update_loot(&mut self, char_name: &str, item_name: &str, value: u64) {
        self.apply_event(MetricsEvent::Loot {
            character_name: char_name.to_owned(),
            item_name: item_name.to_owned(),
            value,
            at: Instant::now(),
        });
    }

    pub fn update_economy(&mut self, char_name: &str, metrics: EconomyMetrics) {
        self.ensure_character(char_name);
        if let Some(char_metrics) = self.per_character.get_mut(char_name) {
            char_metrics.economy.total_items_looted = char_metrics
                .economy
                .total_items_looted
                .saturating_add(metrics.total_items);
            char_metrics.economy.total_plat_earned = char_metrics
                .economy
                .total_plat_earned
                .saturating_add(metrics.total_plat);
        }
        self.time_windows.record(
            char_name,
            Instant::now(),
            AggregateMetrics {
                items_looted: metrics.total_items,
                plat_earned: metrics.total_plat,
                timestamp_count: 1,
                ..Default::default()
            },
        );
        self.recalculate_fleet_totals();
    }

    pub fn update_group_coordination(
        &mut self,
        _char_name: &str,
        _metrics: GroupCoordinationMetrics,
    ) {
        // Group coordination has no per-character aggregate fields yet. Keeping
        // the method preserves the collector surface for future event hooks.
    }

    pub fn get_character_metrics(&self, char_name: &str) -> Option<&CharacterMetrics> {
        self.per_character.get(char_name)
    }

    pub fn get_character_metrics_mut(&mut self, char_name: &str) -> Option<&mut CharacterMetrics> {
        self.per_character.get_mut(char_name)
    }

    pub fn get_fleet_metrics(&self) -> &FleetMetrics {
        &self.fleet_aggregates
    }

    pub fn get_fleet_metrics_mut(&mut self) -> &mut FleetMetrics {
        &mut self.fleet_aggregates
    }

    pub fn get_windowed_metrics(&self, window: TimeWindow) -> HashMap<String, AggregateMetrics> {
        self.time_windows.get(window)
    }

    pub fn get_fleet_windowed_metrics(&self, window: TimeWindow) -> AggregateMetrics {
        self.time_windows.fleet_aggregate_at(window, Instant::now())
    }

    pub fn tick(&mut self) {
        self.drain_events();
    }

    pub fn characters(&self) -> Vec<&String> {
        self.per_character.keys().collect()
    }

    pub fn character_count(&self) -> usize {
        self.per_character.len()
    }

    fn apply_event(&mut self, event: MetricsEvent) {
        match event {
            MetricsEvent::CombatDamage {
                character_name,
                damage,
                at,
            } => self.apply_combat_damage(&character_name, damage, at),
            MetricsEvent::CombatKill { character_name, at } => {
                self.ensure_character(&character_name);
                if let Some(metrics) = self.per_character.get_mut(&character_name) {
                    metrics.combat.total_kills = metrics.combat.total_kills.saturating_add(1);
                }
                self.time_windows.record(
                    &character_name,
                    at,
                    AggregateMetrics {
                        kills: 1,
                        timestamp_count: 1,
                        ..Default::default()
                    },
                );
                self.recalculate_fleet_totals();
            }
            MetricsEvent::Movement {
                character_name,
                distance,
                stuck_events,
                route_efficiency,
                at,
            } => self.apply_movement_delta(
                &character_name,
                distance,
                stuck_events,
                route_efficiency,
                at,
            ),
            MetricsEvent::Loot {
                character_name,
                item_name,
                value,
                at,
            } => {
                self.ensure_character(&character_name);
                if let Some(metrics) = self.per_character.get_mut(&character_name) {
                    metrics.economy.record_loot(&item_name, value);
                }
                self.time_windows.record(
                    &character_name,
                    at,
                    AggregateMetrics {
                        items_looted: 1,
                        plat_earned: value,
                        timestamp_count: 1,
                        ..Default::default()
                    },
                );
                self.recalculate_fleet_totals();
            }
            MetricsEvent::System {
                character_name,
                memory_bytes,
                ipc_latency_ms,
                at,
            } => {
                self.ensure_character(&character_name);
                if let Some(metrics) = self.per_character.get_mut(&character_name) {
                    if let Some(memory_bytes) = memory_bytes {
                        metrics.system.record_memory(memory_bytes);
                    }
                    if let Some(latency_ms) = ipc_latency_ms {
                        metrics.system.record_ipc_latency(latency_ms);
                    }
                }
                self.time_windows.record(
                    &character_name,
                    at,
                    AggregateMetrics {
                        ipc_messages: u64::from(ipc_latency_ms.is_some()),
                        ipc_latency_ms: ipc_latency_ms.unwrap_or_default(),
                        system_memory_bytes: memory_bytes,
                        timestamp_count: 1,
                        ..Default::default()
                    },
                );
                self.recalculate_fleet_totals();
            }
        }
    }

    fn apply_combat_damage(&mut self, character_name: &str, damage: u64, at: Instant) {
        self.ensure_character(character_name);
        if let Some(metrics) = self.per_character.get_mut(character_name) {
            metrics.combat.record_damage(damage);
        }
        self.time_windows.record(
            character_name,
            at,
            AggregateMetrics {
                damage_dealt: damage,
                timestamp_count: 1,
                ..Default::default()
            },
        );
        let character_dps = self
            .time_windows
            .character_aggregate_at(character_name, TimeWindow::OneMin, at)
            .dps;
        if let Some(metrics) = self.per_character.get_mut(character_name) {
            metrics.combat.record_dps_sample(character_dps);
        }
        self.recalculate_fleet_totals();
        let fleet_dps = self
            .time_windows
            .fleet_aggregate_at(TimeWindow::OneMin, at)
            .dps;
        self.fleet_aggregates.record_dps_sample(fleet_dps);
    }

    fn apply_movement_delta(
        &mut self,
        character_name: &str,
        distance: f64,
        stuck_events: u32,
        route_efficiency: Option<f64>,
        at: Instant,
    ) {
        self.ensure_character(character_name);
        if let Some(metrics) = self.per_character.get_mut(character_name) {
            metrics.movement.record_distance(distance);
            metrics.movement.stuck_events =
                metrics.movement.stuck_events.saturating_add(stuck_events);
            if let Some(efficiency) = route_efficiency {
                metrics.movement.record_route_efficiency(efficiency);
            }
        }
        self.time_windows.record(
            character_name,
            at,
            AggregateMetrics {
                movement_distance: distance,
                stuck_events,
                timestamp_count: 1,
                ..Default::default()
            },
        );
        self.recalculate_fleet_totals();
    }

    fn ensure_character(&mut self, char_name: &str) {
        self.per_character
            .entry(char_name.to_owned())
            .or_insert_with(|| CharacterMetrics::new(char_name));
        self.fleet_aggregates.active_characters = self.per_character.len();
    }

    fn recalculate_fleet_totals(&mut self) {
        let mut fleet = FleetMetrics::new();
        fleet.active_characters = self.per_character.len();
        for metrics in self.per_character.values() {
            fleet.total_damage_dealt = fleet
                .total_damage_dealt
                .saturating_add(metrics.combat.total_damage_dealt);
            fleet.total_damage_taken = fleet
                .total_damage_taken
                .saturating_add(metrics.combat.total_damage_taken);
            fleet.total_kills = fleet.total_kills.saturating_add(metrics.combat.total_kills);
            fleet.total_deaths = fleet
                .total_deaths
                .saturating_add(metrics.combat.total_deaths);
            fleet.total_items_looted = fleet
                .total_items_looted
                .saturating_add(metrics.economy.total_items_looted);
            fleet.total_plat_earned = fleet
                .total_plat_earned
                .saturating_add(metrics.economy.total_plat_earned);
            fleet.total_plat_spent = fleet
                .total_plat_spent
                .saturating_add(metrics.economy.total_plat_spent);
            fleet.movement_distance += metrics.movement.total_distance;
            fleet.stuck_events = fleet
                .stuck_events
                .saturating_add(metrics.movement.stuck_events);
            fleet.ipc_messages = fleet
                .ipc_messages
                .saturating_add(metrics.system.ipc_messages);
        }
        fleet.fleet_dps_samples = self.fleet_aggregates.fleet_dps_samples.clone();
        fleet.recalculate_fleet_dps();
        self.fleet_aggregates = fleet;
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MetricsCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsCollector")
            .field("character_count", &self.per_character.len())
            .field("fleet_aggregates", &self.fleet_aggregates)
            .field("time_windows", &self.time_windows)
            .field("last_positions", &self.last_positions.len())
            .finish_non_exhaustive()
    }
}

fn metrics_character_name(
    pid: u32,
    client_names: &HashMap<u32, String>,
    state: &GameState,
) -> String {
    client_names
        .get(&pid)
        .filter(|name| !name.trim().is_empty())
        .cloned()
        .or_else(|| {
            state.local_player.as_ref().and_then(|player| {
                let name = if player.displayed_name.trim().is_empty() {
                    &player.name
                } else {
                    &player.displayed_name
                };
                (!name.trim().is_empty()).then(|| name.clone())
            })
        })
        .unwrap_or_else(|| format!("pid:{pid}"))
}

fn distance_3d(previous: PositionSample, current: PositionSample) -> f64 {
    let dx = f64::from(current.x - previous.x);
    let dy = f64::from(current.y - previous.y);
    let dz = f64::from(current.z - previous.z);
    (dx.mul_add(dx, dy.mul_add(dy, dz * dz))).sqrt()
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn push_bounded(samples: &mut Vec<f64>, value: f64, capacity: usize) {
    if samples.len() >= capacity {
        samples.remove(0);
    }
    samples.push(value);
}

fn average(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.iter().sum::<f64>() / samples.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_collector_is_empty() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.character_count(), 0);
        assert!(collector.get_fleet_metrics().fleet_dps_samples.is_empty());
    }

    #[test]
    fn update_combat_records_damage_from_window_duration() {
        let mut collector = MetricsCollector::new();
        collector.update_combat(
            "Warrior01",
            CombatMetrics {
                dps: 100.0,
                mana_consumed: 50,
                endurance_consumed: 25,
                avg_pull_to_kill_secs: 10.0,
                total_kills: 1,
            },
        );

        let metrics = collector.get_character_metrics("Warrior01").unwrap();
        assert_eq!(metrics.combat.total_damage_dealt, 1_000);
        assert_eq!(metrics.combat.total_kills, 1);
    }

    #[test]
    fn movement_updates_do_not_pollute_damage_totals() {
        let mut collector = MetricsCollector::new();
        collector.update_movement(
            "Monk01",
            MovementMetrics {
                stuck_percentage: 5.0,
                total_distance: 500.0,
                stuck_event_count: 1,
                route_efficiency: 0.85,
            },
        );

        let character = collector.get_character_metrics("Monk01").unwrap();
        assert_eq!(character.movement.total_distance, 500.0);
        assert_eq!(collector.get_fleet_metrics().total_damage_dealt, 0);
        assert_eq!(collector.get_fleet_metrics().movement_distance, 500.0);
    }

    #[test]
    fn loot_records_per_character_and_fleet_totals() {
        let mut collector = MetricsCollector::new();
        collector.update_loot("Cleric01", "Staff of the Magi", 5_000);

        let metrics = collector.get_character_metrics("Cleric01").unwrap();
        assert_eq!(metrics.economy.total_items_looted, 1);
        assert_eq!(metrics.economy.total_plat_earned, 5_000);
        assert_eq!(collector.get_fleet_metrics().total_items_looted, 1);
    }

    #[test]
    fn time_window_filters_old_damage_and_calculates_dps() {
        let mut collector = MetricsCollector::new();
        let now = Instant::now();
        collector.record_combat_damage_at("Mage01", 999, now - Duration::from_secs(70));
        collector.record_combat_damage_at("Mage01", 300, now - Duration::from_secs(30));
        collector.record_combat_damage_at("Mage01", 300, now - Duration::from_secs(2));

        let windowed = collector.get_windowed_metrics(TimeWindow::OneMin);
        let mage = windowed.get("Mage01").unwrap();
        assert_eq!(mage.damage_dealt, 600);
        assert!((mage.dps - 10.0).abs() < 0.01);
    }

    #[test]
    fn queued_events_drain_without_blocking() {
        let mut collector = MetricsCollector::new();
        assert!(collector.enqueue_combat_damage("Rogue01", 240));
        assert_eq!(collector.drain_events(), 1);

        let metrics = collector.get_character_metrics("Rogue01").unwrap();
        assert_eq!(metrics.combat.total_damage_dealt, 240);
    }

    #[test]
    fn aggregate_metrics_adds_system_fields() {
        let mut a = AggregateMetrics {
            damage_dealt: 100,
            ipc_messages: 1,
            ipc_latency_ms: 5,
            ..Default::default()
        };
        let b = AggregateMetrics {
            damage_dealt: 200,
            ipc_messages: 2,
            ipc_latency_ms: 15,
            system_memory_bytes: Some(4096),
            ..Default::default()
        };
        a.add(&b);

        assert_eq!(a.damage_dealt, 300);
        assert_eq!(a.ipc_messages, 3);
        assert_eq!(a.average_ipc_latency_ms(), 20.0 / 3.0);
        assert_eq!(a.system_memory_bytes, Some(4096));
    }
}
