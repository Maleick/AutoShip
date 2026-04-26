//! Multi-zone A* pathfinder across the zone graph.
//!
//! Implements [`find_path`] — an A* search over [`ZoneGraph`] that weights
//! edges by a composite [`TransitionCost`] (time + faction + mana).  Because
//! the zone graph has no spatial coordinates, the heuristic is admissible zero
//! (`h = 0`), reducing A* to Dijkstra's algorithm while keeping the interface
//! future-proof for location-aware heuristics.
//!
//! # Example
//!
//! ```
//! use textquest_common::navigation::multi_zone_pathfinder::find_path;
//! use textquest_common::nav::{ZoneGraph, ZoneNode, ZoneConnection};
//!
//! let mut graph = ZoneGraph::default();
//! graph.zones.insert(1, ZoneNode {
//!     zone_id: 1, name: "East Commons".into(),
//!     min_level: 1, max_level: 20,
//!     connections: vec![ZoneConnection { dest_zone_id: 2, transfer_type: 0, disabled: false }],
//! });
//! graph.zones.insert(2, ZoneNode {
//!     zone_id: 2, name: "West Commons".into(),
//!     min_level: 1, max_level: 20,
//!     connections: vec![],
//! });
//!
//! let path = find_path(1, 2, &graph);
//! assert_eq!(path, vec![1, 2]);
//! ```

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::nav::ZoneGraph;
use crate::navigation::zone_graph::{ZoneId, cost_for_transfer_type};

// ─── Internal priority-queue node ─────────────────────────────────────────────

/// A node on the A* open list, ordered by ascending `f = g + h`.
#[derive(Debug, Clone)]
struct AStarNode {
    /// `f` score: `g + h`.  Stored as ordered float for `BinaryHeap`.
    f: OrderedFloat,
    /// `g` score: actual cost from start to this node.
    g: f32,
    /// The zone this node represents.
    zone_id: ZoneId,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
    }
}
impl Eq for AStarNode {}

impl Ord for AStarNode {
    /// Reverses comparison so `BinaryHeap` (max-heap) becomes a min-heap.
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.cmp(&self.f)
    }
}
impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Newtype wrapper around `f32` that implements `Ord` by treating NaN as
/// greater than all finite values (so broken costs are sorted last).
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(if self.0.is_nan() {
            Ordering::Greater
        } else {
            Ordering::Less
        })
    }
}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Find the minimum-cost path from `from` to `to` across `graph` using A*.
///
/// Edge weights come from [`cost_for_transfer_type`] applied to each
/// [`ZoneConnection::transfer_type`].  Disabled connections are skipped.
///
/// # Returns
///
/// An ordered `Vec<ZoneId>` from `from` to `to` (both inclusive).  Returns an
/// empty `Vec` when:
/// - `from` or `to` is not present in `graph`, or
/// - no path exists between them.
///
/// Returns `vec![from]` when `from == to`.
///
/// # Complexity
///
/// O((V + E) log V) where V = number of zones, E = number of connections.
#[must_use]
pub fn find_path(from: ZoneId, to: ZoneId, graph: &ZoneGraph) -> Vec<ZoneId> {
    if from == to {
        return vec![from];
    }
    if !graph.zones.contains_key(&from) || !graph.zones.contains_key(&to) {
        return vec![];
    }

    // g_score[zone] = best known cost from `from` to `zone`.
    let mut g_score: HashMap<ZoneId, f32> = HashMap::new();
    g_score.insert(from, 0.0);

    // came_from[zone] = predecessor on the cheapest known path.
    let mut came_from: HashMap<ZoneId, ZoneId> = HashMap::new();

    let mut open: BinaryHeap<AStarNode> = BinaryHeap::new();
    open.push(AStarNode {
        f: OrderedFloat(heuristic(from, to)),
        g: 0.0,
        zone_id: from,
    });

    while let Some(current) = open.pop() {
        if current.zone_id == to {
            return reconstruct_path(&came_from, from, to);
        }

        // Skip stale entries (we may have pushed the same zone multiple times
        // with different g values before discovering the cheapest).
        let best_g = g_score.get(&current.zone_id).copied().unwrap_or(f32::MAX);
        if current.g > best_g {
            continue;
        }

        let Some(node) = graph.zones.get(&current.zone_id) else {
            continue;
        };

        for conn in &node.connections {
            if conn.disabled {
                continue;
            }
            if !graph.zones.contains_key(&conn.dest_zone_id) {
                // Dangling reference — skip.
                continue;
            }

            let edge_cost = cost_for_transfer_type(conn.transfer_type).total();
            let tentative_g = current.g + edge_cost;

            let neighbour_g = g_score.get(&conn.dest_zone_id).copied().unwrap_or(f32::MAX);
            if tentative_g < neighbour_g {
                g_score.insert(conn.dest_zone_id, tentative_g);
                came_from.insert(conn.dest_zone_id, current.zone_id);
                let f = tentative_g + heuristic(conn.dest_zone_id, to);
                open.push(AStarNode {
                    f: OrderedFloat(f),
                    g: tentative_g,
                    zone_id: conn.dest_zone_id,
                });
            }
        }
    }

    // No path found.
    vec![]
}

