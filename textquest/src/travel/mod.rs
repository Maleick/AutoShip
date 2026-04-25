//! Travel planning subsystem — portal database, zone routing utilities, and
//! group travel coordination plans.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub mod portals;

pub use portals::{Portal, PortalDatabase, PortalDatabaseFile, PortalKind, ZoneSafeCamp};

const DEFAULT_GROUP_WAIT_SECS: u64 = 120;
const DEFAULT_PORTAL_WINDOW_SECS: u64 = 30;

/// A 3D EverQuest location.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TravelPoint {
    /// East/west coordinate.
    pub x: f32,
    /// North/south coordinate.
    pub y: f32,
    /// Vertical coordinate.
    pub z: f32,
}

impl From<[f32; 3]> for TravelPoint {
    fn from(coords: [f32; 3]) -> Self {
        Self {
            x: coords[0],
            y: coords[1],
            z: coords[2],
        }
    }
}

/// Per-zone travel destination loaded from `config/travel.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelDestination {
    /// Optional gather point before a portal, spire, or translocator.
    #[serde(default)]
    pub portal_location: Option<TravelPoint>,
    /// Optional safe meeting point after zoning.
    #[serde(default)]
    pub safe_camp: Option<TravelPoint>,
    /// Spells or click actions required for this destination.
    #[serde(default)]
    pub requires_spells: Vec<String>,
    /// Ordered zone short names for non-portal travel.
    #[serde(default)]
    pub zone_path: Vec<String>,
    /// Max wait for slow group members at gather/regroup points.
    #[serde(default = "default_group_wait_secs")]
    pub group_wait_secs: u64,
    /// Time to keep the group together while a temporary portal is active.
    #[serde(default = "default_portal_window_secs")]
    pub portal_window_secs: u64,
}

impl Default for TravelDestination {
    fn default() -> Self {
        Self {
            portal_location: None,
            safe_camp: None,
            requires_spells: Vec::new(),
            zone_path: Vec::new(),
            group_wait_secs: DEFAULT_GROUP_WAIT_SECS,
            portal_window_secs: DEFAULT_PORTAL_WINDOW_SECS,
        }
    }
}

fn default_group_wait_secs() -> u64 {
    DEFAULT_GROUP_WAIT_SECS
}

fn default_portal_window_secs() -> u64 {
    DEFAULT_PORTAL_WINDOW_SECS
}

/// On-disk TOML format for travel destinations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TravelConfigFile {
    /// Zone short name to destination metadata, loaded from `[travel.<zone>]`.
    #[serde(default)]
    pub travel: BTreeMap<String, TravelDestination>,
}

/// On-disk TOML format for point-of-interest targets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoiFile {
    /// Point-of-interest definitions loaded from `[[poi]]` entries.
    #[serde(default)]
    pub poi: Vec<PoiDefinition>,
}

/// Single POI definition from `config/poi/<zone>.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoiDefinition {
    /// Canonical name used for `:find` input matching.
    pub name: String,
    /// Search text to identify the spawned target in EQ spawn lists.
    #[serde(rename = "spawn_search")]
    pub spawn_search: String,
    /// Optional explicit point override to path directly to a coordinate.
    #[serde(default)]
    pub pos: Option<TravelPoint>,
    /// Optional slash commands to run after arrival.
    #[serde(default)]
    pub post_arrival: Vec<String>,
}

/// Resolved POI route target used by TUI routing.
#[derive(Debug, Clone, PartialEq)]
pub struct FindMatch {
    /// Human-facing POI name.
    pub poi_name: String,
    /// Destination zone short name.
    pub zone: String,
    /// Fallback spawn query string when `position` is not set.
    pub spawn_search: String,
    /// Optional direct destination coordinate.
    pub position: Option<TravelPoint>,
    /// Slash commands to send after arrival.
    pub post_arrival: Vec<String>,
}

impl FindMatch {
    fn from_entry(zone: String, poi: &PoiDefinition) -> Self {
        let mut post_arrival = poi.post_arrival.clone();

        if post_arrival.is_empty() {
            post_arrival.extend_from_slice(&infer_default_post_arrival(&poi.spawn_search));
        }

        Self {
            poi_name: poi.name.clone(),
            zone,
            spawn_search: poi.spawn_search.clone(),
            position: poi.pos,
            post_arrival,
        }
    }
}

fn infer_default_post_arrival(spawn_search: &str) -> Vec<String> {
    let target = spawn_search.trim();
    if target.is_empty() {
        return Vec::new();
    }

    let mut hooks = vec![format!("/target {}", target)];
    let lower = target.to_ascii_lowercase();
    hooks.push(format!("/hail {}", target));
    if lower == "banker" {
        hooks.push(String::from("/open bank"));
    }
    hooks
}

