//! AA spend automation — prioritized alternate advancement point spending.
//!
//! Tracks available AA points and a prioritized list of abilities, automatically
//! selecting the next ability to purchase based on priority and affordability.

/// A single alternate advancement ability with rank progression.
#[derive(Debug, Clone)]
pub struct AaAbility {
    /// Unique ability identifier from the EQ AA table.
    pub id: u32,
    /// Human-readable ability name.
    pub name: String,
    /// Current rank (0 = not yet purchased).
    pub current_rank: u8,
    /// Maximum purchasable rank.
    pub max_rank: u8,
    /// AA point cost per rank.
    pub cost_per_rank: u32,
    /// Spend priority (lower number = higher priority).
    pub priority: u8,
}

impl AaAbility {
    /// Returns `true` if the ability has reached its maximum rank.
    pub fn is_maxed(&self) -> bool {
        self.current_rank >= self.max_rank
    }
}

/// Configuration for the AA spend system.
#[derive(Debug, Clone)]
pub struct AaSpendConfig {
    /// Whether automatic AA spending is enabled.
    pub enabled: bool,
    /// Minimum AA points to keep in reserve (never spend below this).
    pub min_reserve_points: u32,
    /// Prioritized list of abilities to purchase.
    pub abilities: Vec<AaAbility>,
}

/// Manages automatic AA point spending based on priority and affordability.
#[derive(Debug)]
pub struct AaSpendManager {
    config: AaSpendConfig,
    available_points: u32,
}

impl AaSpendManager {
    /// Creates a new manager with the given configuration.
    pub fn new(config: AaSpendConfig) -> Self {
        Self {
            config,
            available_points: 0,
        }
    }

    /// Updates the current available AA point total.
    pub fn set_available_points(&mut self, pts: u32) {
        self.available_points = pts;
    }

    /// Returns the current available AA points.
    pub fn available_points(&self) -> u32 {
        self.available_points
    }

    /// Returns whether automatic spending is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns the spendable points (available minus reserve).
    fn spendable(&self) -> u32 {
        self.available_points
            .saturating_sub(self.config.min_reserve_points)
    }

    /// Returns the highest-priority ability that is not yet maxed and affordable.
    ///
    /// Abilities are selected by lowest `priority` value first. Among equal
    /// priorities, the first in the list wins.
    pub fn next_to_purchase(&self) -> Option<&AaAbility> {
        if !self.config.enabled {
            return None;
        }
        let budget = self.spendable();
        self.config
            .abilities
            .iter()
            .filter(|a| !a.is_maxed() && a.cost_per_rank <= budget)
            .min_by_key(|a| a.priority)
    }

    /// Records a successful purchase of the given ability, incrementing its rank.
    ///
    /// Returns `false` if the ability is already at max rank or not found.
    pub fn record_purchase(&mut self, ability_id: u32) -> bool {
        if let Some(ability) = self
            .config
            .abilities
            .iter_mut()
            .find(|a| a.id == ability_id)
        {
            if ability.is_maxed() {
                return false;
            }
            ability.current_rank += 1;
            self.available_points = self.available_points.saturating_sub(ability.cost_per_rank);
            true
        } else {
            false
        }
    }

    /// Returns all abilities that have reached their maximum rank.
    pub fn fully_trained(&self) -> Vec<&AaAbility> {
        self.config
            .abilities
            .iter()
            .filter(|a| a.is_maxed())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ability(id: u32, name: &str, cost: u32, priority: u8, max_rank: u8) -> AaAbility {
        AaAbility {
            id,
            name: name.to_string(),
            current_rank: 0,
            max_rank,
            cost_per_rank: cost,
            priority,
        }
    }

    fn make_config(abilities: Vec<AaAbility>) -> AaSpendConfig {
        AaSpendConfig {
            enabled: true,
            min_reserve_points: 0,
            abilities,
        }
    }

    #[test]
    fn next_purchase_returns_highest_priority() {
        let config = make_config(vec![
            make_ability(1, "Run Speed", 5, 3, 3),
            make_ability(2, "Combat Agility", 2, 1, 5),
            make_ability(3, "Natural Durability", 3, 2, 3),
        ]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(10);

        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 2, "should pick lowest priority number");
    }

    #[test]
    fn next_purchase_skips_maxed_abilities() {
        let mut abilities = vec![
            make_ability(1, "Run Speed", 5, 1, 1),
            make_ability(2, "Combat Agility", 2, 2, 5),
        ];
        abilities[0].current_rank = 1; // maxed
        let config = make_config(abilities);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(10);

        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 2);
    }

