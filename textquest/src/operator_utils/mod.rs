//! Operator utilities: clipboard export and persistent scratchpad.
//!
//! This module provides utilities for operators to manage notes and export data.

pub mod clipboard;
pub mod scratchpad;

pub use clipboard::copy_to_clipboard;
pub use scratchpad::{Note, Scratchpad};
