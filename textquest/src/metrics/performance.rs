//! Real-time performance monitoring facade.
//!
//! This module is the narrow API layer that higher-level surfaces can wrap for
//! operator dashboards, Lua bindings, persistence, and exports.

use serde::{Deserialize, Serialize};

use crate::metrics::collector::MetricsCollector;

/// Minimal read API for live performance metrics.
pub trait PerformanceMetricsApi {
    /// Average DPS for a character in the current collector window.
    fn get_dps(&self, character_name: &str) -> f64;
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
}

impl PerformanceMetricsApi for PerformanceMonitor<'_> {
    fn get_dps(&self, character_name: &str) -> f64 {
        average_character_dps(self.collector, character_name)
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

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        return 0.0;
    }

    numerator / denominator
}
