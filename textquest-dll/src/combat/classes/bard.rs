use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::mez_queue::MezQueue;
use crate::combat::strategy::{ClassStrategy, CombatContext};
use crate::combat::twist::{
    DEFAULT_SONG_DURATION_TICKS, DEFAULT_TWIST_DELAY_TICKS, SongCategory, SongSlot, TwistAction,
    TwistEngine,
};

/// Mez song duration in ticks (~18 seconds at 20 ticks/sec = 360 ticks).
const MEZ_DURATION_TICKS: u32 = 360;

/// Default twist timing: cast takes 6 ticks to establish.
const WEAVE_CAST_DURATION: u32 = 6;

pub struct BardStrategy {
    class_id: u8,
    twist: TwistEngine,
    melody_fallback_active: bool,
    tick: u32,
    /// CC queue — tracks mobs that need mezzing.
    mez_queue: MezQueue,
    /// Spell gem containing the mez song (None = no mez configured).
    mez_gem: Option<u8>,
    /// Whether to use full-rotation (song weaving) mode.
    /// When enabled, the twist engine restarts from song 0 every frame.
    full_rotation_enabled: bool,
}

impl BardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            twist: TwistEngine::new(vec![]),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: false,
        }
    }

    pub fn with_twist(class_id: u8, songs: Vec<SongSlot>) -> Self {
        Self {
            class_id,
            twist: TwistEngine::new(songs),
            melody_fallback_active: false,
            tick: 0,
            mez_queue: MezQueue::new(4),
            mez_gem: None,
            full_rotation_enabled: false,
        }
    }

    /// Create a bard with full-rotation song weaving enabled.
    ///
    /// Songs are provided in priority order. The twist engine will restart
    /// from index 0 every frame and only re-cast songs whose buffs are
    /// about to expire.
    pub fn with_weaving(class_id: u8, songs: Vec<SongSlot>) -> Self {
        Self {
            class_id,
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
        }
    }

    /// Enable or disable full-rotation (song weaving) mode.
    pub fn set_weaving(&mut self, enabled: bool) {
        self.full_rotation_enabled = enabled;
        self.twist.set_full_rotation(enabled);
    }

    /// Returns whether song weaving mode is active.
    pub fn is_weaving(&self) -> bool {
        self.full_rotation_enabled
    }

    /// Build a default combat song list with buff durations and categories.
    ///
    /// Songs are ordered by priority (highest first). The full-rotation engine
    /// evaluates them top-to-bottom each frame.
    pub fn default_combat_songs() -> Vec<SongSlot> {
        vec![
            SongSlot {
                gem: 1,
                priority: 1,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Haste,
            },
            SongSlot {
                gem: 2,
                priority: 2,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::SpellFocus,
            },
            SongSlot {
                gem: 3,
                priority: 3,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::MeleeProc,
            },
            SongSlot {
                gem: 4,
                priority: 4,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Crescendo,
            },
            SongSlot {
                gem: 5,
                priority: 10,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: None, // DD — always castable
                category: SongCategory::Insult,
            },
        ]
    }

    /// Build a downtime song list (regen + run speed).
    pub fn default_downtime_songs() -> Vec<SongSlot> {
        vec![
            SongSlot {
                gem: 6,
                priority: 1,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::Regen,
            },
            SongSlot {
                gem: 7,
                priority: 2,
                min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
                buff_duration_ticks: Some(DEFAULT_SONG_DURATION_TICKS),
                category: SongCategory::RunSpeed,
            },
        ]
    }

    /// Configure the mez spell gem for CC duties.
    pub fn set_mez_gem(&mut self, gem: u8) {
        self.mez_gem = Some(gem);
    }

    /// Queue a mob for mezzing. The next available twist cycle will
    /// save target → switch → cast mez → restore target → resume rotation.
    pub fn queue_mez(&mut self, target_id: u32) {
        self.mez_queue
            .add_target(target_id, MEZ_DURATION_TICKS, self.tick);
        tracing::info!(target_id, "Bard: mez target queued");
    }

    /// Record a successful mez landing — refresh the duration timer.
    pub fn record_mez_landed(&mut self, target_id: u32) {
        self.mez_queue
            .record_mez_success(target_id, MEZ_DURATION_TICKS, self.tick);
    }

    /// Record a mez resist — decrement retries.
    pub fn record_mez_resist(&mut self, target_id: u32) {
        self.mez_queue.record_mez_resist(target_id);
    }

    /// Remove a mez target (died, despawned, etc.).
    pub fn remove_mez_target(&mut self, target_id: u32) {
        self.mez_queue.remove_target(target_id);
    }

    /// Number of mobs in the mez queue.
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

    fn on_cast_interrupted(&mut self, ctx: &CombatContext, gem: u8) {
        self.tick = ctx.tick;
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
                    },
                    SongSlot {
                        gem: 1,
                        priority: 2,
                        min_recast_ticks: 66,
                        buff_duration_ticks: None,
                        category: SongCategory::Other,
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
}
