//! Combat discipline scheduler — MQ2Melee parity.
//!
//! Manages combat disc activation with priority ordering, cooldown tracking,
//! endurance gating, auto-attack pause on enrage/mez, and class-specific
//! melee skill scheduling (kick/bash/slam/backstab/tiger claw).
//!
//! Ranger headshot cooldown tracking and rogue backstab positioning are
//! handled as first-class features.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use textquest_common::types::ClientId;

// EQ class IDs
pub const CLASS_WARRIOR: u8 = 1;
pub const CLASS_PALADIN: u8 = 3;
pub const CLASS_RANGER: u8 = 4;
pub const CLASS_SHADOWKNIGHT: u8 = 5;
pub const CLASS_MONK: u8 = 7;
pub const CLASS_ROGUE: u8 = 9;
pub const CLASS_BEASTLORD: u8 = 15;
pub const CLASS_BERSERKER: u8 = 16;

/// Headshot cooldown per EQ TLP era — 15s base.
const HEADSHOT_COOLDOWN: Duration = Duration::from_secs(15);

/// A combat discipline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatDisc {
    pub name: String,
    pub endurance_cost: u32,
    pub cooldown: Duration,
    /// Higher value fires first.
    pub priority: u8,
    pub class_ids: Vec<u8>,
}

/// A class melee skill (kick, bash, backstab, tiger claw, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeleeSkill {
    pub name: String,
    pub endurance_cost: u32,
    pub cooldown: Duration,
    pub class_ids: Vec<u8>,
    /// Backstab and certain monk attacks require being behind the target.
    pub requires_behind: bool,
}

/// Per-character melee disc configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeleeDiscConfig {
    pub enabled: bool,
    /// Endurance percent below which discs and skills are skipped.
    pub endurance_threshold_pct: u8,
    pub discs: Vec<CombatDisc>,
    pub skills: Vec<MeleeSkill>,
    /// Pause auto-attack when target enters enrage (flee) state.
    pub pause_on_enrage: bool,
    /// Pause auto-attack when target is mezzed.
    pub pause_on_mez: bool,
    /// Emit `HeadshotReady` notifications for rangers.
    pub headshot_tracking: bool,
}

impl Default for MeleeDiscConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endurance_threshold_pct: 20,
            discs: Vec::new(),
            skills: default_skills_for_all_classes(),
            pause_on_enrage: true,
            pause_on_mez: true,
            headshot_tracking: true,
        }
    }
}

/// Return sensible default skills covering the common melee archetypes.
fn default_skills_for_all_classes() -> Vec<MeleeSkill> {
    vec![
        MeleeSkill {
            name: "Kick".into(),
            endurance_cost: 5,
            cooldown: Duration::from_secs(8),
            class_ids: vec![
                CLASS_WARRIOR,
                CLASS_PALADIN,
                CLASS_RANGER,
                CLASS_SHADOWKNIGHT,
                CLASS_MONK,
                CLASS_BEASTLORD,
            ],
            requires_behind: false,
        },
        MeleeSkill {
            name: "Bash".into(),
            endurance_cost: 5,
            cooldown: Duration::from_secs(8),
            class_ids: vec![CLASS_WARRIOR, CLASS_PALADIN, CLASS_SHADOWKNIGHT],
            requires_behind: false,
        },
        MeleeSkill {
            name: "Slam".into(),
            endurance_cost: 5,
            cooldown: Duration::from_secs(8),
            class_ids: vec![CLASS_WARRIOR],
            requires_behind: false,
        },
        MeleeSkill {
            name: "Backstab".into(),
            endurance_cost: 15,
            cooldown: Duration::from_secs(10),
            class_ids: vec![CLASS_ROGUE],
            requires_behind: true,
        },
        MeleeSkill {
            name: "Tiger Claw".into(),
            endurance_cost: 8,
            cooldown: Duration::from_secs(6),
            class_ids: vec![CLASS_MONK],
            requires_behind: false,
        },
        MeleeSkill {
            name: "Flying Kick".into(),
            endurance_cost: 10,
            cooldown: Duration::from_secs(12),
            class_ids: vec![CLASS_MONK],
            requires_behind: false,
        },
        MeleeSkill {
            name: "Frenzy".into(),
            endurance_cost: 15,
            cooldown: Duration::from_secs(6),
            class_ids: vec![CLASS_BERSERKER],
            requires_behind: false,
        },
    ]
}

/// Input snapshot for one scheduler tick.
#[derive(Debug, Clone)]
pub struct MeleeTickInput {
    pub client_id: ClientId,
    pub class_id: u8,
    pub endurance_current: u32,
    pub endurance_max: u32,
    /// Target is in enrage/flee state — stop attacking to prevent running
    /// characters off the pull area.
    pub target_enraging: bool,
    /// Target has a mez effect — stop attacking to preserve crowd control.
    pub target_mezzed: bool,
    /// Character is positioned behind the target (for backstab eligibility).
    pub target_behind: bool,
    pub combat_active: bool,
}

