//! Heuristic suggestion engine — pure functions from session aggregates to
//! operator-actionable config knob suggestions.
//!
//! No I/O is performed here. All rules are deterministic functions of their
//! input aggregates.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

// ── Public types ──────────────────────────────────────────────────────────────

/// Opaque session identifier.
pub type SessionId = String;

/// A config knob that the operator can tune.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnobId {
    PullCadenceSeconds,
    CampAggroRadius,
    MedBreakManaPct,
    RetreatHpPct,
    CombatAbilityPriority,
    HealTriggerPct,
    RouteWaypoints,
}

impl std::fmt::Display for KnobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnobId::PullCadenceSeconds => write!(f, "pull_cadence_seconds"),
            KnobId::CampAggroRadius => write!(f, "camp_aggro_radius"),
            KnobId::MedBreakManaPct => write!(f, "med_break_mana_pct"),
            KnobId::RetreatHpPct => write!(f, "retreat_hp_pct"),
            KnobId::CombatAbilityPriority => write!(f, "combat_ability_priority"),
            KnobId::HealTriggerPct => write!(f, "heal_trigger_pct"),
            KnobId::RouteWaypoints => write!(f, "route_waypoints"),
        }
    }
}

/// Operator-actionable suggestion for a single config knob / character pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub knob: KnobId,
    pub character: String,
    pub current: Value,
    pub proposed: Value,
    pub rationale: String,
    /// Normalized to [0, 1].
    pub confidence: f32,
    pub session_evidence: Vec<SessionId>,
}

// ── Session aggregates ────────────────────────────────────────────────────────

/// Mana snapshot at a specific point in a pull.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManaSnapshot {
    /// Mana percentage at the start of the pull (0–100).
    pub mana_pct: f32,
}

/// Aggregate metrics for a single session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionAggregate {
    pub session_id: SessionId,
    /// Mana percentages sampled at the start of each pull.
    pub pull_mana_pcts: Vec<f32>,
    /// Number of adds engaged while already in combat, per pull.
    pub adds_while_engaged_per_pull: Vec<f32>,
    /// Mana percentage at the moment the first ability of a pull is cast.
    pub mana_at_first_cast: Vec<f32>,
    /// HP percentage during survived close calls (near-death events).
    pub hp_at_close_calls: Vec<f32>,
    /// Ability contribution scores: ability name → score.
    pub ability_scores: HashMap<String, f32>,
    /// HP percentages at moments when a heal would have landed in time but didn't.
    pub hp_when_heal_missed: Vec<f32>,
    /// Waypoint traversal counts: waypoint_id → count.
    pub waypoint_traversals: HashMap<String, u32>,
    /// High-value spawns observed near each waypoint: waypoint_id → count.
    pub waypoint_spawn_detections: HashMap<String, u32>,
    /// High-value spawns observed near candidate positions: position_key → count.
    pub candidate_spawn_observations: HashMap<String, u32>,
}

/// Current operator-configured knob values for a character.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentKnobs {
    pub pull_cadence_seconds: f32,
    pub camp_aggro_radius: f32,
    pub med_break_mana_pct: f32,
    pub retreat_hp_pct: f32,
    pub combat_ability_priority: Vec<String>,
    pub heal_trigger_pct: f32,
    /// Minimum mana percentage required to execute a full opener.
    pub required_for_full_opener_pct: f32,
}

// ── Deduplication key ─────────────────────────────────────────────────────────

/// Session-scoped deduplication state.
#[derive(Debug, Clone, Default)]
pub struct DedupeState {
    /// (knob, character) → last proposed value emitted.
    emitted: HashMap<(String, String), Value>,
}

impl DedupeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if this suggestion should be emitted (new or changed value).
    fn should_emit(&mut self, knob: &KnobId, character: &str, proposed: &Value) -> bool {
        let key = (knob.to_string(), character.to_owned());
        match self.emitted.get(&key) {
            Some(prev) if prev == proposed => false,
            _ => {
                self.emitted.insert(key, proposed.clone());
                true
            }
        }
    }
}