#[derive(Debug, Clone)]
struct ZonePoiEntry {
    zone: String,
    definition: PoiDefinition,
}

/// Resolver that combines per-zone POI tables and optional zone graph adjacency.
#[derive(Debug, Clone)]
pub struct FindRouter {
    /// Directory containing one `config/poi/<zone>.toml` per zone.
    poi_dir: PathBuf,
}

impl FindRouter {
    /// Default POI directory: `config/poi`.
    #[must_use]
    pub fn default_path() -> PathBuf {
        Path::new("config/poi").to_path_buf()
    }

    /// Construct a router using a specific POI directory.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            poi_dir: path.into(),
        }
    }

    /// Construct a router using the default POI directory.
    #[must_use]
    pub fn load_default() -> Self {
        Self::new(Self::default_path())
    }

    /// All configured POI names (deduplicated by normalized form).
    pub fn all_poi_names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = self
            .load_entries()?
            .into_iter()
            .map(|entry| entry.definition.name)
            .collect();
        names.sort_by_key(|s| normalize_name(s));
        names.dedup_by_key(|s| normalize_name(s));
        Ok(names)
    }

    /// Resolve a requested POI.
    ///
    /// Order of resolution:
    /// 1) explicit destination zone,
    /// 2) current zone,
    /// 3) adjacent zone in `zone_graph`.
    pub fn resolve(
        &self,
        poi_query: &str,
        current_zone: Option<&str>,
        destination_zone: Option<&str>,
        zone_graph: Option<&textquest_common::nav::ZoneGraph>,
    ) -> Result<Option<FindMatch>> {
        let normalized = normalize_name(poi_query);
        if normalized.is_empty() {
            return Ok(None);
        }

        let entries = self.load_entries()?;
        if entries.is_empty() {
            return Ok(None);
        }

        if let Some(zone) = destination_zone {
            if let Some(found) = find_in_zone(&entries, zone, &normalized) {
                return Ok(Some(found));
            }
            return Ok(None);
        }

        if let Some(zone) = current_zone
            && let Some(found) = find_in_zone(&entries, zone, &normalized)
        {
            return Ok(Some(found));
        }

        if let (Some(graph), Some(zone)) = (zone_graph, current_zone) {
            if let Some(found) = find_in_adjacent(
                &entries,
                graph,
                zone,
                &normalized,
            ) {
                return Ok(Some(found));
            }
        }

        Ok(None)
    }

    fn load_entries(&self) -> Result<Vec<ZonePoiEntry>> {
        let mut entries: Vec<ZonePoiEntry> = Vec::new();

        let path = &self.poi_dir;
        if !path.exists() {
            return Ok(Vec::new());
        }

        for file in fs::read_dir(path)
            .with_context(|| format!("Failed to read POI directory {}", path.display()))?
        {
            let file = file.with_context(|| format!("Failed to read POI entry in {}", path.display()))?;
            let p = file.path();
            if !p.is_file() {
                continue;
            }

            if p.extension().is_none_or(|ext| ext != "toml") {
                continue;
            }

            let zone = p
                .file_stem()
                .and_then(|s| s.to_str())
                .map(normalize_name)
                .unwrap_or_default();
            if zone.is_empty() {
                continue;
            }

            let raw = fs::read_to_string(&p).with_context(|| {
                format!("Failed to read POI file {}", p.display())
            })?;
            let file: PoiFile = toml::from_str(&raw).with_context(|| {
                format!("Failed to parse POI file {}", p.display())
            })?;

            let mut file_items = file
                .poi
                .into_iter()
                .filter(|poi| !poi.name.trim().is_empty())
                .map(|poi| ZonePoiEntry {
                    zone: zone.clone(),
                    definition: poi,
                })
                .collect::<Vec<_>>();

            entries.append(&mut file_items);
        }

        entries.sort_by(|a, b| normalize_name(&a.definition.name).cmp(&normalize_name(&b.definition.name)));
        Ok(entries)
    }
}

fn normalize_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '_', '-'], "")
}

fn find_in_zone(
    entries: &[ZonePoiEntry],
    zone: &str,
    poi_query: &str,
) -> Option<FindMatch> {
    let zone = normalize_name(zone);
    entries
        .iter()
        .find(|entry| {
            normalize_name(&entry.zone) == zone
                && normalize_name(&entry.definition.name) == poi_query
        })
        .map(|entry| FindMatch::from_entry(entry.zone.clone(), &entry.definition))
}

