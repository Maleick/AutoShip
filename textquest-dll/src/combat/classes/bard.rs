use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    CastResult, SpellEntry, TargetSelector,
};

use crate::combat::{
    mez_queue::MezQueue,
    rotation::{self, RotationGroup},
    strategy::{ClassStrategy, CombatContext},
    twist::{
        DEFAULT_SONG_DURATION_TICKS, DEFAULT_TWIST_DELAY_TICKS, InstrumentSlot,
        InstrumentSwapAction, InstrumentSwapEngine, InstrumentType, SongCategory, SongSlot,
        TwistAction, TwistEngine,
    },
};

/// Mez song duration in ticks (~18 seconds at 20 ticks/sec = 360 ticks).
const MEZ_DURATION_TICKS: u32 = 360;

/// Default twist timing: cast takes 6 ticks to establish.
const WEAVE_CAST_DURATION: u32 = 6;
const BARD_MANA_FLOOR_PCT: f32 = 20.0;
const BARD_PROC_MANA_PCT: f32 = 15.0;
const BARD_DEBUFF_MANA_PCT: f32 = 25.0;
const BARD_INSULT_MANA_PCT: f32 = 35.0;
const BARD_DEBUFF_TARGET_HP_PCT: f32 = 60.0;
const BARD_MELEE_ENDURANCE_PCT: f32 = 10.0;

pub struct BardStrategy {
    class_id: u8,
    live_safe_mode: bool,
    twist: TwistEngine,
    melody_fallback_active: bool,
    tick: u32,
    mez_queue: MezQueue,
    mez_gem: Option<u8>,
    full_rotation_enabled: bool,
    instrument_swap: InstrumentSwapEngine,
    instrument_swap_enabled: bool,
}

