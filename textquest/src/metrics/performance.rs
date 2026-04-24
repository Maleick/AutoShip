//! Real-time performance monitoring facade.
//!
//! This module is the narrow API layer that higher-level surfaces can wrap for
//! operator dashboards, Lua bindings, persistence, and exports.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::metrics::collector::MetricsCollector;

/// Minimal read API for live performance metrics.
pub trait PerformanceMetricsApi {
    /// Average DPS for a character in the current collector window.
    fn get_dps(&self, character_name: &str) -> f64;

    /// Aggregated dashboard state for TUI/API consumers.
    fn dashboard_snapshot(&self) -> PerformanceDashboardSnapshot;
}

/// Borrowed view over the live metrics collector.
#[derive(Debug, Clone, Copy)]
pub struct PerformanceMonitor<'a> {
    collector: &'a MetricsCollector,
}

impl<'a> PerformanceMonitor<'a> {
    /// Create a live performance monitor backed by an existing collector.
    pub fn new(collector: &'a MetricsCollector) -> Self {
        Self { collector }
    }

    /// Average DPS for a character in the current collector window.
    pub fn get_dps(&self, character_name: &str) -> f64 {
        average_character_dps(self.collector, character_name)
    }

    /// Fleet-level farming efficiency snapshot for dashboard/API consumers.
    pub fn get_farm_efficiency(&self) -> FarmEfficiencySnapshot {
        FarmEfficiencySnapshot::from_collector(self.collector)
    }

    /// Aggregated dashboard state for TUI/API consumers.
    pub fn dashboard_snapshot(&self) -> PerformanceDashboardSnapshot {
        PerformanceDashboardSnapshot::from_collector(self.collector)
    }
}

impl PerformanceMetricsApi for PerformanceMonitor<'_> {
    fn get_dps(&self, character_name: &str) -> f64 {
        average_character_dps(self.collector, character_name)
    }

    fn dashboard_snapshot(&self) -> PerformanceDashboardSnapshot {
        PerformanceDashboardSnapshot::from_collector(self.collector)
    }
}

impl MetricsCollector {
    /// Borrow this collector through the performance monitoring facade.
    pub fn performance(&self) -> PerformanceMonitor<'_> {
        PerformanceMonitor::new(self)
    }
}

/// Fleet efficiency values derived from the live metrics collector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FarmEfficiencySnapshot {
    /// Characters with at least one metric sample in the collector.
    pub character_count: usize,
    /// Sum of average DPS across tracked characters.
    pub fleet_dps: f64,
    /// Total kills recorded by the collector.
    pub total_kills: u32,
    /// Total items looted by the collector.
    pub items_looted: u32,
    /// Total platinum earned by the collector.
    pub plat_earned: u64,
    /// Items looted per kill. Zero when no kills have been recorded.
    pub loot_combat_ratio: f64,
    /// Platinum earned per kill. Zero when no kills have been recorded.
    pub plat_per_kill: f64,
    /// Total stuck detections across tracked characters.
    pub stuck_events: u32,
}

/// Dashboard tabs supported by the performance monitor surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceDashboardTab {
    Combat,
    Movement,
    Loot,
    System,
}

/// Per-character DPS row for the combat dashboard tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterDpsSnapshot {
    pub character_name: String,
    pub dps: f64,
}

/// Combat metrics aggregated for the dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardCombatMetrics {
    pub fleet_dps: f64,
    pub character_dps: Vec<CharacterDpsSnapshot>,
    pub combat_duration_secs: u64,
    pub encounter_count: u32,
    pub combat_frequency_per_hour: f64,
    pub spell_cast_rate_per_min: f64,
    pub healing_output: u32,
    pub miss_rate: f64,
    pub resist_rate: f64,
}

impl DashboardCombatMetrics {
    fn zeroed() -> Self {
        Self {
            fleet_dps: 0.0,
            character_dps: Vec::new(),
            combat_duration_secs: 0,
            encounter_count: 0,
            combat_frequency_per_hour: 0.0,
            spell_cast_rate_per_min: 0.0,
            healing_output: 0,
            miss_rate: 0.0,
            resist_rate: 0.0,
        }
    }
}

/// Movement metrics aggregated for the dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardMovementMetrics {
    pub navigation_success_rate: f64,
    pub stuck_detection_count: u32,
    pub movement_latency_ms: f64,
    pub distance_traveled: f64,
}

impl DashboardMovementMetrics {
    fn zeroed() -> Self {
        Self {
            navigation_success_rate: 0.0,
            stuck_detection_count: 0,
            movement_latency_ms: 0.0,
            distance_traveled: 0.0,
        }
    }
}

/// Loot metrics aggregated for the dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardLootMetrics {
    pub items_looted_per_hour: f64,
    pub plat_collected_per_hour: f64,
    pub loot_to_combat_ratio: f64,
    pub items_looted: u32,
    pub plat_collected: u64,
    pub valuable_items: u32,
    pub vendor_trash_items: u32,
}