fn find_in_adjacent(
    entries: &[ZonePoiEntry],
    graph: &textquest_common::nav::ZoneGraph,
    current_zone: &str,
    poi_query: &str,
) -> Option<FindMatch> {
    let current_zone_id = graph
        .zones
        .iter()
        .find(|(_, node)| normalize_zone(node.name.as_str()) == normalize_zone(current_zone))
        .map(|(id, _)| *id);
    let current_zone_id = current_zone_id?;

    let node = graph.zones.get(&current_zone_id)?;
    for conn in &node.connections {
        if conn.disabled {
            continue;
        }
        let zone_id = conn.dest_zone_id;
        let zone_name = graph
            .zones
            .get(&zone_id)
            .map(|n| n.name.as_str())
            .unwrap_or_default();

        if let Some(found) = find_in_zone(entries, zone_name, poi_query) {
            return Some(found);
        }
    }
    None
}

fn normalize_zone(zone: &str) -> String {
    zone.trim().to_ascii_lowercase().replace([' ', '_', '-'], "")
}

impl TravelConfigFile {
    /// Load `config/travel.toml`, returning an empty config when it is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_or_default(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read travel config: {}", path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("Failed to parse travel config: {}", path.display()))
    }

    /// Lookup a configured destination by zone short name.
    #[must_use]
    pub fn destination(&self, zone: &str) -> Option<&TravelDestination> {
        let zone = normalize_zone(zone);
        self.travel
            .iter()
            .find(|(name, _)| normalize_zone(name) == zone)
            .map(|(_, destination)| destination)
    }
}

/// Ordered action type in a group travel plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TravelStepKind {
    /// Move every group member to the portal or travel start.
    GatherAtPortal,
    /// Wait until every expected group member reaches the current waypoint.
    WaitForGroup,
    /// Cast a port spell, trigger a spire, or use a translocator.
    SummonGroup,
    /// Cross the zone line or complete the teleport transition.
    ZoneTransition,
    /// Move the group to a safe destination camp.
    RegroupAtSafeCamp,
}

/// One executable step in a planned group travel route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelStep {
    /// Step type.
    pub kind: TravelStepKind,
    /// Human-readable action summary.
    pub label: String,
    /// Target zone for this step.
    pub zone: String,
    /// Optional movement target for nav integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TravelPoint>,
    /// Optional slash command that can be sent to the nav/cast layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Step timeout in seconds.
    pub timeout_secs: u64,
}

/// Planned route and coordination metadata for a destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelPlan {
    /// Destination zone short name.
    pub destination_zone: String,
    /// Portal selected from the portal database, if one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portal: Option<Portal>,
    /// Destination safe camp selected from travel config or built-in camps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_camp: Option<TravelPoint>,
    /// Required spells or actions for the selected route.
    pub required_actions: Vec<String>,
    /// Ordered coordination steps.
    pub steps: Vec<TravelStep>,
}

/// Minimal planning interface for the travel subsystem.
pub trait TravelPlanner {
    /// Build an ordered coordination plan for `destination_zone`.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination is empty or no configured route,
    /// portal, or safe camp is known.
    fn travel_to(&self, destination_zone: &str) -> Result<TravelPlan>;
}

/// Travel coordinator that combines destination config with portal metadata.
pub struct TravelCoordinator {
    config: TravelConfigFile,
    portals: PortalDatabase,
}

impl TravelCoordinator {
    /// Default config path: `config/travel.toml`.
    #[must_use]
    pub fn default_config_path() -> PathBuf {
        Path::new("config/travel.toml").to_path_buf()
    }

    /// Load travel config and portal data from default paths.
    ///
    /// # Errors
    ///
    /// Returns an error if either existing config file cannot be parsed.
    pub fn load_default() -> Result<Self> {
        Ok(Self {
            config: TravelConfigFile::load_or_default(Self::default_config_path())?,
            portals: PortalDatabase::load_default()?,
        })
    }

    /// Construct directly from in-memory config and portal data.
    #[must_use]
    pub fn new(config: TravelConfigFile, portals: PortalDatabase) -> Self {
        Self { config, portals }
    }

