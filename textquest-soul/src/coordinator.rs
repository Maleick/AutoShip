use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use textquest_common::{
    ipc::Command,
    soul::{MoodState, PersonalityTraits, SoulAction, SoulEvent, SpeechStyle},
    types::{ClientId, GameState},
};

use super::{
    alerts::{Alert, AnomalyDetector},
    audit::{AuditActionType, SoulAuditLogger},
    config::{CharacterSoulConfig, EdginessLevel, SoulConfig},
    idle::{IdleScheduler, IdleTransition},
    llm::{
        LlmPriority, LlmProvider, LlmRequest, Situation, fallback::TraitDrivenResponder,
        priority_queue::LlmRequestQueue,
    },
    memory::MemoryStore,
    personality::{PersonalityEngine, SoulContext},
    social::SocialGraph,
    suppression::{GameStateContext, SuppressionRules},
};

/// Maximum number of entries in the IPC command queue before overflow drops
/// occur.
const IPC_QUEUE_MAX: usize = 512;
/// Default time-to-live for buffered IPC commands.
const IPC_QUEUE_TTL_SECS: u64 = 10;

/// Priority of a queued IPC command — determines drop order on overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IpcCommandPriority {
    /// Idle/ambient commands (dropped first on overflow).
    Low = 0,
    /// Normal game commands.
    Normal = 1,
    /// Player-facing or time-critical commands (never dropped on overflow).
    High = 2,
}

/// An IPC command buffered for later delivery when the pipe is unavailable.
#[derive(Debug)]
struct QueuedCommand {
    client_id: ClientId,
    command: Command,
    priority: IpcCommandPriority,
    queued_at: Instant,
}

/// Lightweight IPC command queue used when the named pipe is unavailable.
///
/// Overflow policy: when the queue is full, the lowest-priority entry is
/// dropped (FIFO within each priority tier).
#[derive(Debug)]
pub struct IpcCommandQueue {
    queue: VecDeque<QueuedCommand>,
    dropped_low: u64,
    dropped_stale: u64,
    ttl: Duration,
}

impl IpcCommandQueue {
    fn new() -> Self {
        Self::with_ttl(Duration::from_secs(IPC_QUEUE_TTL_SECS))
    }

    fn with_ttl(ttl: Duration) -> Self {
        Self {
            queue: VecDeque::new(),
            dropped_low: 0,
            dropped_stale: 0,
            ttl,
        }
    }

    /// Enqueue a command.  Drops the oldest low-priority entry on overflow.
    fn push(&mut self, client_id: ClientId, command: Command, priority: IpcCommandPriority) {
        if self.queue.len() >= IPC_QUEUE_MAX {
            // Find the first low-priority entry and drop it.
            if let Some(pos) = self
                .queue
                .iter()
                .position(|q| q.priority == IpcCommandPriority::Low)
            {
                self.queue.remove(pos);
                self.dropped_low += 1;
                tracing::debug!(
                    dropped_total = self.dropped_low,
                    "ipc_queue: dropped low-priority command on overflow"
                );
            } else {
                // No low-priority entries to drop — discard the new entry if it's low priority,
                // otherwise evict the oldest normal entry.
                if priority == IpcCommandPriority::Low {
                    self.dropped_low += 1;
                    return;
                }
                if let Some(pos) = self
                    .queue
                    .iter()
                    .position(|q| q.priority == IpcCommandPriority::Normal)
                {
                    self.queue.remove(pos);
                } else {
                    // Queue is full of high-priority entries; preserve existing
                    // commands and discard the new one to maintain the hard cap.
                    return;
                }
            }
        }
        self.queue.push_back(QueuedCommand {
            client_id,
            command,
            priority,
            queued_at: Instant::now(),
        });
    }

    /// Drain all queued commands, returning them for dispatch.
    pub fn drain(&mut self) -> Vec<(ClientId, Command)> {
        let now = Instant::now();
        let mut drained = Vec::with_capacity(self.queue.len());
        while let Some((client_id, command)) = self.pop_with_now(now) {
            drained.push((client_id, command));
        }
        drained
    }

    /// Pop the oldest queued command in FIFO order.
    #[cfg(test)]
    fn pop(&mut self) -> Option<(ClientId, Command)> {
        self.pop_with_now(Instant::now())
    }

