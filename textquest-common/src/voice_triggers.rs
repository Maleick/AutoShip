//! Shared voice trigger engine, queue, and operator controls.
//!
//! The engine is intentionally runtime-agnostic. It accepts synthetic game
//! snapshots, evaluates the built-in and custom trigger catalog, and emits
//! TTS-ready requests after applying debouncing, coalescing, privacy
//! redaction, and operator mute controls.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub type VoiceTriggerId = String;

const DEFAULT_DEBOUNCE_SECONDS: u64 = 30;
const P0_DEBOUNCE_SECONDS: u64 = 5;
const P2_STALE_AFTER_SECONDS: u64 = 5;
const COALESCE_WINDOW: Duration = Duration::from_millis(1_500);

/// Severity tiers mirrored from `/improve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSeverityTier {
    Emergency,
    Warning,
    Info,
    Ambient,
}

impl VoiceSeverityTier {
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Emergency => 0,
            Self::Warning => 1,
            Self::Info => 2,
            Self::Ambient => 3,
        }
    }

    #[must_use]
    pub const fn default_debounce_seconds(self) -> u64 {
        match self {
            Self::Emergency => P0_DEBOUNCE_SECONDS,
            Self::Warning | Self::Info | Self::Ambient => DEFAULT_DEBOUNCE_SECONDS,
        }
    }

    #[must_use]
    pub const fn is_mergeable(self) -> bool {
        matches!(self, Self::Warning | Self::Info)
    }

    #[must_use]
    pub const fn allowed_by_floor(self, floor: Self) -> bool {
        self.rank() <= floor.rank()
    }
}

/// Operator-selected speech roles for character-specific mute controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceRole {
    Cleric,
    Tank,
    Wizard,
    Other,
}

/// Character snapshot used by the trigger predicates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoicePartyMember {
    pub name: String,
    pub role: VoiceRole,
    pub hp_percent: Option<u8>,
    pub mana_percent: Option<u8>,
    pub heal_per_sec: Option<f64>,
    pub incoming_dps: Option<f64>,
    pub hits_taken_in_window: u8,
    pub hits_window_seconds: Option<u64>,
    pub last_ch_landed_seconds_ago: Option<u64>,
    pub buff_fades_in_seconds: Option<u64>,
}

impl Default for VoicePartyMember {
    fn default() -> Self {
        Self {
            name: String::new(),
            role: VoiceRole::Other,
            hp_percent: None,
            mana_percent: None,
            heal_per_sec: None,
            incoming_dps: None,
            hits_taken_in_window: 0,
            hits_window_seconds: None,
            last_ch_landed_seconds_ago: None,
            buff_fades_in_seconds: None,
        }
    }
}

impl VoicePartyMember {
    #[must_use]
    pub fn new(name: impl Into<String>, role: VoiceRole) -> Self {
        Self {
            name: name.into(),
            role,
            ..Self::default()
        }
    }
}

/// Combat snapshot shared across trigger evaluations.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceCombatSnapshot {
    pub incoming_dps: Option<f64>,
    pub mob_hp_percent: Option<u8>,
    pub last_ch_landed_seconds_ago: Option<u64>,
    pub combat_idle_seconds: u64,
    pub mez_break_on_non_target: bool,
    pub mez_break_target: Option<String>,
}

/// Loot snapshot shared across trigger evaluations.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceLootSnapshot {
    pub is_rare: bool,
    pub item_name: Option<String>,
}

/// XP/progression snapshot shared across trigger evaluations.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceProgressSnapshot {
    pub level_before: Option<u8>,
    pub level_after: Option<u8>,
}

/// Pull cadence snapshot shared across trigger evaluations.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoicePullSnapshot {
    pub unintended_adds_count: u8,
    pub unintended_adds_window_seconds: Option<u64>,
    pub named_respawn_in_seconds: Option<u64>,
    pub pull_interval_p50_seconds: Option<f64>,
    pub pull_interval_latest_seconds: Option<f64>,
}

/// Buff snapshot shared across trigger evaluations.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceBuffSnapshot {
    pub tank_buff_fades_in_seconds: Option<u64>,
}

/// Runtime snapshot consumed by the trigger engine.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VoiceTriggerContext {
    pub party: Vec<VoicePartyMember>,
    pub combat: VoiceCombatSnapshot,
    pub loot: VoiceLootSnapshot,
    pub progress: VoiceProgressSnapshot,
    pub pull: VoicePullSnapshot,
    pub buff: VoiceBuffSnapshot,
}

