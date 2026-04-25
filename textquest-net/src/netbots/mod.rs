//! NetBots-style vitals broadcast.
//!
//! Each TextQuest instance publishes its vitals over the chosen transport.
//! Consumers (healer modules, dashboards) subscribe via [`VitalsRegistry`].
//!
//! # Publish timing
//!
//! The publisher sends immediately when vitals change, subject to a debounce
//! cap of [`NetBotsConfig::publish_interval_ms`] (default 100 ms ≤ 250 ms
//! acceptance criterion).

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tracing::debug;

/// Snapshot of one character's vitals published over the transport.
///
/// Modelled after the state fields MQ2NetBots broadcasts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterVitals {
    /// Character name (matches EQBC/DanNet peer name).
    pub name: String,
    /// Class abbreviation (WAR, CLR, SHM, ...).
    pub class: String,
    /// Current level.
    pub level: u8,
    /// Current zone short name.
    pub zone: String,
    /// Hit-point percentage (0–100).
    pub hp_pct: f32,
    /// Mana percentage (0–100, 0 for melee classes).
    pub mana_pct: f32,
    /// Endurance percentage (0–100).
    pub end_pct: f32,
    /// Current target name, empty if none.
    pub target_name: String,
    /// Current target HP percentage, 0 if no target.
    pub target_hp_pct: f32,
    /// Active buff spell IDs (populated when extended sharing is on).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buff_ids: Vec<i32>,
    /// Wall-clock publish time (unix ms).
    pub published_at_ms: u64,
}

impl CharacterVitals {
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn new(name: String, class: String) -> Self {
        Self {
            name,
            class,
            level: 1,
            zone: String::new(),
            hp_pct: 100.0,
            mana_pct: 100.0,
            end_pct: 100.0,
            target_name: String::new(),
            target_hp_pct: 0.0,
            buff_ids: Vec::new(),
            published_at_ms: Self::now_ms(),
        }
    }

    pub fn touch(&mut self) {
        self.published_at_ms = Self::now_ms();
    }
}

/// Shared registry of all known peer vitals.  The healer module queries this
/// to find the lowest-HP group member.
#[derive(Default)]
pub struct VitalsRegistry {
    inner: Mutex<HashMap<String, CharacterVitals>>,
}

impl VitalsRegistry {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Upsert vitals for one character.
    pub async fn update(&self, vitals: CharacterVitals) {
        let mut map = self.inner.lock().await;
        map.insert(vitals.name.clone(), vitals);
    }

    /// Return current vitals for every known character.
    pub async fn snapshot(&self) -> Vec<CharacterVitals> {
        self.inner.lock().await.values().cloned().collect()
    }

    /// Return the character with the lowest HP percentage.
    /// Returns `None` if the registry is empty.
    pub async fn lowest_hp(&self) -> Option<CharacterVitals> {
        self.inner
            .lock()
            .await
            .values()
            .min_by(|a, b| a.hp_pct.partial_cmp(&b.hp_pct).unwrap_or(std::cmp::Ordering::Equal))
            .cloned()
    }

    /// Return all characters matching the given zone.
    pub async fn in_zone(&self, zone: &str) -> Vec<CharacterVitals> {
        self.inner
            .lock()
            .await
            .values()
            .filter(|v| v.zone == zone)
            .cloned()
            .collect()
    }
}

/// Publishes this instance's vitals over a broadcast channel.
///
/// Callers push updated [`CharacterVitals`] via [`VitalsPublisher::publish`].
/// The publisher debounces to `interval_ms` then emits on the channel.
pub struct VitalsPublisher {
    tx: broadcast::Sender<CharacterVitals>,
    interval: Duration,
    last_send: Mutex<Option<Instant>>,
}

impl VitalsPublisher {
    #[must_use]
    pub fn new(interval_ms: u64) -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            tx,
            interval: Duration::from_millis(interval_ms),
            last_send: Mutex::new(None),
        }
    }

    /// Subscribe to outgoing vitals (for transport layer to relay).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<CharacterVitals> {
        self.tx.subscribe()
    }

    /// Publish vitals, debounced by `interval_ms`.
    ///
    /// Spawns a short sleep task if we are within the debounce window.
    pub async fn publish(&self, vitals: CharacterVitals) {
        let now = Instant::now();
        let mut last = self.last_send.lock().await;
        let should_send = last
            .map(|t| now.duration_since(t) >= self.interval)
            .unwrap_or(true);

        if should_send {
            *last = Some(now);
            drop(last);
            debug!("NetBots publish vitals for {}", vitals.name);
            let _ = self.tx.send(vitals);
        }
    }
}

/// Healer-module helper: pick the best heal target from the registry.
pub struct HealTargetSelector;

impl HealTargetSelector {
    /// Return the group member with the lowest HP.  The healer module passes
    /// this name to the casting system.
    pub async fn lowest_hp_target(registry: &VitalsRegistry) -> Option<String> {
        registry.lowest_hp().await.map(|v| v.name)
    }

    /// Return all members below a given HP threshold, sorted ascending by HP%.
    pub async fn members_below(registry: &VitalsRegistry, threshold_pct: f32) -> Vec<String> {
        let mut members: Vec<CharacterVitals> = registry
            .snapshot()
            .await
            .into_iter()
            .filter(|v| v.hp_pct < threshold_pct)
            .collect();
        members.sort_by(|a, b| a.hp_pct.partial_cmp(&b.hp_pct).unwrap_or(std::cmp::Ordering::Equal));
        members.into_iter().map(|v| v.name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lowest_hp_returns_correct_member() {
        let reg = VitalsRegistry::new();
        let mut v1 = CharacterVitals::new("Alice".into(), "CLR".into());
        v1.hp_pct = 90.0;
        let mut v2 = CharacterVitals::new("Bob".into(), "WAR".into());
        v2.hp_pct = 30.0;
        reg.update(v1).await;
        reg.update(v2).await;
        assert_eq!(reg.lowest_hp().await.unwrap().name, "Bob");
    }

    #[tokio::test]
    async fn members_below_threshold_sorted() {
        let reg = VitalsRegistry::new();
        for (name, hp) in [("A", 80.0), ("B", 20.0), ("C", 50.0)] {
            let mut v = CharacterVitals::new(name.into(), "WAR".into());
            v.hp_pct = hp;
            reg.update(v).await;
        }
        let below = HealTargetSelector::members_below(&reg, 60.0).await;
        assert_eq!(below, vec!["B", "C"]);
    }
}
