//! Wishlist and reserve rules for per-character and group loot policies.
//!
//! Rules define keep/sell/bank/distribute actions for specific items, with
//! priority-based conflict resolution when multiple rules match the same item.

use std::collections::HashMap;
use textquest_common::types::ClientId;

/// What to do with a looted item when a wishlist rule matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WishlistAction {
    /// Keep the item on the character who looted it.
    Keep,
    /// Vendor (sell) the item for coin.
    Vendor,
    /// Move the item to the shared bank.
    Bank,
    /// Give the item to a specific character.
    Distribute(ClientId),
}

/// Optional conditions that must be met for a `WishlistRule` to apply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleConditions {
    /// Minimum character level for the rule to apply.
    pub min_level: Option<u8>,
    /// Maximum character level for the rule to apply.
    pub max_level: Option<u8>,
    /// Class mask: bit-per-class bitmask (0 = any class).
    /// Bit 0 = Warrior, 1 = Cleric, … 15 = Berserker (EQ class order).
    pub class_mask: u32,
}

impl RuleConditions {
    /// Returns `true` when there are no restrictions (the rule always applies).
    pub fn is_unrestricted(&self) -> bool {
        self.min_level.is_none() && self.max_level.is_none() && self.class_mask == 0
    }

    /// Evaluates whether `level` and `class_bit` satisfy the conditions.
    ///
    /// `class_bit` is the zero-based class index (0 = Warrior, etc.).
    pub fn matches(&self, level: u8, class_bit: u8) -> bool {
        if let Some(min) = self.min_level
            && level < min
        {
            return false;
        }
        if let Some(max) = self.max_level
            && level > max
        {
            return false;
        }
        if self.class_mask != 0 && (self.class_mask & (1u32 << class_bit)) == 0 {
            return false;
        }
        true
    }
}

/// A single loot policy rule for one item.
///
/// Higher `priority` values win when multiple rules match the same item for
/// the same character.  Tie-breaking is deterministic: Keep > Distribute >
/// Bank > Vendor (safety-first order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WishlistRule {
    /// EQ item ID this rule applies to.
    pub item_id: u32,
    /// What to do when the rule matches.
    pub action: WishlistAction,
    /// Conflict-resolution priority (higher wins).
    pub priority: u8,
    /// Optional level/class restrictions.  `None` means the rule always applies.
    pub conditions: Option<RuleConditions>,
}

impl WishlistRule {
    /// Create an unconditional rule with a given priority.
    pub fn new(item_id: u32, action: WishlistAction, priority: u8) -> Self {
        Self {
            item_id,
            action,
            priority,
            conditions: None,
        }
    }

    /// Returns `true` if this rule is eligible for `level` / `class_bit`.
    pub fn applies_to(&self, level: u8, class_bit: u8) -> bool {
        match &self.conditions {
            None => true,
            Some(c) => c.matches(level, class_bit),
        }
    }
}

/// A reservation that ensures a character always receives a minimum count of
/// an item before general loot distribution happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveRule {
    /// EQ item ID being reserved.
    pub item_id: u32,
    /// How many of the item should be reserved for this character.
    pub count: u32,
    /// The character the reservation is for.
    pub character: ClientId,
    /// Human-readable reason (e.g. "BiS upgrade", "tradeskill mat").
    pub reason: String,
}

impl ReserveRule {
    /// Convenience constructor.
    pub fn new(item_id: u32, count: u32, character: ClientId, reason: impl Into<String>) -> Self {
        Self {
            item_id,
            count,
            character,
            reason: reason.into(),
        }
    }
}

/// Manages per-character and group-wide wishlist rules.
///
/// Rules are evaluated in this order:
/// 1. Per-character rules (keyed by `ClientId`)
/// 2. Group-default rules
///
/// Within each tier, the rule with the highest `priority` wins.  When
/// priorities are equal the action with the safest outcome wins:
/// Keep > Distribute > Bank > Vendor.
#[derive(Debug, Default)]
pub struct WishlistManager {
    /// Per-character rules: `ClientId` → list of rules.
    char_rules: HashMap<ClientId, Vec<WishlistRule>>,
    /// Group-wide fallback rules applied to every character.
    group_rules: Vec<WishlistRule>,
    /// Reserve rules (checked separately from wishlist priority).
    reserve_rules: Vec<ReserveRule>,
}

