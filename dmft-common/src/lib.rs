//! Shared types and utilities for the DMFT workspace.
//!
//! This crate contains domain types, IPC protocol definitions, EQ memory offsets,
//! and common structures used by both the external orchestrator (`dmft`) and the
//! injected DLL (`dmft-dll`).

/// Combat-related shared types (class roles, spell metadata, assist targets).
pub mod combat;
/// IPC command and response enums for orchestrator-to-DLL communication.
pub mod ipc;
/// Login automation shared types (credentials, server selection, login phases).
pub mod login;
/// Navigation shared types (waypoints, zones, pathfinding requests).
pub mod nav;
/// Hot-updatable offset database backed by JSON.
pub mod offset_db;
/// EQ memory addresses and struct field offsets (preferred-base, rebased at runtime).
pub mod offsets;
/// Wire protocol definitions for serialized IPC messages.
pub mod protocol;
/// Soul Engine shared types (LLM personalities, memory, social dynamics).
pub mod soul;
/// Common type aliases and utility structures.
pub mod types;