impl VoiceTriggerContext {
    #[must_use]
    pub fn party_member(&self, role: VoiceRole) -> Option<&VoicePartyMember> {
        self.party.iter().find(|member| member.role == role)
    }

    #[must_use]
    pub fn non_tank_with_hits(&self, min_hits: u8, within_seconds: u64) -> Option<&VoicePartyMember> {
        self.party
            .iter()
            .filter(|member| {
                member.role != VoiceRole::Tank
                    && member.hits_taken_in_window >= min_hits
                    && member
                        .hits_window_seconds
                        .map(|window| window <= within_seconds)
                        .unwrap_or(false)
            })
            .max_by_key(|member| member.hits_taken_in_window)
    }
}

/// Result of a predicate evaluation. The same match can provide values for
/// template rendering and an optional speaker for mute/coalescing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VoiceMatch {
    pub speaker: Option<String>,
    pub values: BTreeMap<String, String>,
}

impl VoiceMatch {
    #[must_use]
    pub fn with_speaker(speaker: impl Into<String>) -> Self {
        let speaker = speaker.into();
        let mut values = BTreeMap::new();
        values.insert("speaker".to_string(), speaker.clone());
        values.insert("character".to_string(), speaker.clone());

        Self {
            speaker: Some(speaker),
            values,
        }
    }