/// Commands emitted by the scheduler each tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeleeCommand {
    ActivateDisc {
        name: String,
    },
    UseSkill {
        name: String,
    },
    PauseAutoAttack,
    ResumeAutoAttack,
    /// Ranger: headshot cooldown has reset, next arrow may proc headshot.
    HeadshotReady,
}

struct ClientMeleeState {
    disc_last_used: HashMap<String, Instant>,
    skill_last_used: HashMap<String, Instant>,
    headshot_last: Option<Instant>,
    is_auto_attacking: bool,
}

impl ClientMeleeState {
    fn new() -> Self {
        Self {
            disc_last_used: HashMap::new(),
            skill_last_used: HashMap::new(),
            headshot_last: None,
            is_auto_attacking: false,
        }
    }
}

/// Per-character combat discipline and melee skill scheduler.
pub struct MeleeDiscScheduler {
    configs: HashMap<ClientId, MeleeDiscConfig>,
    states: HashMap<ClientId, ClientMeleeState>,
}

impl MeleeDiscScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            states: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, client_id: ClientId, config: MeleeDiscConfig) {
        self.configs.insert(client_id, config);
        self.states.insert(client_id, ClientMeleeState::new());
    }

    pub fn get_config(&self, client_id: ClientId) -> Option<&MeleeDiscConfig> {
        self.configs.get(&client_id)
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.configs.remove(&client_id);
        self.states.remove(&client_id);
    }

    /// Advance the scheduler and return any commands to execute this tick.
    pub fn tick(&mut self, input: &MeleeTickInput) -> Vec<MeleeCommand> {
        let Some(config) = self.configs.get(&input.client_id).cloned() else {
            return vec![];
        };
        if !config.enabled || !input.combat_active {
            return vec![];
        }

        let state = self
            .states
            .entry(input.client_id)
            .or_insert_with(ClientMeleeState::new);

        let now = Instant::now();
        let endurance_pct = if input.endurance_max == 0 {
            0u8
        } else {
            ((input.endurance_current * 100) / input.endurance_max).min(100) as u8
        };

        let mut commands = Vec::new();

        // Pause auto-attack on enrage or mez — safety first.
        if (config.pause_on_enrage && input.target_enraging)
            || (config.pause_on_mez && input.target_mezzed)
        {
            if state.is_auto_attacking {
                commands.push(MeleeCommand::PauseAutoAttack);
                state.is_auto_attacking = false;
            }
            return commands;
        }

        // Resume if previously paused.
        if !state.is_auto_attacking {
            commands.push(MeleeCommand::ResumeAutoAttack);
            state.is_auto_attacking = true;
        }

        // Endurance gate — preserve endurance for running/evading.
        if endurance_pct < config.endurance_threshold_pct {
            return commands;
        }

        // Class melee skills — one per tick, first ready skill wins.
        for skill in &config.skills {
            if !skill.class_ids.contains(&input.class_id) {
                continue;
            }
            if skill.requires_behind && !input.target_behind {
                continue;
            }
            if input.endurance_current < skill.endurance_cost {
                continue;
            }
            let ready = state
                .skill_last_used
                .get(&skill.name)
                .map_or(true, |&last| now.duration_since(last) >= skill.cooldown);
            if ready {
                state.skill_last_used.insert(skill.name.clone(), now);
                commands.push(MeleeCommand::UseSkill {
                    name: skill.name.clone(),
                });
                break;
            }
        }

        // Combat discs — highest priority ready disc wins.
        let mut sorted_discs = config.discs.clone();
        sorted_discs.sort_by(|a, b| b.priority.cmp(&a.priority));
        for disc in &sorted_discs {
            if !disc.class_ids.contains(&input.class_id) {
                continue;
            }
            if input.endurance_current < disc.endurance_cost {
                continue;
            }
            let ready = state
                .disc_last_used
                .get(&disc.name)
                .map_or(true, |&last| now.duration_since(last) >= disc.cooldown);
            if ready {
                state.disc_last_used.insert(disc.name.clone(), now);
                commands.push(MeleeCommand::ActivateDisc {
                    name: disc.name.clone(),
                });
                break;
            }
        }

        // Ranger headshot window notification.
        if config.headshot_tracking && input.class_id == CLASS_RANGER {
            let ready = state
                .headshot_last
                .map_or(true, |last| now.duration_since(last) >= HEADSHOT_COOLDOWN);
            if ready {
                state.headshot_last = Some(now);
                commands.push(MeleeCommand::HeadshotReady);
            }
        }

        commands
    }
}

