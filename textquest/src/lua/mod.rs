//! Lua API bindings for TextQuest.
//!
//! This module exposes TextQuest core functionality to Lua scripts via mlua.
//! The API is organized into domains: player, group, navigation, combat, state, config, logging, events.

pub mod bindings;
pub mod error;
pub mod loader;
pub mod sandbox;
pub mod types;

pub use bindings::LuaBindings;
pub use error::LuaApiError;
pub use loader::{create_loader, LoadedScript, LuaLoaderError, ScriptLoader, ScriptState};
pub use types::*;