    #[must_use]
    pub fn with_value(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn merge(mut self, other: VoiceMatch) -> Self {
        if self.speaker.is_none() {
            self.speaker = other.speaker.clone();
        }

        if let Some(speaker) = &self.speaker {
            self.values
                .entry("speaker".to_string())
                .or_insert_with(|| speaker.clone());
            self.values
                .entry("character".to_string())
                .or_insert_with(|| speaker.clone());
        }

        for (key, value) in other.values {
            self.values.entry(key).or_insert(value);
        }

        self
    }
}

/// Predicate tree for built-in and custom triggers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[derive(Default)]
pub enum VoicePredicate {
    #[default]
    Always,
    AllOf { conditions: Vec<VoicePredicate> },
    AnyOf { conditions: Vec<VoicePredicate> },
    Not { condition: Box<VoicePredicate> },
    ClericOomImminent,
    TankChChainBreak,
    UnintendedAdds { count: u8, within_seconds: u64 },
    MezBreak,
    LowManaDps { mana_below: u8, mob_hp_above: u8 },
    RespawnWindowOpen { within_seconds: u64 },
    BuffAboutToFade { within_seconds: u64 },
    AggroSwap { hits: u8, within_seconds: u64 },
    LootNamedDrop,
    Ding { min_level_delta: u8 },
    PullCadenceDrift { multiplier: f64 },
    IdleTooLong { seconds: u64 },
}


impl VoicePredicate {
    #[must_use]
    pub fn evaluate(&self, ctx: &VoiceTriggerContext) -> Option<VoiceMatch> {
        match self {
            Self::Always => Some(VoiceMatch::default()),
            Self::AllOf { conditions } => {
                let mut merged = VoiceMatch::default();
                for condition in conditions {
                    merged = merged.merge(condition.evaluate(ctx)?);
                }
                Some(merged)
            }
            Self::AnyOf { conditions } => conditions.iter().find_map(|condition| condition.evaluate(ctx)),
            Self::Not { condition } => condition.evaluate(ctx).is_none().then_some(VoiceMatch::default()),
            Self::ClericOomImminent => {
                let cleric = ctx.party_member(VoiceRole::Cleric)?;
                let incoming_dps = ctx.combat.incoming_dps.or(cleric.incoming_dps)?;
                let heal_per_sec = cleric.heal_per_sec?;

                (cleric.mana_percent? < 15 && incoming_dps > heal_per_sec * 0.8).then(|| {
                    VoiceMatch::with_speaker(cleric.name.clone())
                })
            }
            Self::TankChChainBreak => {
                let tank = ctx.party_member(VoiceRole::Tank)?;
                let landed = tank.last_ch_landed_seconds_ago.or(ctx.combat.last_ch_landed_seconds_ago)?;

                (landed > 7 && tank.hp_percent? < 60).then(|| VoiceMatch::with_speaker(tank.name.clone()))
            }
            Self::UnintendedAdds { count, within_seconds } => {
                (ctx.pull.unintended_adds_count >= *count
                    && ctx
                        .pull
                        .unintended_adds_window_seconds
                        .map(|window| window <= *within_seconds)
                        .unwrap_or(false))
                .then_some(VoiceMatch::default())
            }
            Self::MezBreak => {
                (ctx.combat.mez_break_on_non_target && ctx.combat.mez_break_target.is_some()).then(|| {
                    let mut matched = VoiceMatch::default();
                    if let Some(target) = &ctx.combat.mez_break_target {
                        matched = matched.with_value("target", target.clone());
                    }
                    matched
                })
            }
            Self::LowManaDps {
                mana_below,
                mob_hp_above,
            } => {
                let wizard = ctx.party_member(VoiceRole::Wizard)?;
                let mana_percent = wizard.mana_percent?;
                let mob_hp = ctx.combat.mob_hp_percent?;

                (mana_percent < *mana_below && mob_hp > *mob_hp_above)
                    .then(|| VoiceMatch::with_speaker(wizard.name.clone()))
            }
            Self::RespawnWindowOpen { within_seconds } => {
                let respawn = ctx.pull.named_respawn_in_seconds?;
                (respawn <= *within_seconds).then(|| {
                    let mut matched = VoiceMatch::default();
                    matched = matched.with_value("named", "named");
                    matched
                })
            }
            Self::BuffAboutToFade { within_seconds } => {
                let tank = ctx.party_member(VoiceRole::Tank)?;
                let fade_in = tank
                    .buff_fades_in_seconds
                    .or(ctx.buff.tank_buff_fades_in_seconds)?;
                (fade_in <= *within_seconds).then(|| VoiceMatch::with_speaker(tank.name.clone()))
            }
            Self::AggroSwap { hits, within_seconds } => {
                let target = ctx.non_tank_with_hits(*hits, *within_seconds)?;
                Some(VoiceMatch::with_speaker(target.name.clone()))
            }
            Self::LootNamedDrop => {
                (ctx.loot.is_rare && ctx.loot.item_name.is_some()).then(|| {
                    let mut matched = VoiceMatch::default();
                    if let Some(item_name) = &ctx.loot.item_name {
                        matched = matched.with_value("item", item_name.clone());
                    }
                    matched
                })
            }
            Self::Ding { min_level_delta } => {
                let before = ctx.progress.level_before?;
                let after = ctx.progress.level_after?;
                let delta = after.saturating_sub(before);
                (delta >= *min_level_delta && after > before).then(|| {
                    VoiceMatch::default()
                        .with_value("level", after.to_string())
                        .with_value("level_delta", delta.to_string())
                })
            }
            Self::PullCadenceDrift { multiplier } => {
                let p50 = ctx.pull.pull_interval_p50_seconds?;
                let latest = ctx.pull.pull_interval_latest_seconds?;
                (p50 > 0.0 && latest > p50 * *multiplier).then_some(VoiceMatch::default())
            }
            Self::IdleTooLong { seconds } => {
                (ctx.combat.combat_idle_seconds >= *seconds).then_some(VoiceMatch::default())
            }
        }
    }
}

/// Configurable trigger definition used for built-ins and custom rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceTriggerDefinition {
    pub id: VoiceTriggerId,
    pub enabled: bool,
    pub severity: VoiceSeverityTier,
    pub clause_template: String,
    pub debounce_seconds: u64,
    pub duck_game_audio: bool,
    pub predicate: VoicePredicate,
}

impl Default for VoiceTriggerDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            severity: VoiceSeverityTier::Warning,
            clause_template: "voice alert.".to_string(),
            debounce_seconds: DEFAULT_DEBOUNCE_SECONDS,
            duck_game_audio: false,
            predicate: VoicePredicate::default(),
        }
    }
}

impl VoiceTriggerDefinition {
    #[must_use]
    pub fn builtin(
        id: impl Into<String>,
        severity: VoiceSeverityTier,
        clause_template: impl Into<String>,
        predicate: VoicePredicate,
    ) -> Self {
        Self {
            id: id.into(),
            enabled: true,
            severity,
            clause_template: clause_template.into(),
            debounce_seconds: severity.default_debounce_seconds(),
            duck_game_audio: matches!(severity, VoiceSeverityTier::Emergency),
            predicate,
        }
    }

    #[must_use]
    pub fn evaluate(&self, ctx: &VoiceTriggerContext) -> Option<VoiceAlert> {
        let matched = self.predicate.evaluate(ctx)?;
        let clause = render_template(&self.clause_template, &matched.values);
        let stale_after = matches!(self.severity, VoiceSeverityTier::Info)
            .then(|| Duration::from_secs(P2_STALE_AFTER_SECONDS));

        Some(VoiceAlert {
            trigger_ids: vec![self.id.clone()],
            severity: self.severity,
            speaker: matched.speaker,
            clause,
            duck_game_audio: self.duck_game_audio,
            debounce_seconds: self.debounce_seconds,
            stale_after,
        })
    }
}

