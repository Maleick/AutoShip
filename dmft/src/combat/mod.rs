//! Combat automation — assist broadcasting, CC assignment, spell database.

/// Camp loop combat integration — bridges camp state machine with combat actions.
pub mod camp_loop;
/// Complete Heal chain coordination for multi-cleric rotations.
pub mod ch_chain;
/// Combat coordinator — manages assist targets and broadcasts commands.
pub mod coordinator;
/// Structured combat event tracking — DPS meters, kill counts, damage aggregation.
pub mod events;
/// Spell database — spell IDs, casting times, resist types, levels.
pub mod spell_db;
