//! MezTracker — crowd control decision engine for enchanters and bards.
//!
//! Tracks mezzable targets, decides between single-target (ST) and AE mez,
//! and caps concurrent active mezzes to prevent mana-starvation.

/// Spell type to cast for a given mez decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MezSpellType {
    /// Single-target mez (default when fewer than `ae_mez_threshold` targets).
    SingleTarget,
    /// AE mez (used when `ae_mez_threshold` or more targets are mezzable).
    AreaEffect,
}

/// Result returned by [`MezTracker::decide`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MezDecision {
    /// Cast the given spell type on (or centered near) the specified target.
    Cast {
        spawn_id: u32,
        spell_type: MezSpellType,
    },
    /// Do not mez — cap already reached or no eligible targets.
    Skip,
}

/// A tracked mob that is currently mezzed or queued for mez.
#[derive(Debug, Clone)]
struct TrackedMob {
    spawn_id: u32,
    /// True while the mob is confirmed under CC effect.
    active: bool,
}

/// Configuration and runtime state for the mez decision engine.
///
/// # Defaults
/// - `ae_mez_threshold = 3` — switch to AE mez when 3+ mezzable targets present.
/// - `max_concurrent_mez = 4` — refuse the 5th simultaneous mez to preserve mana.
pub struct MezTracker {
    /// Number of mezzable targets required before switching to AE mez.
    pub ae_mez_threshold: u8,
    /// Maximum number of simultaneous active mezzes allowed.
    pub max_concurrent_mez: u8,
    /// Currently tracked (active) mezzed mobs.
    active_mezzes: Vec<TrackedMob>,
}

impl Default for MezTracker {
    fn default() -> Self {
        Self::new(3, 4)
    }
}

impl MezTracker {
    /// Create a tracker with explicit thresholds.
    pub fn new(ae_mez_threshold: u8, max_concurrent_mez: u8) -> Self {
        Self {
            ae_mez_threshold,
            max_concurrent_mez,
            active_mezzes: Vec::new(),
        }
    }

    /// Number of currently active (confirmed) mezzes.
    pub fn active_count(&self) -> usize {
        self.active_mezzes.iter().filter(|m| m.active).count()
    }

    /// Decide whether to mez and which spell type to use.
    ///
    /// # Parameters
    /// - `mezzable_targets` — spawn IDs of all mobs that *could* be mezzed right now.
    ///
    /// Returns [`MezDecision::Skip`] if:
    /// - No mezzable targets are provided.
    /// - Active mez count equals `max_concurrent_mez`.
    ///
    /// Otherwise returns the first untracked target and the appropriate spell type.
    pub fn decide(&self, mezzable_targets: &[u32]) -> MezDecision {
        if mezzable_targets.is_empty() {
            return MezDecision::Skip;
        }

        // Enforce concurrent cap
        if self.active_count() >= self.max_concurrent_mez as usize {
            return MezDecision::Skip;
        }

        // Pick first target not already actively mezzed
        let already_active: std::collections::HashSet<u32> = self
            .active_mezzes
            .iter()
            .filter(|m| m.active)
            .map(|m| m.spawn_id)
            .collect();

        let target = mezzable_targets
            .iter()
            .find(|id| !already_active.contains(id))
            .copied();

        match target {
            None => MezDecision::Skip,
            Some(spawn_id) => {
                let spell_type = if mezzable_targets.len() >= self.ae_mez_threshold as usize {
                    MezSpellType::AreaEffect
                } else {
                    MezSpellType::SingleTarget
                };
                MezDecision::Cast { spawn_id, spell_type }
            }
        }
    }

    /// Record that a mez was successfully applied to `spawn_id`.
    pub fn record_mez(&mut self, spawn_id: u32) {
        if !self.active_mezzes.iter().any(|m| m.spawn_id == spawn_id) {
            self.active_mezzes.push(TrackedMob { spawn_id, active: true });
        } else if let Some(m) = self.active_mezzes.iter_mut().find(|m| m.spawn_id == spawn_id) {
            m.active = true;
        }
    }

