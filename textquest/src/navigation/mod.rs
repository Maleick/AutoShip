//! Return-state machine for camp auto-return — NoAggro/NotLooting gates and
//! jitter delay.
//!
//! See [`camp_spot::CampReturnMachine`] for the main entry point.

/// Return-state machine: NoAggro/NotLooting gates + jitter pre-move delay.
pub mod camp_spot;

/// 2-D positioning helpers and arrival detection for camp return.
pub mod positioning;
