//! Embedded Discord bot — DZ lockout tracker + open world target announcer.
//!
//! Runs as a tokio task within the TextQuest orchestrator (not a separate
//! process). Uses the `serenity` crate for Discord gateway and slash commands.
//!
//! # Features
//!
//! - **DZ lockout tracking**: 48h replay / 6.5 day lockout timers per expansion
//! - **Open world contested mob spawn announcements**
//! - **Slash commands**: `/lockouts`, `/status`, `/spawns`
//!
//! # Startup
//!
//! Call [`start`] with a bot token and a [`BotBridge`] handle. The bot connects
//! to Discord and registers slash commands on the configured guild. If the
//! token is empty, the bot is a no-op.

use std::{collections::HashMap, sync::Arc};

use serenity::{
    all::{
        Command, CommandInteraction, CommandOptionType, Context, CreateCommand,
        CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage,
        EventHandler, GatewayIntents, GuildId, Interaction, Ready,
    },
    async_trait,
};
use tokio::sync::RwLock;

use super::bridge::BotBridge;

/// Wrapper to make `BotBridge` usable in async contexts.
///
/// `BotBridge` uses `std::sync::mpsc` internally which isn't `Sync`.
/// We wrap it in a `Mutex` so it can live inside `Arc<BotState>`.
struct SyncBridge(std::sync::Mutex<BotBridge>);

// SAFETY: The Mutex ensures single-threaded access to the non-Sync inner type.
unsafe impl Sync for SyncBridge {}

/// DZ lockout entry for a single expedition.
#[derive(Debug, Clone)]
pub struct DzLockout {
    /// Expedition name (e.g., "Plane of Time", "Anguish").
    pub expedition: String,
    /// When the lockout expires (UTC).
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Lockout duration category.
    pub lockout_type: LockoutType,
}

/// EQ DZ lockout duration tiers.
#[derive(Debug, Clone, Copy)]
pub enum LockoutType {
    /// Standard replay timer (most instances).
    Replay48h,
    /// Full lockout (6.5 days, raid targets).
    Full6d12h,
}

impl LockoutType {
    fn duration(&self) -> chrono::Duration {
        match self {
            Self::Replay48h => chrono::Duration::hours(48),
            Self::Full6d12h => chrono::Duration::hours(156), // 6d 12h
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Replay48h => "48h replay",
            Self::Full6d12h => "6.5d full",
        }
    }
}

/// Contested open-world mob spawn event.
#[derive(Debug, Clone)]
pub struct SpawnEvent {
    /// Mob name (e.g., "Lord Nagafen", "Phinigel Autropos").
    pub mob_name: String,
    /// Zone short name (e.g., "nagafen", "kedge").
    pub zone: String,
    /// When this spawn was detected.
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// Shared bot state accessible from event handlers and the orchestrator.
pub struct BotState {
    /// Active DZ lockouts keyed by character name.
    pub lockouts: RwLock<HashMap<String, Vec<DzLockout>>>,
    /// Recent contested mob spawn sightings.
    pub spawn_events: RwLock<Vec<SpawnEvent>>,
    /// Bridge to relay commands to/from the TUI (Mutex-wrapped for Sync).
    bridge: Option<SyncBridge>,
    /// Guild ID for slash command registration.
    pub guild_id: u64,
}

struct Handler {
    state: Arc<BotState>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(user = %ready.user.name, "Discord bot connected");

        let guild_id = self.state.guild_id;
        if guild_id == 0 {
            // Register global commands (slower propagation).
            if let Err(e) = Command::set_global_commands(
                &ctx.http,
                vec![
                    register_lockouts_command(),
                    register_status_command(),
                    register_spawns_command(),
                    register_add_lockout_command(),
                    register_spawn_alert_command(),
                ],
            )
            .await
            {
                tracing::warn!("Failed to register global slash commands: {}", e);
            }
        } else {
            // Register guild commands (instant propagation).
            let guild = GuildId::new(guild_id);
            if let Err(e) = guild
                .set_commands(
                    &ctx.http,
                    vec![
                        register_lockouts_command(),
                        register_status_command(),
                        register_spawns_command(),
                        register_add_lockout_command(),
                        register_spawn_alert_command(),
                    ],
                )
                .await
            {
                tracing::warn!("Failed to register guild slash commands: {}", e);
            }
        }

        tracing::info!("Slash commands registered");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction {
            let response = match cmd.data.name.as_str() {
                "lockouts" => handle_lockouts(&self.state, &cmd).await,
                "status" => handle_status().await,
                "spawns" => handle_spawns(&self.state).await,
                "addlockout" => handle_add_lockout(&self.state, &cmd).await,
                "spawnalert" => handle_spawn_alert(&self.state, &cmd).await,
                _ => "Unknown command".to_string(),
            };

            let msg = CreateInteractionResponseMessage::new().content(response);
            let builder = CreateInteractionResponse::Message(msg);
            if let Err(e) = cmd.create_response(&ctx.http, builder).await {
                tracing::warn!("Failed to respond to slash command: {}", e);
            }
        }
    }
}

