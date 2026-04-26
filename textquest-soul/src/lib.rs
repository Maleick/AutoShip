//! Soul Engine — LLM-driven character personalities, persistent memory, social
//! dynamics.

/// Soul operator alerts and anomaly detection.
pub mod alerts;
/// Soul audit logging — append-only JSONL log of key state changes.
pub mod audit;
/// Inter-character banter — proximity and relationship-based dialogue
/// triggering.
pub mod banter;
/// Soul Engine configuration — API keys, model settings, personality tuning.
pub mod config;
/// Soul Engine configuration validation — validates SoulConfig fields and
/// returns structured errors.
pub mod config_validator;
/// Soul coordinator — orchestrates personality, memory, idle, and social
/// systems.
pub mod coordinator;
/// SQLite schema validation and migration framework for Soul Engine.
pub mod db_validation;
pub mod debrief_aggregator;
/// Tier-1 heuristic engine — rule-based pattern detection for gameplay
/// anti-patterns (downtime, mana bottleneck, pull rate, camp drift).
pub mod heuristics;
/// Idle behavior system — generates ambient actions when characters are not
/// busy.
pub mod idle;
/// Streaming anomaly-detection pipeline for the self-improvement loop.
pub mod improve;
/// LLM integration — provider trait, request/response types, fallback
/// generation.
pub mod llm;
/// Persistent memory — stores character experiences and relationships across
/// sessions.
pub mod memory;
/// Natural mood decay over time.
pub mod mood_decay;
/// Operator controls and safety mechanisms.
pub mod operator;
/// Personality system — trait-based character archetypes and mood modeling.
pub mod personality;
/// Personality drift — gradual trait changes driven by in-game experiences.
pub mod personality_drift;
/// LLM request rate limiting.
pub mod rate_limiter;
/// Error handling and recovery — retry, fallback, reload, and restart
/// strategies.
pub mod recovery;
/// Resource leak detection and cleanup verification.
pub mod resource_checks;
/// LanceDB-backed semantic memory and embedding utilities.
pub mod semantic_memory;
/// Keyword-based sentiment scoring.
pub mod sentiment;
/// Social dynamics — inter-character relationships, group cohesion, banter
/// triggers.
pub mod social;
/// Speech style evolution — catchphrase learning and adoption mechanics.
pub mod speech_evolution;
/// Game-state suppression rules — prevent soul actions from interfering with
/// orchestrator loops.
pub mod suppression;
/// Zone classification — environmental metadata that constrains idle behavior
/// selection.
pub mod zone_classifier;
/// Zone metadata and environment-aware idle constraints.
pub mod zones;
