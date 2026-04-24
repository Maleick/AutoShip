//! Shared branding constants — keeps TUI and web UI wordmark/version in lockstep.
//!
//! Any surface that renders the product name or version MUST read from here
//! rather than hardcoding a literal. This module is the single source of truth
//! for the `textquest` crate; the web frontend mirrors these values manually
//! from `textquest-web/frontend/src/App.tsx` (see issue for drift prevention).

/// Uppercase wordmark — matches the Cinzel `TEXTQUEST` treatment in the web UI.
pub const WORDMARK: &str = "TEXTQUEST";

/// Crate version pulled at compile time from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
