//! Camp loop state machine — pulls, fights, loots, meds, buffs.

/// AA spend automation — prioritized alternate advancement point spending.
pub mod aa_spend;
/// Banking cycle controller — deposit plat, consolidate currency to a mule.
pub mod banking;
/// Buff tracking and rebuffing logic.
pub mod buffs;
/// Crowd control assignment and tracking.
pub mod cc;
/// Per-class ability configuration for the camp loop.
pub mod class_config;
/// Collectible and tribute management — collection quest progress, tribute
/// automation.
pub mod collectibles;
/// Camp loop configuration — timers, thresholds, zone settings.
pub mod config;
/// Event trigger system — configurable condition → action rules for game events
/// (OpenVanilla parity).
pub mod event_triggers;
/// Auto-forage automation — periodic `/forage` command dispatch.
pub mod forage;
/// Hunt mode — patrol-based pulling with waypoint routes.
pub mod hunt;
/// Loot rules — need/greed/pass, item filters, distribution.
pub mod loot;
/// Camp personality — per-character behavioral preferences.
pub mod personality;
/// Combat positioning — melee range, backstab, facing.
pub mod positioning;
/// Progression tracking — kill counts, XP rates, level milestones.
pub mod progression;
/// Puller logic — pull target selection, pathing, split management.
pub mod puller;
/// Quest tracking and task automation — objective progress, auto-completion,
/// reward claiming.
pub mod quest_tracker;
/// Recovery phase — med, heal, rebuff between pulls.
pub mod recovery;
/// Skill leveling and training automation — tracks skill levels, mastery, and
/// tradeskill sessions.
pub mod skill_tracker;
/// Camp loop state machine — idle, pulling, fighting, looting, recovering.
pub mod state;
/// Tell relaying and chat forwarding — OpenVanilla MQ2RelayTells parity.
pub mod tell_relay;
/// Vendor automation — sell junk, buy supplies, inventory management.
pub mod vendor;
