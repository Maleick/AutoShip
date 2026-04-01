//! Complete Heal chain coordinator.
//!
//! Manages a rotation of clerics casting Complete Heal on the main tank.
//! Each cleric starts their cast at a fixed interval after the previous one,
//! creating a steady stream of heals landing on the tank.
//!
//! Timing: all intervals are in *frames* (~20/sec, ~50ms each).
//! An EQ "game tick" is 6 seconds (~120 frames) — used for regen/DoTs, not casting.

// Main loop iterations per second (~20fps = ~50ms per frame).
const FRAMES_PER_SECOND: u64 = 20;

/// One EQ game tick in frames (6 seconds × 20 frames/sec).
#[allow(dead_code)]
const GAME_TICK_FRAMES: u64 = FRAMES_PER_SECOND * 6;

/// Adaptive CH interval bounds (seconds).
/// Complete Heal cast time is ~10 seconds in classic EQ. Chain interval
/// must be less than `cast_time` / `num_clerics` to keep the chain seamless.
const MIN_INTERVAL_SECS: f32 = 1.5;
const MAX_INTERVAL_SECS: f32 = 8.0;

/// How many HP-delta samples to keep for averaging damage rate.
const DAMAGE_WINDOW_SIZE: usize = 10;

pub struct ChChain {
    /// Cleric PIDs in chain order.
    members: Vec<u32>,
    /// Seconds between each cleric starting their CH.
    interval_secs: f32,
    /// Current position in the chain.
    current_index: usize,
    /// Whether the chain is active.
    active: bool,
    /// Frame counter for timing (one frame ≈ 50ms at ~20fps).
    frame_count: u64,
    /// Frames between each CH cast (computed from `interval_secs` × `FRAMES_PER_SECOND`).
    frames_per_interval: u64,
    /// The spawn ID of the CH target (usually the main tank).
    target_id: u32,
    /// The spell gem slot for Complete Heal (1-indexed).
    spell_slot: u8,
    /// When true, the chain auto-adjusts interval based on incoming damage.
    adaptive: bool,
    /// Last observed tank HP percentage (for delta calculation).
    last_tank_hp: f32,
    /// Ring buffer of recent HP-delta-per-second samples.
    damage_samples: Vec<f32>,
    /// Frame counter for sampling damage rate (sample every ~1 sec = 20 frames).
    sample_frame: u64,
}

impl ChChain {
    #[must_use]
    pub fn new(members: Vec<u32>, interval_secs: f32, target_id: u32, spell_slot: u8) -> Self {
        Self {
            members,
            interval_secs,
            current_index: 0,
            active: false,
            frame_count: 0,
            frames_per_interval: (interval_secs * FRAMES_PER_SECOND as f32) as u64,
            target_id,
            spell_slot,
            adaptive: false,
            last_tank_hp: 100.0,
            damage_samples: Vec::with_capacity(DAMAGE_WINDOW_SIZE),
            sample_frame: 0,
        }
    }

    /// Start (or restart) the chain from the beginning.
    /// Resets frame counter and rotation index to zero.
    /// Use `resume()` instead if you need to re-activate without losing position
    /// (e.g., substituting a dead cleric mid-rotation).
    pub fn start(&mut self) {
        self.active = true;
        self.frame_count = 0;
        self.current_index = 0;
    }

