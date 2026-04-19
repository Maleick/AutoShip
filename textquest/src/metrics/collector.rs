use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::metrics::baseline_scorecard::{
    CombatMetrics, EconomyMetrics, GroupCoordinationMetrics, MovementMetrics,
};

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
            Self::SixtyMin => Duration::from_secs(3600),
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
        self.timestamp_count += other.timestamp_count;
    }

    fn average_dps(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.dps / self.timestamp_count as f64
    }

    fn kill_rate(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.kills as f64 / self.timestamp_count as f64
    }

    fn loot_rate(&self) -> f64 {
        if self.timestamp_count == 0 {
            return 0.0;
        }
        self.items_looted as f64 / self.timestamp_count as f64
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterMetrics {
    pub name: String,
    pub combat: CharacterCombatMetrics,
    pub movement: CharacterMovementMetrics,
    pub economy: CharacterEconomyMetrics,
    pub session_start: i64,
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
            dps_samples: Vec::with_capacity(3600),
            ..Default::default()
        }
    }

    fn record_damage(&mut self, damage: u64) {
        self.total_damage_dealt += damage;
    }

    fn record_dps_sample(&mut self, dps: f64) {
        if self.dps_samples.len() >= 3600 {
            self.dps_samples.remove(0);
        }
        self.dps_samples.push(dps);
    }

    fn average_dps(&self) -> f64 {
        if self.dps_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.dps_samples.iter().sum();
        sum / self.dps_samples.len() as f64
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
            route_efficiencies: Vec::with_capacity(3600),
            ..Default::default()
        }
    }

    fn record_distance(&mut self, distance: f64) {
        self.total_distance += distance;
    }

    fn record_route_efficiency(&mut self, efficiency: f64) {
        if self.route_efficiencies.len() >= 3600 {
            self.route_efficiencies.remove(0);
        }
        self.route_efficiencies.push(efficiency);
    }

    fn average_route_efficiency(&self) -> f64 {
        if self.route_efficiencies.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.route_efficiencies.iter().sum();
        sum / self.route_efficiencies.len() as f64
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
            loot_events: Vec::with_capacity(100),
            ..Default::default()
        }
    }

    fn record_loot(&mut self, item_name: &str, value: u64) {
        self.total_items_looted += 1;
        self.total_plat_earned += value;
        if self.loot_events.len() >= 100 {
            self.loot_events.remove(0);
        }
        self.loot_events.push(LootEvent {
            item_name: item_name.to_owned(),
            value,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        });
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
}

impl FleetMetrics {
    fn new() -> Self {
        Self {
            fleet_dps_samples: Vec::with_capacity(3600),
            ..Default::default()
        }
    }

    fn record_dps_sample(&mut self, dps: f64) {
        if self.fleet_dps_samples.len() >= 3600 {
            self.fleet_dps_samples.remove(0);
        }
        self.fleet_dps_samples.push(dps);
        self.recalculate_fleet_dps();
    }

    fn recalculate_fleet_dps(&mut self) {
        if self.fleet_dps_samples.is_empty() {
            self.fleet_dps = 0.0;
            return;
        }
        let sum: f64 = self.fleet_dps_samples.iter().sum();
        self.fleet_dps = sum / self.fleet_dps_samples.len() as f64;
    }
}

struct TimeWindowBuffer {
    entries: Vec<(Instant, AggregateMetrics)>,
    capacity: usize,
}

impl TimeWindowBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, timestamp: Instant, metrics: AggregateMetrics) {
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((timestamp, metrics));
    }

    fn aggregate(&self, window: TimeWindow) -> AggregateMetrics {
        let cutoff = Instant::now() - window.duration();
        let mut agg = AggregateMetrics::default();
        for (ts, m) in &self.entries {
            if *ts >= cutoff {
                agg.add(m);
            }
        }
        agg
    }
}

#[derive(Debug, Clone, Default)]
pub struct TimeWindowMetrics {
    last_1min: TimeWindowBuffer,
    last_5min: TimeWindowBuffer,
    last_60min: TimeWindowBuffer,
}

impl TimeWindowMetrics {
    fn new() -> Self {
        Self {
            last_1min: TimeWindowBuffer::new(60),
            last_5min: TimeWindowBuffer::new(300),
            last_60min: TimeWindowBuffer::new(3600),
        }
    }

