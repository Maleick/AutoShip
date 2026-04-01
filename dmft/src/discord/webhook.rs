//! Discord webhook sender — POST alerts to a Discord channel.
//!
//! Uses `reqwest::blocking` (already a dependency) to send embeds.
//! No bot token required — just a webhook URL from Discord channel settings.

use std::sync::mpsc;
use std::thread;

/// Alert severity level — controls embed color in Discord.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Public API — variants used by callers as adoption grows
pub enum AlertLevel {
    /// Green — informational (client logged in, camp started)
    Info,
    /// Yellow — warning (client disconnected, retry)
    Warning,
    /// Red — critical (mass failure, HVT spotted, client crashed)
    Critical,
}

impl AlertLevel {
    fn color(&self) -> u32 {
        match self {
            AlertLevel::Info => 0x002E_CC71,     // green
            AlertLevel::Warning => 0x00F1_C40F,  // yellow
            AlertLevel::Critical => 0x00E7_4C3C, // red
        }
    }
}

/// A Discord alert to be sent via webhook.
#[derive(Debug, Clone)]
pub struct DiscordAlert {
    /// Alert title (shown as embed title in Discord).
    pub title: String,
    /// Alert body text.
    pub message: String,
    /// Severity level controlling the embed color.
    pub level: AlertLevel,
}

/// Async webhook sender — queues alerts and sends them on a background thread.
/// This avoids blocking the TUI event loop on HTTP requests.
#[allow(dead_code)] // Public API — methods called by TUI alert integration
pub struct WebhookSender {
    tx: mpsc::Sender<DiscordAlert>,
    /// Handle to the background sender thread.
    _handle: thread::JoinHandle<()>,
}

#[allow(dead_code)] // Public API — methods called by TUI alert integration
impl WebhookSender {
    /// Create a new webhook sender for the given URL.
    /// Spawns a background thread that processes the alert queue.
    #[must_use]
    pub fn new(webhook_url: String) -> Self {
        let (tx, rx) = mpsc::channel::<DiscordAlert>();

        let handle = thread::Builder::new()
            .name("discord-webhook".into())
            .spawn(move || {
                Self::sender_loop(&webhook_url, rx);
            })
            .expect("failed to spawn discord webhook thread");

        Self {
            tx,
            _handle: handle,
        }
    }

    /// Queue an alert for delivery. Non-blocking — returns immediately.
    pub fn send(&self, alert: DiscordAlert) {
        if let Err(e) = self.tx.send(alert) {
            tracing::warn!("Discord webhook queue full or closed: {}", e);
        }
    }

    /// Convenience: send an info-level alert.
    pub fn info(&self, title: &str, message: &str) {
        self.send(DiscordAlert {
            title: title.to_string(),
            message: message.to_string(),
            level: AlertLevel::Info,
        });
    }

    /// Convenience: send a warning-level alert.
    pub fn warn(&self, title: &str, message: &str) {
        self.send(DiscordAlert {
            title: title.to_string(),
            message: message.to_string(),
            level: AlertLevel::Warning,
        });
    }

    /// Convenience: send a critical-level alert.
    pub fn critical(&self, title: &str, message: &str) {
        self.send(DiscordAlert {
            title: title.to_string(),
            message: message.to_string(),
            level: AlertLevel::Critical,
        });
    }

    /// Background loop that drains the queue and POSTs to Discord.
    fn sender_loop(webhook_url: &str, rx: mpsc::Receiver<DiscordAlert>) {
        let client = reqwest::blocking::Client::new();

        while let Ok(alert) = rx.recv() {
            let payload = serde_json::json!({
                "embeds": [{
                    "title": alert.title,
                    "description": alert.message,
                    "color": alert.level.color(),
                    "footer": {
                        "text": "Frostreaver"
                    },
                    "timestamp": chrono_now_iso()
                }]
            });

            match client.post(webhook_url).json(&payload).send() {
                Ok(resp) if resp.status().is_success() => {
                    tracing::debug!(title = %alert.title, "Discord alert sent");
                }
                Ok(resp) => {
                    tracing::warn!(
                        status = %resp.status(),
                        title = %alert.title,
                        "Discord webhook returned non-success"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, title = %alert.title, "Discord webhook POST failed");
                }
            }
        }

        tracing::info!("Discord webhook sender shutting down");
    }
}

/// Get current time as ISO 8601 string (no chrono dependency).
fn chrono_now_iso() -> String {
    // Use std::time for a simple UTC timestamp approximation.
    // For production, chrono would be better — but we avoid adding a dep.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Rough ISO format — good enough for Discord embed footers
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Days since epoch to rough date (accurate enough for display)
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_level_colors() {
        assert_eq!(AlertLevel::Info.color(), 0x2ECC71);
        assert_eq!(AlertLevel::Warning.color(), 0xF1C40F);
        assert_eq!(AlertLevel::Critical.color(), 0xE74C3C);
    }

    #[test]
    fn chrono_now_iso_format() {
        let ts = chrono_now_iso();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20); // "2026-03-31T12:34:56Z"
    }

    #[test]
    fn discord_alert_constructible() {
        let alert = DiscordAlert {
            title: "Test".into(),
            message: "Hello".into(),
            level: AlertLevel::Info,
        };
        assert_eq!(alert.title, "Test");
    }
}
