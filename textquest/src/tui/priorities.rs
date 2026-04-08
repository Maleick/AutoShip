//! Priority queue types for the heal/buff/debuff visibility panel.

/// Reason a character is blocked from executing its next action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockedReason {
    Oom,
    Cooldown,
    Range,
    Medding,
    Dead,
    Feigned,
    Interrupted,
    NoTarget,
}

impl BlockedReason {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Oom => "OOM",
            Self::Cooldown => "CD",
            Self::Range => "RANGE",
            Self::Medding => "MED",
            Self::Dead => "DEAD",
            Self::Feigned => "FD",
            Self::Interrupted => "INT",
            Self::NoTarget => "NOTGT",
        }
    }
}

/// A single entry in a priority queue (heal, buff, or debuff).
#[derive(Debug, Clone)]
pub struct PriorityEntry {
    pub target: String,
    pub spell: String,
    pub rank: u8,
    pub active: bool,
}

/// Per-character snapshot of automation intent and priority queues.
#[derive(Debug, Clone)]
pub struct PrioritySnapshot {
    pub name: String,
    pub role_label: String,
    pub current_intent: String,
    pub blocked: Option<BlockedReason>,
    pub heal_queue: Vec<PriorityEntry>,
    pub buff_queue: Vec<PriorityEntry>,
    pub debuff_queue: Vec<PriorityEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_reason_labels_are_short() {
        let reasons = [
            BlockedReason::Oom,
            BlockedReason::Cooldown,
            BlockedReason::Range,
            BlockedReason::Medding,
            BlockedReason::Dead,
            BlockedReason::Feigned,
            BlockedReason::Interrupted,
            BlockedReason::NoTarget,
        ];
        for reason in &reasons {
            let label = reason.label();
            assert!(!label.is_empty());
            assert!(label.len() <= 6, "label too long: {label}");
        }
    }

    #[test]
    fn priority_snapshot_defaults_empty_queues() {
        let snap = PrioritySnapshot {
            name: "Test".to_string(),
            role_label: "CLR".to_string(),
            current_intent: "Idle".to_string(),
            blocked: None,
            heal_queue: vec![],
            buff_queue: vec![],
            debuff_queue: vec![],
        };
        assert!(snap.heal_queue.is_empty());
        assert!(snap.blocked.is_none());
    }

    #[test]
    fn demo_priority_snapshots_cycle_deterministically() {
        let a = crate::tui::demo_data::demo_priority_snapshots(0);
        let b = crate::tui::demo_data::demo_priority_snapshots(0);
        assert_eq!(a.len(), b.len());
        for (sa, sb) in a.iter().zip(b.iter()) {
            assert_eq!(sa.name, sb.name);
            assert_eq!(sa.current_intent, sb.current_intent);
            assert_eq!(sa.blocked, sb.blocked);
        }
    }

    #[test]
    fn demo_snapshots_have_four_members() {
        let snaps = crate::tui::demo_data::demo_priority_snapshots(0);
        assert_eq!(snaps.len(), 4);
        assert_eq!(snaps[0].role_label, "CLR");
        assert_eq!(snaps[1].role_label, "ENC");
        assert_eq!(snaps[2].role_label, "WAR");
        assert_eq!(snaps[3].role_label, "WIZ");
    }

    #[test]
    fn demo_snapshots_phase_changes() {
        // Phase 0 (tick 0-7) vs Phase 2 (tick 16-23) should differ.
        let phase0 = crate::tui::demo_data::demo_priority_snapshots(0);
        let phase2 = crate::tui::demo_data::demo_priority_snapshots(16);
        // Cleric: Healing vs Medding.
        assert_ne!(phase0[0].current_intent, phase2[0].current_intent);
        // Wizard: Nuking vs OOM.
        assert_ne!(phase0[3].current_intent, phase2[3].current_intent);
    }
}
