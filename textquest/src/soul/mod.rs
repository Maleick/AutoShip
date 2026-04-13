//! Soul Engine — LLM-driven character personalities, persistent memory, social dynamics.

/// Soul audit logging — append-only JSONL log of key state changes.
pub mod audit;
/// Soul Engine configuration — API keys, model settings, personality tuning.
pub mod config;
/// Soul coordinator — orchestrates personality, memory, idle, and social systems.
pub mod coordinator;
/// SQLite schema validation and migration framework for Soul Engine.
pub mod db_validation;
/// Idle behavior system — generates ambient actions when characters are not busy.
pub mod idle;
/// LLM integration — provider trait, request/response types, fallback generation.
pub mod llm;
/// Persistent memory — stores character experiences and relationships across sessions.
pub mod memory;
/// Performance regression tests for Soul Engine latency budgets.
#[cfg(test)]
mod perf_tests;
/// Personality system — trait-based character archetypes and mood modeling.
pub mod personality;
/// Social dynamics — inter-character relationships, group cohesion, banter triggers.
pub mod social;