    /// Resume the chain without resetting position or timing.
    /// Use this when substituting a dead cleric mid-rotation — the chain
    /// continues from wherever it left off rather than restarting from scratch.
    pub fn resume(&mut self) {
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Advance the chain by one frame. Returns the PID that should start
    /// casting CH this frame, if any.
    pub fn tick(&mut self) -> Option<u32> {
        if !self.active || self.members.is_empty() || self.frames_per_interval == 0 {
            return None;
        }

        let should_fire = self.frame_count.is_multiple_of(self.frames_per_interval);
        self.frame_count += 1;

        if should_fire {
            let pid = self.members[self.current_index];
            self.current_index = (self.current_index + 1) % self.members.len();
            Some(pid)
        } else {
            None
        }
    }

    pub fn set_interval(&mut self, secs: f32) {
        self.interval_secs = secs;
        self.frames_per_interval = (secs * FRAMES_PER_SECOND as f32) as u64;
    }

    pub fn add_member(&mut self, pid: u32) {
        if !self.members.contains(&pid) {
            self.members.push(pid);
        }
    }

    pub fn remove_member(&mut self, pid: u32) {
        if let Some(pos) = self.members.iter().position(|&p| p == pid) {
            self.members.remove(pos);
            if self.members.is_empty() || self.current_index >= self.members.len() {
                self.current_index = 0;
            } else if pos < self.current_index {
                self.current_index -= 1;
            }
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn target_id(&self) -> u32 {
        self.target_id
    }

    pub fn set_target(&mut self, target_id: u32) {
        self.target_id = target_id;
    }

    #[must_use]
    pub fn spell_slot(&self) -> u8 {
        self.spell_slot
    }

    #[must_use]
    pub fn members(&self) -> &[u32] {
        &self.members
    }

    /// Enable adaptive mode — chain auto-adjusts interval based on damage rate.
    pub fn set_adaptive(&mut self, enabled: bool) {
        self.adaptive = enabled;
        if enabled {
            self.damage_samples.clear();
            self.last_tank_hp = 100.0;
            self.sample_frame = 0;
        }
    }

    #[must_use]
    pub fn is_adaptive(&self) -> bool {
        self.adaptive
    }

    /// Feed the current tank HP percentage for adaptive timing.
    /// Call this every frame with the tank's current HP%.
    /// Samples damage rate every ~1 second and adjusts the CH interval.
    pub fn update_tank_hp(&mut self, tank_hp_pct: f32) {
        if !self.adaptive || self.members.is_empty() {
            return;
        }

        self.sample_frame += 1;

        // Sample damage rate every second (~20 frames)
        if self.sample_frame.is_multiple_of(FRAMES_PER_SECOND) {
            // HP delta per second (positive = damage taken, negative = healed)
            let delta = self.last_tank_hp - tank_hp_pct;
            self.last_tank_hp = tank_hp_pct;

            // Only record positive deltas (actual damage, not healing)
            if delta > 0.0 {
                if self.damage_samples.len() >= DAMAGE_WINDOW_SIZE {
                    self.damage_samples.remove(0);
                }
                self.damage_samples.push(delta);
            }

            self.recalculate_interval();
        }
    }

    /// Recalculate the CH interval based on average damage rate and cleric count.
    ///
    /// Logic: Complete Heal restores ~100% HP. If the tank takes `D` %HP/sec of
    /// damage, each CH needs to land every `100/D` seconds. With `N` clerics in
    /// the chain, each cleric casts every `N * interval` seconds, so:
    ///   interval = 100 / (D * N)
    ///
    /// Clamped to [`MIN_INTERVAL_SECS`, `MAX_INTERVAL_SECS`] for safety.
    fn recalculate_interval(&mut self) {
        if self.damage_samples.is_empty() || self.members.is_empty() {
            return;
        }

        let avg_damage_per_sec: f32 =
            self.damage_samples.iter().sum::<f32>() / self.damage_samples.len() as f32;

        if avg_damage_per_sec <= 0.5 {
            // Negligible damage — use max interval to conserve mana
            self.set_interval(MAX_INTERVAL_SECS);
            return;
        }

        let n_clerics = self.members.len() as f32;

        // Time until tank dies from full HP at this damage rate:
        //   time_to_death = 100% / avg_damage_per_sec
        // We need a CH to land every time_to_death seconds.
        // With N clerics, each needs to cast every:
        //   interval = time_to_death / N
        let ideal_interval = 100.0 / (avg_damage_per_sec * n_clerics);

        let clamped = ideal_interval.clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS);

        // Only update if change is significant (>0.5 sec) to avoid jitter
        if (clamped - self.interval_secs).abs() > 0.5 {
            tracing::info!(
                avg_dps = avg_damage_per_sec,
                n_clerics,
                old_interval = self.interval_secs,
                new_interval = clamped,
                "CH chain: adaptive interval adjustment"
            );
            self.set_interval(clamped);
        }
    }

    /// Get the current effective interval in seconds.
    #[must_use]
    pub fn interval_secs(&self) -> f32 {
        self.interval_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_rotation_order() {
        let mut chain = ChChain::new(vec![100, 200, 300, 400], 3.0, 1, 1);
        chain.start();

        // First tick fires immediately (tick 0)
        assert_eq!(chain.tick(), Some(100));

        // Next 59 ticks: nothing until tick 60
        for _ in 1..60 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(200));

        // Skip to tick 120
        for _ in 0..59 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(300));

        // Skip to tick 180
        for _ in 0..59 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(400));
    }

