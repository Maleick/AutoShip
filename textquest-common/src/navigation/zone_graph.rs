//! Zone graph types for multi-zone pathfinding.
//!
//! Re-exports the canonical `ZoneGraph`, `ZoneNode`, and `ZoneConnection`
//! types from [`crate::nav`] and defines the `ZoneId` type alias and
//! `TransitionCost` edge-weight model used by the A* pathfinder.

pub use crate::nav::{ZoneConnection, ZoneGraph, ZoneNode};

/// Canonical zone identifier — EQ zone ID (unsigned 16-bit integer).
pub type ZoneId = u16;

/// Composite cost for a zone-to-zone transition.
///
/// A* uses the total weighted cost as the edge weight when choosing the
/// optimal path across the zone graph. Lower is better.
///
/// | Field    | Unit      | Meaning                                              |
/// |----------|-----------|------------------------------------------------------|
/// | `time`   | seconds   | Expected travel time for this transition type.       |
/// | `faction` | points   | Faction penalty (hostile faction = high cost).       |
/// | `mana`   | percent   | Estimated mana cost as a fraction of max mana (0–1). |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionCost {
    /// Expected travel / zoning time in seconds.
    pub time: f32,
    /// Faction cost — higher means this route risks faction loss.
    pub faction: f32,
    /// Mana cost as a fraction of max mana (0.0 = free, 1.0 = full bar).
    pub mana: f32,
}

impl TransitionCost {
    /// Compute the scalar edge weight used by A*.
    ///
    /// Weights are tuned so that slower or more costly transitions are
    /// deprioritised relative to quick zone-line hops.
    ///
    /// ```
    /// use textquest_common::navigation::zone_graph::TransitionCost;
    /// let cost = TransitionCost { time: 10.0, faction: 0.0, mana: 0.0 };
    /// assert!((cost.total() - 10.0).abs() < f32::EPSILON);
    /// ```
    #[must_use]
    pub fn total(&self) -> f32 {
        // Combine components with fixed weights:
        //   time   weight = 1.0  (primary driver)
        //   faction weight = 0.5  (secondary; faction loss is recoverable)
        //   mana   weight = 5.0  (tertiary; mana cost maps to ~5 s of regen)
        self.time + 0.5 * self.faction + 5.0 * self.mana
    }
}

/// Return the `TransitionCost` for a given `transfer_type` byte.
///
/// Values mirror EQ's `ZoneGuideManagerClient` transfer type enumeration:
///
/// | Code | Transfer method    |
/// |------|--------------------|
/// | 0    | Zone line (walk)   |
/// | 1    | Translocator spell |
/// | 2    | Wizard/Druid port  |
/// | 3+   | Unknown / other    |
#[must_use]
pub fn cost_for_transfer_type(transfer_type: u8) -> TransitionCost {
    match transfer_type {
        // Zone line: fast, no faction, no mana.
        0 => TransitionCost {
            time: 10.0,
            faction: 0.0,
            mana: 0.0,
        },
        // Translocator: moderate time, slight faction risk, low mana.
        1 => TransitionCost {
            time: 20.0,
            faction: 5.0,
            mana: 0.1,
        },
        // Wizard / Druid port: fast travel, no faction, significant mana.
        2 => TransitionCost {
            time: 5.0,
            faction: 0.0,
            mana: 0.4,
        },
        // Unknown / future types: penalise heavily so they're only chosen
        // when no cheaper alternative exists.
        _ => TransitionCost {
            time: 60.0,
            faction: 10.0,
            mana: 0.5,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_line_is_cheapest() {
        let zone_line = cost_for_transfer_type(0).total();
        let translocator = cost_for_transfer_type(1).total();
        let port = cost_for_transfer_type(2).total();
        assert!(
            zone_line < translocator,
            "zone line should be cheaper than translocator"
        );
        assert!(
            zone_line < port,
            "zone line should be cheaper than port (mana cost)"
        );
    }

    #[test]
    fn total_cost_combines_components() {
        let cost = TransitionCost {
            time: 10.0,
            faction: 2.0,
            mana: 0.2,
        };
        // 10.0 + 0.5*2.0 + 5.0*0.2 = 10.0 + 1.0 + 1.0 = 12.0
        assert!((cost.total() - 12.0).abs() < 1e-4);
    }

    #[test]
    fn unknown_transfer_type_is_expensive() {
        let unknown = cost_for_transfer_type(99).total();
        let zone_line = cost_for_transfer_type(0).total();
        assert!(unknown > zone_line * 4.0);
    }
}
