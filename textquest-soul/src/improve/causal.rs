//! Dependency-graph explain-away post-processor.
//!
//! Maintains a causal DAG (`petgraph::DiGraph`) where parent signals (e.g.
//! cleric debuff active, missing buff, gear-slot delta) suppress correlated
//! child anomaly kinds by demoting their severity from Major → Minor and
//! setting the `caused_by` link.
//!
//! Parent signals are boolean context flags passed in from the session. When
//! a parent fires, all child anomaly kinds it explains away are demoted.

use petgraph::graph::{DiGraph, NodeIndex};

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

// ── Graph node kinds ──────────────────────────────────────────────────────────

/// Nodes in the causal DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausalNode {
    /// Parent signal: a cleric debuff was active this session.
    ClericDebuff,
    /// Parent signal: a required buff was missing this session.
    MissingBuff,
    /// Parent signal: a gear-slot delta (item change/loss) was detected.
    GearDelta,
    /// Child: DPS drop detected by EWMA.
    DpsDrop,
    /// Child: death cluster detected by Page-Hinkley.
    DeathCluster,
    /// Child: mana P10 collapse detected by MAD.
    ManaCollapse,
    /// Child: stuck-event rate spike detected by BOCPD.
    StuckRate,
    /// Child: loot-rate drop detected by STL+MAD.
    LootRate,
}

// ── CausalGraph ───────────────────────────────────────────────────────────────

/// Petgraph-backed causal DAG and explain-away logic.
pub struct CausalGraph {
    graph: DiGraph<CausalNode, ()>,
    // Parent signal node indices (for fast lookup)
    cleric_debuff_idx: NodeIndex,
    missing_buff_idx: NodeIndex,
    gear_delta_idx: NodeIndex,
}

impl CausalGraph {
    pub fn new() -> Self {
        let mut g: DiGraph<CausalNode, ()> = DiGraph::new();

        // Add parent signal nodes
        let cleric = g.add_node(CausalNode::ClericDebuff);
        let buff = g.add_node(CausalNode::MissingBuff);
        let gear = g.add_node(CausalNode::GearDelta);

        // Add child anomaly nodes
        let dps = g.add_node(CausalNode::DpsDrop);
        let death = g.add_node(CausalNode::DeathCluster);
        let mana = g.add_node(CausalNode::ManaCollapse);
        let stuck = g.add_node(CausalNode::StuckRate);
        let loot = g.add_node(CausalNode::LootRate);

        // Causal edges: parent → child (parent explains child)
        // Cleric debuff active → higher deaths and lower DPS
        g.add_edge(cleric, death, ());
        g.add_edge(cleric, dps, ());
        // Missing buff → DPS drop and mana collapse
        g.add_edge(buff, dps, ());
        g.add_edge(buff, mana, ());
        // Gear delta (item loss/swap) → DPS drop and loot rate anomaly
        g.add_edge(gear, dps, ());
        g.add_edge(gear, loot, ());

        // Suppress stuck-rate anomalies when gear changes are active (route change confusion)
        g.add_edge(gear, stuck, ());

        Self {
            graph: g,
            cleric_debuff_idx: cleric,
            missing_buff_idx: buff,
            gear_delta_idx: gear,
        }
    }

    /// Apply explain-away: when parent signals are active, demote correlated
    /// child anomalies from Major → Minor and set `caused_by = parent_event_id`.
    ///
    /// Active parent signals are passed as boolean flags from the session metrics.
    pub fn explain_away(
        &self,
        mut events: Vec<AnomalyEvent>,
        cleric_debuff: bool,
        missing_buff: bool,
        gear_delta: bool,
    ) -> Vec<AnomalyEvent> {
        // Collect which child kinds are explained by active parents.
        let mut explained: Vec<AnomalyKind> = Vec::new();

        if cleric_debuff {
            explained.extend(self.children_of(self.cleric_debuff_idx));
        }
        if missing_buff {
            explained.extend(self.children_of(self.missing_buff_idx));
        }
        if gear_delta {
            explained.extend(self.children_of(self.gear_delta_idx));
        }

        if explained.is_empty() {
            return events;
        }

        // Assign a synthetic parent-signal anomaly ID for the caused_by link.
        // We use u64::MAX - offset as a stable sentinel for causal parents.
        let parent_id: u64 = u64::MAX;

        for ev in &mut events {
            if explained.iter().any(|k| same_kind(k, &ev.kind)) {
                if ev.severity >= Severity::Average {
                    ev.severity = Severity::Minor;
                    ev.caused_by = Some(parent_id);
                }
            }
        }

        events
    }

    /// Return the child `AnomalyKind`s reachable from `parent` in the DAG.
    fn children_of(&self, parent: NodeIndex) -> Vec<AnomalyKind> {
        self.graph
            .neighbors(parent)
            .filter_map(|n| node_to_kind(self.graph.node_weight(n)?))
            .collect()
    }
}

impl Default for CausalGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a `CausalNode` to its corresponding `AnomalyKind` skeleton (only for children).
fn node_to_kind(node: &CausalNode) -> Option<AnomalyKind> {
    match node {
        CausalNode::DpsDrop => Some(AnomalyKind::DpsDropEwma {
            class: String::new(),
            zone: String::new(),
        }),
        CausalNode::DeathCluster => Some(AnomalyKind::DeathClusterPageHinkley),
        CausalNode::ManaCollapse => Some(AnomalyKind::ManaCollapseMad),
        CausalNode::StuckRate => Some(AnomalyKind::StuckRateBocpd {
            route_id: 0,
            node_id: 0,
        }),
        CausalNode::LootRate => Some(AnomalyKind::LootRateStlMad {
            camp: String::new(),
        }),
        // Parent nodes do not have anomaly kinds.
        _ => None,
    }
}

