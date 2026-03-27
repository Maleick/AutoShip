//! High-level zone routing — plans multi-zone travel and coordinates group transitions.

use dmft_common::nav::{IndexedQueue, Waypoint};
use dmft_common::types::ClientId;
use std::collections::HashMap;

/// A step in a multi-zone travel plan.
#[derive(Debug, Clone)]
pub enum TravelStep {
    /// Walk to a position within the current zone.
    WalkTo { waypoints: Vec<Waypoint> },
    /// Zone transition: walk to zone line and enter.
    ZoneTo {
        zone_name: String,
        zone_line_pos: Waypoint,
    },
    /// Port: caster ports the group (requires port-class character).
    PortTo {
        zone_name: String,
        caster_id: ClientId,
    },
    /// Wait for staggered entry (random delay before zoning).
    StaggerWait {
        min_secs: u32,
        max_secs: u32,
    },
}

/// A complete travel plan for one character.
pub struct TravelPlan {
    pub client_id: ClientId,
    steps: IndexedQueue<TravelStep>,
}

impl TravelPlan {
    pub fn new(client_id: ClientId, steps: Vec<TravelStep>) -> Self {
        let mut queue = IndexedQueue::new();
        queue.set_items(steps);
        Self {
            client_id,
            steps: queue,
        }
    }

    pub fn current(&self) -> Option<&TravelStep> {
        self.steps.current()
    }

    pub fn advance(&mut self) -> bool {
        self.steps.advance()
    }

    pub fn is_complete(&self) -> bool {
        self.steps.index() >= self.steps.len()
    }
}

/// Generates stagger delays for a group of characters zoning together.
/// Returns map of client_id -> delay in seconds.
pub fn generate_zone_staggers(
    client_ids: &[ClientId],
    min_secs: u32,
    max_secs: u32,
    seed: u32,
) -> HashMap<ClientId, u32> {
    let max_secs = max_secs.max(min_secs);
    let range = max_secs - min_secs;
    let mut result = HashMap::new();

    for &id in client_ids {
        // Deterministic but varied delay per character.
        let hash = id.wrapping_mul(dmft_common::nav::KNUTH_HASH).wrapping_add(seed);
        let delay = min_secs + (hash % (range + 1));
        result.insert(id, delay);
    }

    result
}

/// Coordinates group travel planning, including porter awareness.
///
/// Porter classes (Druid = 6, Wizard = 12) can teleport the group for
/// long-distance travel. When porters are registered and the route spans
/// multiple zones, the planner should prefer ports over walking.
pub struct GroupRouter {
    /// Client IDs of characters that can cast port/teleport spells.
    porters: Vec<ClientId>,
}

impl GroupRouter {
    pub fn new() -> Self {
        Self {
            porters: Vec::new(),
        }
    }

    /// Register characters that can port the group.
    /// Typically Druids (class 6) and Wizards (class 12).
    pub fn set_porters(&mut self, porter_ids: Vec<ClientId>) {
        tracing::info!(count = porter_ids.len(), "Registered porters for routing");
        self.porters = porter_ids;
    }

    /// Whether any porters are available for long-distance travel.
    pub fn has_porters(&self) -> bool {
        !self.porters.is_empty()
    }

    /// Plan travel for a group of characters.
    ///
    /// When porters are available and the route is long-distance (multiple zone
    /// transitions), the planner would prefer PortTo steps over walking. For now,
    /// port-based routing is a future enhancement — all travel uses staggered
    /// zone transitions.
    pub fn plan_travel(
        &self,
        client_ids: &[ClientId],
        _class_map: &HashMap<ClientId, u8>,
        _from_zone: &str,
        _to_zone: &str,
    ) -> Vec<TravelPlan> {
        // Future: when from_zone and to_zone are far apart (3+ zone transitions)
        // and self.has_porters(), generate PortTo steps using the nearest porter
        // instead of walking the full route.
        let staggers = generate_zone_staggers(client_ids, 5, 60, 42);

        client_ids
            .iter()
            .map(|&id| {
                let delay = staggers.get(&id).copied().unwrap_or(5);
                TravelPlan::new(
                    id,
                    vec![TravelStep::StaggerWait {
                        min_secs: delay,
                        max_secs: delay,
                    }],
                )
            })
            .collect()
    }
}

/// Convenience wrapper that creates a one-shot travel plan without porter awareness.
pub fn plan_group_travel(
    client_ids: &[ClientId],
    class_map: &HashMap<ClientId, u8>,
    from_zone: &str,
    to_zone: &str,
) -> Vec<TravelPlan> {
    let router = GroupRouter::new();
    router.plan_travel(client_ids, class_map, from_zone, to_zone)
}