impl Default for MeleeDiscScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warrior_input(endurance_pct: u8, combat: bool) -> MeleeTickInput {
        let max = 1000u32;
        let current = (max * endurance_pct as u32) / 100;
        MeleeTickInput {
            client_id: 1,
            class_id: CLASS_WARRIOR,
            endurance_current: current,
            endurance_max: max,
            target_enraging: false,
            target_mezzed: false,
            target_behind: false,
            combat_active: combat,
        }
    }

    fn enabled_config() -> MeleeDiscConfig {
        MeleeDiscConfig {
            enabled: true,
            ..MeleeDiscConfig::default()
        }
    }

    #[test]
    fn disabled_produces_no_commands() {
        let mut sched = MeleeDiscScheduler::new();
        let mut cfg = enabled_config();
        cfg.enabled = false;
        sched.set_config(1, cfg);
        let cmds = sched.tick(&warrior_input(100, true));
        assert!(cmds.is_empty());
    }

    #[test]
    fn pauses_on_enrage() {
        let mut sched = MeleeDiscScheduler::new();
        sched.set_config(1, enabled_config());
        // seed auto-attacking state
        sched.tick(&warrior_input(100, true));
        // force auto-attack = true manually
        sched.states.get_mut(&1).unwrap().is_auto_attacking = true;

        let mut input = warrior_input(100, true);
        input.target_enraging = true;
        let cmds = sched.tick(&input);
        assert!(cmds.contains(&MeleeCommand::PauseAutoAttack));
    }

    #[test]
    fn pauses_on_mez() {
        let mut sched = MeleeDiscScheduler::new();
        sched.set_config(1, enabled_config());
        sched
            .states
            .entry(1)
            .or_insert_with(ClientMeleeState::new)
            .is_auto_attacking = true;

        let mut input = warrior_input(100, true);
        input.target_mezzed = true;
        let cmds = sched.tick(&input);
        assert!(cmds.contains(&MeleeCommand::PauseAutoAttack));
    }

    #[test]
    fn no_skills_below_endurance_threshold() {
        let mut sched = MeleeDiscScheduler::new();
        let mut cfg = enabled_config();
        cfg.endurance_threshold_pct = 30;
        sched.set_config(1, cfg);
        // 20% endurance — below threshold
        let cmds = sched.tick(&warrior_input(20, true));
        assert!(
            !cmds
                .iter()
                .any(|c| matches!(c, MeleeCommand::UseSkill { .. }))
        );
    }

    #[test]
    fn warrior_uses_kick() {
        let mut sched = MeleeDiscScheduler::new();
        sched.set_config(1, enabled_config());
        let cmds = sched.tick(&warrior_input(100, true));
        assert!(
            cmds.iter()
                .any(|c| matches!(c, MeleeCommand::UseSkill { name } if name == "Kick")),
            "Warrior should use Kick: {cmds:?}"
        );
    }

    #[test]
    fn rogue_backstab_requires_behind() {
        let mut sched = MeleeDiscScheduler::new();
        let mut cfg = MeleeDiscConfig {
            enabled: true,
            ..MeleeDiscConfig::default()
        };
        sched.set_config(2, cfg);

        let mut input = MeleeTickInput {
            client_id: 2,
            class_id: CLASS_ROGUE,
            endurance_current: 1000,
            endurance_max: 1000,
            target_enraging: false,
            target_mezzed: false,
            target_behind: false,
            combat_active: true,
        };
        let cmds = sched.tick(&input);
        assert!(
            !cmds
                .iter()
                .any(|c| matches!(c, MeleeCommand::UseSkill { name } if name == "Backstab")),
            "Backstab should NOT fire when not behind target"
        );

        input.target_behind = true;
        let cmds = sched.tick(&input);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, MeleeCommand::UseSkill { name } if name == "Backstab")),
            "Backstab SHOULD fire when behind target"
        );
    }

    #[test]
    fn ranger_headshot_ready_emitted() {
        let mut sched = MeleeDiscScheduler::new();
        let cfg = MeleeDiscConfig {
            enabled: true,
            headshot_tracking: true,
            ..MeleeDiscConfig::default()
        };
        sched.set_config(3, cfg);
        let input = MeleeTickInput {
            client_id: 3,
            class_id: CLASS_RANGER,
            endurance_current: 1000,
            endurance_max: 1000,
            target_enraging: false,
            target_mezzed: false,
            target_behind: false,
            combat_active: true,
        };
        let cmds = sched.tick(&input);
        assert!(
            cmds.contains(&MeleeCommand::HeadshotReady),
            "Ranger should get HeadshotReady on first tick"
        );
    }

    #[test]
    fn disc_activates_by_priority() {
        let mut sched = MeleeDiscScheduler::new();
        let mut cfg = enabled_config();
        cfg.discs = vec![
            CombatDisc {
                name: "LowPrio".into(),
                endurance_cost: 50,
                cooldown: Duration::from_secs(60),
                priority: 1,
                class_ids: vec![CLASS_WARRIOR],
            },
            CombatDisc {
                name: "HighPrio".into(),
                endurance_cost: 50,
                cooldown: Duration::from_secs(60),
                priority: 10,
                class_ids: vec![CLASS_WARRIOR],
            },
        ];
        sched.set_config(1, cfg);
        let cmds = sched.tick(&warrior_input(100, true));
        assert!(
            cmds.iter()
                .any(|c| matches!(c, MeleeCommand::ActivateDisc { name } if name == "HighPrio")),
            "High-priority disc should activate first"
        );
    }
}