impl DashboardLootMetrics {
    fn zeroed() -> Self {
        Self {
            items_looted_per_hour: 0.0,
            plat_collected_per_hour: 0.0,
            loot_to_combat_ratio: 0.0,
            items_looted: 0,
            plat_collected: 0,
            valuable_items: 0,
            vendor_trash_items: 0,
        }
    }
}

/// System metrics placeholder for DLL/IPC producers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardSystemMetrics {
    pub dll_cpu_percent: f64,
    pub memory_consumption_mb: f64,
    pub ipc_messages_per_sec: f64,
    pub script_execution_time_ms: f64,
    pub frame_latency_ms: f64,
}

impl DashboardSystemMetrics {
    fn zeroed() -> Self {
        Self {
            dll_cpu_percent: 0.0,
            memory_consumption_mb: 0.0,
            ipc_messages_per_sec: 0.0,
            script_execution_time_ms: 0.0,
            frame_latency_ms: 0.0,
        }
    }
}

/// Severity level for dashboard anomaly alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceAlertSeverity {
    Warning,
    Critical,
}

/// Lightweight dashboard alert emitted from aggregated metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceDashboardAlert {
    pub severity: PerformanceAlertSeverity,
    pub metric: String,
    pub message: String,
}

/// Aggregated performance-monitor dashboard state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceDashboardSnapshot {
    pub captured_at_unix_secs: u64,
    pub tabs: Vec<PerformanceDashboardTab>,
    pub combat: DashboardCombatMetrics,
    pub movement: DashboardMovementMetrics,
    pub loot: DashboardLootMetrics,
    pub system: DashboardSystemMetrics,
    pub alerts: Vec<PerformanceDashboardAlert>,
}

impl PerformanceDashboardSnapshot {
    /// Build a dashboard snapshot from live collector state.
    pub fn from_collector(collector: &MetricsCollector) -> Self {
        Self::from_collector_at(collector, current_unix_secs())
    }

    fn from_collector_at(collector: &MetricsCollector, captured_at_unix_secs: u64) -> Self {
        let mut combat = DashboardCombatMetrics::zeroed();
        let mut movement = DashboardMovementMetrics::zeroed();
        let mut loot = DashboardLootMetrics::zeroed();
        let mut route_efficiency_sum = 0.0;
        let mut route_efficiency_count = 0;
        let mut spells_cast = 0;
        let mut earliest_session_start = None;

        let mut character_names = collector.characters();
        character_names.sort();

        for character_name in character_names {
            if let Some(metrics) = collector.get_character_metrics(character_name) {
                let dps = average_samples(&metrics.combat.dps_samples);
                combat.fleet_dps += dps;
                combat.character_dps.push(CharacterDpsSnapshot {
                    character_name: metrics.name.clone(),
                    dps,
                });
                combat.combat_duration_secs += metrics.combat.active_combat_time_secs;
                combat.encounter_count += metrics.combat.total_kills;
                combat.healing_output += metrics.combat.total_healing_done;
                spells_cast += metrics.combat.total_spells_cast;

                movement.stuck_detection_count += metrics.movement.stuck_events;
                movement.distance_traveled += metrics.movement.total_distance;
                route_efficiency_sum += metrics.movement.route_efficiencies.iter().sum::<f64>();
                route_efficiency_count += metrics.movement.route_efficiencies.len();

                loot.items_looted += metrics.economy.total_items_looted;
                loot.plat_collected += metrics.economy.total_plat_earned;
                loot.valuable_items += metrics
                    .economy
                    .loot_events
                    .iter()
                    .filter(|event| event.value > 0)
                    .count() as u32;

                earliest_session_start = match earliest_session_start {
                    Some(current) => Some(current.min(metrics.session_start)),
                    None => Some(metrics.session_start),
                };
            }
        }

        let session_hours = elapsed_hours(earliest_session_start, captured_at_unix_secs);
        if session_hours > 0.0 {
            combat.combat_frequency_per_hour = combat.encounter_count as f64 / session_hours;
            loot.items_looted_per_hour = loot.items_looted as f64 / session_hours;
            loot.plat_collected_per_hour = loot.plat_collected as f64 / session_hours;
        }

        if combat.combat_duration_secs > 0 {
            let combat_minutes = combat.combat_duration_secs as f64 / 60.0;
            combat.spell_cast_rate_per_min = spells_cast as f64 / combat_minutes;
        }

        if route_efficiency_count > 0 {
            movement.navigation_success_rate = route_efficiency_sum / route_efficiency_count as f64;
        }

        loot.loot_to_combat_ratio = ratio(loot.items_looted as f64, combat.encounter_count as f64);
        loot.vendor_trash_items = loot.items_looted.saturating_sub(loot.valuable_items);

        let alerts = build_alerts(&combat, &movement);

        Self {
            captured_at_unix_secs,
            tabs: vec![
                PerformanceDashboardTab::Combat,
                PerformanceDashboardTab::Movement,
                PerformanceDashboardTab::Loot,
                PerformanceDashboardTab::System,
            ],
            combat,
            movement,
            loot,
            system: DashboardSystemMetrics::zeroed(),
            alerts,
        }
    }
}

