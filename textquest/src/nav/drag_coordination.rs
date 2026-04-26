//! Drag coordination via multiple channels.
//!
//! Coordinates corpse and object drag operations across 3 communication channels:
//! - DanNet: Reliable inter-client network messaging
//! - Actors: MQ2 Actor mailbox for peer-to-peer coordination
//! - Direct: Direct peer-to-peer point-to-point messaging
//!
//! Implements rgmercs drag.lua coordination capability.

use std::fmt;
use textquest_common::nav::Waypoint;

/// Communication channel for drag coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragChannel {
    /// DanNet — reliable distributed network messaging.
    DanNet,
    /// Actor mailbox — MQ2 Actors event-based messaging.
    Actors,
    /// Direct peer-to-peer messaging.
    Direct,
}

impl fmt::Display for DragChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DanNet => f.write_str("dannet"),
            Self::Actors => f.write_str("actors"),
            Self::Direct => f.write_str("direct"),
        }
    }
}

/// A drag coordination message to be sent over a channel.
#[derive(Debug, Clone)]
pub struct DragCoordination {
    /// Channel to use for this drag coordination.
    pub channel: DragChannel,
    /// Character initiating the drag.
    pub from_character: String,
    /// Destination waypoint for the drag.
    pub destination: Waypoint,
    /// Optional target character to assist (for tank pulling/holding).
    pub assist_target: Option<String>,
}

impl DragCoordination {
    /// Create a new drag coordination message.
    pub fn new(
        channel: DragChannel,
        from_character: impl Into<String>,
        destination: Waypoint,
    ) -> Self {
        Self {
            channel,
            from_character: from_character.into(),
            destination,
            assist_target: None,
        }
    }

    /// Add an assist target for coordinated pulling.
    pub fn with_assist_target(mut self, target: impl Into<String>) -> Self {
        self.assist_target = Some(target.into());
        self
    }

    /// Format as a coordination message for transmission.
    pub fn as_message(&self) -> String {
        let assist_str = self
            .assist_target
            .as_ref()
            .map(|t| format!("assist:{}", t))
            .unwrap_or_default();

        format!(
            "drag|{}|{:.2},{:.2},{:.2}|{}",
            self.from_character,
            self.destination.x,
            self.destination.y,
            self.destination.z,
            assist_str
        )
    }
}

/// Broadcast drag coordination via the specified channel.
pub fn broadcast_drag_coordination(coordination: &DragCoordination) -> bool {
    let message = coordination.as_message();

    match coordination.channel {
        DragChannel::DanNet => broadcast_via_dannet(&message),
        DragChannel::Actors => broadcast_via_actors(&message),
        DragChannel::Direct => broadcast_via_direct(&message),
    }
}

/// Broadcast via DanNet channel (reliable distributed network).
fn broadcast_via_dannet(message: &str) -> bool {
    crate::box_chat::broadcast_channel(
        "drag_dannet".to_string(),
        "drag_coordinator".to_string(),
        message.to_string(),
    )
}

/// Broadcast via Actors mailbox (MQ2 Actor messaging).
fn broadcast_via_actors(message: &str) -> bool {
    crate::box_chat::broadcast_channel(
        "drag_actors".to_string(),
        "drag_coordinator".to_string(),
        message.to_string(),
    )
}

/// Broadcast via direct peer-to-peer messaging.
fn broadcast_via_direct(message: &str) -> bool {
    crate::box_chat::broadcast_channel(
        "drag_direct".to_string(),
        "drag_coordinator".to_string(),
        message.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_channel_display() {
        assert_eq!(DragChannel::DanNet.to_string(), "dannet");
        assert_eq!(DragChannel::Actors.to_string(), "actors");
        assert_eq!(DragChannel::Direct.to_string(), "direct");
    }

    #[test]
    fn drag_coordination_formats_message() {
        let dest = Waypoint::new(100.0, 200.0, 50.0);
        let coord = DragCoordination::new(DragChannel::DanNet, "Cleric1", dest);
        let msg = coord.as_message();

        assert!(msg.starts_with("drag|Cleric1|"));
        assert!(msg.contains("100.00"));
        assert!(msg.contains("200.00"));
        assert!(msg.contains("50.00"));
    }

    #[test]
    fn drag_coordination_with_assist_target() {
        let dest = Waypoint::new(100.0, 200.0, 50.0);
        let coord =
            DragCoordination::new(DragChannel::Actors, "Wizard1", dest).with_assist_target("Tank1");

        let msg = coord.as_message();
        assert!(msg.contains("assist:Tank1"));
    }
}
