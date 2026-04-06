//! Discord integration — webhook alerts, command bridge, and embedded bot.
//!
//! Three integration layers:
//! 1. **Webhook alerts** (outbound): POST to a Discord webhook URL for HVT alerts,
//!    client crashes, mass failures, and status updates. Zero extra dependencies.
//! 2. **Command bridge** (bidirectional): Channel-based interface for the Discord bot
//!    to relay commands into the TUI and receive responses.
//! 3. **Embedded bot** (serenity): Runs as a tokio task within the orchestrator.
//!    DZ lockout tracking, contested mob spawn announcements, slash commands.

pub mod bot;
pub mod bridge;
pub mod webhook;
