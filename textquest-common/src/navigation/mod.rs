//! Multi-zone navigation and pathfinding.
//!
//! This module contains the zone graph type re-exports, transition cost model,
//! and A* multi-zone pathfinder.  Types originating in [`crate::nav`] are
//! re-exported here for convenience; the pathfinding algorithm lives in
//! [`multi_zone_pathfinder`].

pub mod multi_zone_pathfinder;
pub mod zone_graph;

pub use zone_graph::{cost_for_transfer_type, TransitionCost, ZoneId};
