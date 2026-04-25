//! Coordinated /say with random delay — qsay coordination for realistic group speech.
//!
//! Implements rgmercs-style qsay (quoted say) with randomized inter-client delays
//! for realistic group communication. Sends coordinated messages across all clients
//! with staggered timing to avoid synchronized speech detection.

use rand::Rng;
use std::time::Duration;

/// Configuration for qsay coordination.
#[derive(Debug, Clone)]
pub struct QsayConfig {
    /// Minimum delay in milliseconds before sending /say.
    pub min_delay_ms: u64,
    /// Maximum delay in milliseconds before sending /say.
    pub max_delay_ms: u64,
}

impl Default for QsayConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: 100,
            max_delay_ms: 2000,
        }
    }
}

/// Qsay message to be broadcast with random delay.
#[derive(Debug, Clone)]
pub struct QsayMessage {
    /// The message text to broadcast via /say.
    pub text: String,
    /// Configuration for delay randomization.
    pub config: QsayConfig,
}

impl QsayMessage {
    /// Create a new qsay message with default delay config.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            config: QsayConfig::default(),
        }
    }

    /// Create a qsay message with custom delay config.
    pub fn with_config(text: impl Into<String>, config: QsayConfig) -> Self {
        Self {
            text: text.into(),
            config,
        }
    }

    /// Get a randomized delay for this message.
    pub fn random_delay(&self) -> Duration {
        let mut rng = rand::thread_rng();
        let delay_ms = rng.gen_range(self.config.min_delay_ms..=self.config.max_delay_ms);
        Duration::from_millis(delay_ms)
    }

    /// Format as a /say slash command.
    pub fn as_command(&self) -> String {
        format!("/say {}", self.text)
    }
}

/// Broadcast a qsay message to all peers with random delay.
pub fn broadcast_qsay(message: &QsayMessage) -> bool {
    let delay = message.random_delay();
    let command = message.as_command();

    // Broadcast via box_chat with metadata indicating this is qsay for peer coordination.
    crate::box_chat::broadcast_channel(
        "qsay".to_string(),
        "qsay_coordinator".to_string(),
        format!("{}|{}", command, delay.as_millis()),
    )
}

/// Broadcast a quick qsay with default configuration.
pub fn qsay(text: impl Into<String>) -> bool {
    broadcast_qsay(&QsayMessage::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qsay_message_formats_as_command() {
        let msg = QsayMessage::new("ready");
        assert_eq!(msg.as_command(), "/say ready");
    }

    #[test]
    fn qsay_message_with_config_respects_delays() {
        let config = QsayConfig {
            min_delay_ms: 100,
            max_delay_ms: 200,
        };
        let msg = QsayMessage::with_config("test", config);
        let delay = msg.random_delay();

        assert!(delay.as_millis() >= 100);
        assert!(delay.as_millis() <= 200);
    }

    #[test]
    fn default_qsay_config_uses_reasonable_bounds() {
        let config = QsayConfig::default();
        assert!(config.min_delay_ms > 0);
        assert!(config.max_delay_ms > config.min_delay_ms);
    }
}
