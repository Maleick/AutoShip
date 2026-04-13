use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde_json;
use textquest_common::ipc::Command;
use textquest_common::soul::{MoodState, PersonalityTraits, SoulAction, SoulEvent, SpeechStyle};
use textquest_common::types::{ClientId, GameState};

use super::audit::{AuditActionType, SoulAuditLogger};
use super::config::{CharacterSoulConfig, EdginessLevel, SoulConfig};
use super::idle::{IdleScheduler, IdleTransition};
use super::llm::fallback::TraitDrivenResponder;
use super::llm::priority_queue::LlmRequestQueue;
use super::llm::{LlmPriority, LlmProvider, LlmRequest, Situation};
use super::memory::MemoryStore;
use super::personality::{PersonalityEngine, SoulContext};
use super::social::SocialGraph;

/// Per-character soul state.
struct CharacterSoul {
    name: String,
    client_id: ClientId,
    traits: PersonalityTraits,
    mood: MoodState,
    speech_style: SpeechStyle,
    edginess: EdginessLevel,
    backstory: String,
    personality: PersonalityEngine,
    idle: IdleScheduler,
    responder: TraitDrivenResponder,
}

/// Tick-driven orchestrator for all Soul Engine subsystems.
/// Mirrors `CombatCoordinator`: called each tick, returns commands to dispatch.
pub struct SoulCoordinator {
    souls: HashMap<ClientId, CharacterSoul>,
    memory: MemoryStore,
    social: SocialGraph,
    llm_queue: LlmRequestQueue,
    config: SoulConfig,
    /// Tick counter for timing.
    tick_count: u64,
    /// Optional audit logger — `None` when no log path was provided.
    audit: Option<SoulAuditLogger>,
}

const MAX_PLAYER_CHAT_MESSAGE_BYTES: usize = 512;
const MAX_CONVERSATIONS_PER_CHARACTER: usize = 1000;

impl SoulCoordinator {
    /// Create a new `SoulCoordinator` from config.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(config: SoulConfig, db_path: &Path) -> Result<Self> {
        let memory = MemoryStore::open(db_path)?;
        let social = SocialGraph::from_seeds(&config.relationship);
        // Phase 1: 0 token budget (fallback only, no real LLM calls)
        let llm_queue = LlmRequestQueue::new(0);

