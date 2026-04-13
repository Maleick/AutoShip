//! Event trigger system — configurable condition → action rules for game events.
//!
//! Inspired by MQ2Events (OpenVanilla parity). Users define triggers that fire
//! actions when game events match their conditions.

/// A game event that can be matched by trigger conditions.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    /// A spawn (NPC or player) died.
    SpawnDied { name: String },
    /// A player character died.
    PlayerDied { character: String },
    /// A chat message was received.
    ChatReceived { message: String },
    /// A character leveled up.
    LeveledUp { character: String },
}

/// Condition that must be satisfied for a trigger to fire.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerCondition {
    /// Fires when a spawn dies and its name contains the given substring (case-insensitive).
    SpawnDeath { name_contains: String },
    /// Fires when a specific player character dies (exact match, case-insensitive).
    PlayerDeath { character: String },
    /// Fires when a chat message contains the given pattern (case-insensitive substring).
    ChatMessage { pattern: String },
    /// Fires when a specific character levels up (exact match, case-insensitive).
    LevelUp { character: String },
    /// Fires on any death event (spawn or player).
    AnyDeath,
}

impl TriggerCondition {
    fn ascii_icontains(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }

        let haystack = haystack.as_bytes();
        let needle = needle.as_bytes();

        if needle.len() > haystack.len() {
            return false;
        }

        haystack.windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        })
    }

    fn ascii_ieq(left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }

    /// Returns true if this condition matches the given game event.
    pub fn matches(&self, event: &GameEvent) -> bool {
        match (self, event) {
            (TriggerCondition::SpawnDeath { name_contains }, GameEvent::SpawnDied { name }) => {
                Self::ascii_icontains(name, name_contains)
            }
            (TriggerCondition::PlayerDeath { character }, GameEvent::PlayerDied { character: c }) => {
                Self::ascii_ieq(c, character)
            }
            (TriggerCondition::ChatMessage { pattern }, GameEvent::ChatReceived { message }) => {
                Self::ascii_icontains(message, pattern)
            }
            (TriggerCondition::LevelUp { character }, GameEvent::LeveledUp { character: c }) => {
                Self::ascii_ieq(c, character)
            }
            (TriggerCondition::AnyDeath, GameEvent::SpawnDied { .. })
            | (TriggerCondition::AnyDeath, GameEvent::PlayerDied { .. }) => true,
            _ => false,
        }
    }
}

/// Action to take when a trigger fires.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerAction {
    /// Send a slash command string via IPC.
    SendIpcCommand(String),
    /// Write a message to the log.
    LogMessage(String),
    /// Send an alert to Discord.
    DiscordAlert(String),
}

/// A named, configurable event trigger: condition → action.
#[derive(Debug, Clone)]
pub struct EventTrigger {
    /// Human-readable name for this trigger.
    pub name: String,
    /// Condition that must match for the trigger to fire.
    pub condition: TriggerCondition,
    /// Action to take when the trigger fires.
    pub action: TriggerAction,
    /// Whether this trigger is active.
    pub enabled: bool,
    /// Number of times this trigger has fired.
    pub fire_count: u64,
}

impl EventTrigger {
    /// Create a new enabled trigger with zero fire count.
    pub fn new(name: impl Into<String>, condition: TriggerCondition, action: TriggerAction) -> Self {
        Self {
            name: name.into(),
            condition,
            action,
            enabled: true,
            fire_count: 0,
        }
    }
}

/// Evaluates all registered triggers against incoming game events.
#[derive(Debug, Default)]
pub struct TriggerEngine {
    /// All registered triggers (enabled and disabled).
    pub triggers: Vec<EventTrigger>,
}

impl TriggerEngine {
    /// Create an empty engine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trigger to the engine.
    pub fn add_trigger(&mut self, trigger: EventTrigger) {
        self.triggers.push(trigger);
    }