    #[test]
    fn next_purchase_skips_unaffordable() {
        let config = make_config(vec![
            make_ability(1, "Expensive AA", 100, 1, 3),
            make_ability(2, "Cheap AA", 2, 2, 5),
        ]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(5);

        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 2, "should skip unaffordable ability");
    }

    #[test]
    fn next_purchase_respects_reserve() {
        let config = AaSpendConfig {
            enabled: true,
            min_reserve_points: 8,
            abilities: vec![make_ability(1, "Run Speed", 5, 1, 3)],
        };
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(10); // 10 - 8 reserve = 2 spendable

        assert!(
            mgr.next_to_purchase().is_none(),
            "cost 5 > spendable 2, should be None"
        );
    }

    #[test]
    fn next_purchase_returns_none_when_disabled() {
        let config = AaSpendConfig {
            enabled: false,
            min_reserve_points: 0,
            abilities: vec![make_ability(1, "Run Speed", 1, 1, 3)],
        };
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(100);

        assert!(mgr.next_to_purchase().is_none());
    }

    #[test]
    fn next_purchase_returns_none_when_all_maxed() {
        let mut ability = make_ability(1, "Run Speed", 1, 1, 2);
        ability.current_rank = 2;
        let config = make_config(vec![ability]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(100);

        assert!(mgr.next_to_purchase().is_none());
    }

    #[test]
    fn record_purchase_increments_rank() {
        let config = make_config(vec![make_ability(1, "Run Speed", 5, 1, 3)]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(20);

        assert!(mgr.record_purchase(1));
        assert_eq!(mgr.available_points(), 15);

        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.current_rank, 1);
    }

    #[test]
    fn record_purchase_returns_false_when_maxed() {
        let mut ability = make_ability(1, "Run Speed", 5, 1, 1);
        ability.current_rank = 1;
        let config = make_config(vec![ability]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(20);

        assert!(!mgr.record_purchase(1));
        assert_eq!(mgr.available_points(), 20, "points should not change");
    }

    #[test]
    fn record_purchase_returns_false_for_unknown_id() {
        let config = make_config(vec![make_ability(1, "Run Speed", 5, 1, 3)]);
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(20);

        assert!(!mgr.record_purchase(999));
    }

    #[test]
    fn fully_trained_returns_maxed_abilities() {
        let mut a1 = make_ability(1, "Run Speed", 5, 1, 2);
        a1.current_rank = 2;
        let a2 = make_ability(2, "Combat Agility", 2, 2, 5);
        let mut a3 = make_ability(3, "Natural Durability", 3, 3, 1);
        a3.current_rank = 1;

        let config = make_config(vec![a1, a2, a3]);
        let mgr = AaSpendManager::new(config);

        let trained = mgr.fully_trained();
        assert_eq!(trained.len(), 2);
        assert!(trained.iter().any(|a| a.id == 1));
        assert!(trained.iter().any(|a| a.id == 3));
    }

    #[test]
    fn full_spend_cycle() {
        let config = AaSpendConfig {
            enabled: true,
            min_reserve_points: 5,
            abilities: vec![
                make_ability(1, "Run Speed", 3, 1, 2),
                make_ability(2, "Combat Agility", 2, 2, 1),
            ],
        };
        let mut mgr = AaSpendManager::new(config);
        mgr.set_available_points(20);

        // First: buy Run Speed rank 1 (priority 1, cost 3)
        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 1);
        assert!(mgr.record_purchase(1));

        // Second: buy Run Speed rank 2 (still priority 1)
        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 1);
        assert!(mgr.record_purchase(1));

        // Run Speed maxed, now Combat Agility
        let next = mgr.next_to_purchase().unwrap();
        assert_eq!(next.id, 2);
        assert!(mgr.record_purchase(2));

        // All maxed
        assert!(mgr.next_to_purchase().is_none());
        assert_eq!(mgr.fully_trained().len(), 2);
        // 20 - 3 - 3 - 2 = 12
        assert_eq!(mgr.available_points(), 12);
    }

    #[test]
    fn is_maxed_boundary() {
        let mut ability = make_ability(1, "Test", 1, 1, 3);
        assert!(!ability.is_maxed());
        ability.current_rank = 2;
        assert!(!ability.is_maxed());
        ability.current_rank = 3;
        assert!(ability.is_maxed());
        ability.current_rank = 4; // over max
        assert!(ability.is_maxed());
    }
}
