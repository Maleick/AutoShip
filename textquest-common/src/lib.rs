//! Shared types and utilities for the TextQuest workspace.
//!
//! This crate contains domain types, IPC protocol definitions, EQ memory
//! offsets, and common structures used by both the external orchestrator
//! (`textquest`) and the injected DLL (`textquest-dll`).

#[doc(hidden)]
pub use paste;

/// Audio alert configuration, registry, and playback backend scaffolding.
pub mod audio_alerts;
/// Auto-group configuration and invite/role controller logic.
pub mod auto_group;
/// Typed, runtime-rebased function bindings.
pub mod bindings;
/// Box-chat config, slash-route parsing, and TCP relay wire types.
pub mod box_chat;
/// Unified box-controller command and state types shared by the runtime, DLL,
/// and web dashboard.
pub mod box_controller;
/// Shared character configuration schema and persistence helpers.
pub mod character_config;
/// Chat channel types, STML stripping, and structured chat event parsing.
pub mod chat;
/// User-defined chat pattern rule engine (MQ2Events/MQ2React parity).
pub mod chat_pattern_rules;
/// Combat-related shared types (class roles, spell metadata, assist targets).
pub mod combat;
/// Shared cryptographic primitives and secret-handling helpers.
pub mod crypto;
/// Economy ledger schema, transaction logging, and trend metrics.
pub mod economy;
/// Unified error handling framework with structured error types and recovery
/// actions.
pub mod errors;
/// ETW-TI event parser and LoadLibrary injection detector.
pub mod etw_ti_detect;
/// SQLite-backed database for Ghidra binary analysis data.
pub mod ghidra_db;
/// GM interaction detection and account safety warning alerts.
pub mod gm_detection;
/// External integration infrastructure for notifications and alerts.
pub mod integrations;
/// Shared inventory-utility parity config, provenance, and rule helpers.
pub mod inventory_utility;
/// IPC command and response enums for orchestrator-to-DLL communication.
pub mod ipc;
/// Launch profiles and session preset translation layer for M8 orchestrator.
pub mod launch_profile;
/// Login automation shared types (credentials, server selection, login phases).
pub mod login;
/// Account ban/suspension detection utilities and registry.
pub mod account_safety;
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
/// Extensibility contracts, plugin lifecycle, and plugin registry scaffolding.
pub mod plugins;
/// Wire protocol definitions for serialized IPC messages.
pub mod protocol;
/// Raid-wide member aggregation, camp snapshots, events, and relay planning.
pub mod raid;
/// Routing scope types for cross-client command dispatch (M8 Orchestrator).
pub mod routing;
/// Safe coordinate types for zone transition recovery and position validation.
pub mod safe_coords;
/// Scan engine for runtime offset auto-detection (Auto Patch #746).
pub mod scan_engine;
/// Byte-pattern signature scanner for offset resolution across EQ patches.
pub mod scanner;
/// NetBots-style cross-client roster summaries shared across runtimes.
pub mod shared_client_state;
/// Soul Engine shared types (LLM personalities, memory, social dynamics).
pub mod soul;
/// Runtime spawn finder snapshot types shared by orchestrator and web.
pub mod spawn_finder;
/// Tradeskill trophy config and swap state machine shared by runtime layers.
pub mod tradeskill_trophy;
/// Common type aliases and utility structures.
pub mod types;
/// Struct size/range validation helpers.
pub mod validation;
/// EQ window title formatting helpers shared by the orchestrator and DLL.
pub mod window_title;
/// Zone transition retry logic with exponential backoff.
pub mod zone_transition;