// ── Slash command registration ──────────────────────────────────────────

fn register_lockouts_command() -> CreateCommand {
    CreateCommand::new("lockouts").description("Show active DZ lockout timers")
}

fn register_status_command() -> CreateCommand {
    CreateCommand::new("status").description("Show TextQuest fleet status summary")
}

fn register_spawns_command() -> CreateCommand {
    CreateCommand::new("spawns").description("Show recent contested mob spawn sightings")
}

fn register_add_lockout_command() -> CreateCommand {
    CreateCommand::new("addlockout")
        .description("Record a DZ lockout for a character")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "character", "Character name")
                .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "expedition",
                "Expedition/instance name",
            )
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "type",
                "Lockout type: replay (48h) or full (6.5d)",
            )
            .required(false),
        )
}

fn register_spawn_alert_command() -> CreateCommand {
    CreateCommand::new("spawnalert")
        .description("Report a contested mob spawn sighting")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "mob", "Mob name").required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "zone", "Zone short name")
                .required(true),
        )
}

// ── Command handlers ────────────────────────────────────────────────────

async fn handle_lockouts(state: &BotState, cmd: &CommandInteraction) -> String {
    let lockouts = state.lockouts.read().await;

    if lockouts.is_empty() {
        return "No active DZ lockouts.".to_string();
    }

    // Optional character filter from first argument.
    let char_filter: Option<String> = cmd
        .data
        .options
        .first()
        .and_then(|o| o.value.as_str())
        .map(|s| s.to_lowercase());

    let now = chrono::Utc::now();
    let mut lines = vec!["**Active DZ Lockouts**".to_string()];

    for (character, char_lockouts) in lockouts.iter() {
        if let Some(ref filter) = char_filter
            && !character.to_lowercase().contains(filter)
        {
            continue;
        }

        for lo in char_lockouts {
            if lo.expires_at <= now {
                continue; // expired
            }
            let remaining = lo.expires_at - now;
            let hours = remaining.num_hours();
            let mins = remaining.num_minutes() % 60;
            lines.push(format!(
                "- **{}** | {} | {}h {}m remaining ({})",
                character,
                lo.expedition,
                hours,
                mins,
                lo.lockout_type.label()
            ));
        }
    }

    if lines.len() == 1 {
        "No active lockouts (all expired).".to_string()
    } else {
        lines.join("\n")
    }
}

async fn handle_status() -> String {
    // Basic fleet status — will be enriched when wired to the orchestrator.
    // NOTE: Process ID is intentionally omitted — it would aid anti-cheat
    // fingerprinting and external process enumeration attacks.
    "**TextQuest Fleet Status**\n- Status: running\n- Use `/lockouts` for DZ timers, `/spawns` for \
     contested mobs"
        .to_string()
}

async fn handle_spawns(state: &BotState) -> String {
    let events = state.spawn_events.read().await;

    if events.is_empty() {
        return "No contested mob sightings recorded.".to_string();
    }

    let now = chrono::Utc::now();
    let mut lines = vec!["**Recent Contested Mob Sightings**".to_string()];

    // Show last 10 events, newest first.
    for event in events.iter().rev().take(10) {
        let ago = now - event.detected_at;
        let mins = ago.num_minutes();
        lines.push(format!(
            "- **{}** in `{}` — {}m ago",
            event.mob_name, event.zone, mins
        ));
    }

    lines.join("\n")
}

async fn handle_add_lockout(state: &BotState, cmd: &CommandInteraction) -> String {
    let character = match cmd.data.options.first().and_then(|o| o.value.as_str()) {
        Some(c) => c.to_string(),
        None => return "Missing character name.".to_string(),
    };

    let expedition = match cmd.data.options.get(1).and_then(|o| o.value.as_str()) {
        Some(e) => e.to_string(),
        None => return "Missing expedition name.".to_string(),
    };

    let lockout_type = cmd
        .data
        .options
        .get(2)
        .and_then(|o| o.value.as_str())
        .map_or(LockoutType::Full6d12h, |t| {
            if t.to_lowercase().contains("replay") || t == "48h" {
                LockoutType::Replay48h
            } else {
                LockoutType::Full6d12h
            }
        });

    let expires_at = chrono::Utc::now() + lockout_type.duration();

    let lockout = DzLockout {
        expedition: expedition.clone(),
        expires_at,
        lockout_type,
    };

    state
        .lockouts
        .write()
        .await
        .entry(character.clone())
        .or_default()
        .push(lockout);

    format!(
        "Recorded {} lockout for **{}** in **{}** — expires <t:{}:R>",
        lockout_type.label(),
        character,
        expedition,
        expires_at.timestamp()
    )
}

