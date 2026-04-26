//! Combat automation — assist broadcasting, CC assignment, spell database.

/// Bard instrument swap before/after each song — MQ2BardSwap parity.
pub mod bard_swap;
/// Camp loop combat integration — bridges camp state machine with combat
/// actions.
pub mod camp_loop;
/// Complete Heal chain coordination for multi-cleric rotations.
pub mod ch_chain;
/// Charm and pet management primitives.
pub mod charm;
/// Per-class combat rotation strategy definitions.
pub mod class_strategy;
/// Combat coordinator — manages assist targets and broadcasts commands.
pub mod coordinator;
/// Structured combat event tracking — DPS meters, kill counts, damage
/// aggregation.
pub mod events;
/// Cross-group heal arbitration — prevents double-healing, priority ordering.
pub mod heal_coordinator;
/// Combat discipline scheduler with cooldown/endurance gating — MQ2Melee parity.
pub mod melee_disc;
/// Mez (crowd-control) immunity tracker with zone-change expiry.
pub mod mez_tracker;
/// Named NPC and boss encounter tracking.
pub mod named;
/// Spell database — spell IDs, casting times, resist types, levels.
pub mod spell_db;
/// Advanced spell optimizer and casting predictor — mana efficiency ranking,
/// cast-time filtering, and haste-adjusted cast time prediction.
pub mod spell_optimizer;
/// Combatant state machine primitives.
pub mod state;
/// Worst-hurt group/pet scanner for heal targeting — MQ2WorstHurt parity.
pub mod worst_hurt;