    fn record(&mut self, metrics: AggregateMetrics) {
        let now = Instant::now();
        self.last_1min.push(now, metrics.clone());
        self.last_5min.push(now, metrics.clone());
        self.last_60min.push(now, metrics);
    }

    fn get(&self, window: TimeWindow) -> HashMap<String, AggregateMetrics> {
        let _ = window;
        HashMap::new()
    }
}

pub struct MetricsCollector {
    per_character: HashMap<String, CharacterMetrics>,
    fleet_aggregates: FleetMetrics,
    time_windows: TimeWindowMetrics,
    last_aggregation: Instant,
    aggregation_interval: Duration,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            per_character: HashMap::new(),
            fleet_aggregates: FleetMetrics::new(),
            time_windows: TimeWindowMetrics::new(),
            last_aggregation: Instant::now(),
            aggregation_interval: Duration::from_secs(1),
        }
    }

    pub fn update_combat(&mut self, char_name: &str, metrics: CombatMetrics) {
        let char_metrics = self
            .per_character
            .entry(char_name.to_owned())
            .or_insert_with(|| CharacterMetrics {
                name: char_name.to_owned(),
                combat: CharacterCombatMetrics::new(),
                movement: CharacterMovementMetrics::new(),
                economy: CharacterEconomyMetrics::new(),
                session_start: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });

        char_metrics.combat.total_damage_dealt += metrics.dps as u64 * 100;
        char_metrics.combat.total_dps_samples.push(metrics.dps);
        if char_metrics.combat.dps_samples.len() > 3600 {
            char_metrics.combat.dps_samples.remove(0);
        }
        char_metrics.combat.total_kills += metrics.total_kills;
        char_metrics.combat.active_combat_time_secs +=
            metrics.avg_pull_to_kill_secs as u64 * metrics.total_kills as u64;

        self.fleet_aggregates.total_damage_dealt += metrics.dps as u64 * 100;
        self.fleet_aggregates.total_kills += metrics.total_kills;
    }

    pub fn update_movement(&mut self, char_name: &str, metrics: MovementMetrics) {
        let char_metrics = self
            .per_character
            .entry(char_name.to_owned())
            .or_insert_with(|| CharacterMetrics {
                name: char_name.to_owned(),
                combat: CharacterCombatMetrics::new(),
                movement: CharacterMovementMetrics::new(),
                economy: CharacterEconomyMetrics::new(),
                session_start: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });

        char_metrics.movement.total_distance += metrics.total_distance;
        char_metrics.movement.stuck_events += metrics.stuck_event_count;
        char_metrics
            .movement
            .route_efficiencies
            .push(metrics.route_efficiency);
        if char_metrics.movement.route_efficiencies.len() > 3600 {
            char_metrics.movement.route_efficiencies.remove(0);
        }

        self.fleet_aggregates.total_damage_dealt += metrics.total_distance as u64;
    }

    pub fn update_loot(&mut self, char_name: &str, item_name: &str, value: u64) {
        let char_metrics = self
            .per_character
            .entry(char_name.to_owned())
            .or_insert_with(|| CharacterMetrics {
                name: char_name.to_owned(),
                combat: CharacterCombatMetrics::new(),
                movement: CharacterMovementMetrics::new(),
                economy: CharacterEconomyMetrics::new(),
                session_start: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });

        char_metrics.economy.record_loot(item_name, value);

        self.fleet_aggregates.total_items_looted += 1;
        self.fleet_aggregates.total_plat_earned += value;
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
        let mut result = HashMap::new();
        let cutoff = Instant::now() - window.duration();

        for (char_name, char_metrics) in &self.per_character {
            let mut agg = AggregateMetrics {
                dps: char_metrics.combat.average_dps(),
                damage_dealt: char_metrics.combat.total_damage_dealt,
                kills: char_metrics.combat.total_kills,
                items_looted: char_metrics.economy.total_items_looted,
                plat_earned: char_metrics.economy.total_plat_earned,
                movement_distance: char_metrics.movement.total_distance,
                stuck_events: char_metrics.movement.stuck_events,
                spells_cast: char_metrics.combat.total_spells_cast,
                healing_done: char_metrics.combat.total_healing_done,
                timestamp_count: 1,
            };
            result.insert(char_name.clone(), agg);
        }

        result
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_aggregation) >= self.aggregation_interval {
            let mut fleet_agg = AggregateMetrics::default();
            for char_metrics in self.per_character.values() {
                fleet_agg.damage_dealt += char_metrics.combat.total_damage_dealt;
                fleet_agg.kills += char_metrics.combat.total_kills;
                fleet_agg.items_looted += char_metrics.economy.total_items_looted;
                fleet_agg.plat_earned += char_metrics.economy.total_plat_earned;
                fleet_agg.movement_distance += char_metrics.movement.total_distance;
                fleet_agg.stuck_events += char_metrics.movement.stuck_events;
                fleet_agg.timestamp_count += 1;
            }
            self.time_windows.record(fleet_agg);
            self.last_aggregation = now;
        }
    }

    pub fn characters(&self) -> Vec<&String> {
        self.per_character.keys().collect()
    }

    pub fn character_count(&self) -> usize {
        self.per_character.len()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_collector_is_empty() {
        let collector = MetricsCollector::new();
        assert!(collector.per_character.is_empty());
        assert_eq!(collector.character_count(), 0);
    }

    #[test]
    fn update_combat_records_damage() {
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
        assert_eq!(metrics.combat.total_damage_dealt, 10000);
        assert_eq!(metrics.combat.total_kills, 1);
    }

    #[test]
    fn update_movement_records_distance() {
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

        let metrics = collector.get_character_metrics("Monk01").unwrap();
        assert_eq!(metrics.movement.total_distance, 500.0);
        assert_eq!(metrics.movement.stuck_events, 1);
    }

    #[test]
    fn update_loot_records_item() {
        let mut collector = MetricsCollector::new();
        collector.update_loot("Cleric01", "Staff of the Magi", 5000);

        let metrics = collector.get_character_metrics("Cleric01").unwrap();
        assert_eq!(metrics.economy.total_items_looted, 1);
        assert_eq!(metrics.economy.total_plat_earned, 5000);
    }

    #[test]
    fn fleet_metrics_aggregate() {
        let mut collector = MetricsCollector::new();
        collector.update_combat(
            "Warrior01",
            CombatMetrics {
                dps: 100.0,
                mana_consumed: 50,
                endurance_consumed: 25,
                avg_pull_to_kill_secs: 10.0,
                total_kills: 5,
            },
        );
        collector.update_combat(
            "Mage01",
            CombatMetrics {
                dps: 150.0,
                mana_consumed: 100,
                endurance_consumed: 0,
                avg_pull_to_kill_secs: 8.0,
                total_kills: 3,
            },
        );

        let fleet = collector.get_fleet_metrics();
        assert_eq!(fleet.total_kills, 8);
    }

    #[test]
    fn time_window_get_returns_map() {
        let collector = MetricsCollector::new();
        let windowed = collector.get_windowed_metrics(TimeWindow::OneMin);
        assert!(windowed.is_empty());
    }

    #[test]
    fn get_unknown_character_returns_none() {
        let collector = MetricsCollector::new();
        assert!(collector.get_character_metrics("Ghost").is_none());
    }

    #[test]
    fn aggregate_metrics_add() {
        let mut a = AggregateMetrics {
            dps: 10.0,
            damage_dealt: 100,
            kills: 2,
            ..Default::default()
        };
        let b = AggregateMetrics {
            dps: 20.0,
            damage_dealt: 200,
            kills: 3,
            ..Default::default()
        };
        a.add(&b);

        assert_eq!(a.dps, 30.0);
        assert_eq!(a.damage_dealt, 300);
        assert_eq!(a.kills, 5);
    }

    #[test]
    fn character_combat_average_dps() {
        let mut combat = CharacterCombatMetrics::new();
        combat.dps_samples.push(100.0);
        combat.dps_samples.push(200.0);
        combat.dps_samples.push(150.0);

        let avg = combat.average_dps();
        assert!((avg - 150.0).abs() < 0.01);
    }

    #[test]
    fn character_movement_average_route_efficiency() {
        let mut movement = CharacterMovementMetrics::new();
        movement.route_efficiencies.push(0.8);
        movement.route_efficiencies.push(0.9);
        movement.route_efficiencies.push(0.85);

        let avg = movement.average_route_efficiency();
        assert!((avg - 0.85).abs() < 0.01);
    }
}
