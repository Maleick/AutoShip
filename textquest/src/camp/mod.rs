//! Camp loop state machine — pulls, fights, loots, meds, buffs.

/// AA spend automation — prioritized alternate advancement point spending.
#[cfg(windows)]
pub mod aa_spend;
/// Banking cycle controller — deposit plat, consolidate currency to a mule.
#[cfg(windows)]
pub mod banking;
/// Buff tracking and rebuffing logic.
#[cfg(windows)]
pub mod buffs;
/// Crowd control assignment and tracking.
#[cfg(windows)]
pub mod cc;
/// Per-class ability configuration for the camp loop.
#[cfg(windows)]
pub mod class_config;
/// Collectible and tribute management — collection quest progress, tribute
/// automation.
#[cfg(windows)]
pub mod collectibles;
/// Camp loop configuration — timers, thresholds, zone settings.
#[cfg(windows)]
pub mod config;
/// Event trigger system — configurable condition → action rules for game events
/// (OpenVanilla parity).
#[cfg(windows)]
pub mod event_triggers;
/// Auto-forage automation — periodic `/forage` command dispatch.
#[cfg(windows)]
pub mod forage;
/// Hunt mode — patrol-based pulling with waypoint routes.
#[cfg(windows)]
pub mod hunt;
/// Loot rules — need/greed/pass, item filters, distribution.
pub mod loot;
/// Camp personality — per-character behavioral preferences.
pub mod personality;
/// Combat positioning — melee range, backstab, facing.
#[cfg(windows)]
pub mod positioning;
/// Progression tracking — kill counts, XP rates, level milestones.
#[cfg(windows)]
pub mod progression;
/// Puller logic — pull target selection, pathing, split management.
#[cfg(windows)]
pub mod puller;
/// Quest tracking and task automation — objective progress, auto-completion,
/// reward claiming.
#[cfg(windows)]
pub mod quest_tracker;
/// Recovery phase — med, heal, rebuff between pulls.
#[cfg(windows)]
pub mod recovery;
/// Skill leveling and training automation — tracks skill levels, mastery, and
/// tradeskill sessions.
#[cfg(windows)]
pub mod skill_tracker;
/// Camp loop state machine — idle, pulling, fighting, looting, recovering.
#[cfg(windows)]
pub mod state;
/// Tell relaying and chat forwarding — OpenVanilla MQ2RelayTells parity.
#[cfg(windows)]
pub mod tell_relay;
/// Vendor automation — sell junk, buy supplies, inventory management.
#[cfg(windows)]
pub mod vendor;