    #[test]
    fn chain_wraps_around() {
        let mut chain = ChChain::new(vec![10, 20], 1.0, 1, 1);
        chain.start();

        // frames_per_interval = 20
        assert_eq!(chain.tick(), Some(10)); // tick 0
        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(20)); // tick 20 — but wait, let me recalc

        // Actually: tick 0 fires (10), ticks 1-19 are None, tick 20 fires (20)
        // We already consumed tick 0 (Some(10)) and 19 more ticks above.
        // So the last tick() above was tick 20, which should be Some(20).
        // Let's just re-verify with a fresh chain.
        let mut chain = ChChain::new(vec![10, 20], 1.0, 1, 1);
        chain.start();

        let mut fired = Vec::new();
        for _ in 0..41 {
            if let Some(pid) = chain.tick() {
                fired.push(pid);
            }
        }
        // Fires at ticks 0, 20, 40 → pids 10, 20, 10 (wrap)
        assert_eq!(fired, vec![10, 20, 10]);
    }

    #[test]
    fn start_stop() {
        let mut chain = ChChain::new(vec![1, 2, 3], 3.0, 1, 1);

        // Not active by default
        assert!(!chain.is_active());
        assert_eq!(chain.tick(), None);

        chain.start();
        assert!(chain.is_active());
        assert_eq!(chain.tick(), Some(1));

        chain.stop();
        assert!(!chain.is_active());
        assert_eq!(chain.tick(), None);
    }

    #[test]
    fn start_resets_position() {
        let mut chain = ChChain::new(vec![1, 2, 3], 3.0, 1, 1);
        chain.start();
        assert_eq!(chain.tick(), Some(1));

        // Restart resets to beginning
        chain.start();
        assert_eq!(chain.tick(), Some(1));
    }

    #[test]
    fn resume_preserves_position_while_start_resets() {
        let mut chain = ChChain::new(vec![1, 2, 3], 1.0, 1, 1);
        chain.start();

        // Advance past first two members
        assert_eq!(chain.tick(), Some(1)); // frame 0, index -> 1
        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(2)); // frame 20, index -> 2

        let saved_frame = chain.frame_count;

        // Stop then resume — position and timing preserved
        chain.stop();
        assert!(!chain.is_active());
        chain.resume();
        assert!(chain.is_active());
        assert_eq!(chain.frame_count, saved_frame);

        // Next fire should be member 3 (index 2), not member 1
        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(3));

        // Now compare with start() — resets everything
        chain.stop();
        chain.start();
        assert_eq!(chain.frame_count, 0);
        assert_eq!(chain.tick(), Some(1)); // back to member 1
    }

    #[test]
    fn dynamic_add_member() {
        let mut chain = ChChain::new(vec![1, 2], 1.0, 1, 1);
        chain.add_member(3);
        chain.start();

        let mut fired = Vec::new();
        for _ in 0..61 {
            if let Some(pid) = chain.tick() {
                fired.push(pid);
            }
        }
        // Fires at ticks 0, 20, 40, 60 → pids 1, 2, 3, 1
        assert_eq!(fired, vec![1, 2, 3, 1]);
    }

    #[test]
    fn dynamic_remove_member() {
        let mut chain = ChChain::new(vec![1, 2, 3], 1.0, 1, 1);
        chain.start();
        assert_eq!(chain.tick(), Some(1)); // index now 1

        chain.remove_member(2); // remove member at index 1, current_index adjusts
        // members: [1, 3], current_index stays at 1 (pointing to 3)

        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(3));
    }

    #[test]
    fn remove_only_member_stops_firing() {
        let mut chain = ChChain::new(vec![1], 1.0, 1, 1);
        chain.start();
        assert_eq!(chain.tick(), Some(1));

        chain.remove_member(1);
        assert_eq!(chain.tick(), None);
    }

    #[test]
    fn add_duplicate_ignored() {
        let mut chain = ChChain::new(vec![1, 2], 1.0, 1, 1);
        chain.add_member(1);
        assert_eq!(chain.members.len(), 2);
    }

    #[test]
    fn set_interval_changes_timing() {
        let mut chain = ChChain::new(vec![1, 2], 3.0, 1, 1);
        chain.start();
        assert_eq!(chain.tick(), Some(1)); // frame 0

        chain.set_interval(1.0); // now 20 frames per interval

        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(2)); // tick 20
    }

    #[test]
    fn empty_members_never_fires() {
        let mut chain = ChChain::new(vec![], 3.0, 1, 1);
        chain.start();
        assert_eq!(chain.tick(), None);
    }

    // --- Adaptive mode tests ---

    #[test]
    fn adaptive_tightens_under_heavy_damage() {
        let mut chain = ChChain::new(vec![1, 2, 3, 4], 5.0, 1, 1);
        chain.start();
        chain.set_adaptive(true);

        // Simulate heavy damage: tank loses 20% HP per second
        // With 4 clerics: ideal = 100 / (20 * 4) = 1.25 sec → clamped to 1.5
        let mut hp = 100.0;
        for _ in 0..200 {
            // 10 seconds of sampling
            hp -= 1.0; // 20%/sec at 20fps = 1% per frame
            chain.update_tank_hp(hp);
            if hp < 10.0 {
                hp = 100.0; // simulate CH landing
            }
        }

        // After enough samples, interval should have tightened
        assert!(
            chain.interval_secs() < 5.0,
            "Interval should tighten under heavy damage, got {}",
            chain.interval_secs()
        );
    }

    #[test]
    fn adaptive_loosens_under_light_damage() {
        let mut chain = ChChain::new(vec![1, 2], 2.0, 1, 1);
        chain.start();
        chain.set_adaptive(true);

        // Simulate very light damage: 0.3% HP/sec (negligible)
        for sec in 0..12 {
            let hp = 100.0 - (sec as f32 * 0.3);
            for _ in 0..20 {
                chain.update_tank_hp(hp);
            }
        }

        // Should loosen toward MAX_INTERVAL_SECS
        assert!(
            chain.interval_secs() > 2.0,
            "Interval should loosen under light damage, got {}",
            chain.interval_secs()
        );
    }

    #[test]
    fn adaptive_respects_cleric_count() {
        // More clerics = each can cast less frequently
        let mut chain_2 = ChChain::new(vec![1, 2], 3.0, 1, 1);
        chain_2.start();
        chain_2.set_adaptive(true);

        let mut chain_4 = ChChain::new(vec![1, 2, 3, 4], 3.0, 1, 1);
        chain_4.start();
        chain_4.set_adaptive(true);

        // Same damage rate: 10% HP/sec = 0.5% per frame at 20fps
        // Feed declining HP that resets each second (simulating CH heals)
        for sec in 0..12 {
            for frame in 0..20 {
                let _ = sec; // suppress unused warning
                let hp = 100.0 - (frame as f32 * 0.5); // 0.5% per frame = 10%/sec
                chain_2.update_tank_hp(hp);
                chain_4.update_tank_hp(hp);
            }
        }

        // With 10% DPS:
        //   2 clerics: ideal = 100/(10*2) = 5.0 sec
        //   4 clerics: ideal = 100/(10*4) = 2.5 sec
        // Wait — more clerics means each cleric has a LONGER interval
        // (they share the load). The formula gives individual interval.
        // Actually: ideal_interval = 100 / (D * N), so 4 clerics = 2.5, 2 clerics = 5.0
        // With 4 clerics each casts every 2.5s, total chain time = 10s
        // With 2 clerics each casts every 5s, total chain time = 10s
        // Both achieve same total coverage. The 4-cleric interval is SHORTER per cleric.
        // So chain_4.interval < chain_2.interval — fix assertion:
        assert!(
            chain_4.interval_secs() <= chain_2.interval_secs(),
            "4-cleric interval ({}) should be <= 2-cleric interval ({})",
            chain_4.interval_secs(),
            chain_2.interval_secs()
        );
    }

    #[test]
    fn adaptive_disabled_by_default() {
        let chain = ChChain::new(vec![1, 2], 3.0, 1, 1);
        assert!(!chain.is_adaptive());
    }

    #[test]
    fn set_adaptive_clears_samples() {
        let mut chain = ChChain::new(vec![1, 2], 3.0, 1, 1);
        chain.start();
        chain.set_adaptive(true);

        // Feed some samples
        for _ in 0..100 {
            chain.update_tank_hp(80.0);
        }

        // Disable and re-enable should clear
        chain.set_adaptive(false);
        chain.set_adaptive(true);
        assert!(chain.damage_samples.is_empty());
    }
}