    /// Build just the portal-gather step for script integrations.
    ///
    /// # Errors
    ///
    /// Returns an error when no configured portal gather point is available.
    pub fn gather_at_portal(&self, destination_zone: &str) -> Result<TravelStep> {
        self.travel_to(destination_zone)?
            .steps
            .into_iter()
            .find(|step| step.kind == TravelStepKind::GatherAtPortal)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No portal gather point configured for '{}'",
                    destination_zone
                )
            })
    }

    /// Build just the summon/portal step for script integrations.
    ///
    /// # Errors
    ///
    /// Returns an error when no portal or configured spell/action is available.
    pub fn summon_group(&self, destination_zone: &str) -> Result<TravelStep> {
        self.travel_to(destination_zone)?
            .steps
            .into_iter()
            .find(|step| step.kind == TravelStepKind::SummonGroup)
            .ok_or_else(|| anyhow::anyhow!("No portal action available for '{}'", destination_zone))
    }

    fn configured_destination(&self, destination_zone: &str) -> Option<&TravelDestination> {
        self.config.destination(destination_zone)
    }

    fn preferred_portal(&self, destination_zone: &str) -> Option<&Portal> {
        let mut portals = self.portals.portals_to_zone(destination_zone, u8::MAX);
        portals.sort_by_key(|portal| (!portal.group_port, portal.min_level));
        portals.into_iter().next()
    }
}

impl TravelPlanner for TravelCoordinator {
    fn travel_to(&self, destination_zone: &str) -> Result<TravelPlan> {
        let destination_zone = normalize_zone(destination_zone);
        if destination_zone.is_empty() {
            anyhow::bail!("destination zone is required");
        }

        let configured = self.configured_destination(&destination_zone);
        let portal = self.preferred_portal(&destination_zone).cloned();
        let safe_camp = configured
            .and_then(|destination| destination.safe_camp)
            .or_else(|| {
                self.portals
                    .safe_camp_for(&destination_zone)
                    .map(|camp| TravelPoint::from(camp.coords))
            })
            .or_else(|| {
                portal
                    .as_ref()
                    .and_then(|portal| portal.landing_coords.map(TravelPoint::from))
            });

        if configured.is_none() && portal.is_none() && safe_camp.is_none() {
            anyhow::bail!("No travel route known for '{}'", destination_zone);
        }

        let destination = configured.cloned().unwrap_or_default();
        let mut required_actions = destination.requires_spells;
        if required_actions.is_empty()
            && let Some(spell_name) = portal.as_ref().and_then(|portal| portal.spell_name.clone())
        {
            required_actions.push(spell_name);
        }

        let mut steps = Vec::new();
        if let Some(portal_location) = destination.portal_location {
            steps.push(nav_step(
                TravelStepKind::GatherAtPortal,
                format!("Gather group for {}", destination_zone),
                destination_zone.clone(),
                portal_location,
                destination.group_wait_secs,
            ));
            steps.push(wait_step(
                "Wait for slowest group member at portal",
                destination_zone.clone(),
                destination.group_wait_secs,
            ));
        }

        if let Some(action) = required_actions.first() {
            steps.push(TravelStep {
                kind: TravelStepKind::SummonGroup,
                label: format!("Activate group travel with {action}"),
                zone: destination_zone.clone(),
                target: None,
                command: Some(format!("/cast \"{action}\"")),
                timeout_secs: destination.portal_window_secs,
            });
        }

        for zone in destination.zone_path {
            steps.push(TravelStep {
                kind: TravelStepKind::ZoneTransition,
                label: format!("Travel through {zone}"),
                zone,
                target: None,
                command: None,
                timeout_secs: destination.group_wait_secs,
            });
            steps.push(wait_step(
                "Regroup after zone transition",
                destination_zone.clone(),
                destination.group_wait_secs,
            ));
        }

        steps.push(TravelStep {
            kind: TravelStepKind::ZoneTransition,
            label: format!("Arrive in {}", destination_zone),
            zone: destination_zone.clone(),
            target: None,
            command: None,
            timeout_secs: destination.group_wait_secs,
        });

        if let Some(camp) = safe_camp {
            steps.push(nav_step(
                TravelStepKind::RegroupAtSafeCamp,
                format!("Regroup at safe camp in {}", destination_zone),
                destination_zone.clone(),
                camp,
                destination.group_wait_secs,
            ));
            steps.push(wait_step(
                "Confirm all group members reached safe camp",
                destination_zone.clone(),
                destination.group_wait_secs,
            ));
        }

        Ok(TravelPlan {
            destination_zone,
            portal,
            safe_camp,
            required_actions,
            steps,
        })
    }
}

/// Build a travel plan from default config and portal data.
///
/// # Errors
///
/// Returns an error if config loading fails or no route is known.
pub fn travel_to(destination_zone: &str) -> Result<TravelPlan> {
    TravelCoordinator::load_default()?.travel_to(destination_zone)
}