    fn pop_with_now(&mut self, now: Instant) -> Option<(ClientId, Command)> {
        while let Some(front) = self.queue.front() {
            let age = now.saturating_duration_since(front.queued_at);
            if age < self.ttl {
                break;
            }

            let dropped = self.queue.pop_front().expect("front entry must exist");
            self.dropped_stale += 1;
            tracing::debug!(
                client_id = dropped.client_id,
                priority = ?dropped.priority,
                age_ms = age.as_millis(),
                ttl_ms = self.ttl.as_millis(),
                dropped_total = self.dropped_stale,
                "ipc_queue: dropped stale command on dequeue"
            );
        }

        self.queue.pop_front().map(|q| (q.client_id, q.command))
    }

    /// Number of commands currently buffered.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Total low-priority commands dropped due to overflow.
    pub fn dropped_low_count(&self) -> u64 {
        self.dropped_low
    }

    /// Total commands dropped because they exceeded the TTL before dequeue.
    pub fn dropped_stale_count(&self) -> u64 {
        self.dropped_stale
    }
}

/// Per-character soul state.
struct CharacterSoul {
    name: String,
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
    /// Sliding-window request timestamps per character for LLM rate limiting.
    character_request_counts: HashMap<ClientId, VecDeque<Instant>>,
    /// Sliding-window request timestamps across all characters for LLM rate
    /// limiting.
    global_request_counts: VecDeque<Instant>,
    suppression: SuppressionRules,
    /// Tick counter for timing.
    tick_count: u64,
    /// Buffered IPC commands waiting for pipe availability.
    ipc_queue: IpcCommandQueue,
    /// Whether the named pipe is currently considered available.
    ipc_available: bool,
    /// Detects runtime anomalies and generates operator alerts.
    anomaly_detector: AnomalyDetector,
    /// Optional JSONL audit logger for key Soul Engine events.
    audit: Option<SoulAuditLogger>,
    /// Sliding one-hour window of chat-derived memory write timestamps, keyed
    /// by client. Used to enforce `max_chat_memory_writes_per_hour`.
    chat_memory_write_timestamps: HashMap<ClientId, VecDeque<Instant>>,
}

const MAX_PLAYER_CHAT_MESSAGE_BYTES: usize = 512;
const MAX_CONVERSATIONS_PER_CHARACTER: usize = 1000;
const SUMMARY_INTERVAL_TICKS: u64 = 720; // 1 hour at 5s/tick

impl SoulCoordinator {
    /// Create a new `SoulCoordinator` from config.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(config: SoulConfig, db_path: &Path) -> Result<Self> {
        let memory = MemoryStore::open(db_path)?;
        let social = SocialGraph::from_seeds(&config.relationship);
        let suppression = config.suppression.clone();
        // Phase 1: 0 token budget (fallback only, no real LLM calls)
        let llm_queue = LlmRequestQueue::new(0);

