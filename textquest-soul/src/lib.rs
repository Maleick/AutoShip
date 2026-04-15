//! Soul Engine — LLM-driven character personalities, persistent memory, social
//! dynamics.

/// Soul operator alerts and anomaly detection.
pub mod alerts;
/// Soul audit logging — append-only JSONL log of key state changes.
pub mod audit;
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
/// Idle behavior system — generates ambient actions when characters are not
/// busy.
pub mod idle;
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
/// LLM request rate limiting.
pub mod rate_limiter;
/// Error handling and recovery — retry, fallback, reload, and restart
/// strategies.
pub mod recovery;
/// Resource leak detection and cleanup verification.
pub mod resource_checks;
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