    /// Evaluate all enabled triggers against an event.
    ///
    /// Increments `fire_count` on each matching trigger and returns a list of
    /// the actions that should be executed.
    pub fn evaluate(&mut self, event: &GameEvent) -> Vec<TriggerAction> {
        let mut actions = Vec::new();
        for trigger in &mut self.triggers {
            if !trigger.enabled {
                continue;
            }
            if trigger.condition.matches(event) {
                trigger.fire_count += 1;
                actions.push(trigger.action.clone());
            }
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Condition matching ────────────────────────────────────────────────────

    #[test]
    fn spawn_death_matches_substring_case_insensitive() {
        let cond = TriggerCondition::SpawnDeath { name_contains: "lord".into() };
        assert!(cond.matches(&GameEvent::SpawnDied { name: "a Gnoll Lord".into() }));
        assert!(cond.matches(&GameEvent::SpawnDied { name: "LORD COMMANDER".into() }));
        assert!(!cond.matches(&GameEvent::SpawnDied { name: "a skeleton".into() }));
    }

    #[test]
    fn spawn_death_does_not_match_other_events() {
        let cond = TriggerCondition::SpawnDeath { name_contains: "gnoll".into() };
        assert!(!cond.matches(&GameEvent::PlayerDied { character: "gnoll".into() }));
        assert!(!cond.matches(&GameEvent::ChatReceived { message: "a gnoll died".into() }));
    }

    #[test]
    fn player_death_matches_case_insensitive() {
        let cond = TriggerCondition::PlayerDeath { character: "Kira".into() };
        assert!(cond.matches(&GameEvent::PlayerDied { character: "kira".into() }));
        assert!(cond.matches(&GameEvent::PlayerDied { character: "KIRA".into() }));
        assert!(!cond.matches(&GameEvent::PlayerDied { character: "Kara".into() }));
    }

    #[test]
    fn player_death_does_not_match_spawn_death() {
        let cond = TriggerCondition::PlayerDeath { character: "Kira".into() };
        assert!(!cond.matches(&GameEvent::SpawnDied { name: "Kira".into() }));
    }

    #[test]
    fn chat_message_matches_substring_case_insensitive() {
        let cond = TriggerCondition::ChatMessage { pattern: "tell me".into() };
        assert!(cond.matches(&GameEvent::ChatReceived { message: "Kira tells you, 'Tell me more'".into() }));
        assert!(cond.matches(&GameEvent::ChatReceived { message: "TELL ME NOW".into() }));
        assert!(!cond.matches(&GameEvent::ChatReceived { message: "silence".into() }));
    }

    #[test]
    fn chat_message_does_not_match_other_events() {
        let cond = TriggerCondition::ChatMessage { pattern: "death".into() };
        assert!(!cond.matches(&GameEvent::SpawnDied { name: "death knight".into() }));
    }

    #[test]
    fn level_up_matches_exact_character_case_insensitive() {
        let cond = TriggerCondition::LevelUp { character: "Warrior".into() };
        assert!(cond.matches(&GameEvent::LeveledUp { character: "warrior".into() }));
        assert!(cond.matches(&GameEvent::LeveledUp { character: "WARRIOR".into() }));
        assert!(!cond.matches(&GameEvent::LeveledUp { character: "Rogue".into() }));
    }

    #[test]
    fn any_death_matches_spawn_died() {
        let cond = TriggerCondition::AnyDeath;
        assert!(cond.matches(&GameEvent::SpawnDied { name: "a goblin".into() }));
    }

    #[test]
    fn any_death_matches_player_died() {
        let cond = TriggerCondition::AnyDeath;
        assert!(cond.matches(&GameEvent::PlayerDied { character: "Kira".into() }));
    }

    #[test]
    fn any_death_does_not_match_chat_or_levelup() {
        let cond = TriggerCondition::AnyDeath;
        assert!(!cond.matches(&GameEvent::ChatReceived { message: "someone died".into() }));
        assert!(!cond.matches(&GameEvent::LeveledUp { character: "Kira".into() }));
    }

    // ── fire_count ────────────────────────────────────────────────────────────

    #[test]
    fn fire_count_increments_on_match() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "on gnoll death",
            TriggerCondition::SpawnDeath { name_contains: "gnoll".into() },
            TriggerAction::LogMessage("gnoll died".into()),
        ));