// ── Statistical helpers ───────────────────────────────────────────────────────

fn percentile(values: &mut Vec<f32>, pct: f32) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((pct / 100.0) * (values.len() - 1) as f32).round() as usize;
    Some(values[idx.min(values.len() - 1)])
}

fn mean(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f32>() / values.len() as f32)
}

/// Confidence: (supporting sessions / max_sessions) × signal_strength, clamped to [0, 1].
fn confidence(supporting: usize, max_sessions: usize, signal_strength: f32) -> f32 {
    if max_sessions == 0 {
        return 0.0;
    }
    ((supporting as f32 / max_sessions as f32) * signal_strength).clamp(0.0, 1.0)
}

// ── Individual rules ──────────────────────────────────────────────────────────

/// Rule 1: `pull_cadence_seconds`
///
/// If mana p10 < 25% across last 5 sessions, suggest current + 2.
pub fn rule_pull_cadence(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(5).collect();

    // Collect all pull-start mana samples across the sessions.
    let mut all_mana: Vec<f32> = recent
        .iter()
        .flat_map(|s| s.pull_mana_pcts.iter().copied())
        .collect();

    if all_mana.is_empty() {
        return None;
    }

    let p10 = percentile(&mut all_mana, 10.0)?;
    if p10 >= 25.0 {
        return None;
    }

    let proposed_val = knobs.pull_cadence_seconds + 2.0;
    let proposed = Value::from(proposed_val as f64);
    if !dedupe.should_emit(&KnobId::PullCadenceSeconds, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let conf = confidence(evidence_ids.len(), 5, 1.0 - p10 / 25.0);

    Some(Suggestion {
        knob: KnobId::PullCadenceSeconds,
        character: character.to_owned(),
        current: Value::from(knobs.pull_cadence_seconds as f64),
        proposed,
        rationale: format!(
            "Mana p10 at pull start was {:.1}% (threshold: 25%) across {} sessions. \
             Increasing pull cadence by 2s gives mana time to recover between pulls.",
            p10,
            evidence_ids.len()
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Rule 2: `camp_aggro_radius`
///
/// If mean adds-while-engaged-per-pull > 0.5 across last 5 sessions, suggest current * 0.8.
pub fn rule_camp_aggro_radius(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(5).collect();

    let all_adds: Vec<f32> = recent
        .iter()
        .flat_map(|s| s.adds_while_engaged_per_pull.iter().copied())
        .collect();

    if all_adds.is_empty() {
        return None;
    }

    let avg = mean(&all_adds)?;
    if avg <= 0.5 {
        return None;
    }

    let proposed_val = knobs.camp_aggro_radius * 0.8;
    let proposed = Value::from((proposed_val * 100.0).round() as f64 / 100.0);
    if !dedupe.should_emit(&KnobId::CampAggroRadius, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let signal = ((avg - 0.5) / 0.5).clamp(0.0, 1.0);
    let conf = confidence(evidence_ids.len(), 5, signal);

    Some(Suggestion {
        knob: KnobId::CampAggroRadius,
        character: character.to_owned(),
        current: Value::from(knobs.camp_aggro_radius as f64),
        proposed,
        rationale: format!(
            "Average adds-while-engaged per pull was {:.2} (threshold: 0.5) across {} sessions. \
             Reducing aggro radius to {:.2} (−20%) should reduce unwanted adds.",
            avg,
            evidence_ids.len(),
            proposed_val
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Rule 3: `med_break_mana_pct`
///
/// If mean mana-at-first-cast < required_for_full_opener, suggest required + 5%.
pub fn rule_med_break_mana(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(5).collect();

    let all_mana: Vec<f32> = recent
        .iter()
        .flat_map(|s| s.mana_at_first_cast.iter().copied())
        .collect();

    if all_mana.is_empty() {
        return None;
    }

    let avg = mean(&all_mana)?;
    if avg >= knobs.required_for_full_opener_pct {
        return None;
    }

    let proposed_val = (knobs.required_for_full_opener_pct + 5.0).min(100.0);
    let proposed = Value::from(proposed_val as f64);
    if !dedupe.should_emit(&KnobId::MedBreakManaPct, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let gap = knobs.required_for_full_opener_pct - avg;
    let signal = (gap / knobs.required_for_full_opener_pct.max(1.0)).clamp(0.0, 1.0);
    let conf = confidence(evidence_ids.len(), 5, signal);

    Some(Suggestion {
        knob: KnobId::MedBreakManaPct,
        character: character.to_owned(),
        current: Value::from(knobs.med_break_mana_pct as f64),
        proposed,
        rationale: format!(
            "Mean mana at first cast was {:.1}%, below the full-opener requirement of {:.1}% \
             across {} sessions. Raising med-break threshold to {:.1}% (+5%) ensures a \
             complete opener each pull.",
            avg,
            knobs.required_for_full_opener_pct,
            evidence_ids.len(),
            proposed_val
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Rule 4: `retreat_hp_pct`
///
/// Pick p95 of "HP at survived close calls" across last 10 sessions.
pub fn rule_retreat_hp(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(10).collect();

    let mut all_hp: Vec<f32> = recent
        .iter()
        .flat_map(|s| s.hp_at_close_calls.iter().copied())
        .collect();

    if all_hp.is_empty() {
        return None;
    }

    let p95 = percentile(&mut all_hp, 95.0)?;
    let proposed = Value::from((p95 * 10.0).round() as f64 / 10.0);
    if !dedupe.should_emit(&KnobId::RetreatHpPct, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let conf = confidence(evidence_ids.len(), 10, 0.8);

    Some(Suggestion {
        knob: KnobId::RetreatHpPct,
        character: character.to_owned(),
        current: Value::from(knobs.retreat_hp_pct as f64),
        proposed,
        rationale: format!(
            "p95 HP during survived close calls was {:.1}% across {} sessions. \
             Setting retreat threshold to this value captures the high-risk tail \
             without triggering on routine low-HP moments.",
            p95,
            evidence_ids.len()
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Rule 5: `combat_ability_priority`
///
/// Rank by ability contribution score; promote top quartile, demote bottom quartile.
pub fn rule_combat_ability_priority(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(5).collect();

    // Aggregate scores across sessions (sum).
    let mut totals: HashMap<String, f32> = HashMap::new();
    for s in &recent {
        for (ability, &score) in &s.ability_scores {
            *totals.entry(ability.clone()).or_insert(0.0) += score;
        }
    }

    if totals.is_empty() {
        return None;
    }

    // Sort descending by aggregate score.
    let mut ranked: Vec<(String, f32)> = totals.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let n = ranked.len();
    let quartile = (n / 4).max(1);

    let bottom: HashSet<&str> = ranked[n.saturating_sub(quartile)..]
        .iter()
        .map(|(a, _)| a.as_str())
        .collect();

    // Build proposed list: current order, promoting top, demoting bottom.
    let mut proposed_order: Vec<String> = knobs
        .combat_ability_priority
        .iter()
        .filter(|a| !bottom.contains(a.as_str()))
        .cloned()
        .collect();

    // Prepend top-quartile abilities not already at front.
    for (ability, _) in ranked[..quartile].iter().rev() {
        if let Some(pos) = proposed_order.iter().position(|a| a == ability) {
            if pos != 0 {
                proposed_order.remove(pos);
                proposed_order.insert(0, ability.clone());
            }
        } else {
            proposed_order.insert(0, ability.clone());
        }
    }

    // Append bottom-quartile abilities at end.
    for (ability, _) in &ranked[n.saturating_sub(quartile)..] {
        if !proposed_order.contains(ability) {
            proposed_order.push(ability.clone());
        }
    }

    let proposed = serde_json::to_value(&proposed_order).ok()?;
    if !dedupe.should_emit(&KnobId::CombatAbilityPriority, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let conf = confidence(evidence_ids.len(), 5, 0.9);

    let top_names: Vec<&str> = ranked[..quartile].iter().map(|(a, _)| a.as_str()).collect();
    let bot_names: Vec<&str> = ranked[n.saturating_sub(quartile)..]
        .iter()
        .map(|(a, _)| a.as_str())
        .collect();

    Some(Suggestion {
        knob: KnobId::CombatAbilityPriority,
        character: character.to_owned(),
        current: serde_json::to_value(&knobs.combat_ability_priority).ok()?,
        proposed,
        rationale: format!(
            "Across {} sessions, top-quartile abilities by contribution: [{}]. \
             Bottom-quartile (demoted): [{}]. Priority order updated accordingly.",
            evidence_ids.len(),
            top_names.join(", "),
            bot_names.join(", ")
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Rule 6: `heal_trigger_pct`
///
/// Pick p10 of "HP when heal would have landed in time but didn't" across last 5 sessions.
pub fn rule_heal_trigger(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    let recent: Vec<_> = sessions.iter().rev().take(5).collect();

    let mut all_hp: Vec<f32> = recent
        .iter()
        .flat_map(|s| s.hp_when_heal_missed.iter().copied())
        .collect();

    if all_hp.is_empty() {
        return None;
    }

    let p10 = percentile(&mut all_hp, 10.0)?;
    // Add a small buffer so we catch near-misses.
    let proposed_val = (p10 + 5.0).min(100.0);
    let proposed = Value::from((proposed_val * 10.0).round() as f64 / 10.0);
    if !dedupe.should_emit(&KnobId::HealTriggerPct, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = recent.iter().map(|s| s.session_id.clone()).collect();
    let signal = (1.0 - p10 / 100.0).clamp(0.0, 1.0);
    let conf = confidence(evidence_ids.len(), 5, signal);

    Some(Suggestion {
        knob: KnobId::HealTriggerPct,
        character: character.to_owned(),
        current: Value::from(knobs.heal_trigger_pct as f64),
        proposed,
        rationale: format!(
            "p10 HP during missed-heal windows was {:.1}% across {} sessions. \
             Setting heal trigger to {:.1}% (p10 + 5%) ensures heals fire \
             before the critical threshold is crossed.",
            p10,
            evidence_ids.len(),
            proposed_val
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

/// Suggestion detail for route waypoint changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaypointChange {
    pub cull: Vec<String>,
    pub promote: Vec<String>,
}

/// Rule 7: `route_waypoints`
///
/// Cull nodes with ≥10 traversals and 0 spawn detections.
/// Flag new candidate nodes with ≥3 high-value spawns observed nearby.
pub fn rule_route_waypoints(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Option<Suggestion> {
    if sessions.is_empty() {
        return None;
    }
    // Use all provided sessions for this rule.
    let mut traversals: HashMap<String, u32> = HashMap::new();
    let mut spawn_detections: HashMap<String, u32> = HashMap::new();
    let mut candidate_observations: HashMap<String, u32> = HashMap::new();

    for s in sessions {
        for (wp, &count) in &s.waypoint_traversals {
            *traversals.entry(wp.clone()).or_insert(0) += count;
        }
        for (wp, &count) in &s.waypoint_spawn_detections {
            *spawn_detections.entry(wp.clone()).or_insert(0) += count;
        }
        for (pos, &count) in &s.candidate_spawn_observations {
            *candidate_observations.entry(pos.clone()).or_insert(0) += count;
        }
    }

    let cull: Vec<String> = traversals
        .iter()
        .filter(|&(wp, &count)| {
            count >= 10 && spawn_detections.get(wp.as_str()).copied().unwrap_or(0) == 0
        })
        .map(|(wp, _)| wp.clone())
        .collect();

    let promote: Vec<String> = candidate_observations
        .iter()
        .filter(|&(_, &count)| count >= 3)
        .map(|(pos, _)| pos.clone())
        .collect();

    if cull.is_empty() && promote.is_empty() {
        return None;
    }

    let change = WaypointChange {
        cull: {
            let mut v = cull.clone();
            v.sort();
            v
        },
        promote: {
            let mut v = promote.clone();
            v.sort();
            v
        },
    };
    let proposed = serde_json::to_value(&change).ok()?;
    if !dedupe.should_emit(&KnobId::RouteWaypoints, character, &proposed) {
        return None;
    }

    let evidence_ids: Vec<SessionId> = sessions
        .iter()
        .map(|s| s.session_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let conf = confidence(
        evidence_ids.len(),
        evidence_ids.len().max(1),
        if cull.is_empty() { 0.6 } else { 0.85 },
    );

    let _ = knobs; // route_waypoints knob not a scalar; current state comes from sessions

    Some(Suggestion {
        knob: KnobId::RouteWaypoints,
        character: character.to_owned(),
        current: Value::Null,
        proposed,
        rationale: format!(
            "Route analysis: {} node(s) have ≥10 traversals with 0 spawn detections \
             (candidates for culling: [{}]). {} candidate position(s) observed ≥3 \
             high-value spawns (candidates for promotion: [{}]).",
            cull.len(),
            change.cull.join(", "),
            promote.len(),
            change.promote.join(", ")
        ),
        confidence: conf,
        session_evidence: evidence_ids,
    })
}

// ── Engine entry point ────────────────────────────────────────────────────────

/// Run all heuristic rules and return de-duplicated suggestions.
pub fn run_all_rules(
    character: &str,
    sessions: &[SessionAggregate],
    knobs: &CurrentKnobs,
    dedupe: &mut DedupeState,
) -> Vec<Suggestion> {
    let mut out = Vec::new();

    if let Some(s) = rule_pull_cadence(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_camp_aggro_radius(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_med_break_mana(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_retreat_hp(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_combat_ability_priority(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_heal_trigger(character, sessions, knobs, dedupe) {
        out.push(s);
    }
    if let Some(s) = rule_route_waypoints(character, sessions, knobs, dedupe) {
        out.push(s);
    }

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_knobs() -> CurrentKnobs {
        CurrentKnobs {
            pull_cadence_seconds: 10.0,
            camp_aggro_radius: 100.0,
            med_break_mana_pct: 70.0,
            retreat_hp_pct: 20.0,
            combat_ability_priority: vec![
                "ability_a".into(),
                "ability_b".into(),
                "ability_c".into(),
                "ability_d".into(),
            ],
            heal_trigger_pct: 40.0,
            required_for_full_opener_pct: 80.0,
        }
    }

    fn session(id: &str) -> SessionAggregate {
        SessionAggregate {
            session_id: id.to_owned(),
            pull_mana_pcts: vec![],
            adds_while_engaged_per_pull: vec![],
            mana_at_first_cast: vec![],
            hp_at_close_calls: vec![],
            ability_scores: HashMap::new(),
            hp_when_heal_missed: vec![],
            waypoint_traversals: HashMap::new(),
            waypoint_spawn_detections: HashMap::new(),
            candidate_spawn_observations: HashMap::new(),
        }
    }

    // ── Rule 1: pull_cadence_seconds ──────────────────────────────────────────

    #[test]
    fn pull_cadence_fires_when_mana_p10_below_threshold() {
        let mut s = session("s1");
        // All samples well below 25% → p10 will be < 25%.
        s.pull_mana_pcts = vec![5.0, 8.0, 10.0, 12.0, 15.0, 6.0, 7.0, 9.0, 11.0, 13.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let suggestion = rule_pull_cadence("Warrior", &[s], &knobs, &mut dedupe);

        let s = suggestion.expect("should fire");
        assert_eq!(s.knob, KnobId::PullCadenceSeconds);
        assert_eq!(s.proposed, Value::from(12.0_f64));
        assert!((0.0..=1.0).contains(&s.confidence));
        assert!(!s.rationale.is_empty());
    }

    #[test]
    fn pull_cadence_no_fire_when_mana_healthy() {
        let mut s = session("s1");
        s.pull_mana_pcts = vec![60.0, 70.0, 80.0, 90.0, 50.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_pull_cadence("Warrior", &[s], &knobs, &mut dedupe).is_none());
    }

    #[test]
    fn pull_cadence_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_pull_cadence("Warrior", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 2: camp_aggro_radius ─────────────────────────────────────────────

    #[test]
    fn aggro_radius_fires_when_mean_adds_above_threshold() {
        let mut s = session("s1");
        s.adds_while_engaged_per_pull = vec![1.0, 1.5, 2.0, 0.8, 1.2];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_camp_aggro_radius("Cleric", &[s], &knobs, &mut dedupe)
            .expect("should fire");

        assert_eq!(sug.knob, KnobId::CampAggroRadius);
        // proposed ≈ 100 * 0.8 = 80
        assert!(sug.proposed.as_f64().unwrap() < 100.0);
        assert!((0.0..=1.0).contains(&sug.confidence));
    }

    #[test]
    fn aggro_radius_no_fire_when_adds_low() {
        let mut s = session("s1");
        s.adds_while_engaged_per_pull = vec![0.1, 0.2, 0.3];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_camp_aggro_radius("Cleric", &[s], &knobs, &mut dedupe).is_none());
    }

    #[test]
    fn aggro_radius_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_camp_aggro_radius("Cleric", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 3: med_break_mana_pct ────────────────────────────────────────────

    #[test]
    fn med_break_fires_when_mana_at_cast_below_required() {
        let mut s = session("s1");
        // required = 80%, but character is casting at 50–60% mana.
        s.mana_at_first_cast = vec![50.0, 55.0, 60.0, 52.0, 58.0];

        let knobs = default_knobs(); // required_for_full_opener_pct = 80
        let mut dedupe = DedupeState::new();
        let sug = rule_med_break_mana("Mage", &[s], &knobs, &mut dedupe).expect("should fire");

        assert_eq!(sug.knob, KnobId::MedBreakManaPct);
        assert_eq!(sug.proposed, Value::from(85.0_f64)); // 80 + 5
    }

    #[test]
    fn med_break_no_fire_when_mana_sufficient() {
        let mut s = session("s1");
        s.mana_at_first_cast = vec![85.0, 90.0, 88.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_med_break_mana("Mage", &[s], &knobs, &mut dedupe).is_none());
    }

    #[test]
    fn med_break_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_med_break_mana("Mage", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 4: retreat_hp_pct ────────────────────────────────────────────────

    #[test]
    fn retreat_hp_fires_and_uses_p95() {
        let mut s = session("s1");
        // 10 survived close calls at various HP levels.
        s.hp_at_close_calls = vec![5.0, 8.0, 10.0, 12.0, 15.0, 6.0, 7.0, 9.0, 11.0, 18.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_retreat_hp("Warrior", &[s], &knobs, &mut dedupe).expect("should fire");

        assert_eq!(sug.knob, KnobId::RetreatHpPct);
        // p95 of sorted [5,6,7,8,9,10,11,12,15,18] → index 8 (0-based) = 15
        assert_eq!(sug.proposed, Value::from(15.0_f64));
        assert!((0.0..=1.0).contains(&sug.confidence));
    }

    #[test]
    fn retreat_hp_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_retreat_hp("Warrior", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 5: combat_ability_priority ──────────────────────────────────────

    #[test]
    fn ability_priority_fires_and_promotes_top_quartile() {
        let mut s = session("s1");
        s.ability_scores = [
            ("ability_a".to_owned(), 100.0),
            ("ability_b".to_owned(), 50.0),
            ("ability_c".to_owned(), 20.0),
            ("ability_d".to_owned(), 5.0),
        ]
        .into_iter()
        .collect();

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_combat_ability_priority("Bard", &[s], &knobs, &mut dedupe)
            .expect("should fire");

        assert_eq!(sug.knob, KnobId::CombatAbilityPriority);
        let proposed: Vec<String> = serde_json::from_value(sug.proposed).unwrap();
        // ability_a (highest) should be first.
        assert_eq!(proposed[0], "ability_a");
        assert!((0.0..=1.0).contains(&sug.confidence));
    }

    #[test]
    fn ability_priority_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_combat_ability_priority("Bard", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 6: heal_trigger_pct ──────────────────────────────────────────────

    #[test]
    fn heal_trigger_fires_and_uses_p10_plus_buffer() {
        let mut s = session("s1");
        s.hp_when_heal_missed = vec![10.0, 15.0, 20.0, 25.0, 30.0, 12.0, 18.0, 22.0, 28.0, 8.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_heal_trigger("Druid", &[s], &knobs, &mut dedupe).expect("should fire");

        assert_eq!(sug.knob, KnobId::HealTriggerPct);
        // sorted: [8,10,12,15,18,20,22,25,28,30]; p10 idx=0 → 8; proposed = 8+5 = 13
        assert_eq!(sug.proposed, Value::from(13.0_f64));
        assert!((0.0..=1.0).contains(&sug.confidence));
    }

    #[test]
    fn heal_trigger_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_heal_trigger("Druid", &[], &knobs, &mut dedupe).is_none());
    }

    // ── Rule 7: route_waypoints ───────────────────────────────────────────────

    #[test]
    fn waypoints_fires_when_cull_candidates_exist() {
        let mut s = session("s1");
        s.waypoint_traversals = [("wp_a".to_owned(), 15u32), ("wp_b".to_owned(), 5u32)]
            .into_iter()
            .collect();
        s.waypoint_spawn_detections = [("wp_b".to_owned(), 3u32)].into_iter().collect();
        // wp_a: 15 traversals, 0 detections → cull
        // wp_b: 5 traversals, 3 detections → keep

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_route_waypoints("Rogue", &[s], &knobs, &mut dedupe).expect("should fire");

        assert_eq!(sug.knob, KnobId::RouteWaypoints);
        let change: WaypointChange = serde_json::from_value(sug.proposed).unwrap();
        assert!(change.cull.contains(&"wp_a".to_owned()));
        assert!(!change.cull.contains(&"wp_b".to_owned()));
    }

    #[test]
    fn waypoints_fires_when_promote_candidates_exist() {
        let mut s = session("s1");
        s.candidate_spawn_observations = [("pos_x".to_owned(), 5u32)].into_iter().collect();

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sug = rule_route_waypoints("Rogue", &[s], &knobs, &mut dedupe).expect("should fire");

        let change: WaypointChange = serde_json::from_value(sug.proposed).unwrap();
        assert!(change.promote.contains(&"pos_x".to_owned()));
    }

    #[test]
    fn waypoints_no_fire_on_empty_evidence() {
        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_route_waypoints("Rogue", &[], &knobs, &mut dedupe).is_none());
    }

    #[test]
    fn waypoints_no_fire_when_no_cull_or_promote() {
        let mut s = session("s1");
        // Only 5 traversals (< 10) and no candidates.
        s.waypoint_traversals = [("wp_a".to_owned(), 5u32)].into_iter().collect();

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        assert!(rule_route_waypoints("Rogue", &[s], &knobs, &mut dedupe).is_none());
    }

    // ── Deduplication ─────────────────────────────────────────────────────────

    #[test]
    fn dedupe_suppresses_same_value_second_call() {
        let mut s = session("s1");
        s.pull_mana_pcts = vec![5.0, 8.0, 10.0, 6.0, 7.0];

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let sessions = vec![s];

        let first = rule_pull_cadence("Warrior", &sessions, &knobs, &mut dedupe);
        let second = rule_pull_cadence("Warrior", &sessions, &knobs, &mut dedupe);

        assert!(first.is_some(), "first call should emit");
        assert!(second.is_none(), "second call with same value should be suppressed");
    }

    #[test]
    fn dedupe_allows_changed_value() {
        let mut s1 = session("s1");
        s1.pull_mana_pcts = vec![5.0, 8.0, 10.0, 6.0, 7.0];

        let mut knobs = default_knobs();
        let mut dedupe = DedupeState::new();

        let _first = rule_pull_cadence("Warrior", &[s1], &knobs, &mut dedupe);

        // Now the knob has been updated; next suggestion should differ.
        knobs.pull_cadence_seconds = 12.0; // proposed would be 14.0 now
        let mut s2 = session("s2");
        s2.pull_mana_pcts = vec![3.0, 4.0, 5.0, 6.0, 7.0];

        let second = rule_pull_cadence("Warrior", &[s2], &knobs, &mut dedupe);
        assert!(second.is_some(), "changed proposed value should re-emit");
    }

    // ── Property: determinism ─────────────────────────────────────────────────

    #[test]
    fn rules_are_deterministic_for_fixed_input() {
        let mut s = session("s1");
        s.pull_mana_pcts = vec![5.0, 8.0, 10.0, 6.0, 7.0];
        s.adds_while_engaged_per_pull = vec![1.0, 1.5, 2.0];
        s.mana_at_first_cast = vec![50.0, 55.0, 60.0];
        s.hp_at_close_calls = vec![5.0, 8.0, 10.0];
        s.ability_scores = [("a".to_owned(), 10.0), ("b".to_owned(), 5.0)]
            .into_iter()
            .collect();
        s.hp_when_heal_missed = vec![10.0, 15.0, 20.0];
        s.waypoint_traversals = [("wp_a".to_owned(), 15u32)].into_iter().collect();

        let knobs = default_knobs();
        let sessions = vec![s];

        let run = |d: &mut DedupeState| run_all_rules("Warrior", &sessions, &knobs, d);

        let mut d1 = DedupeState::new();
        let mut d2 = DedupeState::new();

        let r1 = run(&mut d1);
        let r2 = run(&mut d2);

        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.knob, b.knob);
            assert_eq!(a.proposed, b.proposed);
            assert_eq!(a.confidence, b.confidence);
        }
    }

    // ── Confidence bounds ─────────────────────────────────────────────────────

    #[test]
    fn all_suggestions_have_confidence_in_unit_interval() {
        let mut s = session("s1");
        s.pull_mana_pcts = vec![5.0, 8.0, 10.0, 6.0, 7.0];
        s.adds_while_engaged_per_pull = vec![1.0, 1.5, 2.0];
        s.mana_at_first_cast = vec![50.0, 55.0, 60.0];
        s.hp_at_close_calls = vec![5.0, 8.0, 10.0];
        s.ability_scores = [("a".to_owned(), 10.0), ("b".to_owned(), 5.0)]
            .into_iter()
            .collect();
        s.hp_when_heal_missed = vec![10.0, 15.0, 20.0];
        s.waypoint_traversals = [("wp_a".to_owned(), 15u32)].into_iter().collect();

        let knobs = default_knobs();
        let mut dedupe = DedupeState::new();
        let suggestions = run_all_rules("Warrior", &[s], &knobs, &mut dedupe);

        for sug in &suggestions {
            assert!(
                (0.0..=1.0).contains(&sug.confidence),
                "Confidence out of bounds for {:?}: {}",
                sug.knob,
                sug.confidence
            );
        }
    }
}
