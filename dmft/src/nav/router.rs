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

/// Determines travel method based on group composition.
/// Port-first: if druids/wizards available, use ports for long-distance travel.
pub fn plan_group_travel(
    client_ids: &[ClientId],
    _class_map: &HashMap<ClientId, u8>,
    _from_zone: &str,
    _to_zone: &str,
) -> Vec<TravelPlan> {
    // TODO: use porters when port coordination is implemented
    // Porter classes: EqClass::Druid (6), EqClass::Wizard (12)
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
