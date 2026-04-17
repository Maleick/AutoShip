use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ItemRow, LootStore};
use crate::camp::loot::{ItemAction, LootRules, classify_item_with_score};

pub type StatWeights = BTreeMap<String, f64>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemScoreConfig {
    #[serde(default)]
    pub min_upgrade_delta: f64,
    #[serde(default = "default_class_weights")]
    pub class_weights: BTreeMap<String, StatWeights>,
}

impl Default for ItemScoreConfig {
    fn default() -> Self {
        Self {
            min_upgrade_delta: 0.0,
            class_weights: default_class_weights(),
        }
    }
}

impl ItemScoreConfig {
    #[must_use]
    pub fn weights_for_class(&self, class_name: &str) -> Option<&StatWeights> {
        let canonical = canonical_class_name(class_name)?;
        self.class_weights
            .get(canonical)
            .or_else(|| self.class_weights.get(class_name))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreableItem {
    pub item_id: Option<i64>,
    pub name: String,
    pub slot: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub stats: BTreeMap<String, f64>,
}

impl ScoreableItem {
    #[must_use]
    pub fn from_item_row(row: &ItemRow, classes: Vec<String>) -> Self {
        Self {
            item_id: Some(row.id),
            name: row.name.clone(),
            slot: row.slot.clone(),
            classes,
            stats: item_stats_from_row(row),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedStatDelta {
    pub stat: String,
    pub candidate: f64,
    pub equipped: f64,
    pub delta: f64,
    pub weight: f64,
    pub weighted_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemScoreComparison {
    pub class_name: String,
    pub slot: Option<String>,
    pub candidate_item_name: String,
    pub equipped_item_name: Option<String>,
    pub candidate_score: f64,
    pub equipped_score: f64,
    pub score_delta: f64,
    pub is_upgrade: bool,
    pub candidate_usable: bool,
    pub slot_match: bool,
    pub breakdown: Vec<WeightedStatDelta>,
}

impl ItemScoreComparison {
    #[must_use]
    pub fn can_drive_loot_fallback(&self) -> bool {
        self.candidate_usable && self.slot_match && !self.breakdown.is_empty()
    }
}

#[must_use]
pub fn compare_item_upgrade(
    class_name: &str,
    config: &ItemScoreConfig,
    candidate: &ScoreableItem,
    equipped: Option<&ScoreableItem>,
) -> ItemScoreComparison {
    let canonical_class = canonical_class_name(class_name).unwrap_or(class_name);
    let weights = config.weights_for_class(class_name);
    let has_weights = weights.is_some();
    let slot_match = equipped
        .is_none_or(|current| slots_match(candidate.slot.as_deref(), current.slot.as_deref()));
    let candidate_usable = item_usable_by_class(candidate, canonical_class);

    let equipped_score = equipped
        .and_then(|current| weights.map(|class_weights| score_item(current, class_weights)))
        .unwrap_or(0.0);
    let candidate_score = if has_weights && candidate_usable && slot_match {
        weights
            .map(|class_weights| score_item(candidate, class_weights))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let score_delta = candidate_score - equipped_score;
    let is_upgrade =
        has_weights && candidate_usable && slot_match && score_delta >= config.min_upgrade_delta;

    let mut breakdown = weights
        .map(|class_weights| build_breakdown(candidate, equipped, class_weights))
        .unwrap_or_default();
    breakdown.sort_by(|left, right| {
        right
            .weighted_delta
            .abs()
            .partial_cmp(&left.weighted_delta.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ItemScoreComparison {
        class_name: canonical_class.to_string(),
        slot: candidate.slot.clone(),
        candidate_item_name: candidate.name.clone(),
        equipped_item_name: equipped.map(|current| current.name.clone()),
        candidate_score,
        equipped_score,
        score_delta,
        is_upgrade,
        candidate_usable,
        slot_match,
        breakdown,
    }
}

impl LootStore {
    pub fn compare_item_upgrade(
        &self,
        class_name: &str,
        config: &ItemScoreConfig,
        candidate_item_id: i64,
        equipped_item_id: Option<i64>,
    ) -> Result<Option<ItemScoreComparison>> {
        let Some(candidate_row) = self.get_item(candidate_item_id)? else {
            return Ok(None);
        };
        let candidate_classes = self.item_classes(candidate_row.id)?;
        let candidate = ScoreableItem::from_item_row(&candidate_row, candidate_classes);

        let equipped = if let Some(item_id) = equipped_item_id {
            if let Some(row) = self.get_item(item_id)? {
                let classes = self.item_classes(row.id)?;
                Some(ScoreableItem::from_item_row(&row, classes))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Some(compare_item_upgrade(
            class_name,
            config,
            &candidate,
            equipped.as_ref(),
        )))
    }

    pub fn classify_item_for_loot(
        &self,
        class_name: &str,
        rules: &LootRules,
        config: &ItemScoreConfig,
        candidate_item_id: i64,
        equipped_item_id: Option<i64>,
    ) -> Result<Option<ItemAction>> {
        let comparison =
            self.compare_item_upgrade(class_name, config, candidate_item_id, equipped_item_id)?;
        let scored_comparison = comparison
            .as_ref()
            .filter(|score| score.can_drive_loot_fallback());
        let mut score_rules = rules.clone();
        score_rules.loot_all = false;
        Ok(comparison.as_ref().map(|score| {
            classify_item_with_score(&score.candidate_item_name, &score_rules, scored_comparison)
        }))
    }
}

fn build_breakdown(
    candidate: &ScoreableItem,
    equipped: Option<&ScoreableItem>,
    weights: &StatWeights,
) -> Vec<WeightedStatDelta> {
    weights
        .iter()
        .map(|(stat, weight)| {
            let candidate_value = *candidate.stats.get(stat).unwrap_or(&0.0);
            let equipped_value = equipped
                .and_then(|item| item.stats.get(stat))
                .copied()
                .unwrap_or(0.0);
            let delta = candidate_value - equipped_value;
            WeightedStatDelta {
                stat: stat.clone(),
                candidate: candidate_value,
                equipped: equipped_value,
                delta,
                weight: *weight,
                weighted_delta: delta * weight,
            }
        })
        .collect()
}

fn score_item(item: &ScoreableItem, weights: &StatWeights) -> f64 {
    weights
        .iter()
        .map(|(stat, weight)| item.stats.get(stat).copied().unwrap_or(0.0) * weight)
        .sum()
}

fn item_usable_by_class(item: &ScoreableItem, class_name: &str) -> bool {
    if item.classes.is_empty() {
        return true;
    }

    let Some(target) = canonical_class_name(class_name) else {
        return false;
    };

    item.classes.iter().any(|class| {
        canonical_class_name(class)
            .map(|candidate| candidate == target)
            .unwrap_or(false)
    })
}

fn slots_match(candidate_slot: Option<&str>, equipped_slot: Option<&str>) -> bool {
    match (candidate_slot, equipped_slot) {
        (Some(candidate), Some(equipped)) => {
            normalize_slot_name(candidate) == normalize_slot_name(equipped)
        }
        (_, None) => true,
        (None, Some(_)) => false,
    }
}

fn item_stats_from_row(row: &ItemRow) -> BTreeMap<String, f64> {
    let mut stats = BTreeMap::new();

    insert_scalar_stat(&mut stats, "AC", row.ac as f64);
    insert_scalar_stat(&mut stats, "HP", row.hp as f64);
    insert_scalar_stat(&mut stats, "MANA", row.mana as f64);
    insert_scalar_stat(&mut stats, "DAMAGE", row.damage as f64);
    insert_scalar_stat(&mut stats, "DELAY", row.delay as f64);
    insert_scalar_stat(&mut stats, "LEVEL_REQ", row.level_req as f64);
    insert_scalar_stat(&mut stats, "WEIGHT", row.weight as f64);

    if let Some(raw) = &row.stats_json
        && let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw)
    {
        for (key, value) in map {
            let Some(number) = json_number_to_f64(&value) else {
                continue;
            };
            let normalized = normalize_stat_key(&key);
            stats.entry(normalized).or_insert(number);
        }
    }

    stats
}

fn insert_scalar_stat(stats: &mut BTreeMap<String, f64>, key: &str, value: f64) {
    if value != 0.0 {
        stats.insert(key.to_string(), value);
    }
}

fn json_number_to_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(raw) => raw.parse::<f64>().ok(),
        _ => None,
    }
}

fn normalize_slot_name(slot: &str) -> String {
    slot.trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
}

fn normalize_stat_key(stat: &str) -> String {
    match stat.trim().to_ascii_lowercase().as_str() {
        "str" | "strength" => "STR".to_string(),
        "sta" | "stamina" => "STA".to_string(),
        "agi" | "agility" => "AGI".to_string(),
        "dex" | "dexterity" => "DEX".to_string(),
        "wis" | "wisdom" => "WIS".to_string(),
        "int" | "intelligence" => "INT".to_string(),
        "cha" | "charisma" => "CHA".to_string(),
        "hp" | "hitpoints" | "hit_points" => "HP".to_string(),
        "mana" => "MANA".to_string(),
        "ac" => "AC".to_string(),
        "atk" | "attack" => "ATK".to_string(),
        "damage" | "dmg" => "DAMAGE".to_string(),
        "delay" => "DELAY".to_string(),
        "weight" => "WEIGHT".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn canonical_class_name(class_name: &str) -> Option<&'static str> {
    match class_name.trim().to_ascii_lowercase().as_str() {
        "war" | "warrior" => Some("Warrior"),
        "clr" | "cleric" => Some("Cleric"),
        "pal" | "paladin" => Some("Paladin"),
        "rng" | "ranger" => Some("Ranger"),
        "sk" | "shd" | "shadowknight" | "shadow_knight" | "shadow knight" => Some("Shadow Knight"),
        "dru" | "druid" => Some("Druid"),
        "mnk" | "monk" => Some("Monk"),
        "brd" | "bard" => Some("Bard"),
        "rog" | "rogue" => Some("Rogue"),
        "shm" | "shaman" => Some("Shaman"),
        "nec" | "necromancer" => Some("Necromancer"),
        "wiz" | "wizard" => Some("Wizard"),
        "mag" | "magician" => Some("Magician"),
        "enc" | "enchanter" => Some("Enchanter"),
        "bst" | "beastlord" => Some("Beastlord"),
        "ber" | "berserker" => Some("Berserker"),
        _ => None,
    }
}

fn default_class_weights() -> BTreeMap<String, StatWeights> {
    let tank = stat_weights(&[
        ("STR", 1.0),
        ("STA", 1.0),
        ("AGI", 0.8),
        ("DEX", 0.6),
        ("AC", 1.3),
        ("HP", 0.2),
        ("DAMAGE", 1.8),
        ("DELAY", -0.2),
        ("WEIGHT", -0.05),
    ]);
    let priest = stat_weights(&[
        ("STA", 0.7),
        ("WIS", 1.4),
        ("HP", 0.2),
        ("MANA", 0.6),
        ("AC", 0.4),
        ("WEIGHT", -0.05),
    ]);
    let int_caster = stat_weights(&[
        ("STA", 0.5),
        ("INT", 1.4),
        ("HP", 0.1),
        ("MANA", 0.7),
        ("AC", 0.2),
        ("WEIGHT", -0.05),
    ]);
    let hybrid = stat_weights(&[
        ("STR", 0.8),
        ("STA", 0.9),
        ("AGI", 0.6),
        ("DEX", 0.5),
        ("WIS", 0.4),
        ("INT", 0.4),
        ("HP", 0.15),
        ("MANA", 0.25),
        ("AC", 0.8),
        ("DAMAGE", 1.2),
        ("DELAY", -0.15),
        ("WEIGHT", -0.05),
    ]);

    BTreeMap::from([
        ("Warrior".to_string(), tank.clone()),
        ("Cleric".to_string(), priest.clone()),
        ("Paladin".to_string(), hybrid.clone()),
        ("Ranger".to_string(), hybrid.clone()),
        ("Shadow Knight".to_string(), hybrid.clone()),
        ("Druid".to_string(), priest.clone()),
        ("Monk".to_string(), tank.clone()),
        ("Bard".to_string(), hybrid.clone()),
        ("Rogue".to_string(), tank.clone()),
        ("Shaman".to_string(), priest.clone()),
        ("Necromancer".to_string(), int_caster.clone()),
        ("Wizard".to_string(), int_caster.clone()),
        ("Magician".to_string(), int_caster.clone()),
        ("Enchanter".to_string(), int_caster.clone()),
        ("Beastlord".to_string(), hybrid.clone()),
        ("Berserker".to_string(), tank),
    ])
}

fn stat_weights(entries: &[(&str, f64)]) -> StatWeights {
    entries
        .iter()
        .map(|(stat, weight)| ((*stat).to_string(), *weight))
        .collect()
}
