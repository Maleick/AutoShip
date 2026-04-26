//! Fleet metrics — shared storage and derived state for events, DPS, loot,
//! lockouts, plat, and admin monitoring.
//!
//! Most metrics live in a single SQLite file with separate tables, while
//! admin-monitoring keeps a bounded in-memory session store for low-latency
//! operational snapshots.

pub mod anomaly;
pub mod admin_monitoring;
pub mod baseline_scorecard;
pub mod bayesian_baseline;
pub mod collector;
pub mod event_hooks;
pub mod events;
pub mod exporter;
pub mod kill_reporter;
pub mod kill_session_store;
pub mod kill_tracker;
pub mod performance;
pub mod persistence;
pub mod plat_tracker;
pub mod progress;
pub mod sampling;
pub mod session_recorder;
mod store;
pub mod types;
pub mod xp_tracker;

pub use admin_monitoring::{
    AdminMonitoringRetention, AdminMonitoringStore, MonitoredSessionState, SessionErrorKind,
    SessionMonitoringSnapshot, sample_process_memory_bytes,
};
pub use event_hooks::EventHookContext;
pub use baseline_scorecard::{
    BaselineScorecard, CombatDelta, CombatMetrics, CoordinationDelta, EconomyDelta, EconomyMetrics,
    GroupCoordinationMetrics, MovementDelta, MovementMetrics, ScorecardDelta,
};
pub use bayesian_baseline::{
    BaselineRegistry, BetaBinomialPrior, CharacterBaseline, DeviationThresholds, GaussianPrior,
    Suggestion,
};
pub use collector::{
    AggregateMetrics, CharacterCombatMetrics, CharacterEconomyMetrics, CharacterMetrics,
    CharacterMovementMetrics, FleetMetrics, LootEvent, MetricsCollector, TimeWindow,
    TimeWindowMetrics,
};
pub use session_recorder::{SessionEvent, SessionEventKind, SessionRecorder, SessionRecorderConfig};
pub use events::{FleetEvent, FleetEventLog};
pub use kill_reporter::KillReporter;
pub use kill_session_store::KillSessionStore;
pub use kill_tracker::{ClientDpsStats, EfficiencyScore, KillRecord, KillTracker, MobStats};
pub use performance::{
    CharacterDpsSnapshot, DashboardCombatMetrics, DashboardLootMetrics, DashboardMovementMetrics,
    DashboardSystemMetrics, FarmEfficiencySnapshot, PerformanceAlertSeverity,
    PerformanceDashboardAlert, PerformanceDashboardSnapshot, PerformanceDashboardTab,
    PerformanceMetricsApi, PerformanceMonitor,
};
pub use plat_tracker::{PlatCategory, PlatSessionStore, PlatTracker, PlatTransaction};
pub use progress::{ClientProgressSnapshot, ProgressReport, ProgressTracker};
pub use anomaly::{
    AnomalyEvent, AnomalyId, AnomalyPipeline, AnomalySeverity, SessionSnapshot,
};
pub use store::MetricsStore;
pub use types::{
    CharacterMetrics as RtCharacterMetrics, CombatMetrics as RtCombatMetrics,
    FleetMetrics as RtFleetMetrics, LootMetrics, MetricWindow,
    MovementMetrics as RtMovementMetrics, SystemMetrics, TimeWindowedMetrics,
};
pub use xp_tracker::{XpSample, XpSessionSnapshot, XpTracker};
pub use persistence::{MetricsPersister, MetricsHistoryRow};
