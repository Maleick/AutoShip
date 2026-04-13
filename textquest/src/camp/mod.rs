//! Camp loop state machine — pulls, fights, loots, meds, buffs.

/// Banking cycle controller — deposit plat, consolidate currency to a mule.
pub mod banking;
/// Buff tracking and rebuffing logic.
pub mod buffs;
/// Collectible and tribute management — collection quest progress, tribute automation.
pub mod collectibles;
/// Crowd control assignment and tracking.
pub mod cc;
/// Per-class ability configuration for the camp loop.
pub mod class_config;
/// Camp loop configuration — timers, thresholds, zone settings.
pub mod config;
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
/// Recovery phase — med, heal, rebuff between pulls.
pub mod recovery;
/// Camp loop state machine — idle, pulling, fighting, looting, recovering.
pub mod state;
/// Vendor automation — sell junk, buy supplies, inventory management.
pub mod vendor;
/// Skill leveling and training automation — tracks skill levels, mastery, and tradeskill sessions.
pub mod skill_tracker;
