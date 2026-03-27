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
    pub fn can_cast(&self, current_mana_pct: f32) -> bool {
        if self.is_healer {
            return true;
        }
        current_mana_pct > self.mana_floor_pct
    }

    /// Should we sit and meditate?
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