    /// Mark a mob's mez as broken (expired, resisted, or mob died).
    pub fn clear_mez(&mut self, spawn_id: u32) {
        self.active_mezzes.retain(|m| m.spawn_id != spawn_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // AE threshold tests
    // ---------------------------------------------------------------------------

    #[test]
    fn ae_mez_selected_when_three_or_more_targets() {
        let tracker = MezTracker::default(); // ae_mez_threshold = 3

        // Fixture: exactly 3 mezzable targets
        let targets = vec![101, 102, 103];
        let decision = tracker.decide(&targets);

        assert_eq!(
            decision,
            MezDecision::Cast {
                spawn_id: 101,
                spell_type: MezSpellType::AreaEffect,
            },
            "Should select AE mez when 3+ mezzable targets are present"
        );
    }

    #[test]
    fn st_mez_selected_when_below_ae_threshold() {
        let tracker = MezTracker::default(); // ae_mez_threshold = 3

        // Fixture: only 2 mezzable targets — below threshold
        let targets = vec![201, 202];
        let decision = tracker.decide(&targets);

        assert_eq!(
            decision,
            MezDecision::Cast {
                spawn_id: 201,
                spell_type: MezSpellType::SingleTarget,
            },
            "Should select ST mez when fewer than ae_mez_threshold targets are present"
        );
    }

    #[test]
    fn ae_mez_selected_when_more_than_threshold() {
        let tracker = MezTracker::default(); // ae_mez_threshold = 3

        // Fixture: 5 targets — well above threshold
        let targets = vec![1, 2, 3, 4, 5];
        let decision = tracker.decide(&targets);

        match decision {
            MezDecision::Cast { spell_type, .. } => {
                assert_eq!(spell_type, MezSpellType::AreaEffect);
            }
            MezDecision::Skip => panic!("Expected Cast, got Skip"),
        }
    }

    // ---------------------------------------------------------------------------
    // max_concurrent_mez cap tests
    // ---------------------------------------------------------------------------

    #[test]
    fn fifth_mez_refused_when_cap_is_four() {
        let mut tracker = MezTracker::default(); // max_concurrent_mez = 4

        // Record 4 active mezzes — reaching the cap
        tracker.record_mez(10);
        tracker.record_mez(20);
        tracker.record_mez(30);
        tracker.record_mez(40);

        assert_eq!(tracker.active_count(), 4, "Should have 4 active mezzes");

        // A 5th unique target is available
        let decision = tracker.decide(&[50]);
        assert_eq!(
            decision,
            MezDecision::Skip,
            "5th mez must be refused when concurrent cap of 4 is reached"
        );
    }

    #[test]
    fn mez_allowed_after_clearing_one() {
        let mut tracker = MezTracker::default(); // max_concurrent_mez = 4

        tracker.record_mez(10);
        tracker.record_mez(20);
        tracker.record_mez(30);
        tracker.record_mez(40);

        // Release one mez
        tracker.clear_mez(10);
        assert_eq!(tracker.active_count(), 3);

        // Now a new mez should be allowed
        let decision = tracker.decide(&[50]);
        assert_ne!(decision, MezDecision::Skip, "Should allow mez after clearing one slot");
    }

    // ---------------------------------------------------------------------------
    // Edge cases
    // ---------------------------------------------------------------------------

    #[test]
    fn skip_when_no_targets() {
        let tracker = MezTracker::default();
        assert_eq!(tracker.decide(&[]), MezDecision::Skip);
    }

    #[test]
    fn skip_already_mezzed_targets() {
        let mut tracker = MezTracker::default();
        tracker.record_mez(100);

        // Only target is already mezzed
        let decision = tracker.decide(&[100]);
        assert_eq!(decision, MezDecision::Skip, "Should skip targets already under mez");
    }

    #[test]
    fn custom_threshold_respected() {
        // ae_mez_threshold = 5, max_concurrent_mez = 2
        let tracker = MezTracker::new(5, 2);

        // 4 targets — below threshold of 5 → ST
        let decision = tracker.decide(&[1, 2, 3, 4]);
        match decision {
            MezDecision::Cast { spell_type, .. } => {
                assert_eq!(spell_type, MezSpellType::SingleTarget);
            }
            MezDecision::Skip => panic!("Expected Cast"),
        }

        // 5 targets — at threshold → AE
        let decision = tracker.decide(&[1, 2, 3, 4, 5]);
        match decision {
            MezDecision::Cast { spell_type, .. } => {
                assert_eq!(spell_type, MezSpellType::AreaEffect);
            }
            MezDecision::Skip => panic!("Expected Cast"),
        }
    }
}