impl FarmEfficiencySnapshot {
    /// Build an efficiency snapshot from live collector state.
    pub fn from_collector(collector: &MetricsCollector) -> Self {
        let fleet = collector.get_fleet_metrics();
        let mut fleet_dps = 0.0;
        let mut stuck_events = 0;

        for character in collector.characters() {
            if let Some(metrics) = collector.get_character_metrics(character) {
                fleet_dps += average_samples(&metrics.combat.dps_samples);
                stuck_events += metrics.movement.stuck_events;
            }
        }

        Self {
            character_count: collector.character_count(),
            fleet_dps,
            total_kills: fleet.total_kills,
            items_looted: fleet.total_items_looted,
            plat_earned: fleet.total_plat_earned,
            loot_combat_ratio: ratio(fleet.total_items_looted as f64, fleet.total_kills as f64),
            plat_per_kill: ratio(fleet.total_plat_earned as f64, fleet.total_kills as f64),
            stuck_events,
        }
    }
}

fn build_alerts(
    combat: &DashboardCombatMetrics,
    movement: &DashboardMovementMetrics,
) -> Vec<PerformanceDashboardAlert> {
    let mut alerts = Vec::new();

    if combat.encounter_count > 0 && combat.fleet_dps <= 0.0 {
        alerts.push(PerformanceDashboardAlert {
            severity: PerformanceAlertSeverity::Critical,
            metric: "combat.fleet_dps".to_owned(),
            message: "Combat encounters recorded with zero fleet DPS".to_owned(),
        });
    }

    if movement.stuck_detection_count >= 3 {
        alerts.push(PerformanceDashboardAlert {
            severity: PerformanceAlertSeverity::Warning,
            metric: "movement.stuck_detection_count".to_owned(),
            message: "Repeated stuck detections recorded this session".to_owned(),
        });
    }

    alerts
}

fn average_character_dps(collector: &MetricsCollector, character_name: &str) -> f64 {
    collector
        .get_character_metrics(character_name)
        .map(|metrics| average_samples(&metrics.combat.dps_samples))
        .unwrap_or(0.0)
}

fn average_samples(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    samples.iter().sum::<f64>() / samples.len() as f64
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn elapsed_hours(earliest_session_start: Option<i64>, captured_at_unix_secs: u64) -> f64 {
    let Some(start) = earliest_session_start else {
        return 0.0;
    };
    let start = start.max(0) as u64;
    let elapsed_secs = captured_at_unix_secs.saturating_sub(start);
    if elapsed_secs == 0 {
        return 0.0;
    }

    elapsed_secs as f64 / 3600.0
}

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        return 0.0;
    }

    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::baseline_scorecard::{
        CombatMetrics as ScorecardCombatMetrics, MovementMetrics as ScorecardMovementMetrics,
    };

    #[test]
    fn dashboard_snapshot_aggregates_live_collector_domains() {
        let mut collector = MetricsCollector::new();
        collector.update_combat(
            "Mage01",
            ScorecardCombatMetrics {
                dps: 120.0,
                mana_consumed: 50,
                endurance_consumed: 0,
                avg_pull_to_kill_secs: 30.0,
                total_kills: 2,
            },
        );
        collector.update_movement(
            "Mage01",
            ScorecardMovementMetrics {
                stuck_percentage: 10.0,
                total_distance: 250.0,
                stuck_event_count: 3,
                route_efficiency: 0.75,
            },
        );
        collector.update_loot("Mage01", "Fine Steel Sword", 25);

        let metrics = collector.get_character_metrics_mut("Mage01").unwrap();
        metrics.session_start = 1_000;
        metrics.combat.total_spells_cast = 6;
        metrics.combat.total_healing_done = 42;

        let snapshot = PerformanceDashboardSnapshot::from_collector_at(&collector, 4_600);

        assert_eq!(snapshot.tabs.len(), 4);
        assert_eq!(snapshot.combat.fleet_dps, 120.0);
        assert_eq!(snapshot.combat.encounter_count, 2);
        assert_eq!(snapshot.combat.spell_cast_rate_per_min, 6.0);
        assert_eq!(snapshot.movement.stuck_detection_count, 3);
        assert_eq!(snapshot.movement.navigation_success_rate, 0.75);
        assert_eq!(snapshot.loot.items_looted_per_hour, 1.0);
        assert_eq!(snapshot.loot.plat_collected_per_hour, 25.0);
        assert_eq!(snapshot.alerts.len(), 1);
    }
}