/// Hot-editable operator configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceTriggerConfig {
    pub enabled: bool,
    pub master_mute: bool,
    pub severity_floor: VoiceSeverityTier,
    pub muted_characters: BTreeSet<String>,
    pub muted_triggers: BTreeSet<String>,
    pub custom_triggers: Vec<VoiceTriggerDefinition>,
}

impl Default for VoiceTriggerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            master_mute: false,
            severity_floor: VoiceSeverityTier::Ambient,
            muted_characters: BTreeSet::new(),
            muted_triggers: BTreeSet::new(),
            custom_triggers: Vec::new(),
        }
    }
}

impl VoiceTriggerConfig {
    #[must_use]
    pub fn with_custom_triggers(mut self, custom_triggers: Vec<VoiceTriggerDefinition>) -> Self {
        self.custom_triggers = custom_triggers;
        self
    }
}

/// Queueable alert before coalescing into TTS requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceAlert {
    pub trigger_ids: Vec<String>,
    pub severity: VoiceSeverityTier,
    pub speaker: Option<String>,
    pub clause: String,
    pub duck_game_audio: bool,
    pub debounce_seconds: u64,
    pub stale_after: Option<Duration>,
}

impl VoiceAlert {
    #[must_use]
    pub fn spoken_text(&self) -> String {
        match &self.speaker {
            Some(speaker) if !speaker.is_empty() => format!("{speaker} {}", self.clause),
            _ => self.clause.clone(),
        }
    }

    #[must_use]
    fn is_mergeable_with(&self, other: &Self, window: Duration) -> bool {
        self.severity.is_mergeable()
            && other.severity.is_mergeable()
            && self.speaker.is_some()
            && self.speaker == other.speaker
            && self.trigger_ids != other.trigger_ids
            && self.debounce_seconds > 0
            && other.debounce_seconds > 0
            && window >= Duration::from_millis(1)
    }

    #[must_use]
    fn merge_with(self, other: Self) -> Self {
        let mut trigger_ids = self.trigger_ids;
        trigger_ids.extend(other.trigger_ids);
        trigger_ids.sort();
        trigger_ids.dedup();

        let severity = if self.severity.rank() <= other.severity.rank() {
            self.severity
        } else {
            other.severity
        };
        let clause = format!(
            "{} and {}.",
            trim_clause(&self.clause),
            trim_clause(&other.clause)
        );

        Self {
            trigger_ids,
            severity,
            speaker: self.speaker,
            clause,
            duck_game_audio: self.duck_game_audio || other.duck_game_audio,
            debounce_seconds: self.debounce_seconds.max(other.debounce_seconds),
            stale_after: if matches!(severity, VoiceSeverityTier::Info) {
                Some(Duration::from_secs(P2_STALE_AFTER_SECONDS))
            } else {
                None
            },
        }
    }

    #[must_use]
    fn into_request(self) -> VoiceSpeechRequest {
        let VoiceAlert {
            trigger_ids,
            severity,
            speaker,
            clause,
            duck_game_audio,
            ..
        } = self;
        let spoken_text = match &speaker {
            Some(speaker) if !speaker.is_empty() => format!("{speaker} {clause}"),
            _ => clause,
        };

        VoiceSpeechRequest {
            trigger_ids,
            severity,
            speaker,
            text: redact_privacy(&spoken_text),
            duck_game_audio,
        }
    }
}

/// Final speech request emitted by the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSpeechRequest {
    pub trigger_ids: Vec<String>,
    pub severity: VoiceSeverityTier,
    pub speaker: Option<String>,
    pub text: String,
    pub duck_game_audio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueuedVoiceAlert {
    sequence: u64,
    queued_at: Instant,
    alert: VoiceAlert,
}

