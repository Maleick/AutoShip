use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use dmft_common::ipc::Command;
use dmft_common::soul::{MoodState, PersonalityTraits, SoulAction, SoulEvent, SpeechStyle};
use dmft_common::types::{ClientId, GameState};

use super::config::{CharacterSoulConfig, EdginessLevel, SoulConfig};
use super::idle::{IdleScheduler, IdleTransition};
use super::llm::fallback::TraitDrivenResponder;
use super::llm::priority_queue::LlmRequestQueue;
use super::llm::{LlmPriority, LlmRequest, Situation};
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
/// Mirrors CombatCoordinator: called each tick, returns commands to dispatch.
pub struct SoulCoordinator {
    souls: HashMap<ClientId, CharacterSoul>,
    memory: MemoryStore,
    social: SocialGraph,
    llm_queue: LlmRequestQueue,
    config: SoulConfig,
    /// Tick counter for timing.
    tick_count: u64,
}

impl SoulCoordinator {
    /// Create a new SoulCoordinator from config.
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
        })
    }

    /// Register a character with the coordinator.
    pub fn register_character(
        &mut self,
        client_id: ClientId,
        char_config: &CharacterSoulConfig,
    ) {
        let edginess = char_config
            .edginess
            .unwrap_or(self.config.edginess);

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
    pub fn tick(
        &mut self,
        states: &HashMap<ClientId, GameState>,
    ) -> Vec<(ClientId, Command)> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.tick_count += 1;
        let mut commands = Vec::new();

        // Collect client IDs to avoid borrow issues
        let client_ids: Vec<ClientId> = self.souls.keys().copied().collect();

        for client_id in client_ids {
            let soul = match self.souls.get_mut(&client_id) {
                Some(s) => s,
                None => continue,
            };

            let state = match states.get(&client_id) {
                Some(s) => s,
                None => continue,
            };

            let in_combat = is_in_combat(state);
            let zone = zone_from_state(state);
            let group_members: Vec<String> = Vec::new(); // TODO: populate from state

            let ctx = SoulContext {
                character_name: &soul.name,
                traits: &soul.traits,
                mood: soul.mood,
                edginess: soul.edginess,
                zone,
                level: state
                    .local_player
                    .as_ref()
                    .map(|p| p.level)
                    .unwrap_or(1),
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
                                channel: dmft_common::soul::SayChannel::Group,
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
                        behavior: dmft_common::soul::IdleBehaviorType::LogOffToSleep,
                    };
                    commands.push((client_id, Command::SoulAction { action }));
                }
                IdleTransition::Stop | IdleTransition::Continue => {}
            }

            // Periodic memory decay (every ~60 ticks = 5 minutes at 5s tick)
            if self.tick_count.is_multiple_of(60) {
                let _ = self.memory.decay_tick(client_id, 0.995);
                let _ = self.memory.prune_low_importance(client_id, 0.05);
            }
        }

        // Process any queued LLM requests
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Use the first soul's responder as the provider for the queue
        // In Phase 2, this will be a real LLM provider
        if let Some(soul) = self.souls.values_mut().next() {
            let results = self.llm_queue.process_all(&mut soul.responder, now_secs);
            for (request, response) in results {
                // Find the client_id for this character
                if let Some((&cid, _)) = self
                    .souls
                    .iter()
                    .find(|(_, s)| s.name == request.character_name)
                {
                    commands.push((
                        cid,
                        Command::Say {
                            channel: dmft_common::soul::SayChannel::Say,
                            message: response.text,
                            target: None,
                        },
                    ));
                }
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

        let soul = match self.souls.get_mut(&client_id) {
            Some(s) => s,
            None => return,
        };

        // Record the conversation
        let _ = self.memory.record_conversation(
            client_id,
            player_name,
            true,
            channel,
            message,
            None,
        );

        // Process mood change from player interaction
        let event = SoulEvent::PlayerChat {
            player_name: player_name.to_string(),
            sentiment: 0.0, // TODO: sentiment analysis in Phase 2
        };
        soul.mood = soul
            .personality
            .process_event(soul.mood, &event, &soul.traits);

        // Record memory
        let _ = self.memory.record(client_id, &event, soul.mood, 2.0);

        // Queue an LLM response (high priority for real players)
        let request = LlmRequest {
            character_name: soul.name.clone(),
            traits: soul.traits.clone(),
            mood: soul.mood,
            speech_style: soul.speech_style.clone(),
            situation: Situation::PlayerChat {
                player_name: player_name.to_string(),
                message: message.to_string(),
                channel: channel.to_string(),
            },
            priority: LlmPriority::High,
            memory_context: Vec::new(), // TODO: populate from recall_about
            backstory: soul.backstory.clone(),
        };

        self.llm_queue.enqueue(request);
    }

    /// Handle a game event (kill, death, loot, zone change, etc.).
    pub fn on_game_event(&mut self, client_id: ClientId, event: SoulEvent) {
        let soul = match self.souls.get_mut(&client_id) {
            Some(s) => s,
            None => return,
        };

        // Update mood
        soul.mood = soul
            .personality
            .process_event(soul.mood, &event, &soul.traits);

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

        // Record memory
        let _ = self.memory.record(client_id, &event, soul.mood, importance);
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

/// Check if a client is currently in combat based on game state.
fn is_in_combat(state: &GameState) -> bool {
    !matches!(
        state.combat_status,
        dmft_common::combat::CombatStatus::Idle
    )
}

/// Extract zone name from game state (placeholder until zone tracking is added).
fn zone_from_state(_state: &GameState) -> &'static str {
    "unknown"
}
