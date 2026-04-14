//! Zone transition management — failure codes, recovery actions, and retry logic.
//!
//! This module handles zone transitions with comprehensive failure code mapping,
//! recovery action planning, and exponential backoff retry logic.

pub mod failure_codes;

pub use failure_codes::{RecoveryAction, ZoneFailureCode, ZoneFailureState};