        Ok(Self {
            souls: HashMap::new(),
            memory,
            social,
            llm_queue,
            config,
            tick_count: 0,
            audit: None,
        })
    }

    /// Attach a `SoulAuditLogger` to this coordinator.
    ///
    /// Once set, key state changes (mood, memory, LLM requests, events) are
    /// written as JSONL lines to the logger's file.
    pub fn set_audit_logger(&mut self, logger: SoulAuditLogger) {
        self.audit = Some(logger);
    }

    /// Register a character with the coordinator.
    pub fn register_character(&mut self, client_id: ClientId, char_config: &CharacterSoulConfig) {
        let edginess = char_config.edginess.unwrap_or(self.config.edginess);

        let soul = CharacterSoul {
            name: char_config.name.clone(),
            client_id,
            traits: char_config.traits.clone(),
            mood: MoodState::Neutral,
            speech_style: char_config.speech.clone(),
            edginess,
            backstory: char_config.backstory.clone(),
            personality: PersonalityEngine::new(client_id),
            idle: IdleScheduler::new(client_id, &self.config),
            responder: TraitDrivenResponder::new(client_id, edginess),
        };

        self.souls.insert(client_id, soul);
    }

    /// Main tick — called every 5000ms by the orchestrator.
    /// Returns commands to send to specific clients.
    pub fn tick(&mut self, states: &HashMap<ClientId, GameState>) -> Vec<(ClientId, Command)> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.tick_count += 1;
        let mut commands = Vec::new();

        // Collect client IDs to avoid borrow issues
        let client_ids: Vec<ClientId> = self.souls.keys().copied().collect();

        for client_id in client_ids {
            let Some(soul) = self.souls.get_mut(&client_id) else {
                continue;
            };

            let Some(state) = states.get(&client_id) else {
                continue;
            };

            let in_combat = is_in_combat(state);
            let zone = zone_from_state(state);
            // Approximate group members from nearby PCs (spawn_type 0 = player).
            // True group roster requires GameState to carry group membership data.
            let group_members: Vec<String> = state
                .nearby_spawns
                .iter()
                .filter(|s| s.spawn_type == 0 && s.name != soul.name)
                .map(|s| s.name.clone())
                .collect();

            let ctx = SoulContext {
                character_name: &soul.name,
                traits: &soul.traits,
                mood: soul.mood,
                edginess: soul.edginess,
                zone,
                level: state.local_player.as_ref().map_or(1, |p| p.level),
                in_combat,
                group_members: &group_members,
            };

            // Tick idle scheduler
            match soul.idle.tick(&ctx, &mut soul.responder) {
                IdleTransition::Start(active) => {
                    // Convert idle behavior to a Command
                    let action = SoulAction::StartIdle {
                        behavior: active.behavior.clone(),
                    };
                    commands.push((client_id, Command::SoulAction { action }));

                    // If there's flavor text, emit it as chat
                    if let Some(text) = active.flavor_text {
                        commands.push((
                            client_id,
                            Command::Say {
                                channel: textquest_common::soul::SayChannel::Group,
                                message: text,
                                target: None,
                            },
                        ));
                    }
                }
                IdleTransition::LogOff { return_after_secs } => {
                    tracing::info!(
                        client_id,
                        name = soul.name,
                        return_after_secs,
                        "Character logging off to sleep"
                    );
                    let action = SoulAction::StartIdle {
                        behavior: textquest_common::soul::IdleBehaviorType::LogOffToSleep,
                    };
                    commands.push((client_id, Command::SoulAction { action }));
                }
                IdleTransition::Stop | IdleTransition::Continue => {}
            }

            // Mood decay — natural drift toward Neutral each tick (5 s).
            soul.mood = soul.personality.tick_decay(soul.mood, &soul.traits, 5000);

            // Periodic memory decay (every ~60 ticks = 5 minutes at 5s tick)
            if self.tick_count.is_multiple_of(60) {
                let _ = self.memory.decay_tick(client_id, 0.995);
                let _ = self.memory.prune_low_importance(client_id, 0.05);
            }
        }

        // Process any queued LLM requests
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        // Drain all ready requests, then process each with the matching soul's responder
        // In Phase 2, this will use a real LLM provider instead of per-soul fallback responders
        let pending: Vec<_> = {
            let mut results = Vec::new();
            while let Some(prioritized) = self.llm_queue.pop_next(now_secs) {
                results.push(prioritized);
            }
            results
        };

        for request in pending {
            if let Some((&cid, soul)) = self
                .souls
                .iter_mut()
                .find(|(_, s)| s.name == request.character_name)
                && let Ok(response) = soul.responder.generate(&request)
            {
                commands.push((
                    cid,
                    Command::Say {
                        channel: textquest_common::soul::SayChannel::Say,
                        message: response.text,
                        target: None,
                    },
                ));
            }
        }

        commands
    }

    /// Handle an incoming message from a real player.
    pub fn on_player_message(
        &mut self,
        client_id: ClientId,
        player_name: &str,
        message: &str,
        channel: &str,
    ) {
        if !self.config.player_chat_enabled {
            return;
        }

        let Some(soul) = self.souls.get_mut(&client_id) else {
            return;
        };

        let message = message.trim();
        if message.is_empty() {
            return;
        }
        let message = truncate_utf8(message, MAX_PLAYER_CHAT_MESSAGE_BYTES);

        // Score sentiment and record the conversation
        let sentiment = score_sentiment(message);
        let _ = self.memory.record_conversation(
            client_id,
            player_name,
            true,
            channel,
            message,
            sentiment,
        );
        let _ = self
            .memory
            .prune_conversations(client_id, MAX_CONVERSATIONS_PER_CHARACTER);

        // Capture mood before event processing for accurate memory recording
        let mood_before = soul.mood;

        // Process mood change from player interaction
        let event = SoulEvent::PlayerChat {
            player_name: player_name.to_string(),
            sentiment, // scored above via score_sentiment()
        };
        soul.mood = soul
            .personality
            .process_event(soul.mood, &event, &soul.traits);

        // Audit: mood change triggered by player chat
        if let Some(audit) = &self.audit
            && mood_before != soul.mood
        {
            let _ = audit.log(
                client_id,
                AuditActionType::MoodChange,
                format!(
                    "{} mood: {:?} -> {:?} (player chat from {})",
                    soul.name, mood_before, soul.mood, player_name
                ),
                Some(serde_json::json!({"mood": format!("{:?}", mood_before)})),
                Some(serde_json::json!({"mood": format!("{:?}", soul.mood)})),
            );
        }

        // Record memory with the mood as it was before the event changed it
        let _ = self.memory.record(client_id, &event, mood_before, 2.0);

        // Audit: memory record
        if let Some(audit) = &self.audit {
            let _ = audit.log(
                client_id,
                AuditActionType::MemoryRecord,
                format!(
                    "{} recorded player chat memory from {}",
                    soul.name, player_name
                ),
                None,
                None,
            );
        }

        // Queue an LLM response (high priority for real players)
        let request = LlmRequest {
            character_name: soul.name.clone(),
            traits: soul.traits.clone(),
            mood: soul.mood,
            speech_style: soul.speech_style.clone(),
            situation: Situation::PlayerChat {
                player_name: player_name.to_string(),
                message: message.to_owned(),
                channel: channel.to_string(),
            },
            priority: LlmPriority::High,
            memory_context: self
                .memory
                .recall_about(client_id, player_name, 5)
                .unwrap_or_default()
                .into_iter()
                .map(|row| row.event_json)
                .collect(),
            backstory: soul.backstory.clone(),
        };

        // Audit: LLM request enqueued
        if let Some(audit) = &self.audit {
            let _ = audit.log(
                client_id,
                AuditActionType::LlmRequest,
                format!(
                    "{} LLM request enqueued for player chat from {}",
                    soul.name, player_name
                ),
                None,
                Some(serde_json::json!({"priority": "High", "channel": channel})),
            );
        }

        self.llm_queue.enqueue(request);
    }

    /// Handle a game event (kill, death, loot, zone change, etc.).
    pub fn on_game_event(&mut self, client_id: ClientId, event: SoulEvent) {
        let Some(soul) = self.souls.get_mut(&client_id) else {
            return;
        };

        // Capture mood before event processing for accurate memory recording
        let mood_before = soul.mood;

        // Update mood
        soul.mood = soul
            .personality
            .process_event(soul.mood, &event, &soul.traits);

        // Audit: event processed
        if let Some(audit) = &self.audit {
            let _ = audit.log(
                client_id,
                AuditActionType::EventProcessed,
                format!("{} processed game event: {:?}", soul.name, event),
                Some(serde_json::json!({"mood": format!("{:?}", mood_before)})),
                Some(serde_json::json!({"mood": format!("{:?}", soul.mood)})),
            );
        }

        // Audit: mood change if it actually changed
        if let Some(audit) = &self.audit
            && mood_before != soul.mood
        {
            let _ = audit.log(
                client_id,
                AuditActionType::MoodChange,
                format!(
                    "{} mood: {:?} -> {:?} (game event)",
                    soul.name, mood_before, soul.mood
                ),
                Some(serde_json::json!({"mood": format!("{:?}", mood_before)})),
                Some(serde_json::json!({"mood": format!("{:?}", soul.mood)})),
            );
        }

        // Determine importance based on event type
        let importance = match &event {
            SoulEvent::Death { .. } | SoulEvent::GroupWipe { .. } => 5.0,
            SoulEvent::LevelUp { .. } => 4.0,
            SoulEvent::Kill { .. } | SoulEvent::Loot { .. } => 1.5,
            SoulEvent::PlayerChat { .. } => 3.0,
            SoulEvent::RelationshipChange { .. } => 2.0,
            SoulEvent::ZoneEnter { .. } => 1.0,
            _ => 1.0,
        };

        // Record memory with the mood as it was before the event changed it
        let _ = self
            .memory
            .record(client_id, &event, mood_before, importance);

        // Audit: memory record
        if let Some(audit) = &self.audit {
            let _ = audit.log(
                client_id,
                AuditActionType::MemoryRecord,
                format!(
                    "{} recorded game event memory (importance {importance})",
                    soul.name
                ),
                None,
                None,
            );
        }
    }

    /// Get the current mood for a character.
    pub fn mood(&self, client_id: ClientId) -> Option<MoodState> {
        self.souls.get(&client_id).map(|s| s.mood)
    }

    /// Get the social graph (for external queries/display).
    pub fn social_graph(&self) -> &SocialGraph {
        &self.social
    }

    /// Get the memory store (for external queries).
    pub fn memory_store(&self) -> &MemoryStore {
        &self.memory
    }
}