/// Admissible heuristic h(n, goal).
///
/// Returns 0 for all inputs — the graph has no spatial coordinates, so we
/// cannot compute a tighter lower bound without risking inadmissibility.
/// This makes A* equivalent to Dijkstra's algorithm on this graph.
#[inline]
fn heuristic(_node: ZoneId, _goal: ZoneId) -> f32 {
    0.0
}

/// Reconstruct the path from `from` to `to` using the `came_from` map.
fn reconstruct_path(came_from: &HashMap<ZoneId, ZoneId>, from: ZoneId, to: ZoneId) -> Vec<ZoneId> {
    let mut path = vec![to];
    let mut current = to;
    while current != from {
        match came_from.get(&current) {
            Some(&prev) => {
                path.push(prev);
                current = prev;
            }
            None => {
                // Should not happen if the algorithm is correct, but handle
                // gracefully rather than panicking.
                return vec![];
            }
        }
    }
    path.reverse();
    path
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{ZoneConnection, ZoneNode};

    // Build a small test graph:
    //
    //   1 --[type=0]--> 2 --[type=0]--> 4
    //   |                               ^
    //   +--[type=2]--> 3 --[type=2]----+
    //
    // type=0 (zone line):  cost=10.0 each → path 1→2→4 = 20.0
    // type=2 (port spell): cost=7.0  each → path 1→3→4 = 14.0  ← cheaper
    fn make_graph() -> ZoneGraph {
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "Start".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![
                    ZoneConnection {
                        dest_zone_id: 2,
                        transfer_type: 0,
                        disabled: false,
                    },
                    ZoneConnection {
                        dest_zone_id: 3,
                        transfer_type: 2,
                        disabled: false,
                    },
                ],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "Middle-A".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![ZoneConnection {
                    dest_zone_id: 4,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        g.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "Middle-B".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![ZoneConnection {
                    dest_zone_id: 4,
                    transfer_type: 2,
                    disabled: false,
                }],
            },
        );
        g.zones.insert(
            4,
            ZoneNode {
                zone_id: 4,
                name: "End".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![],
            },
        );
        g
    }

    #[test]
    fn same_zone_returns_singleton() {
        let g = make_graph();
        assert_eq!(find_path(1, 1, &g), vec![1]);
    }

    #[test]
    fn unknown_source_returns_empty() {
        let g = make_graph();
        assert!(find_path(99, 4, &g).is_empty());
    }

    #[test]
    fn unknown_dest_returns_empty() {
        let g = make_graph();
        assert!(find_path(1, 99, &g).is_empty());
    }

    #[test]
    fn no_path_returns_empty() {
        let g = make_graph();
        // Zone 4 has no outgoing connections.
        assert!(find_path(4, 1, &g).is_empty());
    }

    #[test]
    fn direct_path() {
        let g = make_graph();
        let path = find_path(1, 2, &g);
        assert_eq!(path, vec![1, 2]);
    }

    #[test]
    fn prefers_lower_cost_path() {
        // 1→3→4 uses type=2 ports (cost ≈7 each = 14 total)
        // 1→2→4 uses type=0 zone lines (cost =10 each = 20 total)
        // A* should prefer 1→3→4.
        let g = make_graph();
        let path = find_path(1, 4, &g);
        assert_eq!(
            path,
            vec![1, 3, 4],
            "A* should choose the lower-cost route via translocator"
        );
    }

    #[test]
    fn skips_disabled_connections() {
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "A".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![
                    ZoneConnection {
                        dest_zone_id: 2,
                        transfer_type: 0,
                        disabled: true, // disabled!
                    },
                    ZoneConnection {
                        dest_zone_id: 3,
                        transfer_type: 0,
                        disabled: false,
                    },
                ],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "B".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![],
            },
        );
        g.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "C".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![ZoneConnection {
                    dest_zone_id: 2,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        // Direct link 1→2 disabled; must go via 1→3→2.
        let path = find_path(1, 2, &g);
        assert_eq!(path, vec![1, 3, 2]);
    }

    #[test]
    fn empty_graph_returns_empty() {
        let g = ZoneGraph::default();
        assert!(find_path(1, 2, &g).is_empty());
    }

    #[test]
    fn path_starts_with_from_and_ends_with_to() {
        let g = make_graph();
        let path = find_path(1, 4, &g);
        assert!(!path.is_empty());
        assert_eq!(*path.first().unwrap(), 1);
        assert_eq!(*path.last().unwrap(), 4);
    }

    #[test]
    fn three_hop_zone_line_path() {
        // Linear chain: 10 → 11 → 12 → 13 (all zone lines)
        let mut g = ZoneGraph::default();
        for id in 10u16..=12 {
            g.zones.insert(
                id,
                ZoneNode {
                    zone_id: id,
                    name: format!("Zone{id}"),
                    min_level: 1,
                    max_level: 50,
                    connections: vec![ZoneConnection {
                        dest_zone_id: id + 1,
                        transfer_type: 0,
                        disabled: false,
                    }],
                },
            );
        }
        g.zones.insert(
            13,
            ZoneNode {
                zone_id: 13,
                name: "Zone13".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![],
            },
        );
        let path = find_path(10, 13, &g);
        assert_eq!(path, vec![10, 11, 12, 13]);
    }
}