/// Check whether two `AnomalyKind` values have the same discriminant.
fn same_kind(a: &AnomalyKind, b: &AnomalyKind) -> bool {
    matches!(
        (a, b),
        (AnomalyKind::DpsDropEwma { .. }, AnomalyKind::DpsDropEwma { .. })
            | (
                AnomalyKind::DeathClusterPageHinkley,
                AnomalyKind::DeathClusterPageHinkley
            )
            | (AnomalyKind::ManaCollapseMad, AnomalyKind::ManaCollapseMad)
            | (AnomalyKind::StuckRateBocpd { .. }, AnomalyKind::StuckRateBocpd { .. })
            | (AnomalyKind::LootRateStlMad { .. }, AnomalyKind::LootRateStlMad { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: AnomalyKind, severity: Severity) -> AnomalyEvent {
        AnomalyEvent {
            id: 1,
            ts: chrono::Utc::now(),
            kind,
            metric_name: "test".into(),
            baseline_mean: 100.0,
            baseline_std: 5.0,
            observed: 40.0,
            severity,
            caused_by: None,
            stale_baseline: false,
        }
    }

    #[test]
    fn no_suppression_when_no_parent_signals() {
        let g = CausalGraph::new();
        let events = vec![
            make_event(
                AnomalyKind::DpsDropEwma {
                    class: "mage".into(),
                    zone: "guk".into(),
                },
                Severity::Major,
            ),
            make_event(AnomalyKind::DeathClusterPageHinkley, Severity::Major),
        ];
        let result = g.explain_away(events, false, false, false);
        assert!(result.iter().all(|e| e.severity == Severity::Major));
        assert!(result.iter().all(|e| e.caused_by.is_none()));
    }

    #[test]
    fn cleric_debuff_suppresses_death_cluster_and_dps() {
        let g = CausalGraph::new();
        let events = vec![
            make_event(
                AnomalyKind::DpsDropEwma {
                    class: "mage".into(),
                    zone: "guk".into(),
                },
                Severity::Major,
            ),
            make_event(AnomalyKind::DeathClusterPageHinkley, Severity::Major),
            make_event(AnomalyKind::ManaCollapseMad, Severity::Major),
        ];
        let result = g.explain_away(events, true, false, false);

        let dps = result
            .iter()
            .find(|e| matches!(e.kind, AnomalyKind::DpsDropEwma { .. }))
            .unwrap();
        let death = result
            .iter()
            .find(|e| e.kind == AnomalyKind::DeathClusterPageHinkley)
            .unwrap();
        let mana = result
            .iter()
            .find(|e| e.kind == AnomalyKind::ManaCollapseMad)
            .unwrap();

        assert_eq!(dps.severity, Severity::Minor, "DPS suppressed by cleric debuff");
        assert!(dps.caused_by.is_some());
        assert_eq!(death.severity, Severity::Minor, "death suppressed by cleric debuff");
        // Mana is NOT a child of cleric debuff — should remain Major
        assert_eq!(mana.severity, Severity::Major, "mana not suppressed");
    }

    #[test]
    fn missing_buff_suppresses_dps_and_mana() {
        let g = CausalGraph::new();
        let events = vec![
            make_event(
                AnomalyKind::DpsDropEwma {
                    class: "war".into(),
                    zone: "velk".into(),
                },
                Severity::Major,
            ),
            make_event(AnomalyKind::ManaCollapseMad, Severity::Major),
        ];
        let result = g.explain_away(events, false, true, false);
        assert!(result.iter().all(|e| e.severity == Severity::Minor));
        assert!(result.iter().all(|e| e.caused_by.is_some()));
    }

    #[test]
    fn minor_events_not_demoted_further() {
        let g = CausalGraph::new();
        let events = vec![make_event(AnomalyKind::DeathClusterPageHinkley, Severity::Minor)];
        let result = g.explain_away(events, true, false, false);
        // Minor events stay Minor but do get the caused_by link removed
        // (already Minor — no change needed, but caused_by should not be set if already minor)
        assert_eq!(result[0].severity, Severity::Minor);
    }

    #[test]
    fn integration_all_parents_fire() {
        let g = CausalGraph::new();
        let events = vec![
            make_event(
                AnomalyKind::DpsDropEwma {
                    class: "enc".into(),
                    zone: "ntov".into(),
                },
                Severity::Major,
            ),
            make_event(AnomalyKind::DeathClusterPageHinkley, Severity::Major),
            make_event(AnomalyKind::ManaCollapseMad, Severity::Major),
            make_event(
                AnomalyKind::StuckRateBocpd {
                    route_id: 1,
                    node_id: 2,
                },
                Severity::Average,
            ),
            make_event(
                AnomalyKind::LootRateStlMad {
                    camp: "camp1".into(),
                },
                Severity::Average,
            ),
        ];
        let result = g.explain_away(events, true, true, true);
        // All events should be demoted with cleric+buff+gear active
        assert!(
            result.iter().all(|e| e.severity == Severity::Minor),
            "all events should be suppressed"
        );
    }
}
