//! Bard TwistEngine — custom song rotation with priority and hold support.

#[derive(Debug, Clone)]
pub struct SongSlot {
    pub gem: u8,
    pub priority: u8,
    pub min_recast_ticks: u32,
}

const TICKS_PER_SECOND: u32 = 20;
pub const DEFAULT_TWIST_DELAY_TICKS: u32 = 66;
pub const MIN_GEM_REFRESH_TICKS: u32 = 60;
const MAX_GEMS: usize = 13;
const MAX_CONSECUTIVE_FAILURES: u8 = 3;

#[derive(Debug, Clone, PartialEq)]
enum TwistState {
    Idle,
    Singing { gem: u8, ticks_remaining: u32 },
    Waiting { ticks_remaining: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TwistAction {
    None,
    Cast { gem: u8 },
    Interrupt,
}

pub struct TwistEngine {
    songs: Vec<SongSlot>,
    rotation_index: usize,
    state: TwistState,
    last_cast_tick: [u32; MAX_GEMS],
    current_tick: u32,
    held_gem: Option<u8>,
    /// Gem that was interrupted and should be re-cast on the next tick.
    interrupted_gem: Option<u8>,
    twist_delay: u32,
    cast_duration: u32,
    active: bool,
    /// Consecutive failures per gem — skip after MAX_CONSECUTIVE_FAILURES.
    consecutive_failures: [u8; MAX_GEMS],
}

impl TwistEngine {
    pub fn new(songs: Vec<SongSlot>) -> Self {
        Self {
            songs,
            rotation_index: 0,
            state: TwistState::Idle,
            last_cast_tick: [0; MAX_GEMS],
            current_tick: 0,
            held_gem: None,
            interrupted_gem: None,
            twist_delay: DEFAULT_TWIST_DELAY_TICKS,
            cast_duration: MIN_GEM_REFRESH_TICKS,
            active: false,
            consecutive_failures: [0; MAX_GEMS],
        }
    }

    pub fn with_timing(songs: Vec<SongSlot>, twist_delay: u32, cast_duration: u32) -> Self {
        Self {
            songs,
            rotation_index: 0,
            state: TwistState::Idle,
            last_cast_tick: [0; MAX_GEMS],
            current_tick: 0,
            held_gem: None,
            interrupted_gem: None,
            twist_delay,
            cast_duration,
            active: false,
            consecutive_failures: [0; MAX_GEMS],
        }
    }

    pub fn start(&mut self) {
        if self.songs.is_empty() {
            return;
        }
        self.active = true;
        self.rotation_index = 0;
        self.state = TwistState::Idle;
        self.interrupted_gem = None;
        self.consecutive_failures = [0; MAX_GEMS];
    }

    pub fn stop(&mut self) -> bool {
        let was_active = self.active;
        self.active = false;
        self.state = TwistState::Idle;
        self.held_gem = None;
        self.interrupted_gem = None;
        was_active
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn hold(&mut self, gem: u8) {
        if self.active {
            self.held_gem = Some(gem);
        }
    }

    pub fn release_hold(&mut self) {
        self.held_gem = None;
    }

    fn gem_ready(&self, gem: u8) -> bool {
        let i = gem as usize;
        if i >= MAX_GEMS {
            return false;
        }
        if self.consecutive_failures[i] >= MAX_CONSECUTIVE_FAILURES {
            return false;
        }
        let last = self.last_cast_tick[i];
        last == 0 || self.current_tick.saturating_sub(last) >= MIN_GEM_REFRESH_TICKS
    }

    fn record_cast(&mut self, gem: u8) {
        let i = gem as usize;
        if i < MAX_GEMS {
            self.last_cast_tick[i] = self.current_tick;
            self.consecutive_failures[i] = 0;
        }
    }

    fn next_song(&self) -> Option<u8> {
        // Re-queue interrupted song first (highest priority)
        if let Some(g) = self.interrupted_gem {
            if self.gem_ready(g) {
                return Some(g);
            }
        }
        // Then check held song
        if let Some(g) = self.held_gem {
            if self.gem_ready(g) {
                return Some(g);
            }
        }
        // Priority preemption: check if any song has higher priority than current rotation slot
        let current_priority = self
            .songs
            .get(self.rotation_index)
            .map_or(u8::MAX, |s| s.priority);
        for s in &self.songs {
            if s.priority < current_priority && self.gem_ready(s.gem) {
                return Some(s.gem);
            }
        }
        // Normal rotation: find next ready song starting from rotation_index
        let n = self.songs.len();
        for offset in 0..n {
            let i = (self.rotation_index + offset) % n;
            if self.gem_ready(self.songs[i].gem) {
                return Some(self.songs[i].gem);
            }
        }
        None
    }

    fn advance_rotation(&mut self, gem: u8) {
        if self.held_gem == Some(gem) {
            self.held_gem = None;
            return;
        }
        let n = self.songs.len();
        for offset in 0..n {
            let i = (self.rotation_index + offset) % n;
            if self.songs[i].gem == gem {
                self.rotation_index = (i + 1) % n;
                return;
            }
        }
    }

    pub fn tick(&mut self, ct: u32) -> TwistAction {
        self.current_tick = ct;
        if !self.active || self.songs.is_empty() {
            return TwistAction::None;
        }
        match &self.state {
            TwistState::Idle => {
                if let Some(g) = self.next_song() {
                    // Clear interrupted flag if we're re-casting the interrupted song
                    if self.interrupted_gem == Some(g) {
                        self.interrupted_gem = None;
                    }
                    self.record_cast(g);
                    self.state = TwistState::Singing {
                        gem: g,
                        ticks_remaining: self.cast_duration,
                    };
                    TwistAction::Cast { gem: g }
                } else {
                    TwistAction::None
                }
            }
            TwistState::Singing {
                gem,
                ticks_remaining,
            } => {
                let gem = *gem;
                let rem = *ticks_remaining;
                let is_held = self.held_gem == Some(gem);
                // Check for held song preemption
                if let Some(h) = self.held_gem {
                    if h != gem && self.gem_ready(h) {
                        self.advance_rotation(gem);
                        self.record_cast(h);
                        self.state = TwistState::Singing {
                            gem: h,
                            ticks_remaining: self.cast_duration,
                        };
                        return TwistAction::Cast { gem: h };
                    }
                }
                // Check for priority preemption (skip if singing a held song)
                if !is_held {
                    let cp = self
                        .songs
                        .iter()
                        .find(|s| s.gem == gem)
                        .map_or(u8::MAX, |s| s.priority);
                    let preempt_gem = self
                        .songs
                        .iter()
                        .find(|s| s.priority < cp && s.gem != gem && self.gem_ready(s.gem))
                        .map(|s| s.gem);
                    if let Some(pg) = preempt_gem {
                        self.advance_rotation(gem);
                        self.record_cast(pg);
                        self.state = TwistState::Singing {
                            gem: pg,
                            ticks_remaining: self.cast_duration,
                        };
                        return TwistAction::Cast { gem: pg };
                    }
                }
                if rem <= 1 {
                    self.advance_rotation(gem);
                    self.state = TwistState::Waiting {
                        ticks_remaining: self.twist_delay.saturating_sub(self.cast_duration),
                    };
                    TwistAction::None
                } else {
                    self.state = TwistState::Singing {
                        gem,
                        ticks_remaining: rem - 1,
                    };
                    TwistAction::None
                }
            }
            TwistState::Waiting { ticks_remaining } => {
                let rem = *ticks_remaining;
                if rem <= 1 {
                    self.state = TwistState::Idle;
                    self.tick(ct)
                } else {
                    self.state = TwistState::Waiting {
                        ticks_remaining: rem - 1,
                    };
                    TwistAction::None
                }
            }
        }
    }

    /// Handle a song interrupt: re-queue the interrupted gem for immediate re-cast.
    /// Resets the gem refresh timer so the song can be cast again right away,
    /// and moves the FSM back to Idle so the next tick picks it up.
    pub fn on_interrupt(&mut self, gem: u8) {
        if !self.active {
            return;
        }
        let i = gem as usize;
        if i < MAX_GEMS {
            self.consecutive_failures[i] += 1;
            if self.consecutive_failures[i] >= MAX_CONSECUTIVE_FAILURES {
                tracing::warn!(
                    gem,
                    failures = self.consecutive_failures[i],
                    "TwistEngine: song failed {MAX_CONSECUTIVE_FAILURES} times, skipping"
                );
                self.interrupted_gem = None;
                self.state = TwistState::Idle;
                return;
            }
            // Clear gem refresh so it can be re-cast immediately.
            self.last_cast_tick[i] = 0;
        }
        self.interrupted_gem = Some(gem);
        self.state = TwistState::Idle;
        tracing::debug!(gem, "TwistEngine: song interrupted, re-queuing");
    }

    /// Returns the currently interrupted gem, if any (for testing/diagnostics).
    pub fn interrupted(&self) -> Option<u8> {
        self.interrupted_gem
    }

    /// Returns the consecutive failure count for a gem (for testing).
    pub fn failure_count(&self, gem: u8) -> u8 {
        let i = gem as usize;
        if i < MAX_GEMS {
            self.consecutive_failures[i]
        } else {
            0
        }
    }

    pub fn set_songs(&mut self, songs: Vec<SongSlot>) {
        let was_active = self.active;
        self.stop();
        self.songs = songs;
        self.last_cast_tick = [0; MAX_GEMS];
        self.interrupted_gem = None;
        self.consecutive_failures = [0; MAX_GEMS];
        if was_active && !self.songs.is_empty() {
            self.start();
        }
    }

    pub fn state_label(&self) -> &'static str {
        match &self.state {
            TwistState::Idle => "idle",
            TwistState::Singing { .. } => "singing",
            TwistState::Waiting { .. } => "waiting",
        }
    }

    pub fn song_count(&self) -> usize {
        self.songs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(g: u8, p: u8) -> SongSlot {
        SongSlot {
            gem: g,
            priority: p,
            min_recast_ticks: DEFAULT_TWIST_DELAY_TICKS,
        }
    }

    fn te(v: Vec<SongSlot>) -> TwistEngine {
        TwistEngine::with_timing(v, 10, 6)
    }

    #[test]
    fn inactive_default() {
        assert!(!TwistEngine::new(vec![s(0, 1)]).is_active());
    }
    #[test]
    fn start_act() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        assert!(e.is_active());
    }
    #[test]
    fn start_empty() {
        let mut e = te(vec![]);
        e.start();
        assert!(!e.is_active());
    }
    #[test]
    fn stop_ret() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        assert!(e.stop());
        assert!(!e.stop());
    }
    #[test]
    fn first_cast() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
    }
    #[test]
    fn inactive_noop() {
        assert_eq!(te(vec![s(0, 1)]).tick(1), TwistAction::None);
    }
    #[test]
    fn rotation() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        for t in 2..=7 {
            assert_eq!(e.tick(t), TwistAction::None);
        }
        for t in 8..=10 {
            assert_eq!(e.tick(t), TwistAction::None);
        }
        assert_eq!(e.tick(11), TwistAction::Cast { gem: 1 });
    }
    #[test]
    fn hold_int() {
        let mut e = te(vec![s(0, 2), s(1, 2)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        e.hold(5);
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 5 });
    }
    #[test]
    fn hold_resume() {
        let mut e = te(vec![s(0, 2), s(1, 2)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        e.hold(5);
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 5 });
        for t in 3..=8 {
            assert_eq!(e.tick(t), TwistAction::None);
        }
        for t in 9..=11 {
            assert_eq!(e.tick(t), TwistAction::None);
        }
        assert_eq!(e.tick(12), TwistAction::Cast { gem: 1 });
    }
    #[test]
    fn preempt() {
        let mut e = te(vec![s(0, 5), s(2, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 2 });
    }
    #[test]
    fn gem_refresh() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        for t in 2..=60 {
            assert_eq!(e.tick(t), TwistAction::None);
        }
        assert_eq!(e.tick(61), TwistAction::Cast { gem: 0 });
    }
    #[test]
    fn release() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.hold(5);
        e.release_hold();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
    }
    #[test]
    fn set_songs_test() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.set_songs(vec![s(3, 1), s(4, 1)]);
        assert!(e.is_active());
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 3 });
    }
    #[test]
    fn set_empty() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.set_songs(vec![]);
        assert!(!e.is_active());
    }
    #[test]
    fn three_songs() {
        let mut e = TwistEngine::with_timing(vec![s(0, 1), s(1, 1), s(2, 1)], 10, 3);
        e.start();
        let mut order = vec![];
        let mut t = 1;
        for _ in 0..3 {
            if let TwistAction::Cast { gem } = e.tick(t) {
                order.push(gem);
            }
            t += 1;
            for _ in 0..9 {
                e.tick(t);
                t += 1;
            }
        }
        assert_eq!(order, vec![0, 1, 2]);
    }
    #[test]
    fn labels() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        assert_eq!(e.state_label(), "idle");
        e.start();
        e.tick(1);
        assert_eq!(e.state_label(), "singing");
        for t in 2..=7 {
            e.tick(t);
        }
        assert_eq!(e.state_label(), "waiting");
    }
    #[test]
    fn consts() {
        assert_eq!(DEFAULT_TWIST_DELAY_TICKS, 66);
        assert_eq!(MIN_GEM_REFRESH_TICKS, 60);
    }

    // --- Interrupt recovery tests ---

    #[test]
    fn interrupt_requeues_song() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        // Start singing gem 0
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        // Interrupt mid-song at tick 3
        e.on_interrupt(0);
        assert_eq!(e.interrupted(), Some(0));
        assert_eq!(e.state_label(), "idle");
        // Next tick should re-cast gem 0
        assert_eq!(e.tick(4), TwistAction::Cast { gem: 0 });
        // Interrupted flag should be cleared
        assert_eq!(e.interrupted(), None);
    }

    #[test]
    fn interrupt_clears_gem_refresh() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        // Normally gem 0 wouldn't be ready again for MIN_GEM_REFRESH_TICKS
        // But interrupt clears the timer
        e.on_interrupt(0);
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 0 });
    }

    #[test]
    fn interrupt_then_normal_rotation_resumes() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        e.on_interrupt(0);
        // Re-cast interrupted song
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 0 });
        // Let it finish singing (6 ticks) + waiting (4 ticks)
        for t in 3..=11 {
            e.tick(t);
        }
        // Next song in rotation should be gem 1
        assert_eq!(e.tick(12), TwistAction::Cast { gem: 1 });
    }

    #[test]
    fn interrupt_inactive_is_noop() {
        let mut e = te(vec![s(0, 1)]);
        // Not started
        e.on_interrupt(0);
        assert_eq!(e.interrupted(), None);
    }

    #[test]
    fn interrupt_during_waiting_phase() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        // Advance through singing into waiting
        for t in 2..=7 {
            e.tick(t);
        }
        assert_eq!(e.state_label(), "waiting");
        // Interrupt during wait — should jump back to idle and re-cast
        e.on_interrupt(0);
        assert_eq!(e.state_label(), "idle");
        assert_eq!(e.tick(8), TwistAction::Cast { gem: 0 });
    }

    #[test]
    fn interrupt_out_of_range_gem_is_safe() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.tick(1);
        // Gem 200 is out of range for MAX_GEMS array, but shouldn't panic
        e.on_interrupt(200);
        assert_eq!(e.interrupted(), Some(200));
        // But gem_ready will return false for out-of-range, so it falls through
        // to normal rotation
        assert_eq!(e.tick(2), TwistAction::None); // gem 0 not ready yet
    }

    #[test]
    fn stop_clears_interrupted() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.tick(1);
        e.on_interrupt(0);
        assert_eq!(e.interrupted(), Some(0));
        e.stop();
        assert_eq!(e.interrupted(), None);
    }

    #[test]
    fn set_songs_clears_interrupted() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.tick(1);
        e.on_interrupt(0);
        e.set_songs(vec![s(3, 1)]);
        assert_eq!(e.interrupted(), None);
    }

    #[test]
    fn interrupt_has_priority_over_held() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        // Both interrupt and hold active — interrupt should win
        e.on_interrupt(0);
        e.hold(5);
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 0 });
    }

    #[test]
    fn interrupt_clears_failure_on_success() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        e.on_interrupt(0);
        assert_eq!(e.failure_count(0), 1);
        // Successful re-cast clears failure counter
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 0 });
        assert_eq!(e.failure_count(0), 0);
    }

    #[test]
    fn interrupt_skips_after_three_failures() {
        let mut e = te(vec![s(0, 1), s(1, 1)]);
        e.start();
        assert_eq!(e.tick(1), TwistAction::Cast { gem: 0 });
        // Three consecutive interrupts without successful re-cast
        e.on_interrupt(0);
        e.on_interrupt(0);
        e.on_interrupt(0);
        assert_eq!(e.failure_count(0), 3);
        assert_eq!(e.interrupted(), None);
        // Gem 0 is skipped — next song plays
        assert_eq!(e.tick(2), TwistAction::Cast { gem: 1 });
    }

    #[test]
    fn start_resets_failures() {
        let mut e = te(vec![s(0, 1)]);
        e.start();
        e.tick(1);
        e.on_interrupt(0);
        e.on_interrupt(0);
        assert_eq!(e.failure_count(0), 2);
        e.start();
        assert_eq!(e.failure_count(0), 0);
    }
}