        Ok(Self {
            souls: HashMap::new(),
            memory,
            social,
            llm_queue,
            config,
            character_request_counts: HashMap::new(),
            global_request_counts: VecDeque::new(),
            suppression,
            tick_count: 0,
            ipc_queue: IpcCommandQueue::new(),
            ipc_available: true,
            anomaly_detector: AnomalyDetector::new(),
            audit: None,
            chat_memory_write_counts: HashMap::new(),
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
        self.anomaly_detector.register_character(client_id);
    }

    /// Check whether an LLM request is allowed for the given character right
    /// now.
    ///
    /// Prunes stale entries (>60s old) on each check.
    /// Returns `true` if both the per-character limit and the global limit have
    /// not been reached.
    pub fn can_request(&mut self, client_id: ClientId) -> bool {
        let window = std::time::Duration::from_secs(60);
        let now = Instant::now();

        // Prune global stale entries
        while let Some(&front) = self.global_request_counts.front() {
            if now.duration_since(front) >= window {
                self.global_request_counts.pop_front();
            } else {
                break;
            }
        }
        if self.global_request_counts.len() >= self.config.max_global_requests as usize {
            return false;
        }

        // Prune per-character stale entries without creating a new entry for unknown
        // clients.
        let mut remove_character_entry = false;
        if let Some(char_counts) = self.character_request_counts.get_mut(&client_id) {
            while let Some(&front) = char_counts.front() {
                if now.duration_since(front) >= window {
                    char_counts.pop_front();
                } else {
                    break;
                }
            }
            remove_character_entry = char_counts.is_empty();
        }
        if remove_character_entry {
            self.character_request_counts.remove(&client_id);
        }

        let char_len = self
            .character_request_counts
            .get(&client_id)
            .map_or(0, VecDeque::len);
        let char_ok = char_len < self.config.max_requests_per_character as usize;

        if char_ok {
            // Record this request
            self.character_request_counts
                .entry(client_id)
                .or_default()
                .push_back(now);
            self.global_request_counts.push_back(now);
            true
        } else {
            false
        }
    }

    /// Main tick — called every 5000ms by the orchestrator.
    /// Returns `(commands, alerts)` so the caller can dispatch commands and act
    /// on anomalies.
    pub fn tick(
        &mut self,
        states: &HashMap<ClientId, GameState>,
    ) -> (Vec<(ClientId, Command)>, Vec<Alert>) {
        if !self.config.enabled {
            return (Vec::new(), Vec::new());
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

            // Extract suppression context from game state
            let suppress_ctx = GameStateContext::from_game_state(state);

            // Tick idle scheduler
            match soul.idle.tick(&ctx, &mut soul.responder) {
                IdleTransition::Start(active) => {
                    // Check if idle should be suppressed by game state
                    if self.suppression.should_suppress_idle(suppress_ctx) {
                        // Don't start this idle behavior; clear the scheduler's active state
                        // so it can retry once suppression lifts.
                        soul.idle.interrupt();
                    } else if self
                        .suppression
                        .should_suppress_behavior_for_casting(&active.behavior, suppress_ctx)
                    {
                        // Don't start this movement-heavy behavior during casting; clear the
                        // scheduler's active state so it can retry once casting suppression lifts.
                        soul.idle.interrupt();
                    } else {
                        // Convert idle behavior to a Command
                        let action = SoulAction::StartIdle {
                            behavior: active.behavior.clone(),
                        };
                        commands.push((client_id, Command::SoulAction { action }));

                        // If there's flavor text, emit it as chat (also subject to suppression)
                        if let Some(text) = active.flavor_text
                            && !self.suppression.should_suppress_chat(suppress_ctx)
                        {
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
                }
                IdleTransition::LogOff { return_after_secs } => {
                    // LogOff is a special case — not suppressed, always allowed
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

            // Periodic memory decay (every ~60 ticks = 5 minutes at 5s tick)
            if self.tick_count.is_multiple_of(60) {
                let _ = self.memory.decay_tick(client_id, 0.995);
                let _ = self.memory.prune_low_importance(client_id, 0.05);
            }
        }

        // Periodic memory summarization check (runs every tick, internally rate-limited
        // to 1h)
        self.check_and_generate_summaries();

        // Process any queued LLM requests
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        // Drain all ready requests, then process each with the matching soul's
        // responder In Phase 2, this will use a real LLM provider instead of
        // per-soul fallback responders
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
                // Check if chat should be suppressed based on current game state
                if let Some(state) = states.get(&cid) {
                    let suppress_ctx = GameStateContext::from_game_state(state);
                    if !self.suppression.should_suppress_chat(suppress_ctx) {
                        commands.push((
                            cid,
                            Command::Say {
                                channel: textquest_common::soul::SayChannel::Say,
                                message: response.text,
                                target: None,
                            },
                        ));
                    }
                    // If chat is suppressed, we discard the response (don't
                    // queue it)
                } else {
                    // No game state available — emit the response anyway
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
        }

        // If the IPC pipe is unavailable, buffer the commands instead of returning
        // them. High-priority commands are buffered; low-priority idle chatter
        // is dropped on overflow.
        if !self.ipc_available {
            for (client_id, cmd) in commands {
                let priority = ipc_command_priority(&cmd);
                self.ipc_queue.push(client_id, cmd, priority);
            }
            return (Vec::new(), Vec::new());
        }

        let alerts = self.anomaly_detector.check();
        (commands, alerts)
    }

    fn check_and_generate_summaries(&mut self) {
        if !self.tick_count.is_multiple_of(SUMMARY_INTERVAL_TICKS) {
            return;
        }

        let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_secs() as i64,
            Err(_) => return,
        };
        let period_end = now;
        let period_start = now.saturating_sub(3600);

        for client_id in self.souls.keys().copied() {
            if let Ok(summary) = self
                .memory
                .generate_summary(client_id, period_start, period_end)
                && !summary.is_empty()
            {
                let _ = self.memory.record_summary(
                    client_id,
                    &period_start.to_string(),
                    &period_end.to_string(),
                    &summary,
                    None,
                );
            }
        }
    }

    /// Check and record a chat-derived memory write for rate limiting.
    ///
    /// Returns `true` if the write is allowed (within the hourly cap), `false`
    /// if it should be dropped. Slides the window on each call.
    fn allow_chat_memory_write(&mut self, client_id: ClientId, now: Instant) -> bool {
        const WINDOW_SECS: u64 = 3600;
        let cap = self.config.max_chat_memory_writes_per_hour as usize;
        let window = self
            .chat_memory_write_counts
            .entry(client_id)
            .or_default();

        // Drain entries older than one hour.
        while let Some(&front) = window.front() {
            if now.duration_since(front).as_secs() >= WINDOW_SECS {
                window.pop_front();
            } else {
                break;
            }
        }

        if window.len() >= cap {
            tracing::warn!(
                client_id,
                window_count = window.len(),
                cap,
                "chat memory write dropped: hourly cap reached"
            );
            return false;
        }

        window.push_back(now);
        true
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

        // Check the hourly cap before borrowing soul — allow_chat_memory_write
        // takes &mut self so it must not be called while soul is live.
        let chat_write_allowed = self.allow_chat_memory_write(client_id, Instant::now());

        let Some(soul) = self.souls.get_mut(&client_id) else {
            return;
        };

        let message = message.trim();
        if message.is_empty() {
            return;
        }
        let message = truncate_utf8(message, MAX_PLAYER_CHAT_MESSAGE_BYTES);

        // Record the conversation
        let _ =
            self.memory
                .record_conversation(client_id, player_name, true, channel, message, 0.0);
        let _ = self
            .memory
            .prune_conversations(client_id, MAX_CONVERSATIONS_PER_CHARACTER);

        // Capture mood before event processing for accurate memory recording
        let mood_before = soul.mood;

        // Process mood change from player interaction
        let event = SoulEvent::PlayerChat {
            player_name: player_name.to_string(),
            sentiment: 0.0, // Neutral default; LLM-based sentiment analysis deferred to M6
        };
        soul.mood = soul
            .personality
            .process_event(soul.mood, &event, &soul.traits);

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

        // Record memory with the mood as it was before the event changed it.
        // Guard against chat-driven DoS: drop the write if the hourly cap is
        // reached for this character. Conversation table is still updated
        // above; only the memories table write is gated.
        let memory_written = if chat_write_allowed {
            let _ = self.memory.record(client_id, &event, mood_before, 2.0);
            true
        } else {
            false
        };

        // Audit: memory record
        if memory_written {
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
        // Notify the anomaly detector that a soul event was received.
        self.anomaly_detector.record_soul_event(client_id);

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

    /// Emit a soul event for a registered client.
    ///
    /// Validates that `client_id` is registered, records the event in memory,
    /// and processes it through the personality engine to update mood.
    ///
    /// # Errors
    ///
    /// Returns an error if `client_id` is not registered.
    pub fn emit_soul_event(&mut self, client_id: ClientId, event: SoulEvent) -> Result<()> {
        if !self.souls.contains_key(&client_id) {
            return Err(anyhow::anyhow!(
                "client_id {client_id} is not registered with SoulCoordinator"
            ));
        }
        self.on_game_event(client_id, event);
        Ok(())
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

    // -- IPC error handling --

    /// Signal that the named pipe became unavailable.
    ///
    /// While unavailable, `tick()` outputs are buffered in the internal IPC
    /// queue instead of being returned to the caller.  Call
    /// `on_ipc_available()` when the pipe reconnects.
    pub fn on_ipc_unavailable(&mut self) {
        if self.ipc_available {
            tracing::warn!("soul_coordinator: IPC pipe unavailable — buffering commands");
            self.ipc_available = false;
        }
    }

    /// Signal that the named pipe is available again.
    ///
    /// Drains the internal buffer and returns all queued commands so the
    /// caller can dispatch them.  The coordinator resumes normal (unbuffered)
    /// operation after this call.
    pub fn on_ipc_available(&mut self) -> Vec<(ClientId, Command)> {
        if !self.ipc_available {
            tracing::info!(
                buffered = self.ipc_queue.len(),
                dropped_low = self.ipc_queue.dropped_low_count(),
                dropped_stale = self.ipc_queue.dropped_stale_count(),
                "soul_coordinator: IPC pipe restored — flushing command queue"
            );
            self.ipc_available = true;
        }
        self.ipc_queue.drain()
    }

    /// Returns true if the IPC pipe is currently considered available.
    pub fn is_ipc_available(&self) -> bool {
        self.ipc_available
    }

    /// Number of commands currently buffered in the IPC queue.
    pub fn ipc_queue_len(&self) -> usize {
        self.ipc_queue.len()
    }

    /// Enqueue a command for later IPC delivery, classifying its priority.
    ///
    /// Used internally and by callers who need to buffer a command after a
    /// send failure without crashing the coordinator.
    pub fn buffer_command(
        &mut self,
        client_id: ClientId,
        command: Command,
        priority: IpcCommandPriority,
    ) {
        self.ipc_queue.push(client_id, command, priority);
    }
}

/// Classify a `Command` into an IPC queue priority tier.
///
/// Player-chat responses and high-importance soul actions are `High`;
/// idle chatter is `Low`; everything else is `Normal`.
fn ipc_command_priority(cmd: &Command) -> IpcCommandPriority {
    match cmd {
        Command::Say { channel, .. } => match channel {
            textquest_common::soul::SayChannel::Tell => IpcCommandPriority::High,
            textquest_common::soul::SayChannel::Group => IpcCommandPriority::Normal,
            _ => IpcCommandPriority::Low,
        },
        Command::SoulAction { action } => match action {
            textquest_common::soul::SoulAction::StartIdle {
                behavior: textquest_common::soul::IdleBehaviorType::LogOffToSleep,
            } => IpcCommandPriority::High,
            _ => IpcCommandPriority::Low,
        },
        _ => IpcCommandPriority::Normal,
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

/// Extract zone short name from game state.
///
/// Returns the zone short name if non-empty, otherwise falls back to "unknown".
fn zone_from_state(state: &GameState) -> &str {
    let zone = state.zone_short_name.as_str();
    if zone.is_empty() { "unknown" } else { zone }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::CombatStatus,
        nav::NavStatus,
        soul::{PersonalityTraits, SpeechStyle},
        types::SpawnData,
    };

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
            active_buffs: vec![],
            pet: None,
            actual_version: None,
            is_zone_changing: false,
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
        let (cmds, _alerts) = coord.tick(&states);
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
    fn zone_from_state_returns_zone_short_name() {
        let state = make_game_state(1);
        // make_game_state sets zone_short_name = "test"
        assert_eq!(zone_from_state(&state), "test");
    }

    #[test]
    fn zone_from_state_falls_back_to_unknown_when_empty() {
        let state = GameState {
            zone_short_name: String::new(),
            ..make_game_state(1)
        };
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
        let (cmds, _alerts) = coord.tick(&states);
        // Should not panic, just skip
        assert!(cmds.is_empty());
    }

    // --- emit_soul_event tests ---

    #[test]
    fn emit_soul_event_ok_for_registered_client() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Warrior01"));
        let result = coord.emit_soul_event(
            1,
            SoulEvent::Kill {
                target: "a goblin".into(),
                zone: "crushbone".into(),
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn emit_soul_event_err_for_unregistered_client() {
        let mut coord = make_coordinator(true);
        let result = coord.emit_soul_event(
            99,
            SoulEvent::Kill {
                target: "a goblin".into(),
                zone: "crushbone".into(),
            },
        );
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("99"), "Error should mention the client_id");
    }

    #[test]
    fn emit_soul_event_death_records_in_memory() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Cleric01"));
        let result = coord.emit_soul_event(
            1,
            SoulEvent::Death {
                killer: Some("a dragon".into()),
                zone: "permafrost".into(),
            },
        );
        assert!(result.is_ok());
        // Memory store should have 1 entry for this client
        let memories = coord.memory_store().recall_about(1, "a dragon", 10);
        // recall_about returns Result; it should succeed and have >= 0 entries (event
        // is recorded)
        assert!(memories.is_ok());
    }

    #[test]
    fn emit_soul_event_multiple_events_accumulate() {
        let mut coord = make_coordinator(true);
        coord.register_character(1, &make_char_config("Rogue01"));
        // Three kill events — all should succeed
        for mob in &["orc pawn", "orc centurion", "orc oracle"] {
            let result = coord.emit_soul_event(
                1,
                SoulEvent::Kill {
                    target: (*mob).into(),
                    zone: "crushbone".into(),
                },
            );
            assert!(result.is_ok(), "emit_soul_event failed for {mob}");
        }
    }

    #[test]
    fn ipc_queue_overflow_with_only_high_priority_is_capped() {
        let mut queue = IpcCommandQueue::new();
        for i in 0..IPC_QUEUE_MAX {
            queue.push(
                i as ClientId,
                Command::StopMovement,
                IpcCommandPriority::High,
            );
        }
        assert_eq!(queue.len(), IPC_QUEUE_MAX);

        queue.push(9_999, Command::StopMovement, IpcCommandPriority::High);
        assert_eq!(queue.len(), IPC_QUEUE_MAX);

        let mut drained_client_ids = Vec::new();
        while let Some((client_id, _command)) = queue.pop() {
            drained_client_ids.push(client_id);
        }

        assert_eq!(drained_client_ids.len(), IPC_QUEUE_MAX);
        assert!(
            !drained_client_ids.contains(&9_999),
            "overflowing high-priority command should be discarded"
        );
        for expected_client_id in 0..IPC_QUEUE_MAX {
            assert!(
                drained_client_ids.contains(&(expected_client_id as ClientId)),
                "existing queued command for client_id={} should be preserved",
                expected_client_id
            );
        }
    }

    #[test]
    fn ipc_queue_drops_stale_commands_on_pop() {
        let mut queue = IpcCommandQueue::new();
        queue.push(42, Command::StopMovement, IpcCommandPriority::Normal);

        queue.queue[0].queued_at = Instant::now() - std::time::Duration::from_secs(11);

        assert!(
            queue.pop().is_none(),
            "stale queued commands should be discarded on dequeue"
        );
        assert_eq!(queue.dropped_stale_count(), 1);
    }

    // --- Chat memory write rate limit tests ---

    fn make_coordinator_with_chat_cap(cap: u32) -> SoulCoordinator {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_memory.db");
        let config = SoulConfig {
            enabled: true,
            max_chat_memory_writes_per_hour: cap,
            ..SoulConfig::default()
        };
        let dir = Box::leak(Box::new(dir));
        let _ = dir;
        SoulCoordinator::new(config, &db_path).unwrap()
    }

    #[test]
    fn chat_memory_writes_under_cap_are_allowed() {
        let mut coord = make_coordinator_with_chat_cap(3);
        let t0 = Instant::now();
        assert!(coord.allow_chat_memory_write(1, t0));
        assert!(coord.allow_chat_memory_write(1, t0));
        assert!(coord.allow_chat_memory_write(1, t0));
    }

    #[test]
    fn chat_memory_writes_over_cap_are_rejected() {
        let mut coord = make_coordinator_with_chat_cap(3);
        let t0 = Instant::now();
        coord.allow_chat_memory_write(1, t0);
        coord.allow_chat_memory_write(1, t0);
        coord.allow_chat_memory_write(1, t0);
        // 4th write exceeds the cap of 3
        assert!(!coord.allow_chat_memory_write(1, t0));
    }

    #[test]
    fn chat_memory_write_window_rolls_after_one_hour() {
        let mut coord = make_coordinator_with_chat_cap(2);
        let t0 = Instant::now();
        // Fill the cap
        coord.allow_chat_memory_write(1, t0);
        coord.allow_chat_memory_write(1, t0);
        assert!(!coord.allow_chat_memory_write(1, t0));

        // After 3600 seconds the window expires; writes should be allowed again
        let t1 = t0 + Duration::from_secs(3600);
        assert!(coord.allow_chat_memory_write(1, t1));
    }

    #[test]
    fn chat_memory_write_cap_is_per_character() {
        let mut coord = make_coordinator_with_chat_cap(1);
        let t0 = Instant::now();
        // Character 1 fills its cap
        assert!(coord.allow_chat_memory_write(1, t0));
        assert!(!coord.allow_chat_memory_write(1, t0));
        // Character 2 is independent
        assert!(coord.allow_chat_memory_write(2, t0));
    }

    #[test]
    fn on_player_message_stops_recording_after_hourly_cap() {
        let mut coord = make_coordinator_with_chat_cap(2);
        coord.config.player_chat_enabled = true;
        coord.register_character(1, &make_char_config("Mage01"));

        let mut states = HashMap::new();
        states.insert(1, make_game_state(1));
        coord.tick(&states);

        // Two messages allowed under cap
        coord.on_player_message(1, "Spammer", "hello 1", "say");
        coord.on_player_message(1, "Spammer", "hello 2", "say");

        // Fetch memories recorded so far
        let after_two = coord
            .memory_store()
            .recall_about(1, "Spammer", 100)
            .unwrap_or_default()
            .len();

        // Third message hits the cap — memory.record() should be skipped
        coord.on_player_message(1, "Spammer", "hello 3 over cap", "say");

        let after_three = coord
            .memory_store()
            .recall_about(1, "Spammer", 100)
            .unwrap_or_default()
            .len();

        assert_eq!(
            after_two, after_three,
            "memory row count should not grow after hourly cap is reached"
        );
    }
}
