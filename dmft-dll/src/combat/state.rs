//! Combatant FSM — the per-character combat state machine.
//!
//! Each injected DLL runs one `Combatant` that drives a single EQ character
//! through the Idle → Engaging → Casting → OnGcd → Engaging loop, with
//! HolyShit emergency overrides evaluated every tick before the normal rotation.

use dmft_common::combat::{CombatConfig, CombatRole, CombatStatus, HolyShitAction};
use dmft_common::types::SpawnData;

use super::gcd::GcdTracker;
use super::holyshit::HolyShitEvaluator;
use super::humanize::CombatPersonality;
use super::mana::ManaGovernor;
use super::strategy::{build_strategy, ClassStrategy, CombatContext, GroupMemberState};

/// Internal FSM states — not exposed outside this module.
/// The public-facing status uses `CombatStatus` from dmft-common.
enum CombatState {
    Idle,
    Engaging { target_id: u32 },
    Casting { spell_slot: u8, ticks_remaining: u32 },
    OnGcd,
    Recovering,
}

/// The main combat state machine for a single EQ character.
pub struct Combatant {
    state: CombatState,
    strategy: Box<dyn ClassStrategy>,
    personality: CombatPersonality,
    gcd: GcdTracker,
    mana_governor: ManaGovernor,
    holyshit: HolyShitEvaluator,
    assist_target: Option<u32>,
    /// Set when a HolyShit Flee action fires. The orchestrator checks this
    /// via `status()` (which returns `CombatStatus::Fleeing`) to know it
    /// should send a flee waypoint to the navigator.
    flee_requested: bool,
    /// Group member snapshots, populated by the orchestrator via IPC.
    /// Required for healer strategies (cleric, druid, shaman) to select
    /// heal targets. Empty until the orchestrator sends group state updates.
    group_members: Vec<GroupMemberState>,
    tick_count: u32,
    config: CombatConfig,
}

impl Combatant {
    pub fn new(class_id: u8, client_id: u32, config: CombatConfig) -> Self {
        let is_healer = matches!(config.role, CombatRole::Healer);
        let strategy = build_strategy(class_id, &config);
        let personality = CombatPersonality::from_client_id(client_id);
        let gcd = GcdTracker::default_gcd();
        let mana_governor = ManaGovernor::new(config.mana_floor, is_healer);
        let holyshit = HolyShitEvaluator::new(config.holyshit_rules.clone());

        tracing::info!(
            class_id,
            client_id,
            holyshit_rules = holyshit.rule_count(),
            "Combatant created"
        );

        Self {
            state: CombatState::Idle,
            strategy,
            personality,
            gcd,
            mana_governor,
            holyshit,
            assist_target: None,
            flee_requested: false,
            group_members: Vec::new(),
            tick_count: 0,
            config,
        }
    }

