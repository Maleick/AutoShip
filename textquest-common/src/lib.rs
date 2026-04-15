//! Shared types and utilities for the TextQuest workspace.
//!
//! This crate contains domain types, IPC protocol definitions, EQ memory
//! offsets, and common structures used by both the external orchestrator
//! (`textquest`) and the injected DLL (`textquest-dll`).

#[doc(hidden)]
pub use paste;

/// Typed, runtime-rebased function bindings.
pub mod bindings;
/// Chat channel types, STML stripping, and structured chat event parsing.
pub mod chat;
/// Combat-related shared types (class roles, spell metadata, assist targets).
pub mod combat;
/// Unified error handling framework with structured error types and recovery
/// actions.
pub mod errors;
/// ETW-TI event parser and LoadLibrary injection detector.
pub mod etw_ti_detect;
/// SQLite-backed database for Ghidra binary analysis data.
pub mod ghidra_db;
/// External integration infrastructure for notifications and alerts.
pub mod integrations;
/// IPC command and response enums for orchestrator-to-DLL communication.
pub mod ipc;
/// Login automation shared types (credentials, server selection, login phases).
pub mod login;
/// Navigation shared types (waypoints, zones, pathfinding requests).
pub mod nav;
/// Observability infrastructure for metrics collection and structured logging.
pub mod observability;
/// Hot-updatable offset database backed by JSON.
pub mod offset_db;
/// EQ memory addresses and struct field offsets (preferred-base, rebased at
/// runtime).
pub mod offsets;
/// Packet capture types — opcode filtering, capture sessions, and disk
/// persistence.
pub mod packet;
/// Pattern database registry for scan entries and offset metadata.
pub mod pattern_db;
/// UDP multicast peer-discovery announcement types.
pub mod peer_discovery;
/// Data persistence framework with schema migration support.
pub mod persistence;
/// Wire protocol definitions for serialized IPC messages.
pub mod protocol;
/// Routing scope types for cross-client command dispatch (M8 Orchestrator).
pub mod routing;
/// Safe coordinate types for zone transition recovery and position validation.
pub mod safe_coords;
/// Scan engine for runtime offset auto-detection (Auto Patch #746).
pub mod scan_engine;
/// Byte-pattern signature scanner for offset resolution across EQ patches.
pub mod scanner;
/// Soul Engine shared types (LLM personalities, memory, social dynamics).
pub mod soul;
/// Common type aliases and utility structures.
pub mod types;
/// Struct size/range validation helpers.
pub mod validation;
/// Zone transition retry logic with exponential backoff.
pub mod zone_transition;
