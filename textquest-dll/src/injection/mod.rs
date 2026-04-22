//! Process injection subsystem.
//!
//! Injection variants for loading the TextQuest payload into `eqgame.exe`.
//!
//! | Module | Variant | Doc |
//! |--------|---------|-----|
//! | [`apc`] | Early Bird APC (B8) | `docs/research/B8-early-bird-apc-injection.md` |
//!
//! The current default injection path (reflective loader via PoolParty) lives
//! in `textquest-dll/src/stealth/thread_pool.rs`. This module houses newer
//! variants that target the pre-AC initialization window.

pub mod apc;
