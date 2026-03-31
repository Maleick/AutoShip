//! Discord integration — webhook alerts and command bridge.
//!
//! Two integration layers:
//! 1. **Webhook alerts** (outbound): POST to a Discord webhook URL for HVT alerts,
//!    client crashes, mass failures, and status updates. Zero extra dependencies.
//! 2. **Command bridge** (bidirectional): Channel-based interface for a Discord bot
//!    to relay commands into the TUI and receive responses.
//!
//! The webhook layer works immediately with just a URL in config.
//! The command bridge is designed for a future `serenity`-based bot.

pub mod webhook;
pub mod bridge;
