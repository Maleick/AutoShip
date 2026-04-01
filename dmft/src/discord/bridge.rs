//! Command bridge — channel-based interface between Discord bot and TUI.
//!
//! The bridge provides two channels:
//! - **Inbound** (Discord → TUI): Commands typed in Discord (e.g., "!login all")
//!   arrive as `BridgeCommand` structs that the TUI polls each tick.
//! - **Outbound** (TUI → Discord): Responses and status updates flow back to
//!   Discord for display in the channel.
//!
//! The bridge itself is transport-agnostic — it just moves messages between
//! two endpoints via `std::sync::mpsc`. A Discord bot (serenity, twilight, or
//! even a simple REST poller) connects to one end; the TUI connects to the other.

use std::sync::mpsc;

/// A command received from Discord to be executed by the TUI.
#[derive(Debug, Clone)]
pub struct BridgeCommand {
    /// The raw command string (e.g., "login all", "camp start permafrost").
    /// No leading `:` — the TUI will treat it exactly like a command-bar input.
    pub command: String,
    /// Discord user who sent the command (for logging/response routing).
    pub sender: String,
    /// Discord channel ID to reply in (for response routing).
    pub channel_id: String,
}

/// A response from the TUI back to Discord.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields read by the Discord bot side
pub struct BridgeResponse {
    /// The status message produced by the TUI command execution.
    pub message: String,
    /// The channel ID to reply in.
    pub channel_id: String,
}

/// The TUI side of the Discord bridge.
///
/// The TUI holds this and calls `poll()` each tick to check for inbound commands,
/// and `respond()` to send results back to Discord.
pub struct TuiBridge {
    /// Receive commands from Discord.
    cmd_rx: mpsc::Receiver<BridgeCommand>,
    /// Send responses back to Discord.
    resp_tx: mpsc::Sender<BridgeResponse>,
}

impl TuiBridge {
    /// Poll for an inbound Discord command. Non-blocking.
    #[must_use]
    pub fn poll(&self) -> Option<BridgeCommand> {
        self.cmd_rx.try_recv().ok()
    }

    /// Send a response back to Discord.
    pub fn respond(&self, response: BridgeResponse) {
        if let Err(e) = self.resp_tx.send(response) {
            tracing::warn!("Discord bridge response channel closed: {}", e);
        }
    }
}

/// The Discord bot side of the bridge.
///
/// The bot holds this and calls `send_command()` when a user types a command
/// in Discord, then `poll_response()` to get the TUI's reply.
#[allow(dead_code)] // Public API — used by Discord bot integration
pub struct BotBridge {
    /// Send commands to the TUI.
    cmd_tx: mpsc::Sender<BridgeCommand>,
    /// Receive responses from the TUI.
    resp_rx: mpsc::Receiver<BridgeResponse>,
}

#[allow(dead_code)] // Public API — used by Discord bot integration
impl BotBridge {
    /// Send a command to the TUI for execution.
    pub fn send_command(
        &self,
        command: BridgeCommand,
    ) -> Result<(), mpsc::SendError<BridgeCommand>> {
        self.cmd_tx.send(command)
    }

    /// Poll for a response from the TUI. Non-blocking.
    #[must_use]
    pub fn poll_response(&self) -> Option<BridgeResponse> {
        self.resp_rx.try_recv().ok()
    }
}

/// Create a linked bridge pair: one for the TUI, one for the Discord bot.
#[allow(dead_code)] // Public API — called when Discord bot is initialized
#[must_use]
pub fn create_bridge() -> (TuiBridge, BotBridge) {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (resp_tx, resp_rx) = mpsc::channel();

    let tui_side = TuiBridge { cmd_rx, resp_tx };
    let bot_side = BotBridge { cmd_tx, resp_rx };

    (tui_side, bot_side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_roundtrip() {
        let (tui, bot) = create_bridge();

        // Bot sends command
        bot.send_command(BridgeCommand {
            command: "login all".into(),
            sender: "TestUser".into(),
            channel_id: "12345".into(),
        })
        .unwrap();

        // TUI receives it
        let cmd = tui.poll().unwrap();
        assert_eq!(cmd.command, "login all");
        assert_eq!(cmd.sender, "TestUser");

        // TUI responds
        tui.respond(BridgeResponse {
            message: "Launching 6 accounts...".into(),
            channel_id: cmd.channel_id,
        });

        // Bot receives response
        let resp = bot.poll_response().unwrap();
        assert_eq!(resp.message, "Launching 6 accounts...");
    }

    #[test]
    fn bridge_poll_empty_returns_none() {
        let (tui, _bot) = create_bridge();
        assert!(tui.poll().is_none());
    }

    #[test]
    fn bridge_multiple_commands() {
        let (tui, bot) = create_bridge();

        for i in 0..3 {
            bot.send_command(BridgeCommand {
                command: format!("cmd{}", i),
                sender: "User".into(),
                channel_id: "ch".into(),
            })
            .unwrap();
        }

        for i in 0..3 {
            let cmd = tui.poll().unwrap();
            assert_eq!(cmd.command, format!("cmd{}", i));
        }
        assert!(tui.poll().is_none());
    }
}
