//! EverQuest data layer — spawn reading, log parsing, map loading, named mob
//! tracking.

/// Anti-cheat state reads for EQ memory inspection.
pub mod cheater;
/// Game Master detection — zone-wide GM alerts with MQ2GMCheck parity.
pub mod gm_detector;
/// High-value target definitions and alert configuration.
pub mod hvt;
/// EQ log file parser — chat channels, loot events, combat messages.
pub mod log_parser;
/// Real-time log file watcher with tail-follow semantics.
pub mod log_watcher;
/// Map data population — terrain, spawns, portals, blockers.
pub mod map_data;
/// SOE `.map` file parser — lines and points for zone map overlays.
pub mod map_parser;
/// Named mob database — spawn names, respawn timers, loot tables.
pub mod named_db;
/// Named mob tracker — live tracking of named spawns across zones.
pub mod named_tracker;
/// Spawn linked list traversal — reads all spawns from EQ memory.
pub mod spawn;
/// Spawn alert feed — pattern-matched spawn notifications and named alerts.
pub mod spawn_alert;
/// Advanced spawn filtering, sorting, and named-mob tracking.
pub mod spawn_filter;
/// EQ data structures — `SpawnInfo`, `GroupInfo`, class/type enums.
pub mod structs;