impl WishlistManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Rule insertion ────────────────────────────────────────────────────

    /// Add a character-specific wishlist rule.
    pub fn add_char_rule(&mut self, char_id: ClientId, rule: WishlistRule) {
        self.char_rules.entry(char_id).or_default().push(rule);
    }

    /// Add a group-wide wishlist rule.
    pub fn add_group_rule(&mut self, rule: WishlistRule) {
        self.group_rules.push(rule);
    }

    /// Add a reserve rule.
    pub fn add_reserve(&mut self, rule: ReserveRule) {
        self.reserve_rules.push(rule);
    }

    // ── Reserve queries ───────────────────────────────────────────────────

    /// Return all reserve rules for a given item, regardless of character.
    pub fn reserves_for_item(&self, item_id: u32) -> Vec<&ReserveRule> {
        self.reserve_rules
            .iter()
            .filter(|r| r.item_id == item_id)
            .collect()
    }

    /// Return the reserve rule for `item_id` / `char_id`, if any.
    pub fn reserve_for_char(&self, item_id: u32, char_id: ClientId) -> Option<&ReserveRule> {
        self.reserve_rules
            .iter()
            .find(|r| r.item_id == item_id && r.character == char_id)
    }

    // ── Rule candidates ───────────────────────────────────────────────────

    /// Collect all eligible rules for `item_id` / `char_id` at `level` / `class_bit`.
    ///
    /// Character-specific rules are returned first (they shadow group rules when
    /// their priority is >= the best group rule).
    fn candidates(
        &self,
        item_id: u32,
        char_id: ClientId,
        level: u8,
        class_bit: u8,
    ) -> Vec<&WishlistRule> {
        let mut out: Vec<&WishlistRule> = Vec::new();

        if let Some(rules) = self.char_rules.get(&char_id) {
            for r in rules {
                if r.item_id == item_id && r.applies_to(level, class_bit) {
                    out.push(r);
                }
            }
        }

        for r in &self.group_rules {
            if r.item_id == item_id && r.applies_to(level, class_bit) {
                out.push(r);
            }
        }

        out
    }
}

/// Numeric weight used to break ties between equal-priority rules.
fn action_weight(action: &WishlistAction) -> u8 {
    match action {
        WishlistAction::Keep => 4,
        WishlistAction::Distribute(_) => 3,
        WishlistAction::Bank => 2,
        WishlistAction::Vendor => 1,
    }
}

/// Resolve the action for `item_id` on character `char_id`.
///
/// Character-level (char_id, level = 1, class_bit = 0) character stats are
/// used when `level` / `class_bit` are not available; callers should pass real
/// values when they have them.
///
/// Returns `WishlistAction::Vendor` when no rules match (safe default).
pub fn resolve_action(item_id: u32, char_id: ClientId, mgr: &WishlistManager) -> WishlistAction {
    resolve_action_with_stats(item_id, char_id, 1, 0, mgr)
}