impl Ord for QueuedVoiceAlert {
    fn cmp(&self, other: &Self) -> Ordering {
        self.alert
            .severity
            .rank()
            .cmp(&other.alert.severity.rank())
            .then_with(|| other.queued_at.cmp(&self.queued_at))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for QueuedVoiceAlert {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Merges adjacent same-character P1/P2 alerts within a 1.5s window.
#[derive(Debug, Clone)]
pub struct TtsCoalescer {
    window: Duration,
}

impl Default for TtsCoalescer {
    fn default() -> Self {
        Self {
            window: COALESCE_WINDOW,
        }
    }
}

impl TtsCoalescer {
    #[must_use]
    fn coalesce(&self, alerts: Vec<QueuedVoiceAlert>, now: Instant) -> Vec<VoiceSpeechRequest> {
        let mut merged = Vec::new();
        let mut current: Option<QueuedVoiceAlert> = None;

        for queued in alerts {
            if let Some(existing) = current.take() {
                if self.can_merge(&existing, &queued, now) {
                    current = Some(QueuedVoiceAlert {
                        sequence: existing.sequence.min(queued.sequence),
                        queued_at: existing.queued_at,
                        alert: existing.alert.merge_with(queued.alert),
                    });
                } else {
                    merged.push(existing.alert.into_request());
                    current = Some(queued);
                }
            } else {
                current = Some(queued);
            }
        }

        if let Some(existing) = current {
            merged.push(existing.alert.into_request());
        }

        merged
    }

    #[must_use]
    fn can_merge(&self, left: &QueuedVoiceAlert, right: &QueuedVoiceAlert, _now: Instant) -> bool {
        left.alert.is_mergeable_with(&right.alert, self.window)
            && right
                .queued_at
                .saturating_duration_since(left.queued_at)
                <= self.window
    }
}

/// Queue that applies debouncing and priority ordering.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct VoiceQueue {
    heap: BinaryHeap<QueuedVoiceAlert>,
    debouncer: HashMap<VoiceTriggerId, Instant>,
    coalescer: TtsCoalescer,
    sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceQueueOutcome {
    Enqueued,
    Debounced,
    DroppedAmbientBusy,
}


impl VoiceQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn clear(&mut self) {
        self.heap.clear();
    }

    #[must_use]
    pub fn enqueue(&mut self, alert: VoiceAlert, now: Instant) -> VoiceQueueOutcome {
        if matches!(alert.severity, VoiceSeverityTier::Ambient) && !self.heap.is_empty() {
            return VoiceQueueOutcome::DroppedAmbientBusy;
        }

        let trigger_id = alert
            .trigger_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown-trigger".to_string());

        if let Some(last_seen) = self.debouncer.get(&trigger_id) {
            let debounce = Duration::from_secs(alert.debounce_seconds);
            if now.saturating_duration_since(*last_seen) < debounce {
                return VoiceQueueOutcome::Debounced;
            }
        }

        self.debouncer.insert(trigger_id, now);
        self.sequence = self.sequence.saturating_add(1);
        self.heap.push(QueuedVoiceAlert {
            sequence: self.sequence,
            queued_at: now,
            alert,
        });

        VoiceQueueOutcome::Enqueued
    }

    #[must_use]
    pub fn drain(&mut self, now: Instant) -> Vec<VoiceSpeechRequest> {
        let mut drained = Vec::new();
        while let Some(queued) = self.heap.pop() {
            if self.is_stale(&queued, now) {
                continue;
            }
            drained.push(queued);
        }

        self.coalescer.coalesce(drained, now)
    }

    fn is_stale(&self, queued: &QueuedVoiceAlert, now: Instant) -> bool {
        queued
            .alert
            .stale_after
            .map(|stale_after| now.saturating_duration_since(queued.queued_at) > stale_after)
            .unwrap_or(false)
    }
}

/// Runtime engine that evaluates snapshots into queued speech requests.
#[derive(Debug, Clone)]
pub struct VoiceTriggerEngine {
    builtins: Vec<VoiceTriggerDefinition>,
    config: VoiceTriggerConfig,
    queue: VoiceQueue,
}

impl Default for VoiceTriggerEngine {
    fn default() -> Self {
        Self::new(VoiceTriggerConfig::default())
    }
}

impl VoiceTriggerEngine {
    #[must_use]
    pub fn new(config: VoiceTriggerConfig) -> Self {
        Self {
            builtins: builtin_voice_triggers(),
            config,
            queue: VoiceQueue::default(),
        }
    }

    #[must_use]
    pub fn config(&self) -> &VoiceTriggerConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: VoiceTriggerConfig) {
        self.config = config;
        if !self.config.enabled || self.config.master_mute {
            self.queue.clear();
        }
    }

    #[must_use]
    pub fn process_snapshot(
        &mut self,
        ctx: &VoiceTriggerContext,
        now: Instant,
    ) -> Vec<VoiceSpeechRequest> {
        if !self.config.enabled || self.config.master_mute {
            self.queue.clear();
            return Vec::new();
        }

        let mut candidates = Vec::new();
        let mut seen_ids = BTreeSet::new();

        for trigger in self.builtins.iter().chain(self.config.custom_triggers.iter()) {
            if !seen_ids.insert(trigger.id.clone()) {
                continue;
            }

            if !trigger.enabled || self.config.muted_triggers.contains(&trigger.id) {
                continue;
            }

            if let Some(alert) = trigger.evaluate(ctx) {
                candidates.push(alert);
            }
        }

        candidates.sort_by_key(|alert| alert.severity.rank());

        for alert in candidates {
            if !alert.severity.allowed_by_floor(self.config.severity_floor) {
                continue;
            }

            if alert
                .speaker
                .as_ref()
                .map(|speaker| self.config.muted_characters.iter().any(|muted| muted.eq_ignore_ascii_case(speaker)))
                .unwrap_or(false)
            {
                continue;
            }

            let _ = self.queue.enqueue(alert, now);
        }

        self.queue.drain(now)
    }
}

fn builtin_voice_triggers() -> Vec<VoiceTriggerDefinition> {
    vec![
        VoiceTriggerDefinition::builtin(
            "cleric_oom_imminent",
            VoiceSeverityTier::Emergency,
            "oom in ten seconds.",
            VoicePredicate::ClericOomImminent,
        ),
        VoiceTriggerDefinition::builtin(
            "tank_ch_chain_break",
            VoiceSeverityTier::Emergency,
            "CH chain broke. Manual heal now.",
            VoicePredicate::TankChChainBreak,
        ),
        VoiceTriggerDefinition::builtin(
            "unintended_adds",
            VoiceSeverityTier::Emergency,
            "Two adds, you pulled the wall.",
            VoicePredicate::UnintendedAdds {
                count: 1,
                within_seconds: 3,
            },
        ),
        VoiceTriggerDefinition::builtin(
            "mez_break",
            VoiceSeverityTier::Warning,
            "Mez broke on {target}.",
            VoicePredicate::MezBreak,
        ),
        VoiceTriggerDefinition::builtin(
            "low_mana_dps",
            VoiceSeverityTier::Warning,
            "low mana, pace it.",
            VoicePredicate::LowManaDps {
                mana_below: 25,
                mob_hp_above: 40,
            },
        ),
        VoiceTriggerDefinition::builtin(
            "respawn_window_open",
            VoiceSeverityTier::Warning,
            "Cazic pop window opens in thirty.",
            VoicePredicate::RespawnWindowOpen { within_seconds: 30 },
        ),
        VoiceTriggerDefinition::builtin(
            "buff_about_to_fade",
            VoiceSeverityTier::Warning,
            "HP buff drops in a minute.",
            VoicePredicate::BuffAboutToFade { within_seconds: 60 },
        ),
        VoiceTriggerDefinition::builtin(
            "aggro_swap",
            VoiceSeverityTier::Warning,
            "you're tanking.",
            VoicePredicate::AggroSwap {
                hits: 3,
                within_seconds: 5,
            },
        ),
        VoiceTriggerDefinition::builtin(
            "loot_named_drop",
            VoiceSeverityTier::Info,
            "{item} dropped.",
            VoicePredicate::LootNamedDrop,
        ),
        VoiceTriggerDefinition::builtin(
            "ding",
            VoiceSeverityTier::Info,
            "Ding, {level}.",
            VoicePredicate::Ding { min_level_delta: 1 },
        ),
        VoiceTriggerDefinition::builtin(
            "pull_cadence_drift",
            VoiceSeverityTier::Info,
            "You're slowing down.",
            VoicePredicate::PullCadenceDrift { multiplier: 1.3 },
        ),
        VoiceTriggerDefinition::builtin(
            "idle_too_long",
            VoiceSeverityTier::Ambient,
            "Still no combat.",
            VoicePredicate::IdleTooLong { seconds: 90 },
        ),
    ]
}

fn render_template(template: &str, values: &BTreeMap<String, String>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

fn redact_privacy(text: &str) -> String {
    static TELL_RE: OnceLock<Regex> = OnceLock::new();
    let re = TELL_RE.get_or_init(|| {
        Regex::new(r"\[tell:[^\]]+\]").expect("voice privacy redaction regex must be valid")
    });

    re.replace_all(text, "[tell:redacted]").into_owned()
}

fn trim_clause(text: &str) -> String {
    text.trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_context() -> VoiceTriggerContext {
        let mut ctx = VoiceTriggerContext::default();
        ctx.party.push({
            let mut cleric = VoicePartyMember::new("Cleric", VoiceRole::Cleric);
            cleric.hp_percent = Some(78);
            cleric.mana_percent = Some(12);
            cleric.heal_per_sec = Some(120.0);
            cleric.incoming_dps = Some(100.0);
            cleric
        });
        ctx.party.push({
            let mut tank = VoicePartyMember::new("Tank", VoiceRole::Tank);
            tank.hp_percent = Some(58);
            tank.last_ch_landed_seconds_ago = Some(8);
            tank.buff_fades_in_seconds = Some(60);
            tank
        });
        ctx.party.push({
            let mut wizard = VoicePartyMember::new("Wizard", VoiceRole::Wizard);
            wizard.hp_percent = Some(82);
            wizard.mana_percent = Some(20);
            wizard.hits_taken_in_window = 3;
            wizard.hits_window_seconds = Some(5);
            wizard
        });
        ctx.combat.incoming_dps = Some(100.0);
        ctx.combat.mob_hp_percent = Some(55);
        ctx.combat.last_ch_landed_seconds_ago = Some(8);
        ctx.combat.combat_idle_seconds = 90;
        ctx.combat.mez_break_on_non_target = true;
        ctx.combat.mez_break_target = Some("Enchanter's add".to_string());
        ctx.loot.is_rare = true;
        ctx.loot.item_name = Some("Helm of Narandi".to_string());
        ctx.progress.level_before = Some(51);
        ctx.progress.level_after = Some(52);
        ctx.pull.unintended_adds_count = 2;
        ctx.pull.unintended_adds_window_seconds = Some(3);
        ctx.pull.named_respawn_in_seconds = Some(30);
        ctx.pull.pull_interval_p50_seconds = Some(20.0);
        ctx.pull.pull_interval_latest_seconds = Some(27.0);
        ctx
    }

    #[test]
    fn all_builtin_triggers_fire_on_synthetic_snapshot_streams() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            ..VoiceTriggerConfig::default()
        });

