//! Self-improvement loop — heuristic suggestion engine.
//!
//! Consumes aggregated session metrics and produces operator-actionable
//! suggestions for tunable config knobs. All computation is pure (no I/O).

pub mod heuristics;

pub use heuristics::{
    CurrentKnobs, DedupeState, KnobId, SessionAggregate, Suggestion, WaypointChange,
    rule_camp_aggro_radius, rule_combat_ability_priority, rule_heal_trigger, rule_med_break_mana,
    rule_pull_cadence, rule_retreat_hp, rule_route_waypoints, run_all_rules,
};