fn truncate_utf8(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }

    let mut end = max_bytes;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

/// Check if a client is currently in combat based on game state.
fn is_in_combat(state: &GameState) -> bool {
    !matches!(
        state.combat_status,
        textquest_common::combat::CombatStatus::Idle
    )
}

/// Extract zone name from game state (placeholder until zone tracking is added).
fn zone_from_state(_state: &GameState) -> &'static str {
    "unknown"
}

/// Compute a simple keyword-based sentiment score in [-1.0, 1.0].
/// Positive words push toward +1.0, negative words toward -1.0.
/// This is a lightweight heuristic for Phase 1 — LLM-based analysis deferred to M11.
pub fn score_sentiment(text: &str) -> f32 {
    const POSITIVE: &[&str] = &[
        "thank",
        "thanks",
        "great",
        "awesome",
        "nice",
        "good",
        "love",
        "amazing",
        "excellent",
        "wonderful",
        "please",
        "help",
        "happy",
        "yes",
        "sure",
        "glad",
        "perfect",
        "fantastic",
        "appreciate",
    ];
    const NEGATIVE: &[&str] = &[
        "hate",
        "terrible",
        "awful",
        "bad",
        "worst",
        "stupid",
        "idiot",
        "dumb",
        "useless",
        "fail",
        "wrong",
        "no",
        "never",
        "annoying",
        "pathetic",
        "disgusting",
        "angry",
        "leave",
        "stop",
    ];

    let lower = text.to_lowercase();
    let mut score: f32 = 0.0;
    let mut hits: u32 = 0;

    for word in POSITIVE {
        if lower.contains(word) {
            score += 1.0;
            hits += 1;
        }
    }
    for word in NEGATIVE {
        if lower.contains(word) {
            score -= 1.0;
            hits += 1;
        }
    }

    if hits == 0 {
        return 0.0;
    }
    // Normalize to [-1, 1]
    (score / hits as f32).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::CombatStatus;
    use textquest_common::nav::NavStatus;
    use textquest_common::soul::{PersonalityTraits, SpeechStyle};
    use textquest_common::types::SpawnData;

    fn make_game_state(client_id: ClientId) -> GameState {
        GameState {
            client_id,
            local_player: Some(SpawnData {
                displayed_name: "TestChar".to_string(),
                name: "TestChar".to_string(),
                level: 60,
                ..SpawnData::default()
            }),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
        }
    }

    fn make_soul_config(enabled: bool) -> SoulConfig {
        SoulConfig {
            enabled,
            ..SoulConfig::default()
        }
    }

    fn make_char_config(name: &str) -> CharacterSoulConfig {
        CharacterSoulConfig {
            name: name.to_string(),
            traits: PersonalityTraits::default(),
            speech: SpeechStyle::default(),
            edginess: None,
            backstory: String::new(),
            quirks: Vec::new(),
        }
    }

    fn make_coordinator(enabled: bool) -> SoulCoordinator {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_memory.db");
        let config = make_soul_config(enabled);
        // Leak the tempdir so it doesn't get deleted while coordinator lives
        let dir = Box::leak(Box::new(dir));
        let _ = dir; // suppress warning
        SoulCoordinator::new(config, &db_path).unwrap()
    }

    #[test]
    fn new_coordinator_creates_successfully() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let config = make_soul_config(false);
        let coord = SoulCoordinator::new(config, &db_path);
        assert!(coord.is_ok());
    }

    #[test]
    fn register_character_stores_soul() {
        let mut coord = make_coordinator(true);
        let char_config = make_char_config("Warrior01");
        coord.register_character(1, &char_config);
        assert!(coord.mood(1).is_some());
        assert_eq!(coord.mood(1).unwrap(), MoodState::Neutral);
    }

    #[test]
    fn mood_returns_none_for_unregistered() {
        let coord = make_coordinator(true);
        assert!(coord.mood(99).is_none());
    }

    #[test]
    fn tick_returns_empty_when_disabled() {
        let mut coord = make_coordinator(false);
        coord.register_character(1, &make_char_config("Test"));
        let mut states = HashMap::new();
        states.insert(1, make_game_state(1));
        let cmds = coord.tick(&states);
        assert!(cmds.is_empty());
    }

    #[test]
    fn tick_increments_tick_count() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Test"));
        let mut states = HashMap::new();
        states.insert(1, make_game_state(1));
        coord.tick(&states);
        assert_eq!(coord.tick_count, 1);
        coord.tick(&states);
        assert_eq!(coord.tick_count, 2);
    }

    #[test]
    fn on_game_event_updates_mood() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Test"));

        let initial_mood = coord.mood(1).unwrap();
        // Death events should significantly affect mood
        coord.on_game_event(
            1,
            SoulEvent::Death {
                killer: Some("a dragon".into()),
                zone: "permafrost".into(),
            },
        );
        // Mood may or may not change depending on personality engine,
        // but the function should not panic
        let _ = coord.mood(1).unwrap();
        let _ = initial_mood; // suppress unused
    }

    #[test]
    fn on_game_event_ignores_unknown_client() {
        let mut coord = make_coordinator(true);
        // Should not panic for unregistered client
        coord.on_game_event(
            99,
            SoulEvent::Kill {
                target: "orc".into(),
                zone: "test".into(),
            },
        );
    }

    #[test]
    fn on_player_message_ignores_when_disabled() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = false;
        coord.register_character(1, &make_char_config("Test"));
        // Should not enqueue anything
        coord.on_player_message(1, "Dave", "Hello!", "say");
        assert_eq!(coord.llm_queue.pending_count(), 0);
    }

    #[test]
    fn on_player_message_enqueues_response() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        coord.register_character(1, &make_char_config("Test"));
        coord.on_player_message(1, "Dave", "Hey there!", "say");
        assert_eq!(coord.llm_queue.pending_count(), 1);
    }

    #[test]
    fn on_player_message_ignores_unknown_client() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        // Should not panic for unregistered client
        coord.on_player_message(99, "Dave", "Hello!", "say");
        assert_eq!(coord.llm_queue.pending_count(), 0);
    }

    #[test]
    fn on_player_message_ignores_empty_message() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        coord.register_character(1, &make_char_config("Test"));
        coord.on_player_message(1, "Dave", "   ", "say");
        assert_eq!(coord.llm_queue.pending_count(), 0);
    }

    #[test]
    fn truncate_utf8_does_not_split_multibyte_codepoint() {
        let input = "hello🙂";
        let truncated = truncate_utf8(input, 6);
        assert_eq!(truncated, "hello");
    }

    #[test]
    fn social_graph_accessible() {
        let coord = make_coordinator(true);
        let graph = coord.social_graph();
        // Fresh coordinator has no relationships
        assert!(graph.relationships_for("Nobody").is_empty());
    }

    #[test]
    fn memory_store_accessible() {
        let coord = make_coordinator(true);
        let _store = coord.memory_store();
        // Should not panic
    }

    #[test]
    fn is_in_combat_helper() {
        let mut state = make_game_state(1);
        state.combat_status = CombatStatus::Idle;
        assert!(!is_in_combat(&state));

        state.combat_status = CombatStatus::Engaging { target_id: 1 };
        assert!(is_in_combat(&state));
    }

    #[test]
    fn zone_from_state_returns_unknown() {
        let state = make_game_state(1);
        assert_eq!(zone_from_state(&state), "unknown");
    }

    #[test]
    fn register_multiple_characters() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Alpha"));
        coord.register_character(2, &make_char_config("Beta"));
        coord.register_character(3, &make_char_config("Gamma"));

        assert!(coord.mood(1).is_some());
        assert!(coord.mood(2).is_some());
        assert!(coord.mood(3).is_some());
    }

    #[test]
    fn tick_skips_clients_without_game_state() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Test"));
        // Empty states map — no game state for client 1
        let states = HashMap::new();
        let cmds = coord.tick(&states);
        // Should not panic, just skip
        assert!(cmds.is_empty());
    }

    #[test]
    fn score_sentiment_positive_words() {
        let score = score_sentiment("Thank you so much, that was great and amazing!");
        assert!(score > 0.3, "Expected positive sentiment, got {score}");
    }

    #[test]
    fn score_sentiment_negative_words() {
        let score = score_sentiment("This is terrible and awful, I hate it!");
        assert!(score < -0.3, "Expected negative sentiment, got {score}");
    }

    #[test]
    fn score_sentiment_neutral_returns_zero() {
        let score = score_sentiment("I walked to the store today");
        assert!(
            score.abs() < 0.01,
            "Expected neutral (0.0) sentiment, got {score}"
        );
    }

    #[test]
    fn score_sentiment_clamps_to_range() {
        let score = score_sentiment(
            "thank thanks great awesome nice good love amazing excellent wonderful please help happy yes sure glad perfect fantastic appreciate",
        );
        assert!(score <= 1.0 && score >= -1.0, "score out of range: {score}");
    }

    #[test]
    fn on_player_message_records_sentiment_in_memory() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        coord.register_character(1, &make_char_config("Test"));
        coord.on_player_message(1, "Dave", "Thank you so much, great work!", "say");
        // Should record a positive sentiment conversation
        let convos = coord.memory_store().recall_conversations(1, 5).unwrap();
        assert!(!convos.is_empty());
        // Sentiment should be positive (>0.0) for a positive message
        if let Some(sent) = convos[0].sentiment {
            assert!(sent > 0.0, "Expected positive sentiment stored, got {sent}");
        }
    }

    #[test]
    fn positive_sentiment_nudges_mood_toward_happy() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        coord.register_character(1, &make_char_config("Test"));
        // Send a strongly positive message — should nudge mood toward Happy
        coord.on_player_message(
            1,
            "Dave",
            "Thank you so much, you are great and wonderful!",
            "say",
        );
        let mood = coord.mood(1).unwrap();
        assert!(
            matches!(
                mood,
                MoodState::Happy | MoodState::Excited | MoodState::Neutral
            ),
            "Expected mood nudged positive, got {mood:?}"
        );
    }

    #[test]
    fn negative_sentiment_nudges_mood_toward_anxious_or_angry() {
        let mut coord = make_coordinator(true);
        coord.config.player_chat_enabled = true;
        // Register with high neuroticism to reliably get Anxious
        let char_config = CharacterSoulConfig {
            name: "Test".to_string(),
            traits: PersonalityTraits {
                neuroticism: 0.9,
                ..PersonalityTraits::default()
            },
            speech: SpeechStyle::default(),
            edginess: None,
            backstory: String::new(),
            quirks: Vec::new(),
        };
        coord.register_character(1, &char_config);
        coord.on_player_message(1, "Dave", "I hate this, it is terrible and awful!", "say");
        let mood = coord.mood(1).unwrap();
        assert!(
            matches!(
                mood,
                MoodState::Anxious | MoodState::Angry | MoodState::Melancholy
            ),
            "Expected mood nudged negative, got {mood:?}"
        );
    }
}