        let event = GameEvent::SpawnDied { name: "a gnoll warrior".into() };
        engine.evaluate(&event);
        engine.evaluate(&event);
        assert_eq!(engine.triggers[0].fire_count, 2);
    }

    #[test]
    fn fire_count_stays_zero_on_no_match() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "on gnoll death",
            TriggerCondition::SpawnDeath { name_contains: "gnoll".into() },
            TriggerAction::LogMessage("gnoll died".into()),
        ));

        engine.evaluate(&GameEvent::SpawnDied { name: "a skeleton".into() });
        assert_eq!(engine.triggers[0].fire_count, 0);
    }

    // ── disabled trigger ──────────────────────────────────────────────────────

    #[test]
    fn disabled_trigger_is_skipped() {
        let mut engine = TriggerEngine::new();
        let mut trigger = EventTrigger::new(
            "disabled",
            TriggerCondition::AnyDeath,
            TriggerAction::LogMessage("death".into()),
        );
        trigger.enabled = false;
        engine.add_trigger(trigger);

        let actions = engine.evaluate(&GameEvent::SpawnDied { name: "a gnoll".into() });
        assert!(actions.is_empty());
        assert_eq!(engine.triggers[0].fire_count, 0);
    }

    // ── multiple triggers matching same event ─────────────────────────────────

    #[test]
    fn multiple_triggers_can_match_same_event() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "any death log",
            TriggerCondition::AnyDeath,
            TriggerAction::LogMessage("something died".into()),
        ));
        engine.add_trigger(EventTrigger::new(
            "any death discord",
            TriggerCondition::AnyDeath,
            TriggerAction::DiscordAlert("mob down".into()),
        ));
        engine.add_trigger(EventTrigger::new(
            "gnoll death cmd",
            TriggerCondition::SpawnDeath { name_contains: "gnoll".into() },
            TriggerAction::SendIpcCommand("/say gnoll down".into()),
        ));

        let actions = engine.evaluate(&GameEvent::SpawnDied { name: "a gnoll shaman".into() });
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&TriggerAction::LogMessage("something died".into())));
        assert!(actions.contains(&TriggerAction::DiscordAlert("mob down".into())));
        assert!(actions.contains(&TriggerAction::SendIpcCommand("/say gnoll down".into())));

        assert_eq!(engine.triggers[0].fire_count, 1);
        assert_eq!(engine.triggers[1].fire_count, 1);
        assert_eq!(engine.triggers[2].fire_count, 1);
    }

    #[test]
    fn non_matching_trigger_not_included_in_multi_match() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "any death",
            TriggerCondition::AnyDeath,
            TriggerAction::LogMessage("death".into()),
        ));
        engine.add_trigger(EventTrigger::new(
            "chat watcher",
            TriggerCondition::ChatMessage { pattern: "tell".into() },
            TriggerAction::DiscordAlert("chat alert".into()),
        ));

        let actions = engine.evaluate(&GameEvent::SpawnDied { name: "a goblin".into() });
        assert_eq!(actions.len(), 1);
        assert_eq!(engine.triggers[0].fire_count, 1);
        assert_eq!(engine.triggers[1].fire_count, 0);
    }

    // ── action variants ───────────────────────────────────────────────────────

    #[test]
    fn send_ipc_command_action_returned() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "cmd trigger",
            TriggerCondition::PlayerDeath { character: "Kira".into() },
            TriggerAction::SendIpcCommand("/corpse".into()),
        ));
        let actions = engine.evaluate(&GameEvent::PlayerDied { character: "Kira".into() });
        assert_eq!(actions, vec![TriggerAction::SendIpcCommand("/corpse".into())]);
    }

    #[test]
    fn discord_alert_action_returned() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(EventTrigger::new(
            "discord trigger",
            TriggerCondition::LevelUp { character: "Kira".into() },
            TriggerAction::DiscordAlert("Kira dinged!".into()),
        ));
        let actions = engine.evaluate(&GameEvent::LeveledUp { character: "Kira".into() });
        assert_eq!(actions, vec![TriggerAction::DiscordAlert("Kira dinged!".into())]);
    }
}
