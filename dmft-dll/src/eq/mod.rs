//! EQ internal function addresses and signatures.
//! These are offsets from the eqgame.exe base address.
//! Derived from MQ2 source headers.

/// CEverQuest::MainLoop offset from EQ base.
/// Derived from dmft_common::offsets::PROCESS_GAME_EVENTS (0x14028E0F0)
/// minus preferred base (0x140000000).
pub const MAIN_LOOP_OFFSET: usize = 0x28E0F0;

/// Movement processing function offset.
pub const MOVE_PLAYER_OFFSET: usize = 0x0; // placeholder

/// Spell casting function offset.
pub const CAST_SPELL_OFFSET: usize = 0x0; // placeholder

/// Set target function offset.
pub const SET_TARGET_OFFSET: usize = 0x0; // placeholder
