/// Governs mana spending decisions — prevents OOM by enforcing a floor.
pub struct ManaGovernor {
    mana_floor_pct: f32,
    is_healer: bool,
}

impl ManaGovernor {
    pub fn new(mana_floor_pct: f32, is_healer: bool) -> Self {
        Self {
            mana_floor_pct,
            is_healer,
        }
    }

    /// Can we afford to cast a spell at current mana level?
    /// Healers ignore the floor during combat (they must always heal).
    #[inline]
    pub fn can_cast(&self, current_mana_pct: f32) -> bool {
        if self.is_healer {
            return true;
        }
        current_mana_pct > self.mana_floor_pct
    }

    /// Should we sit and meditate?
    #[inline]
    pub fn should_med(&self, current_mana_pct: f32, in_combat: bool) -> bool {
        if in_combat && self.is_healer {
            return false;
        }
        current_mana_pct < self.mana_floor_pct
    }

    pub fn mana_floor(&self) -> f32 {
        self.mana_floor_pct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_cast_returns_false_below_floor() {
        let gov = ManaGovernor::new(20.0, false);
        assert!(!gov.can_cast(15.0));
        assert!(!gov.can_cast(20.0));
    }

    #[test]
    fn can_cast_returns_true_above_floor() {
        let gov = ManaGovernor::new(20.0, false);
        assert!(gov.can_cast(21.0));
        assert!(gov.can_cast(100.0));
    }

    #[test]
    fn healer_can_always_cast() {
        let gov = ManaGovernor::new(20.0, true);
        assert!(gov.can_cast(5.0));
        assert!(gov.can_cast(0.0));
        assert!(gov.can_cast(100.0));
    }

    #[test]
    fn should_med_below_floor_out_of_combat() {
        let gov = ManaGovernor::new(30.0, false);
        assert!(gov.should_med(25.0, false));
    }

    #[test]
    fn should_not_med_above_floor() {
        let gov = ManaGovernor::new(30.0, false);
        assert!(!gov.should_med(35.0, false));
        assert!(!gov.should_med(35.0, true));
    }

    #[test]
    fn healer_should_not_med_in_combat() {
        let gov = ManaGovernor::new(30.0, true);
        assert!(!gov.should_med(10.0, true));
    }

    #[test]
    fn healer_should_med_out_of_combat_below_floor() {
        let gov = ManaGovernor::new(30.0, true);
        assert!(gov.should_med(10.0, false));
    }

    #[test]
    fn mana_floor_returns_configured_value() {
        let gov = ManaGovernor::new(42.5, false);
        assert!((gov.mana_floor() - 42.5).abs() < f32::EPSILON);
    }

    #[test]
    fn can_cast_at_exact_floor_returns_false() {
        let gov = ManaGovernor::new(50.0, false);
        assert!(!gov.can_cast(50.0)); // not > floor, only ==
    }

    #[test]
    fn should_med_at_exact_floor_returns_false() {
        let gov = ManaGovernor::new(30.0, false);
        assert!(!gov.should_med(30.0, false)); // not < floor, only ==
    }

    #[test]
    fn zero_mana_floor() {
        let gov = ManaGovernor::new(0.0, false);
        assert!(gov.can_cast(0.1));
        assert!(!gov.can_cast(0.0));
    }

    #[test]
    fn hundred_percent_floor_dps_can_never_cast() {
        let gov = ManaGovernor::new(100.0, false);
        assert!(!gov.can_cast(99.0));
        assert!(!gov.can_cast(100.0));
    }

    #[test]
    fn healer_ignores_hundred_percent_floor() {
        let gov = ManaGovernor::new(100.0, true);
        assert!(gov.can_cast(0.0)); // healer always casts
    }
}