/// Build the portal-gather step from default config and portal data.
///
/// # Errors
///
/// Returns an error if config loading fails or no portal gather point is known.
pub fn gather_at_portal(destination_zone: &str) -> Result<TravelStep> {
    TravelCoordinator::load_default()?.gather_at_portal(destination_zone)
}

/// Build the portal/summon step from default config and portal data.
///
/// # Errors
///
/// Returns an error if config loading fails or no portal action is known.
pub fn summon_group(destination_zone: &str) -> Result<TravelStep> {
    TravelCoordinator::load_default()?.summon_group(destination_zone)
}

fn nav_step(
    kind: TravelStepKind,
    label: String,
    zone: String,
    target: TravelPoint,
    timeout_secs: u64,
) -> TravelStep {
    TravelStep {
        kind,
        label,
        zone,
        target: Some(target),
        command: Some(format!("/nav loc {} {} {}", target.x, target.y, target.z)),
        timeout_secs,
    }
}

fn wait_step(label: &str, zone: String, timeout_secs: u64) -> TravelStep {
    TravelStep {
        kind: TravelStepKind::WaitForGroup,
        label: label.to_string(),
        zone,
        target: None,
        command: None,
        timeout_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinator() -> TravelCoordinator {
        let raw = r#"
            [travel.poknowledge]
            portal_location = { x = 100.0, y = 200.0, z = 3.0 }
            safe_camp = { x = 150.0, y = 250.0, z = 3.0 }
            requires_spells = ["Translocate", "Evacuate"]
        "#;
        let config: TravelConfigFile = toml::from_str(raw).expect("travel config");
        let portals =
            PortalDatabase::from_builtin_defaults(PathBuf::from("config/portals/portals.toml"));
        TravelCoordinator::new(config, portals)
    }

    #[test]
    fn travel_to_builds_configured_group_plan() {
        let plan = coordinator().travel_to("PoKnowledge").expect("plan");

        assert_eq!(plan.destination_zone, "poknowledge");
        assert_eq!(plan.required_actions, vec!["Translocate", "Evacuate"]);
        assert_eq!(
            plan.safe_camp,
            Some(TravelPoint {
                x: 150.0,
                y: 250.0,
                z: 3.0
            })
        );
        assert_eq!(plan.steps[0].kind, TravelStepKind::GatherAtPortal);
        assert_eq!(plan.steps[1].kind, TravelStepKind::WaitForGroup);
        assert!(
            plan.steps
                .iter()
                .any(|step| step.kind == TravelStepKind::RegroupAtSafeCamp)
        );
    }

    #[test]
    fn summon_group_returns_configured_cast_step() {
        let step = coordinator()
            .summon_group("poknowledge")
            .expect("summon step");

        assert_eq!(step.kind, TravelStepKind::SummonGroup);
        assert_eq!(step.command.as_deref(), Some("/cast \"Translocate\""));
    }

    #[test]
    fn built_in_portal_database_can_seed_unconfigured_destination() {
        let coordinator = TravelCoordinator::new(
            TravelConfigFile::default(),
            PortalDatabase::from_builtin_defaults(""),
        );

        let plan = coordinator.travel_to("iceclad").expect("plan");

        assert_eq!(plan.destination_zone, "iceclad");
        assert!(plan.portal.is_some());
        assert!(plan.safe_camp.is_some());
        assert!(!plan.required_actions.is_empty());
    }

    #[test]
    fn resolve_uses_adjacent_zone_when_current_zone_has_no_match() {
        use textquest_common::nav::{ZoneConnection, ZoneGraph, ZoneNode};

        let mut graph = ZoneGraph::default();
        graph.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: String::from("qeynos"),
                min_level: 1,
                max_level: 100,
                connections: vec![ZoneConnection {
                    dest_zone_id: 2,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        graph.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: String::from("nro"),
                min_level: 1,
                max_level: 100,
                connections: Vec::new(),
            },
        );

        let tmp = std::env::temp_dir().join(format!("textquest-find-router-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create fixture dir");

        std::fs::write(tmp.join("qeynos.toml"), r#"
            [[poi]]
            name = "Mage"
            spawn_search = "Apprentice Mage"
        "#)
        .expect("write current zone poi fixture");
        std::fs::write(tmp.join("nro.toml"), r#"
            [[poi]]
            name = "Banker"
            spawn_search = "Moklin Bankkeeper"
        "#)
        .expect("write adjacent zone poi fixture");

        let router = FindRouter::new(&tmp);
        let found = router
            .resolve("banker", Some("qeynos"), None, Some(&graph))
            .expect("resolve")
            .expect("expected fallback to adjacent zone match");

        assert_eq!(found.poi_name, "Banker");
        assert_eq!(found.zone, "nro");
    }
}
