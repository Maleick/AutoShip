//! Economy system — Krono farm, vendor automation, loot distribution, banking.
//!
//! This module contains the failure detection and recovery routing layer
//! for the economy loop (M10).

pub mod failure_handling;

pub use failure_handling::{
    FailureHistory, FailureRouter, FailureState, FailureType, RecoveryAction,
};
