//! Minimal combatant state machine used by combat scenario validation.

use super::class_strategy::CombatAction;

/// State for a combatant participating in a simulated rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatantState {
    /// No active target.
    Idle,
    /// A pull has started against the named target.
    Pulling { target_spawn: String },
    /// The combatant is executing rotation actions.
    Rotating { target_spawn: String },
    /// A spell cast is in progress.
    Casting { spell_name: String },
    /// A melee swing is in progress.
    Swinging,
    /// The last action was interrupted.
    Interrupted,
    /// The pull completed.
    Complete,
}

/// Small deterministic FSM for scenario-side combat validation.
#[derive(Debug, Clone)]
pub struct Combatant {
    /// Spawn ID of the simulated combatant.
    pub source_id: u32,
    state: CombatantState,
    pulls_started: u32,
}

impl Combatant {
    /// Construct an idle combatant.
    #[must_use]
    pub fn new(source_id: u32) -> Self {
        Self {
            source_id,
            state: CombatantState::Idle,
            pulls_started: 0,
        }
    }

    /// Current FSM state.
    #[must_use]
    pub fn state(&self) -> &CombatantState {
        &self.state
    }

    /// Number of pulls started by this combatant.
    #[must_use]
    pub fn pulls_started(&self) -> u32 {
        self.pulls_started
    }

    /// Begin a pull.
    pub fn start_pull(&mut self, target_spawn: impl Into<String>) {
        self.pulls_started += 1;
        self.state = CombatantState::Pulling {
            target_spawn: target_spawn.into(),
        };
    }

    /// Mark the rotation body as active.
    pub fn start_rotation(&mut self, target_spawn: impl Into<String>) {
        self.state = CombatantState::Rotating {
            target_spawn: target_spawn.into(),
        };
    }

    /// Start one rotation action.
    pub fn start_action(&mut self, action: &CombatAction) {
        self.state = match action {
            CombatAction::Spell { name, .. } => CombatantState::Casting {
                spell_name: name.clone(),
            },
            CombatAction::Melee { .. } => CombatantState::Swinging,
        };
    }

    /// Mark the active action complete and return to rotating.
    pub fn complete_action(&mut self, target_spawn: impl Into<String>) {
        self.state = CombatantState::Rotating {
            target_spawn: target_spawn.into(),
        };
    }

    /// Mark the active action interrupted.
    pub fn interrupt_action(&mut self) {
        self.state = CombatantState::Interrupted;
    }

    /// Finish the current pull.
    pub fn complete_pull(&mut self) {
        self.state = CombatantState::Complete;
    }
}