        let ctx = base_context();
        let fired = engine.process_snapshot(&ctx, Instant::now());
        let ids: BTreeSet<_> = fired
            .iter()
            .flat_map(|request| request.trigger_ids.iter().cloned())
            .collect();

        for expected in [
            "cleric_oom_imminent",
            "tank_ch_chain_break",
            "unintended_adds",
            "mez_break",
            "low_mana_dps",
            "respawn_window_open",
            "buff_about_to_fade",
            "aggro_swap",
            "loot_named_drop",
            "ding",
            "pull_cadence_drift",
            "idle_too_long",
        ] {
            assert!(ids.contains(expected), "missing builtin trigger {expected}");
        }
    }

    #[test]
    fn debouncer_blocks_repeat_fires_for_the_default_window() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            ..VoiceTriggerConfig::default()
        });
        let ctx = base_context();
        let now = Instant::now();

        let first = engine.process_snapshot(&ctx, now);
        assert!(!first.is_empty());

        let second = engine.process_snapshot(&ctx, now + Duration::from_secs(29));
        assert!(second.is_empty(), "repeat should be debounced before 30s");

        let third = engine.process_snapshot(&ctx, now + Duration::from_secs(31));
        assert!(!third.is_empty(), "repeat should be allowed after 30s");
    }

    #[test]
    fn p0_repeat_fires_after_the_short_debounce_window() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            ..VoiceTriggerConfig::default()
        });
        let mut ctx = VoiceTriggerContext::default();
        ctx.party.push({
            let mut cleric = VoicePartyMember::new("Cleric", VoiceRole::Cleric);
            cleric.mana_percent = Some(10);
            cleric.heal_per_sec = Some(120.0);
            cleric.incoming_dps = Some(140.0);
            cleric
        });
        ctx.combat.incoming_dps = Some(140.0);

        let now = Instant::now();
        let first = engine.process_snapshot(&ctx, now);
        assert!(!first.is_empty());

        let second = engine.process_snapshot(&ctx, now + Duration::from_secs(4));
        assert!(second.is_empty(), "P0 repeat should be debounced for 5 seconds");

        let third = engine.process_snapshot(&ctx, now + Duration::from_secs(5));
        assert!(!third.is_empty(), "P0 repeat should fire again after 5 seconds");
    }

    #[test]
    fn coalescer_merges_adjacent_same_character_warning_alerts() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            ..VoiceTriggerConfig::default()
        });
        let mut ctx = VoiceTriggerContext::default();
        let mut wizard = VoicePartyMember::new("Wizard", VoiceRole::Wizard);
        wizard.mana_percent = Some(20);
        wizard.hits_taken_in_window = 3;
        wizard.hits_window_seconds = Some(5);
        ctx.party.push(wizard);
        ctx.combat.mob_hp_percent = Some(60);

        let fired = engine.process_snapshot(&ctx, Instant::now());
        assert_eq!(fired.len(), 1, "same-character P1 alerts should coalesce");
        assert_eq!(fired[0].speaker.as_deref(), Some("Wizard"));
        assert!(fired[0].text.contains("low mana"));
        assert!(fired[0].text.contains("you're tanking"));
    }

    #[test]
    fn p0_alerts_preempt_the_queue_and_duck_game_audio() {
        let mut queue = VoiceQueue::default();
        let now = Instant::now();
        let _ = queue.enqueue(
            VoiceAlert {
                trigger_ids: vec!["low_mana_dps".to_string()],
                severity: VoiceSeverityTier::Warning,
                speaker: Some("Wizard".to_string()),
                clause: "low mana, pace it.".to_string(),
                duck_game_audio: false,
                debounce_seconds: DEFAULT_DEBOUNCE_SECONDS,
                stale_after: None,
            },
            now,
        );
        let _ = queue.enqueue(
            VoiceAlert {
                trigger_ids: vec!["cleric_oom_imminent".to_string()],
                severity: VoiceSeverityTier::Emergency,
                speaker: Some("Cleric".to_string()),
                clause: "oom in ten seconds.".to_string(),
                duck_game_audio: true,
                debounce_seconds: P0_DEBOUNCE_SECONDS,
                stale_after: None,
            },
            now + Duration::from_millis(1),
        );

        let drained = queue.drain(now + Duration::from_millis(2));
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].severity, VoiceSeverityTier::Emergency);
        assert!(drained[0].duck_game_audio);
    }

    #[test]
    fn operator_controls_support_character_trigger_master_mute_and_floor() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            severity_floor: VoiceSeverityTier::Warning,
            muted_characters: BTreeSet::from(["Wizard".to_string()]),
            muted_triggers: BTreeSet::from(["pull_cadence_drift".to_string()]),
            ..VoiceTriggerConfig::default()
        });
        let ctx = base_context();
        let fired = engine.process_snapshot(&ctx, Instant::now());

        assert!(fired.iter().all(|request| request.severity <= VoiceSeverityTier::Warning));
        assert!(!fired.iter().any(|request| request.speaker.as_deref() == Some("Wizard")));
        assert!(!fired.iter().any(|request| request.trigger_ids.iter().any(|id| id == "pull_cadence_drift")));

        engine.update_config(VoiceTriggerConfig {
            enabled: true,
            master_mute: true,
            ..VoiceTriggerConfig::default()
        });
        assert!(engine.process_snapshot(&ctx, Instant::now()).is_empty());
    }

    #[test]
    fn privacy_redaction_never_speaks_tell_hashes() {
        let text = redact_privacy("Wizard [tell:deadbeef] low mana.");
        assert!(!text.contains("[tell:deadbeef]"));
        assert!(text.contains("[tell:redacted]"));
    }

    #[test]
    fn custom_triggers_fire_without_code_changes() {
        let mut engine = VoiceTriggerEngine::new(VoiceTriggerConfig {
            enabled: true,
            muted_triggers: BTreeSet::from(["low_mana_dps".to_string()]),
            custom_triggers: vec![VoiceTriggerDefinition {
                id: "custom_pace_it".to_string(),
                enabled: true,
                severity: VoiceSeverityTier::Warning,
                clause_template: "custom pace it.".to_string(),
                debounce_seconds: DEFAULT_DEBOUNCE_SECONDS,
                duck_game_audio: false,
                predicate: VoicePredicate::LowManaDps {
                    mana_below: 25,
                    mob_hp_above: 40,
                },
            }],
            ..VoiceTriggerConfig::default()
        });

        let mut ctx = VoiceTriggerContext::default();
        let mut wizard = VoicePartyMember::new("Wizard", VoiceRole::Wizard);
        wizard.mana_percent = Some(20);
        ctx.party.push(wizard);
        ctx.combat.mob_hp_percent = Some(60);

        let fired = engine.process_snapshot(&ctx, Instant::now());
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].trigger_ids, vec!["custom_pace_it".to_string()]);
        assert!(fired[0].text.contains("custom pace it"));
    }
}
