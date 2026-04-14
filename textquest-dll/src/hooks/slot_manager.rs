//! Dynamic HWBP slot planning.
//!
//! Tracks which logical hooks should occupy DR0-DR3 for a given game state and
//! provides transitions for slot rotation.

use std::fmt;

use super::hwbp::MAX_SLOTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HookGameState {
    Login = 0,
    InGame = 1,
    ZoneLoading = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HookKind {
    GiveTime,
    ProcessGameEvents,
    RealRenderWorld,
    DspChat,
    SetGameState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotRotation {
    pub state: HookGameState,
    pub assignments: [Option<HookKind>; MAX_SLOTS],
    pub changed: bool,
}

impl fmt::Display for HookGameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HookGameState::Login => write!(f, "Login"),
            HookGameState::InGame => write!(f, "InGame"),
            HookGameState::ZoneLoading => write!(f, "ZoneLoading"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HookSlotManager {
    state: HookGameState,
    assignments: [Option<HookKind>; MAX_SLOTS],
}

impl HookSlotManager {
    pub const fn new() -> Self {
        Self {
            state: HookGameState::Login,
            assignments: [None; MAX_SLOTS],
        }
    }

    pub const fn state(&self) -> HookGameState {
        self.state
    }

    pub const fn assignments(&self) -> &[Option<HookKind>; MAX_SLOTS] {
        &self.assignments
    }

    pub fn rotate_hooks(&mut self, state: HookGameState) -> SlotRotation {
        let next = build_plan(state);
        let changed = next != self.assignments;
        self.state = state;
        self.assignments = next;

        SlotRotation {
            state,
            assignments: next,
            changed,
        }
    }
}

fn build_plan(state: HookGameState) -> [Option<HookKind>; MAX_SLOTS] {
    let priority = match state {
        HookGameState::Login => &[HookKind::GiveTime][..],
        HookGameState::InGame => &[
            HookKind::ProcessGameEvents,
            HookKind::RealRenderWorld,
            HookKind::DspChat,
        ],
        HookGameState::ZoneLoading => &[HookKind::ProcessGameEvents, HookKind::SetGameState],
    };

    let mut assignments = [None; MAX_SLOTS];
    for (slot, kind) in priority.iter().copied().enumerate() {
        if slot >= MAX_SLOTS {
            break;
        }
        assignments[slot] = Some(kind);
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigns_login_priority() {
        let mut mgr = HookSlotManager::new();
        let plan = mgr.rotate_hooks(HookGameState::Login);
        assert_eq!(mgr.state(), HookGameState::Login);
        assert_eq!(plan.assignments[0], Some(HookKind::GiveTime));
        assert_eq!(plan.assignments[1], None);
        assert!(plan.changed);
    }

    #[test]
    fn assigns_ingame_priority() {
        let mut mgr = HookSlotManager::new();
        let plan = mgr.rotate_hooks(HookGameState::InGame);
        assert_eq!(mgr.state(), HookGameState::InGame);
        assert_eq!(plan.assignments[0], Some(HookKind::ProcessGameEvents));
        assert_eq!(plan.assignments[1], Some(HookKind::RealRenderWorld));
        assert_eq!(plan.assignments[2], Some(HookKind::DspChat));
        assert_eq!(plan.assignments[3], None);
    }

    #[test]
    fn assigns_zoneloading_priority() {
        let mut mgr = HookSlotManager::new();
        let plan = mgr.rotate_hooks(HookGameState::ZoneLoading);
        assert_eq!(mgr.state(), HookGameState::ZoneLoading);
        assert_eq!(plan.assignments[0], Some(HookKind::ProcessGameEvents));
        assert_eq!(plan.assignments[1], Some(HookKind::SetGameState));
        assert_eq!(plan.assignments[2], None);
        assert_eq!(plan.assignments[3], None);
    }

    #[test]
    fn rotate_reports_no_change_for_same_state() {
        let mut mgr = HookSlotManager::new();
        mgr.rotate_hooks(HookGameState::InGame);
        let second = mgr.rotate_hooks(HookGameState::InGame);
        assert!(!second.changed);
        assert_eq!(second.assignments, mgr.assignments);
    }

    #[test]
    fn rotate_updates_state() {
        let mut mgr = HookSlotManager::new();
        assert_eq!(mgr.state(), HookGameState::Login);
        let _ = mgr.rotate_hooks(HookGameState::ZoneLoading);
        assert_eq!(mgr.state(), HookGameState::ZoneLoading);
        let second_plan = mgr.rotate_hooks(HookGameState::ZoneLoading);
        assert_eq!(*mgr.assignments(), second_plan.assignments);
    }

    #[test]
    fn new_manager_has_login_state_and_all_none_assignments() {
        let mgr = HookSlotManager::new();
        assert_eq!(mgr.state(), HookGameState::Login);
        for slot in mgr.assignments().iter() {
            assert_eq!(*slot, None);
        }
    }

    #[test]
    fn state_transition_between_different_states_reports_changed() {
        let mut mgr = HookSlotManager::new();
        mgr.rotate_hooks(HookGameState::Login);
        let plan = mgr.rotate_hooks(HookGameState::InGame);
        assert!(
            plan.changed,
            "transitioning from Login to InGame should report changed"
        );
    }

    #[test]
    fn game_state_display_formatting() {
        assert_eq!(format!("{}", HookGameState::Login), "Login");
        assert_eq!(format!("{}", HookGameState::InGame), "InGame");
        assert_eq!(format!("{}", HookGameState::ZoneLoading), "ZoneLoading");
    }

    #[test]
    fn slot_rotation_changed_flag_reflects_assignment_diff() {
        let mut mgr = HookSlotManager::new();
        // First rotate: initial assignments are all None, Login sets GiveTime in slot 0.
        let first = mgr.rotate_hooks(HookGameState::Login);
        assert!(first.changed);
        // Second rotate with same state: no diff.
        let second = mgr.rotate_hooks(HookGameState::Login);
        assert!(!second.changed);
        // Transition to ZoneLoading: different assignment set.
        let third = mgr.rotate_hooks(HookGameState::ZoneLoading);
        assert!(third.changed);
    }
}