    /// Advance the combat FSM by one game tick.
    pub fn tick(
        &mut self,
        player: &SpawnData,
        target: Option<&SpawnData>,
        nearby: &[SpawnData],
    ) {
        self.tick_count += 1;
        self.gcd.tick();

        // Build context snapshot for this tick.
        let ctx = CombatContext {
            player,
            target,
            nearby_enemies: nearby,
            group_members: &self.group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat: !matches!(self.state, CombatState::Idle | CombatState::Recovering),
        };

        // --- HolyShit evaluation (always runs first) ---
        if let Some(action) = self.holyshit.evaluate(&ctx) {
            match action {
                HolyShitAction::CastSpell(slot) => {
                    tracing::warn!(slot, "HolyShit: casting emergency spell");
                    crate::eq::cast_spell(*slot, 0); // spell_id 0 = use whatever is in the gem
                    self.gcd.consume();
                    self.state = CombatState::Casting {
                        spell_slot: *slot,
                        ticks_remaining: 20,
                    };
                    return;
                }
                HolyShitAction::UseAbility(ability_id) => {
                    tracing::warn!(ability_id, "HolyShit: using emergency ability");
                    crate::eq::do_combat_ability(*ability_id as i32, true);
                    self.gcd.consume();
                    self.state = CombatState::OnGcd;
                    return;
                }
                HolyShitAction::UseItem(item_id) => {
                    tracing::warn!(item_id, "HolyShit: using emergency item");
                    // Item usage not yet wired — log for now
                    self.gcd.consume();
                    self.state = CombatState::OnGcd;
                    return;
                }
                HolyShitAction::Flee => {
                    tracing::warn!("HolyShit: FLEE — disengaging and requesting flee movement");
                    self.assist_target = None;
                    self.flee_requested = true;
                    self.state = CombatState::Idle;
                    return;
                }
            }
        }

        // Decrement cast ticks before the state check so the transition fires
        // on the correct tick (when ticks_remaining reaches 0).
        if let CombatState::Casting { ticks_remaining, .. } = &mut self.state {
            *ticks_remaining = ticks_remaining.saturating_sub(1);
        }

        // --- Normal state machine ---
        match &self.state {
            CombatState::Idle => {
                // Wait for an explicit engage command — do nothing.
            }

            CombatState::Engaging { .. } => {
                if !self.gcd.is_ready() {
                    return;
                }

                // Check mana governor
                let mana_pct = player.mana_pct();

                if !self.mana_governor.can_cast(mana_pct) {
                    tracing::debug!(mana_pct, "Mana too low, transitioning to Recovering");
                    self.state = CombatState::Recovering;
                    return;
                }

                // Ask strategy for next spell
                if let Some(spell) = self.strategy.select_spell(&ctx) {
                    tracing::debug!(
                        slot = spell.slot,
                        name = %spell.name,
                        "Strategy selected spell"
                    );

                    // Call the real EQ CastSpell function via FFI
                    crate::eq::cast_spell(spell.slot, spell.spell_id);

                    // Apply humanization delay (cast_start_delay absorbed into cast time)
                    let cast_delay = self.personality.next_cast_delay() as u32;
                    self.gcd.consume();
                    self.state = CombatState::Casting {
                        spell_slot: spell.slot,
                        ticks_remaining: 20 + cast_delay, // base ~1s + jitter
                    };
                }
                // If strategy returns None, stay in Engaging and try next tick.
            }

            CombatState::Casting { ticks_remaining, .. } => {
                if *ticks_remaining == 0 {
                    tracing::trace!("Cast complete, transitioning to OnGcd");
                    self.state = CombatState::OnGcd;
                }
            }

            CombatState::OnGcd => {
                if self.gcd.is_ready() {
                    // Re-check if we still have an assist target or engagement
                    if self.assist_target.is_some() || target.is_some() {
                        let tid = self
                            .assist_target
                            .or_else(|| target.map(|t| t.spawn_id))
                            .unwrap_or(0);
                        self.state = CombatState::Engaging { target_id: tid };
                    } else {
                        self.state = CombatState::Idle;
                    }
                }
            }

            CombatState::Recovering => {
                let mana_pct = player.mana_pct();

                // Recover until we're above the floor
                if !self.mana_governor.should_med(mana_pct, false) {
                    tracing::debug!(mana_pct, "Mana recovered, transitioning to Idle");
                    self.state = CombatState::Idle;
                }
            }
        }

    }

    /// Return the public-facing combat status for IPC reporting.
    pub fn status(&self) -> CombatStatus {
        if self.flee_requested {
            return CombatStatus::Fleeing;
        }

        match &self.state {
            CombatState::Idle => CombatStatus::Idle,
            CombatState::Engaging { target_id } => CombatStatus::Engaging {
                target_id: *target_id,
            },
            CombatState::Casting {
                spell_slot,
                ticks_remaining: _,
            } => {
                let tid = self.assist_target.unwrap_or(0);
                CombatStatus::Casting {
                    spell_slot: *spell_slot,
                    target_id: tid,
                }
            }
            CombatState::OnGcd => CombatStatus::OnGcd,
            CombatState::Recovering => CombatStatus::Recovering,
        }
    }

    /// Set the main-assist target that this combatant should attack.
    pub fn set_assist_target(&mut self, spawn_id: u32) {
        tracing::info!(spawn_id, "Assist target set");
        self.assist_target = Some(spawn_id);
    }

    /// Begin combat against a specific target.
    /// Note: `on_engage` will be called on the strategy during the next `tick()`
    /// when a real player snapshot is available.
    pub fn engage(&mut self, target_id: u32) {
        tracing::info!(target_id, "Engaging target");
        self.state = CombatState::Engaging { target_id };
    }

    /// Stop combat — return to idle.
    pub fn disengage(&mut self) {
        tracing::info!("Disengaging from combat");
        self.assist_target = None;
        self.state = CombatState::Idle;
    }

    /// Update group member snapshots (called when the orchestrator sends group state).
    /// Required for healer strategies to function — without this, healers have no
    /// targets to evaluate.
    pub fn set_group_members(&mut self, members: Vec<GroupMemberState>) {
        self.group_members = members;
    }

    /// Whether a HolyShit Flee was triggered and not yet acknowledged.
    pub fn flee_requested(&self) -> bool {
        self.flee_requested
    }

    /// Clear the flee flag after the orchestrator has dispatched a flee waypoint.
    pub fn clear_flee_requested(&mut self) {
        self.flee_requested = false;
    }
}