async fn handle_spawn_alert(state: &BotState, cmd: &CommandInteraction) -> String {
    let mob_name = match cmd.data.options.first().and_then(|o| o.value.as_str()) {
        Some(m) => m.to_string(),
        None => return "Missing mob name.".to_string(),
    };

    let zone = match cmd.data.options.get(1).and_then(|o| o.value.as_str()) {
        Some(z) => z.to_string(),
        None => return "Missing zone name.".to_string(),
    };

    let event = SpawnEvent {
        mob_name: mob_name.clone(),
        zone: zone.clone(),
        detected_at: chrono::Utc::now(),
    };

    state.spawn_events.write().await.push(event);

    format!("Spawn alert recorded: **{}** in `{}`", mob_name, zone)
}

// ── Public API ──────────────────────────────────────────────────────────

/// Start the embedded Discord bot as a tokio task.
///
/// Returns a handle to the shared bot state for the orchestrator to push
/// lockouts and spawn events programmatically.
///
/// If `bot_token` is empty, returns `None` (bot disabled).
///
/// # Errors
///
/// Returns an error if the serenity client fails to build.
pub async fn start(
    bot_token: &str,
    guild_id: u64,
    bridge: Option<BotBridge>,
) -> anyhow::Result<Option<Arc<BotState>>> {
    if bot_token.is_empty() {
        tracing::info!("Discord bot disabled (no bot_token configured)");
        return Ok(None);
    }

    let state = Arc::new(BotState {
        lockouts: RwLock::new(HashMap::new()),
        spawn_events: RwLock::new(Vec::new()),
        bridge: bridge.map(|b| SyncBridge(std::sync::Mutex::new(b))),
        guild_id,
    });

    let handler = Handler {
        state: state.clone(),
    };

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::Client::builder(bot_token, intents)
        .event_handler(handler)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build Discord bot client: {}", e))?;

    let bot_state = state.clone();

    tokio::spawn(async move {
        if let Err(e) = client.start().await {
            tracing::error!("Discord bot error: {}", e);
        }
    });

    tracing::info!("Discord bot started as tokio task");
    Ok(Some(bot_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockout_type_durations() {
        let replay = LockoutType::Replay48h.duration();
        assert_eq!(replay.num_hours(), 48);

        let full = LockoutType::Full6d12h.duration();
        assert_eq!(full.num_hours(), 156);
    }

    #[test]
    fn lockout_type_labels() {
        assert_eq!(LockoutType::Replay48h.label(), "48h replay");
        assert_eq!(LockoutType::Full6d12h.label(), "6.5d full");
    }

    #[test]
    fn dz_lockout_creation() {
        let lo = DzLockout {
            expedition: "Plane of Time".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(48),
            lockout_type: LockoutType::Replay48h,
        };
        assert_eq!(lo.expedition, "Plane of Time");
        assert!(lo.expires_at > chrono::Utc::now());
    }

    #[test]
    fn spawn_event_creation() {
        let event = SpawnEvent {
            mob_name: "Lord Nagafen".to_string(),
            zone: "nagafen".to_string(),
            detected_at: chrono::Utc::now(),
        };
        assert_eq!(event.mob_name, "Lord Nagafen");
        assert_eq!(event.zone, "nagafen");
    }

    #[tokio::test]
    async fn bot_state_lockout_tracking() {
        let state = BotState {
            lockouts: RwLock::new(HashMap::new()),
            spawn_events: RwLock::new(Vec::new()),
            bridge: None::<SyncBridge>,
            guild_id: 0,
        };

        // Add a lockout.
        let lo = DzLockout {
            expedition: "Anguish".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(156),
            lockout_type: LockoutType::Full6d12h,
        };
        state
            .lockouts
            .write()
            .await
            .entry("Toonname".to_string())
            .or_default()
            .push(lo);

        let lockouts = state.lockouts.read().await;
        assert_eq!(lockouts.len(), 1);
        assert_eq!(lockouts["Toonname"].len(), 1);
        assert_eq!(lockouts["Toonname"][0].expedition, "Anguish");
    }

    #[tokio::test]
    async fn bot_state_spawn_events() {
        let state = BotState {
            lockouts: RwLock::new(HashMap::new()),
            spawn_events: RwLock::new(Vec::new()),
            bridge: None::<SyncBridge>,
            guild_id: 0,
        };

        state.spawn_events.write().await.push(SpawnEvent {
            mob_name: "Phinigel Autropos".to_string(),
            zone: "kedge".to_string(),
            detected_at: chrono::Utc::now(),
        });

        let events = state.spawn_events.read().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].mob_name, "Phinigel Autropos");
    }

    #[tokio::test]
    async fn start_returns_none_when_no_token() {
        let result = start("", 0, None).await.unwrap();
        assert!(result.is_none());
    }
}