impl BardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            live_safe_mode: false,
            twist: TwistEngine::new(vec![]),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: false,
            instrument_swap: InstrumentSwapEngine::new(),
            instrument_swap_enabled: true,
        }
    }

    pub fn live_safe(class_id: u8) -> Self {
        Self {
            class_id,
            live_safe_mode: true,
            twist: TwistEngine::new(vec![]),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: false,
            instrument_swap: InstrumentSwapEngine::new(),
            instrument_swap_enabled: true,
        }
    }

    pub fn with_twist(class_id: u8, songs: Vec<SongSlot>) -> Self {
        Self {
            class_id,
            live_safe_mode: false,
            twist: TwistEngine::new(songs),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: false,
            instrument_swap: InstrumentSwapEngine::new(),
            instrument_swap_enabled: true,
        }
    }

    pub fn with_weaving(class_id: u8, songs: Vec<SongSlot>) -> Self {
        Self {
            class_id,
            live_safe_mode: false,
            twist: TwistEngine::with_full_rotation(
                songs,
                DEFAULT_TWIST_DELAY_TICKS,
                WEAVE_CAST_DURATION,
            ),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: true,
            instrument_swap: InstrumentSwapEngine::new(),
            instrument_swap_enabled: true,
        }
    }

    pub fn set_weaving(&mut self, enabled: bool) {
        self.full_rotation_enabled = enabled;
        self.twist.set_full_rotation(enabled);
    }

    pub fn is_weaving(&self) -> bool {
        self.full_rotation_enabled
    }

    pub fn default_combat_songs() -> Vec<SongSlot> {
        vec![
            SongSlot {
                gem: 1,
                priority: 1,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Haste,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
            SongSlot {
                gem: 2,
                priority: 2,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::SpellFocus,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
            SongSlot {
                gem: 3,
                priority: 3,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::MeleeProc,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
            SongSlot {
                gem: 4,
                priority: 4,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Crescendo,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
            SongSlot {
                gem: 5,
                priority: 10,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: None,
                category: SongCategory::Insult,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
        ]
    }

    pub fn default_downtime_songs() -> Vec<SongSlot> {
        vec![
            SongSlot {
                gem: 6,
                priority: 1,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Regen,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
            SongSlot {
                gem: 7,
                priority: 2,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::RunSpeed,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            },
        ]
    }

    pub fn set_mez_gem(&mut self, gem: u8) {
        self.mez_gem = Some(gem);
    }

    pub fn queue_mez(&mut self, target_id: u32) {
        self.mez_queue
            .add_target(target_id, MEZ_DURATION_TICKS, self.tick);
        tracing::info!(target_id, "Bard: mez target queued");
    }

    pub fn record_mez_landed(&mut self, target_id: u32) {
        self.mez_queue
            .record_mez_success(target_id, MEZ_DURATION_TICKS, self.tick);
    }

    pub fn record_mez_resist(&mut self, target_id: u32) {
        self.mez_queue.record_mez_resist(target_id);
    }

    pub fn remove_mez_target(&mut self, target_id: u32) {
        self.mez_queue.remove_target(target_id);
    }

    pub fn mez_queue_len(&self) -> usize {
        self.mez_queue.len()
    }

    fn melody_command(spells: &[SpellEntry]) -> String {
        if spells.is_empty() {
            return "/melody".into();
        }
        format!(
            "/melody {}",
            spells
                .iter()
                .map(|s| s.slot.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }

    fn spells_to_songs(spells: &[SpellEntry]) -> Vec<SongSlot> {
        spells
            .iter()
            .map(|s| SongSlot {
                gem: s.slot,
                priority: s.priority,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Other,
                instrument_type: None,
                instrument_slot: InstrumentSlot::Primary,
            })
            .collect()
    }

    pub fn is_twisting(&self) -> bool {
        self.twist.is_active()
    }

    pub fn hold_song(&mut self, gem: u8) {
        self.twist.hold(gem);
    }

    pub fn release_hold(&mut self) {
        self.twist.release_hold();
    }

    pub fn set_instrument_swap_enabled(&mut self, enabled: bool) {
        self.instrument_swap_enabled = enabled;
        self.instrument_swap.set_enabled(enabled);
    }

    pub fn is_instrument_swap_enabled(&self) -> bool {
        self.instrument_swap_enabled
    }

    pub fn configure_instrument(
        &mut self,
        set_index: usize,
        inst_type: InstrumentType,
        item_id: u32,
    ) {
        self.instrument_swap
            .configure_instrument(set_index, inst_type, item_id);
    }

    pub fn set_equipped(&mut self, slot: InstrumentSlot, item_id: Option<u32>) {
        self.instrument_swap.set_equipped(slot, item_id);
    }

    pub fn equipped_item(&self, slot: InstrumentSlot) -> Option<u32> {
        self.instrument_swap.equipped_item(slot)
    }

    pub fn active_instrument_type(&self) -> Option<InstrumentType> {
        self.instrument_swap.active_instrument_type()
    }

    pub fn prepare_for_song(&mut self, song: &SongSlot) -> InstrumentSwapAction {
        self.instrument_swap.prepare_for_song(song)
    }

    pub fn restore_after_cast(&mut self) -> InstrumentSwapAction {
        self.instrument_swap.restore_after_cast()
    }

    pub fn is_instrument_ready(&self, song: &SongSlot) -> bool {
        self.instrument_swap.is_instrument_ready(song)
    }

    pub fn instrument_swap_mut(&mut self) -> &mut InstrumentSwapEngine {
        &mut self.instrument_swap
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "BattleSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "War March of the Mastruq".into(),
                        min_level: 65,
                        spell_id: 4871,
                    },
                    AbilityCandidate {
                        name: "Warsong of Zek".into(),
                        min_level: 62,
                        spell_id: 3374,
                    },
                    AbilityCandidate {
                        name: "Warsong of the Vah Shir".into(),
                        min_level: 60,
                        spell_id: 2610,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ManaSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Echo of the Trusik".into(),
                        min_level: 65,
                        spell_id: 4872,
                    },
                    AbilityCandidate {
                        name: "Chorus of Marr".into(),
                        min_level: 64,
                        spell_id: 3372,
                    },
                    AbilityCandidate {
                        name: "Wind of Marr".into(),
                        min_level: 62,
                        spell_id: 3651,
                    },
                    AbilityCandidate {
                        name: "Composition of Ervaj".into(),
                        min_level: 60,
                        spell_id: 1452,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "FocusSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Harmony of Sound".into(),
                        min_level: 65,
                        spell_id: 3375,
                    },
                    AbilityCandidate {
                        name: "Druzzil's Disillusionment".into(),
                        min_level: 62,
                        spell_id: 3364,
                    },
                    AbilityCandidate {
                        name: "Aura of Insight".into(),
                        min_level: 55,
                        spell_id: 8926,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ProcSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Call of the Muse".into(),
                        min_level: 65,
                        spell_id: 4112,
                    },
                    AbilityCandidate {
                        name: "Melody of Mischief".into(),
                        min_level: 62,
                        spell_id: 3365,
                    },
                    AbilityCandidate {
                        name: "Ervaj's Lost Composition".into(),
                        min_level: 60,
                        spell_id: 2936,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DebuffSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Requiem of Time".into(),
                        min_level: 64,
                        spell_id: 3066,
                    },
                    AbilityCandidate {
                        name: "Dreams of Thule".into(),
                        min_level: 62,
                        spell_id: 3030,
                    },
                    AbilityCandidate {
                        name: "Fufil's Diminishing Dirge".into(),
                        min_level: 60,
                        spell_id: 4210,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "InsultSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dark Echo".into(),
                        min_level: 65,
                        spell_id: 4873,
                    },
                    AbilityCandidate {
                        name: "Saryrn's Scream of Pain".into(),
                        min_level: 61,
                        spell_id: 3366,
                    },
                    AbilityCandidate {
                        name: "Brusco's Bombastic Bellow".into(),
                        min_level: 55,
                        spell_id: 1747,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "CrowdControlSong".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Lullaby of Morell".into(),
                        min_level: 65,
                        spell_id: 3376,
                    },
                    AbilityCandidate {
                        name: "Silent Song of Quellious".into(),
                        min_level: 61,
                        spell_id: 3361,
                    },
                    AbilityCandidate {
                        name: "Kelin's Lugubrious Lament".into(),
                        min_level: 12,
                        spell_id: 728,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "TravelSong".into(),
                candidates: vec![AbilityCandidate {
                    name: "Selo's Song of Travel".into(),
                    min_level: 51,
                    spell_id: 1750,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut group = rotation::group(
                    "Downtime",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Downtime,
                );
                group.steps_per_frame = 1;
                group.entries = vec![
                    rotation::entry("TravelSong", ActionType::Song("TravelSong".into())),
                    rotation::entry("ManaSong", ActionType::Song("ManaSong".into())),
                ];
                group
            },
            {
                let mut group = rotation::group(
                    "CombatSongs",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                group.steps_per_frame = 1;
                group.entries = vec![
                    rotation::entry("BattleSong", ActionType::Song("BattleSong".into())),
                    rotation::entry("ManaSong", ActionType::Song("ManaSong".into())),
                    rotation::entry("FocusSong", ActionType::Song("FocusSong".into())),
                    rotation::entry_if(
                        "ProcSong",
                        ActionType::Song("ProcSong".into()),
                        ConditionExpr::ManaAbove(BARD_PROC_MANA_PCT),
                    ),
                    rotation::entry_if(
                        "DebuffSong",
                        ActionType::Song("DebuffSong".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(BARD_DEBUFF_MANA_PCT),
                            ConditionExpr::TargetHpAbove(BARD_DEBUFF_TARGET_HP_PCT),
                        ]),
                    ),
                    rotation::entry_if(
                        "InsultSong",
                        ActionType::Song("InsultSong".into()),
                        ConditionExpr::ManaAbove(BARD_INSULT_MANA_PCT),
                    ),
                ];
                group
            },
        ]
    }
}

impl ClassStrategy for BardStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            None
        }
    }

    fn select_spell(&self, _ctx: &CombatContext) -> Option<SpellEntry> {
        None
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        self.tick = ctx.tick;
        if self.live_safe_mode {
            self.melody_fallback_active = false;
            return;
        }
        if !ctx.config.spells.is_empty() {
            let songs = Self::spells_to_songs(&ctx.config.spells);
            if songs.len() >= 2 {
                self.twist.set_songs(songs);
                self.twist.set_full_rotation(self.full_rotation_enabled);
                self.twist.start();
                if let TwistAction::Cast { gem } = self.twist.tick(ctx.tick) {
                    crate::eq::slash_command(&format!("/cast {gem}"));
                }
                return;
            }
        }
        if ctx.config.spells.is_empty() {
            return;
        }
        crate::eq::slash_command(&Self::melody_command(&ctx.config.spells));
        self.melody_fallback_active = true;
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        self.tick = ctx.tick;
        if self.live_safe_mode {
            self.mez_queue.prune_expired(ctx.tick);
            return;
        }
        if !ctx.in_combat {
            if self.twist.is_active() {
                self.twist.stop();
            }
            if self.melody_fallback_active {
                crate::eq::slash_command("/melody");
                self.melody_fallback_active = false;
            }
            self.mez_queue.prune_expired(ctx.tick);
            return;
        }

        // Mez queue has highest priority — check before song rotation.
        if let Some(mez_gem) = self.mez_gem {
            if let Some(target_id) = self.mez_queue.next_refresh_target(ctx.tick) {
                // Interrupt current song and cast mez on the target.
                // The combat FSM uses CastSpell.target_id to do
                // save-target → switch → cast → restore.
                // Save current target, switch, cast mez, then restore.
                let current_target = ctx.target.map(|t| t.spawn_id);
                tracing::info!(
                    target_id,
                    mez_gem,
                    ?current_target,
                    "Bard: casting mez from queue"
                );
                crate::eq::slash_command(&format!("/target id {target_id}"));
                crate::eq::slash_command(&format!("/cast {mez_gem}"));
                if let Some(original) = current_target {
                    crate::eq::slash_command(&format!("/target id {original}"));
                }
                return;
            }
        }

        if self.twist.is_active() {
            if let TwistAction::Cast { gem } = self.twist.tick(ctx.tick) {
                crate::eq::slash_command(&format!("/cast {gem}"));
            }
        }
    }

    fn on_cast_outcome(&mut self, ctx: &CombatContext, gem: u8, result: CastResult) {
        if result == CastResult::Recovering {
            self.on_cast_interrupted(ctx, gem);
            return;
        }
        ClassStrategy::on_cast_outcome(self, ctx, gem, result);
    }

    fn on_cast_interrupted(&mut self, ctx: &CombatContext, gem: u8) {
        self.tick = ctx.tick;
        if self.live_safe_mode {
            return;
        }
        if self.twist.is_active() {
            self.twist.on_interrupt(gem);
            tracing::info!(gem, "Bard: song interrupted, re-queuing via TwistEngine");
        }
        // Melody fallback doesn't need special interrupt handling —
        // EQ's /melody auto-resumes the rotation.
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::Support
    }

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        if self.live_safe_mode {
            Some(Self::build_rotations())
        } else {
            None
        }
    }

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::types::SpawnData;

    fn sp(id: i32, n: &str, sl: u8) -> SpellEntry {
        SpellEntry {
            slot: sl,
            spell_id: id,
            name: n.into(),
            min_mana_pct: 0.0,
            priority: 1,
            is_aoe: false,
        }
    }

    fn s(g: u8, p: u8) -> SongSlot {
        SongSlot {
            gem: g,
            priority: p,
            min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
            buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
            category: SongCategory::Other,
            instrument_type: None,
            instrument_slot: InstrumentSlot::Primary,
        }
    }

    fn cx<'a>(
        p: &'a SpawnData,
        c: &'a textquest_common::combat::CombatConfig,
        ic: bool,
        t: u32,
    ) -> CombatContext<'a> {
        CombatContext {
            player: p,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: c,
            tick: t,
            in_combat: ic,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
    }

    #[test]
    fn melody_cmd() {
        assert_eq!(
            BardStrategy::melody_command(&[sp(1, "A", 1), sp(2, "B", 3)]),
            "/melody 1 3"
        );
    }
    #[test]
    fn melody_empty() {
        assert_eq!(BardStrategy::melody_command(&[]), "/melody");
    }
    #[test]
    fn spell_none() {
        assert!(
            BardStrategy::new(8)
                .select_spell(&cx(
                    &SpawnData::default(),
                    &textquest_common::combat::CombatConfig::default(),
                    true,
                    0
                ))
                .is_none()
        );
    }
    #[test]
    fn role() {
        assert_eq!(BardStrategy::new(8).role(), CombatRole::Support);
    }
    #[test]
    fn id() {
        assert_eq!(BardStrategy::new(8).class_id(), 8);
    }
    #[test]
    fn engage_twist() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        assert!(!b.melody_fallback_active);
    }
    #[test]
    fn engage_melody() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(!b.is_twisting());
        assert!(b.melody_fallback_active);
    }
    #[test]
    fn engage_empty() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig::default();
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(!b.is_twisting());
        assert!(!b.melody_fallback_active);
    }
    #[test]
    fn disengage_twist() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        b.on_action_complete(&cx(&p, &c, false, 50));
        assert!(!b.is_twisting());
    }
    #[test]
    fn disengage_melody() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        b.on_action_complete(&cx(&p, &c, false, 50));
        assert!(!b.melody_fallback_active);
    }
    #[test]
    fn combat_keeps() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        b.on_action_complete(&cx(&p, &c, true, 50));
        assert!(b.is_twisting());
    }
    #[test]
    fn full_cycle() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        b.on_action_complete(&cx(&p, &c, false, 100));
        assert!(!b.is_twisting());
        b.on_engage(&cx(&p, &c, true, 200));
        assert!(b.is_twisting());
    }
    #[test]
    fn preconfigured() {
        assert_eq!(
            BardStrategy::with_twist(
                8,
                vec![
                    SongSlot {
                        gem: 0,
                        priority: 1,
                        min_recast_ticks: 66,
                        buff_duration_ticks: None,
                        category: SongCategory::Other,
                        instrument_type: None,
                        instrument_slot: InstrumentSlot::Primary,
                    },
                    SongSlot {
                        gem: 1,
                        priority: 2,
                        min_recast_ticks: 66,
                        buff_duration_ticks: None,
                        category: SongCategory::Other,
                        instrument_type: None,
                        instrument_slot: InstrumentSlot::Primary,
                    }
                ]
            )
            .twist
            .song_count(),
            2
        );
    }
    #[test]
    fn hold_rel() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        b.hold_song(5);
        b.release_hold();
        assert!(b.is_twisting());
    }
    #[test]
    fn conv() {
        let s = BardStrategy::spells_to_songs(&[sp(1, "A", 1), sp(2, "B", 3)]);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].gem, 1);
        assert_eq!(s[1].gem, 3);
    }

    // --- Interrupt recovery tests ---

    #[test]
    fn interrupt_requeues_in_twist() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        // Simulate an interrupt on gem 1
        b.on_cast_interrupted(&cx(&p, &c, true, 5), 1);
        assert!(b.is_twisting()); // Still active
        assert_eq!(b.twist.interrupted(), Some(1));
    }

    #[test]
    fn interrupt_recovering_requeues_in_twist() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        b.on_cast_outcome(
            &cx(&p, &c, true, 5),
            1,
            textquest_common::combat::CastResult::Recovering,
        );
        assert!(b.is_twisting());
        assert_eq!(b.twist.interrupted(), Some(1));
    }

    #[test]
    fn interrupt_when_not_twisting_is_noop() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1)], // Only 1 spell → melody fallback
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(!b.is_twisting());
        // Interrupt shouldn't do anything special for melody mode
        b.on_cast_interrupted(&cx(&p, &c, true, 5), 1);
        assert_eq!(b.twist.interrupted(), None);
    }

    #[test]
    fn interrupt_then_action_complete_recasts() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        // Interrupt gem 1
        b.on_cast_interrupted(&cx(&p, &c, true, 5), 1);
        // on_action_complete should advance the twist engine which will
        // pick up the interrupted gem
        b.on_action_complete(&cx(&p, &c, true, 6));
        // After the action complete ticks the engine, the interrupted gem
        // should have been cleared (it was re-cast)
        assert_eq!(b.twist.interrupted(), None);
    }

    // --- Mez queue tests ---

    #[test]
    fn queue_mez_adds_target() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        b.queue_mez(1000);
        assert_eq!(b.mez_queue_len(), 1);
    }

    #[test]
    fn queue_mez_no_duplicates() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        b.queue_mez(1000);
        b.queue_mez(1000);
        assert_eq!(b.mez_queue_len(), 1);
    }

    #[test]
    fn remove_mez_target_works() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        b.queue_mez(1000);
        b.queue_mez(2000);
        b.remove_mez_target(1000);
        assert_eq!(b.mez_queue_len(), 1);
    }

    #[test]
    fn mez_resist_decrements() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        b.queue_mez(1000);
        b.record_mez_resist(1000);
        b.record_mez_resist(1000);
        b.record_mez_resist(1000);
        // After 3 resists, no retries left — target won't appear in queue
        assert_eq!(b.mez_queue_len(), 1); // still tracked, but won't be returned
    }

    #[test]
    fn mez_landed_refreshes_timer() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        b.tick = 100;
        b.queue_mez(1000); // expires at 100 + 360 = 460
        b.tick = 400;
        b.record_mez_landed(1000); // refreshes to 400 + 360 = 760
        // At tick 400, was about to expire (400+40 >= 460), now safe until 760
        assert_eq!(b.mez_queue_len(), 1);
    }

    #[test]
    fn no_mez_without_mez_gem() {
        let mut b = BardStrategy::new(8);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        // Queue a mez target but no mez_gem configured
        b.queue_mez(1000);
        // on_action_complete should just do normal twist, not mez
        b.on_action_complete(&cx(&p, &c, true, 500));
        // Twist should still be active (wasn't interrupted for mez)
        assert!(b.is_twisting());
    }

    #[test]
    fn mez_gem_configured_queues_work() {
        let mut b = BardStrategy::new(8);
        b.set_mez_gem(5);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "A", 1), sp(2, "B", 2)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        // Queue mez with a target that's in the refresh window
        b.tick = 0;
        b.queue_mez(1000); // expires at 360, refresh_buffer=40
        // At tick 320+, target is in refresh window (320+40 >= 360)
        // The on_action_complete at that tick should trigger the mez cast
        // (we can't test the actual slash_command side effects, but we can
        // verify the queue was checked)
        assert_eq!(b.mez_queue_len(), 1);
    }

    // --- Weaving mode tests ---

    #[test]
    fn with_weaving_constructor_enables_full_rotation() {
        let songs = BardStrategy::default_combat_songs();
        let b = BardStrategy::with_weaving(8, songs);
        assert!(b.is_weaving());
    }

    #[test]
    fn set_weaving_toggles_mode() {
        let mut b = BardStrategy::new(8);
        assert!(!b.is_weaving());
        b.set_weaving(true);
        assert!(b.is_weaving());
        b.set_weaving(false);
        assert!(!b.is_weaving());
    }

    #[test]
    fn default_combat_songs_count_and_categories() {
        let songs = BardStrategy::default_combat_songs();
        assert_eq!(songs.len(), 5);
        assert_eq!(songs[0].category, SongCategory::Haste);
        assert_eq!(songs[1].category, SongCategory::SpellFocus);
        assert_eq!(songs[2].category, SongCategory::MeleeProc);
        assert_eq!(songs[3].category, SongCategory::Crescendo);
        assert_eq!(songs[4].category, SongCategory::Insult);
        // Insult (DD) has no buff duration
        assert!(songs[4].buff_duration_ticks.is_none());
        // All others have durations
        for s in &songs[..4] {
            assert!(s.buff_duration_ticks.is_some());
        }
    }

    #[test]
    fn default_downtime_songs_count_and_categories() {
        let songs = BardStrategy::default_downtime_songs();
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].category, SongCategory::Regen);
        assert_eq!(songs[1].category, SongCategory::RunSpeed);
    }

    #[test]
    fn engage_with_weaving_sets_full_rotation() {
        let mut b = BardStrategy::new(8);
        b.set_weaving(true);
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "Haste", 1), sp(2, "Focus", 2), sp(3, "Proc", 3)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        assert!(b.twist.is_full_rotation());
    }

    #[test]
    fn engage_without_weaving_no_full_rotation() {
        let mut b = BardStrategy::new(8);
        assert!(!b.is_weaving());
        let p = SpawnData::default();
        let c = textquest_common::combat::CombatConfig {
            spells: vec![sp(1, "Haste", 1), sp(2, "Focus", 2), sp(3, "Proc", 3)],
            ..Default::default()
        };
        b.on_engage(&cx(&p, &c, true, 0));
        assert!(b.is_twisting());
        assert!(!b.twist.is_full_rotation());
    }

    #[test]
    fn weaving_songs_have_correct_gems() {
        let songs = BardStrategy::default_combat_songs();
        let gems: Vec<u8> = songs.iter().map(|s| s.gem).collect();
        assert_eq!(gems, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn weaving_songs_priority_ascending() {
        let songs = BardStrategy::default_combat_songs();
        for window in songs.windows(2) {
            assert!(
                window[0].priority <= window[1].priority,
                "Songs should be in priority order"
            );
        }
    }

    fn bard_known_spells() -> Vec<textquest_common::combat::KnownAbility> {
        BardStrategy::build_ability_sets()
            .iter()
            .flat_map(|set| set.candidates.iter())
            .map(|candidate| textquest_common::combat::KnownAbility {
                name: candidate.name.clone(),
                spell_id: candidate.spell_id,
                level: candidate.min_level,
            })
            .collect()
    }

    #[test]
    fn live_safe_bard_exposes_rotation_groups() {
        let bard = BardStrategy::live_safe(8);
        let groups = bard.rotation_groups().expect("bard rotation groups");
        let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
        assert_eq!(names, vec!["Downtime", "CombatSongs"]);
    }

    #[test]
    fn live_safe_bard_ability_lines_resolve_at_60() {
        let resolved = textquest_common::combat::resolve_abilities(
            &BardStrategy::build_ability_sets(),
            &bard_known_spells(),
            60,
        );
        assert_eq!(
            resolved.get("BattleSong").unwrap().ability_name,
            "Warsong of the Vah Shir"
        );
        assert_eq!(
            resolved.get("ManaSong").unwrap().ability_name,
            "Composition of Ervaj"
        );
        assert_eq!(
            resolved.get("ProcSong").unwrap().ability_name,
            "Ervaj's Lost Composition"
        );
        assert_eq!(
            resolved.get("DebuffSong").unwrap().ability_name,
            "Fufil's Diminishing Dirge"
        );
    }

    #[test]
    fn live_safe_bard_ability_lines_resolve_at_61() {
        let resolved = textquest_common::combat::resolve_abilities(
            &BardStrategy::build_ability_sets(),
            &bard_known_spells(),
            61,
        );
        assert_eq!(
            resolved.get("InsultSong").unwrap().ability_name,
            "Saryrn's Scream of Pain"
        );
        assert_eq!(
            resolved.get("CrowdControlSong").unwrap().ability_name,
            "Silent Song of Quellious"
        );
    }

    #[test]
    fn live_safe_bard_ability_lines_resolve_at_62() {
        let resolved = textquest_common::combat::resolve_abilities(
            &BardStrategy::build_ability_sets(),
            &bard_known_spells(),
            62,
        );
        assert_eq!(
            resolved.get("BattleSong").unwrap().ability_name,
            "Warsong of Zek"
        );
        assert_eq!(
            resolved.get("ManaSong").unwrap().ability_name,
            "Wind of Marr"
        );
        assert_eq!(
            resolved.get("FocusSong").unwrap().ability_name,
            "Druzzil's Disillusionment"
        );
        assert_eq!(
            resolved.get("ProcSong").unwrap().ability_name,
            "Melody of Mischief"
        );
        assert_eq!(
            resolved.get("DebuffSong").unwrap().ability_name,
            "Dreams of Thule"
        );
    }

    #[test]
    fn live_safe_bard_ability_lines_resolve_at_65() {
        let resolved = textquest_common::combat::resolve_abilities(
            &BardStrategy::build_ability_sets(),
            &bard_known_spells(),
            65,
        );
        assert_eq!(
            resolved.get("BattleSong").unwrap().ability_name,
            "War March of the Mastruq"
        );
        assert_eq!(
            resolved.get("ManaSong").unwrap().ability_name,
            "Echo of the Trusik"
        );
        assert_eq!(
            resolved.get("FocusSong").unwrap().ability_name,
            "Harmony of Sound"
        );
        assert_eq!(
            resolved.get("ProcSong").unwrap().ability_name,
            "Call of the Muse"
        );
        assert_eq!(
            resolved.get("InsultSong").unwrap().ability_name,
            "Dark Echo"
        );
        assert_eq!(
            resolved.get("CrowdControlSong").unwrap().ability_name,
            "Lullaby of Morell"
        );
    }

    #[test]
    fn live_safe_bard_rotation_orders_core_songs_before_low_value_dps() {
        let bard = BardStrategy::live_safe(8);
        let combat = bard
            .rotation_groups()
            .unwrap()
            .into_iter()
            .find(|group| group.name == "CombatSongs")
            .expect("combat group");
        let names: Vec<String> = combat.entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(
            names,
            vec![
                "BattleSong",
                "ManaSong",
                "FocusSong",
                "ProcSong",
                "DebuffSong",
                "InsultSong",
            ]
        );
    }

    #[test]
    fn live_safe_bard_config_matches_runtime_profiles() {
        #[derive(serde::Deserialize)]
        struct ResourceThresholds {
            mana_floor_pct: f32,
            proc_mana_pct: f32,
            debuff_mana_pct: f32,
            insult_mana_pct: f32,
            melee_endurance_pct: f32,
        }

        #[derive(serde::Deserialize)]
        struct ConfiguredAbility {
            line: String,
            name: String,
            min_level: u8,
        }

        #[derive(serde::Deserialize)]
        struct BardClassConfig {
            resource_thresholds: ResourceThresholds,
            combat_abilities: Vec<ConfiguredAbility>,
            cc_abilities: Vec<ConfiguredAbility>,
            buff_abilities: Vec<ConfiguredAbility>,
        }

        let config: BardClassConfig =
            toml::from_str(include_str!("../../../../config/classes/bard.toml"))
                .expect("parse bard config");

        assert_eq!(
            config.resource_thresholds.mana_floor_pct,
            BARD_MANA_FLOOR_PCT
        );
        assert_eq!(config.resource_thresholds.proc_mana_pct, BARD_PROC_MANA_PCT);
        assert_eq!(
            config.resource_thresholds.debuff_mana_pct,
            BARD_DEBUFF_MANA_PCT
        );
        assert_eq!(
            config.resource_thresholds.insult_mana_pct,
            BARD_INSULT_MANA_PCT
        );
        assert_eq!(
            config.resource_thresholds.melee_endurance_pct,
            BARD_MELEE_ENDURANCE_PCT
        );

        let runtime_lines: std::collections::HashMap<String, Vec<(String, u8)>> =
            BardStrategy::build_ability_sets()
                .into_iter()
                .map(|set| {
                    (
                        set.name,
                        set.candidates
                            .into_iter()
                            .map(|candidate| (candidate.name, candidate.min_level))
                            .collect(),
                    )
                })
                .collect();

        let combat_lines: std::collections::HashMap<String, Vec<(String, u8)>> = config
            .combat_abilities
            .into_iter()
            .fold(std::collections::HashMap::new(), |mut lines, ability| {
                lines
                    .entry(ability.line)
                    .or_insert_with(Vec::new)
                    .push((ability.name, ability.min_level));
                lines
            });

        for line in [
            "BattleSong",
            "ManaSong",
            "FocusSong",
            "ProcSong",
            "DebuffSong",
            "InsultSong",
        ] {
            assert_eq!(
                combat_lines.get(line),
                runtime_lines.get(line),
                "{line} parity"
            );
        }

        let cc_names: Vec<String> = config
            .cc_abilities
            .into_iter()
            .map(|ability| ability.line)
            .collect();
        assert!(cc_names.contains(&"CrowdControlSong".to_string()));

        let buff_names: Vec<String> = config
            .buff_abilities
            .into_iter()
            .map(|ability| ability.line)
            .collect();
        assert!(buff_names.contains(&"TravelSong".to_string()));
    }
}
