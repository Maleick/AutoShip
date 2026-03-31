//! Combatant FSM — the per-character combat state machine.
//!
//! Each injected DLL runs one `Combatant` that drives a single EQ character
//! through the Idle → Engaging → Casting → OnGcd → Engaging loop, with
//! HolyShit emergency overrides evaluated every tick before the normal rotation.

use std::collections::HashMap;

use dmft_common::combat::{CombatConfig, CombatRole, CombatStatus, HolyShitAction};
use dmft_common::types::SpawnData;

use super::dot_tracker::DotTracker;
use super::gcd::GcdTracker;
use super::holyshit::HolyShitEvaluator;
use super::humanize::CombatPersonality;
use super::mana::ManaGovernor;
use super::skill_cooldowns::{SkillCooldownTracker, default_cooldown};
use super::strategy::{ClassStrategy, CombatContext, GroupMemberState, build_strategy};

/// Maximum spell range in EQ units. Spells beyond this distance will not fire.
const MAX_SPELL_RANGE: f32 = 200.0;

/// Pet classes that should issue `/pet attack` on engage.
const PET_CLASSES: &[u8] = &[5, 10, 11, 13, 15]; // SK, Shaman, Necro, Mage, Beastlord

/// Internal FSM states — not exposed outside this module.
/// The public-facing status uses `CombatStatus` from dmft-common.
enum CombatState {
    Idle,
    Engaging {
        target_id: u32,
    },
    Casting {
        spell_slot: u8,
        ticks_remaining: u32,
    },
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
    /// True when we just entered Engaging state — triggers on_engage callback.
    needs_on_engage: bool,
    /// Group member snapshots, populated by the orchestrator via IPC.
    /// Required for healer strategies (cleric, druid, shaman) to select
    /// heal targets. Empty until the orchestrator sends group state updates.
    group_members: Vec<GroupMemberState>,
    skill_cooldowns: SkillCooldownTracker,
    /// Discipline cooldowns keyed by spell_id → ticks remaining.
    disc_cooldowns: HashMap<i32, u32>,
    dot_tracker: DotTracker,
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
            needs_on_engage: false,
            group_members: Vec::new(),
            skill_cooldowns: SkillCooldownTracker::new(),
            disc_cooldowns: HashMap::new(),
            dot_tracker: DotTracker::new(),
            tick_count: 0,
            config,
        }
    }

    /// Advance the combat FSM by one frame (~50ms, ~20/sec).
    /// Note: an EQ "game tick" is 6 seconds (~120 frames); this runs every frame.
    pub fn tick(&mut self, player: &SpawnData, target: Option<&SpawnData>, nearby: &[SpawnData]) {
        self.tick_count += 1;
        self.gcd.tick();
        self.skill_cooldowns.tick();

        // Tick discipline cooldowns
        self.disc_cooldowns.retain(|_, ticks| {
            *ticks = ticks.saturating_sub(1);
            *ticks > 0
        });

        // --- Zone/disconnect safety guard ---
        // If we're in an active combat state but our target has vanished (zoned,
        // despawned, server disconnect back to char select), auto-disengage to
        // prevent the FSM from getting stuck in Engaging/Casting forever.
        if matches!(
            self.state,
            CombatState::Engaging { .. } | CombatState::Casting { .. } | CombatState::OnGcd
        ) && target.is_none()
        {
            tracing::warn!("Combat target lost (zone/despawn/disconnect) — auto-disengaging");
            let cleanup_ctx = CombatContext {
                player,
                target: None,
                nearby_enemies: nearby,
                group_members: &self.group_members,
                config: &self.config,
                tick: self.tick_count,
                in_combat: false,
                ch_chain_slot: None,
            };
            self.strategy.on_action_complete(&cleanup_ctx);
            crate::eq::toggle_auto_attack(false);
            // Clear DoT tracking — target is gone (zone/despawn/disconnect).
            if let CombatState::Engaging { target_id } = &self.state {
                self.dot_tracker.clear_target(*target_id);
            }
            self.dot_tracker.prune_expired(self.tick_count);
            self.assist_target = None;
            self.flee_requested = false;
            self.state = CombatState::Idle;
            return;
        }

        // Fire melee skills when engaging (independent of GCD/spell casting)
        if matches!(self.state, CombatState::Engaging { .. }) {
            let class_id = self.strategy.class_id();
            self.tick_melee_skills(class_id, player);
            self.tick_disciplines(player);
        }

        // Build context snapshot for this tick.
        let ctx = CombatContext {
            player,
            target,
            nearby_enemies: nearby,
            group_members: &self.group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat: !matches!(self.state, CombatState::Idle | CombatState::Recovering),
            ch_chain_slot: None,
        };

        // --- Call on_engage when first entering Engaging state ---
        if self.needs_on_engage {
            self.needs_on_engage = false;
            self.strategy.on_engage(&ctx);
        }

        // --- HolyShit evaluation (always runs first) ---
        if let Some(action) = self.holyshit.evaluate(&ctx) {
            // If HolyShit fires while we're mid-cast, notify the strategy that
            // the current cast was interrupted so class-specific state gets cleaned
            // up (e.g., bard melody index, cleric rez_pending).
            if matches!(self.state, CombatState::Casting { .. }) {
                self.strategy.on_action_complete(&ctx);
            }

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
                    crate::eq::slash_command(&format!("/useitem {item_id}"));
                    self.gcd.consume();
                    self.state = CombatState::OnGcd;
                    return;
                }
                HolyShitAction::Flee => {
                    tracing::warn!("HolyShit: FLEE — disengaging and requesting flee movement");
                    // Notify strategy of combat end so class-specific cleanup runs
                    // (e.g., bard stops /melody).
                    let flee_ctx = CombatContext {
                        player,
                        target,
                        nearby_enemies: nearby,
                        group_members: &self.group_members,
                        config: &self.config,
                        tick: self.tick_count,
                        in_combat: false,
                        ch_chain_slot: None,
                    };
                    self.strategy.on_action_complete(&flee_ctx);
                    self.assist_target = None;
                    self.flee_requested = true;
                    self.state = CombatState::Idle;
                    return;
                }
            }
        }

        // Decrement cast ticks before the state check so the transition fires
        // on the correct tick (when ticks_remaining reaches 0).
        if let CombatState::Casting {
            ticks_remaining, ..
        } = &mut self.state
        {
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

                // Ask strategy for a spell target (may differ from assist target).
                // Healers target lowest-HP group member, enchanters target off-mobs
                // for mez, etc. This only influences spell targeting — it does NOT
                // override the assist target for auto-attack.
                if let Some(spell_target) = self.strategy.select_target(&ctx)
                    && target.is_none_or(|t| t.spawn_id != spell_target)
                {
                    tracing::debug!(
                        spell_target,
                        assist = ?self.assist_target,
                        "Strategy selected different spell target"
                    );
                    crate::eq::slash_command(&format!("/target id {spell_target}"));
                }

                // Range check — don't cast if target is too far away
                if let Some(t) = target {
                    let dist = distance_3d(player, t);
                    if dist > MAX_SPELL_RANGE {
                        tracing::debug!(dist, "Target out of spell range, waiting");
                        return;
                    }
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

            CombatState::Casting {
                ticks_remaining, ..
            } => {
                // Healer heal-cancel: if lowest HP member recovered above 85%,
                // duck to interrupt the heal and save mana.
                if matches!(self.config.role, CombatRole::Healer) && *ticks_remaining > 5 {
                    let all_healthy = ctx
                        .group_members
                        .iter()
                        .filter(|m| m.hp_pct > 0.0)
                        .all(|m| m.hp_pct >= 85.0);
                    if all_healthy && !ctx.group_members.is_empty() {
                        tracing::info!("Healer: canceling heal — group HP recovered above 85%");
                        // Duck to interrupt cast (write STANDSTATE=4 briefly)
                        crate::eq::slash_command("/duck");
                        self.state = CombatState::OnGcd;
                        self.gcd.consume();
                        return;
                    }
                }

                if *ticks_remaining == 0 {
                    tracing::trace!("Cast complete, transitioning to OnGcd");
                    // Notify strategy that a cast/action completed (e.g., bard twist advance)
                    let ctx = CombatContext {
                        player,
                        target,
                        nearby_enemies: nearby,
                        group_members: &self.group_members,
                        config: &self.config,
                        tick: self.tick_count,
                        in_combat: true,
                        ch_chain_slot: None,
                    };
                    self.strategy.on_action_complete(&ctx);
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
    /// Issues `/face` to turn toward the target (melee misses without facing),
    /// `/pet attack` for pet classes, and immediate taunt for tanks without aggro.
    pub fn engage(&mut self, target_id: u32) {
        tracing::info!(target_id, "Engaging target");

        // Face the target so melee attacks connect
        crate::eq::slash_command("/face");

        // Pet classes: send pet to attack with /pet focus for single-target
        let class_id = self.strategy.class_id();
        if PET_CLASSES.contains(&class_id) {
            crate::eq::slash_command("/pet attack");
            crate::eq::slash_command("/pet focus");
            tracing::info!(class_id, "Sent /pet attack + /pet focus");
        }

        // Tanks: immediate taunt to establish aggro on engage
        let role = self.strategy.role();
        if matches!(role, CombatRole::MainTank | CombatRole::OffTank) {
            crate::eq::use_skill(73, None); // skill 73 = taunt
            self.skill_cooldowns.consume(73, super::skill_cooldowns::skill_timers::TAUNT.1);
            tracing::info!("Tank: immediate taunt on engage");
        }

        crate::eq::toggle_auto_attack(true);
        self.needs_on_engage = true;
        self.state = CombatState::Engaging { target_id };
    }

    /// Stop combat — return to idle.
    pub fn disengage(&mut self) {
        tracing::info!("Disengaging from combat");

        // Clear DoT tracking for the current target (target died or we're done).
        if let CombatState::Engaging { target_id } = &self.state {
            self.dot_tracker.clear_target(*target_id);
        }

        // Notify strategy of kill/disengage for state cleanup
        let player = SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &self.group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat: false,
            ch_chain_slot: None,
        };
        self.strategy.on_action_complete(&ctx);

        crate::eq::toggle_auto_attack(false);

        // Pet classes: call pet back on disengage so it doesn't pull adds
        let class_id = self.strategy.class_id();
        if PET_CLASSES.contains(&class_id) {
            crate::eq::slash_command("/pet back");
            tracing::info!(class_id, "Sent /pet back on disengage");
        }

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

    /// Fire class-appropriate melee skills (kick, bash, taunt, backstab, etc.)
    /// Called every tick while Engaging. Each skill fires independently as soon
    /// as its individual cooldown expires.
    fn tick_melee_skills(&mut self, class_id: u8, player: &SpawnData) {
        // Endurance check — melee skills cost endurance, don't fire if too low
        let end_pct = if player.endurance_max > 0 {
            (player.endurance_current as f32 / player.endurance_max as f32) * 100.0
        } else {
            100.0
        };
        if end_pct < 10.0 {
            return; // conserve endurance
        }

        // Build the skill list for this class
        let skills: &[u32] = match class_id {
            1 => &[73, 30],         // Warrior: taunt, kick
            3 => &[73, 10, 30],     // Paladin: taunt, bash, kick
            5 => &[73, 10, 30],     // Shadow Knight: taunt, bash, kick
            7 => &[26, 38, 52, 23], // Monk: flying kick, round kick, tiger claw, eagle strike
            9 => &[8],              // Rogue: backstab
            15 => &[30, 26],        // Beastlord: kick, flying kick
            16 => &[30],            // Berserker: kick (frenzy via abilities)
            _ => &[30],             // Generic: kick
        };

        // Fire each skill independently when its cooldown is ready
        for &skill_id in skills {
            if self.skill_cooldowns.is_ready(skill_id) {
                crate::eq::use_skill(skill_id, None);
                if let Some(cd) = default_cooldown(skill_id) {
                    self.skill_cooldowns.consume(skill_id, cd);
                }
            }
        }
    }

    /// Fire disciplines (combat abilities) when conditions are met.
    /// Called every tick while Engaging. Only one discipline fires per tick
    /// since they share the GCD. Disciplines are evaluated in priority order
    /// (lower priority number = higher priority).
    fn tick_disciplines(&mut self, player: &SpawnData) {
        if self.config.disciplines.is_empty() {
            return;
        }

        let hp_pct = player.hp_pct();
        let end_pct = if player.endurance_max > 0 {
            (player.endurance_current as f32 / player.endurance_max as f32) * 100.0
        } else {
            100.0
        };

        // Sort by priority (lower = higher priority). Clone to avoid borrowing
        // config while we mutate disc_cooldowns.
        let mut discs = self.config.disciplines.clone();
        discs.sort_by_key(|d| d.priority);

        for disc in &discs {
            // Skip if on cooldown
            if self.disc_cooldowns.contains_key(&disc.spell_id) {
                continue;
            }

            // Skip if HP outside valid range
            if hp_pct < disc.min_hp_pct || hp_pct > disc.max_hp_pct {
                continue;
            }

            // Skip if endurance too low
            if end_pct < disc.min_endurance_pct {
                continue;
            }

            tracing::debug!(
                name = %disc.name,
                spell_id = disc.spell_id,
                "Firing discipline"
            );
            crate::eq::do_combat_ability(disc.spell_id, true);
            self.disc_cooldowns.insert(disc.spell_id, disc.cooldown_ticks);

            // Only one disc per tick
            return;
        }
    }
}

/// 3D Euclidean distance between two spawns.
fn distance_3d(a: &SpawnData, b: &SpawnData) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatConfig;

    fn test_config() -> CombatConfig {
        CombatConfig::default()
    }

    fn test_player() -> SpawnData {
        let mut p = SpawnData::default();
        p.name = "TestPlayer".into();
        p.spawn_id = 1;
        p
    }

    fn test_target() -> SpawnData {
        let mut t = SpawnData::default();
        t.name = "TestMob".into();
        t.spawn_id = 100;
        t
    }

    #[test]
    fn new_combatant_starts_idle() {
        let c = Combatant::new(1, 0, test_config());
        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn zone_disconnect_auto_disengages() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Force into Engaging state
        c.state = CombatState::Engaging { target_id: 100 };
        c.assist_target = Some(100);

        // Tick with no target (simulates zone/disconnect)
        c.tick(&player, None, &[]);

        // Should have auto-disengaged back to Idle
        assert!(matches!(c.status(), CombatStatus::Idle));
        assert!(c.assist_target.is_none());
    }

    #[test]
    fn zone_disconnect_during_casting_auto_disengages() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Force into Casting state
        c.state = CombatState::Casting {
            spell_slot: 1,
            ticks_remaining: 10,
        };

        // Tick with no target
        c.tick(&player, None, &[]);

        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn idle_with_no_target_stays_idle() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Idle + no target should NOT trigger zone guard (already safe)
        c.tick(&player, None, &[]);

        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn engaging_with_target_stays_engaging() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };

        // Tick WITH target — should stay in combat
        c.tick(&player, Some(&target), &[]);

        assert!(!matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn zone_guard_clears_flee_requested() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Simulate: flee was requested, then zone happens while engaging
        c.state = CombatState::Engaging { target_id: 100 };
        c.assist_target = Some(100);
        c.flee_requested = true;

        // Zone/disconnect — target vanishes
        c.tick(&player, None, &[]);

        // flee_requested must be cleared so status() doesn't stick on Fleeing
        assert!(matches!(c.status(), CombatStatus::Idle));
        assert!(!c.flee_requested());
    }

    #[test]
    fn distance_3d_basic() {
        let mut a = SpawnData::default();
        a.x = 0.0;
        a.y = 0.0;
        a.z = 0.0;
        let mut b = SpawnData::default();
        b.x = 3.0;
        b.y = 4.0;
        b.z = 0.0;
        assert!((distance_3d(&a, &b) - 5.0).abs() < 0.01);
    }

    // --- Discipline tests ---

    use dmft_common::combat::DisciplineEntry;

    fn make_disc(name: &str, spell_id: i32, priority: u8, cooldown: u32) -> DisciplineEntry {
        DisciplineEntry {
            name: name.to_string(),
            spell_id,
            priority,
            cooldown_ticks: cooldown,
            min_hp_pct: 0.0,
            max_hp_pct: 100.0,
            min_endurance_pct: 0.0,
        }
    }

    fn config_with_discs(discs: Vec<DisciplineEntry>) -> CombatConfig {
        let mut cfg = CombatConfig::default();
        cfg.disciplines = discs;
        cfg
    }

    fn player_with_hp_end(hp: i64, hp_max: i64, end: i32, end_max: u32) -> SpawnData {
        let mut p = SpawnData::default();
        p.name = "TestPlayer".into();
        p.spawn_id = 1;
        p.hp_current = hp;
        p.hp_max = hp_max;
        p.endurance_current = end;
        p.endurance_max = end_max;
        p
    }

    #[test]
    fn discipline_fires_when_conditions_met() {
        let cfg = config_with_discs(vec![make_disc("Mighty Strike", 1001, 1, 100)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);

        c.state = CombatState::Engaging { target_id: 100 };
        let target = test_target();
        c.tick(&player, Some(&target), &[]);

        // Disc should be on cooldown now (meaning it fired)
        assert!(c.disc_cooldowns.contains_key(&1001));
        assert_eq!(c.disc_cooldowns[&1001], 100);
    }

    #[test]
    fn discipline_respects_cooldown() {
        let cfg = config_with_discs(vec![make_disc("Mighty Strike", 1001, 1, 100)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };

        // First tick fires the disc
        c.tick(&player, Some(&target), &[]);
        assert!(c.disc_cooldowns.contains_key(&1001));
        let cd_after_first = c.disc_cooldowns[&1001];

        // Second tick should NOT re-fire (still on cooldown).
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        // Cooldown should be decremented, not reset to 100
        assert!(c.disc_cooldowns[&1001] < cd_after_first);
    }

    #[test]
    fn discipline_respects_hp_range() {
        let mut disc = make_disc("Defensive", 2001, 1, 200);
        disc.min_hp_pct = 20.0;
        disc.max_hp_pct = 50.0;
        let cfg = config_with_discs(vec![disc]);

        // Player at full HP -- should NOT fire (hp_pct = 100%, outside [20, 50])
        let mut c = Combatant::new(1, 0, cfg.clone());
        let player_full = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player_full, Some(&target), &[]);
        assert!(
            !c.disc_cooldowns.contains_key(&2001),
            "Should not fire at full HP"
        );

        // Player at 40% HP -- should fire (inside [20, 50])
        let mut c2 = Combatant::new(1, 0, cfg);
        let player_low = player_with_hp_end(400, 1000, 500, 500);
        c2.state = CombatState::Engaging { target_id: 100 };
        c2.tick(&player_low, Some(&target), &[]);
        assert!(
            c2.disc_cooldowns.contains_key(&2001),
            "Should fire at 40% HP"
        );
    }

    #[test]
    fn only_one_discipline_fires_per_tick() {
        let cfg = config_with_discs(vec![
            make_disc("Mighty Strike", 1001, 1, 100),
            make_disc("Fellstrike", 1002, 2, 100),
        ]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        // Only the higher-priority (lower number) disc should have fired
        assert!(
            c.disc_cooldowns.contains_key(&1001),
            "Priority 1 disc should fire"
        );
        assert!(
            !c.disc_cooldowns.contains_key(&1002),
            "Priority 2 disc should NOT fire on same tick"
        );
    }

    #[test]
    fn discipline_respects_endurance_minimum() {
        let mut disc = make_disc("Mighty Strike", 1001, 1, 100);
        disc.min_endurance_pct = 50.0;
        let cfg = config_with_discs(vec![disc]);

        // Player with only 10% endurance -- should NOT fire
        let mut c = Combatant::new(1, 0, cfg);
        let player_low_end = player_with_hp_end(1000, 1000, 50, 500);
        let target = test_target();
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player_low_end, Some(&target), &[]);
        assert!(
            !c.disc_cooldowns.contains_key(&1001),
            "Should not fire with low endurance"
        );
    }

    #[test]
    fn disc_cooldown_expires_and_disc_refires() {
        let cfg = config_with_discs(vec![make_disc("Quick Disc", 3001, 1, 3)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        // Fire the disc
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);
        assert!(c.disc_cooldowns.contains_key(&3001));

        // Tick 3 more times (cooldown=3). Each tick() decrements at the start.
        for _ in 0..3 {
            c.state = CombatState::Engaging { target_id: 100 };
            c.tick(&player, Some(&target), &[]);
        }

        // After 3 ticks the cooldown expired and the disc re-fired,
        // so it should be back on cooldown with the full duration.
        assert!(
            c.disc_cooldowns.contains_key(&3001),
            "Disc should re-fire after cooldown expires"
        );
    }
}