/// Like `resolve_action` but caller supplies character level and class bit.
pub fn resolve_action_with_stats(
    item_id: u32,
    char_id: ClientId,
    level: u8,
    class_bit: u8,
    mgr: &WishlistManager,
) -> WishlistAction {
    let candidates = mgr.candidates(item_id, char_id, level, class_bit);

    if candidates.is_empty() {
        return WishlistAction::Vendor; // safe fallback
    }

    // Pick highest priority; break ties by action weight.
    let best = candidates
        .into_iter()
        .max_by_key(|r| (r.priority, action_weight(&r.action)))
        .expect("non-empty");

    best.action.clone()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM_A: u32 = 1001;
    const ITEM_B: u32 = 2002;
    const CHAR_1: ClientId = 1;
    const CHAR_2: ClientId = 2;

    fn simple_mgr() -> WishlistManager {
        let mut mgr = WishlistManager::new();
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Keep, 10));
        mgr.add_char_rule(CHAR_1, WishlistRule::new(ITEM_B, WishlistAction::Bank, 5));
        mgr
    }

    // 1. Group rule returns the configured action.
    #[test]
    fn group_rule_resolves_keep() {
        let mgr = simple_mgr();
        assert_eq!(resolve_action(ITEM_A, CHAR_1, &mgr), WishlistAction::Keep);
        assert_eq!(resolve_action(ITEM_A, CHAR_2, &mgr), WishlistAction::Keep);
    }

    // 2. Per-character rule resolves correctly for the right char.
    #[test]
    fn char_rule_resolves_bank() {
        let mgr = simple_mgr();
        assert_eq!(resolve_action(ITEM_B, CHAR_1, &mgr), WishlistAction::Bank);
    }

    // 3. Missing rule falls back to Vendor.
    #[test]
    fn missing_rule_falls_back_to_vendor() {
        let mgr = simple_mgr();
        // CHAR_2 has no rule for ITEM_B and there is no group rule either.
        assert_eq!(resolve_action(ITEM_B, CHAR_2, &mgr), WishlistAction::Vendor);
    }

    // 4. Higher priority wins over lower priority.
    #[test]
    fn higher_priority_wins() {
        let mut mgr = WishlistManager::new();
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Vendor, 1));
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Keep, 99));
        assert_eq!(resolve_action(ITEM_A, CHAR_1, &mgr), WishlistAction::Keep);
    }

    // 5. Priority tie resolved by action weight (Keep > Distribute > Bank > Vendor).
    #[test]
    fn tie_broken_by_action_weight() {
        let mut mgr = WishlistManager::new();
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Vendor, 5));
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Keep, 5));
        assert_eq!(resolve_action(ITEM_A, CHAR_1, &mgr), WishlistAction::Keep);
    }

    // 6. Character rule overrides group rule when char priority >= group priority.
    #[test]
    fn char_rule_overrides_group_rule() {
        let mut mgr = WishlistManager::new();
        mgr.add_group_rule(WishlistRule::new(ITEM_A, WishlistAction::Vendor, 10));
        mgr.add_char_rule(CHAR_1, WishlistRule::new(ITEM_A, WishlistAction::Keep, 10));
        // Same priority — char rule is a candidate alongside group rule.
        // Keep has higher weight than Vendor, so Keep wins.
        assert_eq!(resolve_action(ITEM_A, CHAR_1, &mgr), WishlistAction::Keep);
    }

    // 7. Distribute action round-trips through resolve correctly.
    #[test]
    fn distribute_action_resolves() {
        let mut mgr = WishlistManager::new();
        mgr.add_char_rule(
            CHAR_1,
            WishlistRule::new(ITEM_A, WishlistAction::Distribute(CHAR_2), 20),
        );
        assert_eq!(
            resolve_action(ITEM_A, CHAR_1, &mgr),
            WishlistAction::Distribute(CHAR_2)
        );
    }

    // 8. Level condition filters out rules when level is too low.
    #[test]
    fn level_condition_filters_rule() {
        let mut mgr = WishlistManager::new();
        let mut rule = WishlistRule::new(ITEM_A, WishlistAction::Keep, 10);
        rule.conditions = Some(RuleConditions {
            min_level: Some(50),
            ..Default::default()
        });
        mgr.add_group_rule(rule);

        // Level 30 — rule should NOT apply → fallback Vendor.
        assert_eq!(
            resolve_action_with_stats(ITEM_A, CHAR_1, 30, 0, &mgr),
            WishlistAction::Vendor
        );
        // Level 60 — rule applies → Keep.
        assert_eq!(
            resolve_action_with_stats(ITEM_A, CHAR_1, 60, 0, &mgr),
            WishlistAction::Keep
        );
    }

    // 9. Class condition filters out rules for wrong class.
    #[test]
    fn class_condition_filters_rule() {
        let mut mgr = WishlistManager::new();
        let mut rule = WishlistRule::new(ITEM_A, WishlistAction::Keep, 10);
        // Only class bit 1 (Cleric).
        rule.conditions = Some(RuleConditions {
            class_mask: 0b10, // bit 1
            ..Default::default()
        });
        mgr.add_group_rule(rule);

        // class_bit 0 (Warrior) — not in mask → Vendor.
        assert_eq!(
            resolve_action_with_stats(ITEM_A, CHAR_1, 1, 0, &mgr),
            WishlistAction::Vendor
        );
        // class_bit 1 (Cleric) — in mask → Keep.
        assert_eq!(
            resolve_action_with_stats(ITEM_A, CHAR_1, 1, 1, &mgr),
            WishlistAction::Keep
        );
    }

    // 10. Reserve lookup returns the right rule.
    #[test]
    fn reserve_lookup() {
        let mut mgr = WishlistManager::new();
        mgr.add_reserve(ReserveRule::new(ITEM_A, 2, CHAR_1, "BiS weapon"));
        mgr.add_reserve(ReserveRule::new(ITEM_A, 1, CHAR_2, "alt"));

        assert_eq!(mgr.reserves_for_item(ITEM_A).len(), 2);
        let r = mgr.reserve_for_char(ITEM_A, CHAR_1).unwrap();
        assert_eq!(r.count, 2);
        assert_eq!(r.reason, "BiS weapon");
        assert!(mgr.reserve_for_char(ITEM_B, CHAR_1).is_none());
    }
}
