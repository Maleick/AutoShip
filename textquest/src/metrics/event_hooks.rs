//! Event hooks for combat, movement, and loot systems.
//!
//! Captures in-game events (damage, movement, loot pickups) and forwards them
//! to the metrics collector for real-time tracking and fleet analytics.

use std::sync::{Arc, mpsc::SyncSender};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::metrics::events::FleetEvent;

/// Hook context for metrics event emission.
#[derive(Clone)]
pub struct EventHookContext {
    event_tx: Arc<Option<SyncSender<FleetEvent>>>,
}

impl EventHookContext {
    /// Create a new event hook context with an optional event channel.
    pub fn new(event_tx: Option<SyncSender<FleetEvent>>) -> Self {
        Self {
            event_tx: Arc::new(event_tx),
        }
    }

    /// Emit a combat round event.
    pub fn emit_combat_round(
        &self,
        pid: u32,
        damage_dealt: u64,
        damage_taken: u64,
        duration_ms: u32,
    ) {
        if let Some(tx) = self.event_tx.as_ref() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let event = FleetEvent::CombatRound {
                pid,
                damage_dealt,
                damage_taken,
                duration_ms,
                timestamp,
            };

            let _ = tx.try_send(event);
        }
    }

    /// Emit a loot pickup event.
    pub fn emit_loot_drop(&self, pid: u32, item_name: String, item_id: u32, zone: String) {
        if let Some(tx) = self.event_tx.as_ref() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let event = FleetEvent::LootDrop {
                pid,
                item_name,
                item_id,
                zone,
                timestamp,
            };

            let _ = tx.try_send(event);
        }
    }

    /// Emit a zone change event for movement tracking.
    pub fn emit_zone_change(&self, pid: u32, from_zone: String, to_zone: String) {
        if let Some(tx) = self.event_tx.as_ref() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let event = FleetEvent::ZoneChange {
                pid,
                from_zone,
                to_zone,
                timestamp,
            };

            let _ = tx.try_send(event);
        }
    }

    /// Emit a kill event.
    pub fn emit_kill(&self, source_pid: u32, target_name: String, target_level: u8, zone: String) {
        if let Some(tx) = self.event_tx.as_ref() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let event = FleetEvent::Kill {
                source_pid,
                target_name,
                target_level,
                zone,
                timestamp,
            };

            let _ = tx.try_send(event);
        }
    }

    /// Emit a death event.
    pub fn emit_death(&self, pid: u32, character_name: String, zone: String) {
        if let Some(tx) = self.event_tx.as_ref() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let event = FleetEvent::Death {
                pid,
                character_name,
                zone,
                timestamp,
            };

            let _ = tx.try_send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_hook_context_without_channel() {
        let ctx = EventHookContext::new(None);
        // Should not panic when emitting events with no channel
        ctx.emit_combat_round(1, 100, 50, 1000);
        ctx.emit_loot_drop(1, "Gold Coin".to_string(), 1001, "PoK".to_string());
        ctx.emit_zone_change(1, "PoK".to_string(), "Crescent".to_string());
        ctx.emit_kill(1, "Giant".to_string(), 50, "PoK".to_string());
        ctx.emit_death(1, "Tester".to_string(), "PoK".to_string());
    }
}
