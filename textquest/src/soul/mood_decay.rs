//! Natural mood decay — moods drift toward neutral over time.

use std::time::Instant;

/// Configuration for the mood decay system.
#[derive(Debug, Clone)]
pub struct MoodDecayConfig {
    /// How much intensity decays per hour (fraction, e.g. 0.2 = 20%/hr).
    pub decay_rate_per_hour: f32,
    /// Minimum seconds between decay ticks.
    pub min_decay_interval_secs: u64,
    /// Whether decay is active.
    pub enabled: bool,
}

impl Default for MoodDecayConfig {
    fn default() -> Self {
        Self {
            decay_rate_per_hour: 0.2,
            min_decay_interval_secs: 30,
            enabled: true,
        }
    }
}

/// A mood intensity value in the range [0.0, 1.0].
///
/// 0.0 = fully neutral; 1.0 = fully in a non-neutral mood.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MoodIntensity {
    pub value: f32,
}

impl MoodIntensity {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
        }
    }
}

/// Manages natural decay of mood intensity toward neutral.
#[derive(Debug)]
pub struct MoodDecayManager {
    pub config: MoodDecayConfig,
    last_tick: Option<Instant>,
    /// Current intensity in [0.0, 1.0].
    pub current_intensity: f32,
}

impl MoodDecayManager {
    pub fn new(config: MoodDecayConfig) -> Self {
        Self {
            config,
            last_tick: None,
            current_intensity: 0.0,
        }
    }

    /// Advance the decay simulation to `now`, applying elapsed decay.
    ///
    /// Returns the new intensity after decay.
    pub fn tick(&mut self, now: Instant) -> f32 {
        if !self.config.enabled {
            return self.current_intensity;
        }

        if let Some(last) = self.last_tick {
            if let Some(elapsed) = now.checked_duration_since(last) {
                let elapsed_secs = elapsed.as_secs_f32();
                if elapsed_secs >= self.config.min_decay_interval_secs as f32 {
                    let elapsed_hours = elapsed_secs / 3600.0;
                    let decay = self.config.decay_rate_per_hour * elapsed_hours;
                    self.current_intensity =
                        (self.current_intensity * (1.0 - decay)).clamp(0.0, 1.0);
                    self.last_tick = Some(now);
                }
            } else {
                self.last_tick = Some(now);
            }
        } else {
            self.last_tick = Some(now);
        }

        self.current_intensity
    }

    /// Reset intensity to `intensity` (from an event trigger).
    ///
    /// Clears the last tick so the next `tick(now)` call initializes the
    /// decay timeline using the caller-provided time source.
    pub fn reset(&mut self, intensity: f32) {
        self.current_intensity = intensity.clamp(0.0, 1.0);
        self.last_tick = None;
    }

    /// Returns true if the mood has decayed close enough to neutral.
    pub fn is_neutral(&self) -> bool {
        self.current_intensity < 0.05
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn manager_at(intensity: f32) -> MoodDecayManager {
        let mut m = MoodDecayManager::new(MoodDecayConfig::default());
        m.current_intensity = intensity;
        m.last_tick = Some(Instant::now());
        m
    }

    #[test]
    fn default_config_values() {
        let cfg = MoodDecayConfig::default();
        assert!((cfg.decay_rate_per_hour - 0.2).abs() < f32::EPSILON);
        assert_eq!(cfg.min_decay_interval_secs, 30);
        assert!(cfg.enabled);
    }

    #[test]
    fn new_manager_starts_at_zero() {
        let m = MoodDecayManager::new(MoodDecayConfig::default());
        assert_eq!(m.current_intensity, 0.0);
        assert!(m.last_tick.is_none());
    }

    #[test]
    fn first_tick_initializes_last_tick() {
        let mut m = MoodDecayManager::new(MoodDecayConfig::default());
        m.current_intensity = 0.5;
        let now = Instant::now();
        let result = m.tick(now);
        // First tick: no elapsed time, intensity unchanged
        assert_eq!(result, 0.5);
        assert!(m.last_tick.is_some());
    }

    #[test]
    fn tick_decays_intensity_after_sufficient_elapsed() {
        let mut m = MoodDecayManager::new(MoodDecayConfig {
            decay_rate_per_hour: 0.5,
            min_decay_interval_secs: 0, // always tick
            enabled: true,
        });
        m.current_intensity = 1.0;
        let t0 = Instant::now();
        m.last_tick = Some(t0);

        // Simulate 1 hour elapsed
        let t1 = t0 + Duration::from_secs(3600);
        let result = m.tick(t1);
        // 1.0 * (1 - 0.5 * 1.0) = 0.5
        assert!((result - 0.5).abs() < 0.001);
    }

    #[test]
    fn tick_does_not_decay_below_interval() {
        let mut m = MoodDecayManager::new(MoodDecayConfig {
            decay_rate_per_hour: 1.0,
            min_decay_interval_secs: 60,
            enabled: true,
        });
        m.current_intensity = 0.8;
        let t0 = Instant::now();
        m.last_tick = Some(t0);

        // Only 10 seconds elapsed — below min interval
        let t1 = t0 + Duration::from_secs(10);
        let result = m.tick(t1);
        assert!((result - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn tick_clamps_to_zero() {
        let mut m = MoodDecayManager::new(MoodDecayConfig {
            decay_rate_per_hour: 100.0, // extreme rate
            min_decay_interval_secs: 0,
            enabled: true,
        });
        m.current_intensity = 0.1;
        let t0 = Instant::now();
        m.last_tick = Some(t0);
        let t1 = t0 + Duration::from_secs(3600);
        let result = m.tick(t1);
        assert!(result >= 0.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn disabled_manager_does_not_decay() {
        let mut m = MoodDecayManager::new(MoodDecayConfig {
            decay_rate_per_hour: 1.0,
            min_decay_interval_secs: 0,
            enabled: false,
        });
        m.current_intensity = 0.8;
        let t0 = Instant::now();
        m.last_tick = Some(t0);
        let t1 = t0 + Duration::from_secs(3600);
        let result = m.tick(t1);
        assert!((result - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_sets_intensity_and_updates_tick() {
        let mut m = manager_at(0.3);
        m.reset(0.9);
        assert!((m.current_intensity - 0.9).abs() < f32::EPSILON);
        assert!(m.last_tick.is_some());
    }

    #[test]
    fn reset_clamps_intensity() {
        let mut m = manager_at(0.0);
        m.reset(1.5);
        assert!((m.current_intensity - 1.0).abs() < f32::EPSILON);
        m.reset(-0.5);
        assert_eq!(m.current_intensity, 0.0);
    }

    #[test]
    fn is_neutral_below_threshold() {
        let mut m = manager_at(0.04);
        assert!(m.is_neutral());
        m.current_intensity = 0.05;
        assert!(!m.is_neutral());
    }

    #[test]
    fn is_neutral_at_zero() {
        let m = manager_at(0.0);
        assert!(m.is_neutral());
    }

    #[test]
    fn mood_intensity_new_clamps() {
        let hi = MoodIntensity::new(2.0);
        assert!((hi.value - 1.0).abs() < f32::EPSILON);
        let lo = MoodIntensity::new(-1.0);
        assert_eq!(lo.value, 0.0);
    }
}
