use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::paths;

pub const DEFAULT_CAMP_CLUSTER_EPS: f32 = 250.0;
pub const DEFAULT_CAMP_CLUSTER_MIN_SAMPLES: usize = 2;
pub const DEFAULT_CAMP_DB_FILENAME: &str = "eq_data.db";
pub const DEFAULT_CAMP_OVERRIDE_PATH: &str = ".textquest/camp_overrides.yaml";

/// Source of a camp record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampSource {
    PeqDerived,
    OperatorOverride,
}

impl CampSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::PeqDerived => "peq-derived",
            Self::OperatorOverride => "operator-override",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "peq-derived" => Ok(Self::PeqDerived),
            "operator-override" => Ok(Self::OperatorOverride),
            other => bail!("Unknown camp source: {other}"),
        }
    }
}

/// Canonical camp row stored in the knowledge database and surfaced by the CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Camp {
    pub camp_id: String,
    pub zone_id: i32,
    pub zone_short_name: String,
    pub spawngroup_ids: Vec<i32>,
    pub centroid_xyz: [f32; 3],
    pub level_min: Option<u8>,
    pub level_max: Option<u8>,
    pub mob_families: Vec<String>,
    pub source: CampSource,
}

/// Raw spawn2 row from the PEQ knowledge snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spawn2Row {
    pub zone_id: i32,
    pub zone_short_name: String,
    pub spawngroup_id: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Raw spawngroup row from the PEQ knowledge snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawngroupRow {
    pub zone_id: i32,
    pub zone_short_name: String,
    pub spawngroup_id: i32,
}

/// Raw spawnentry row from the PEQ knowledge snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnentryRow {
    pub spawngroup_id: i32,
    pub npc_id: i32,
    pub npc_name: Option<String>,
    pub level: Option<u8>,
}

/// In-memory PEQ snapshot used by the camp enumerator.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PeqSnapshot {
    #[serde(default)]
    pub spawn2: Vec<Spawn2Row>,
    #[serde(default)]
    pub spawngroups: Vec<SpawngroupRow>,
    #[serde(default)]
    pub spawnentries: Vec<SpawnentryRow>,
}

/// Operator override YAML file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CampOverrideFile {
    #[serde(default)]
    pub merge: Vec<CampMergeOverride>,
    #[serde(default)]
    pub split: Vec<CampSplitOverride>,
}

/// Merge directive in the operator override file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampMergeOverride {
    pub camp_ids: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Split directive in the operator override file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampSplitOverride {
    pub camp_id: String,
    pub partitions: Vec<CampSplitPartition>,
    #[serde(default)]
    pub note: Option<String>,
}

/// One partition produced by a split directive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampSplitPartition {
    pub spawngroup_ids: Vec<i32>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Audit trail entry for camp refreshes and overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampAuditEntry {
    pub action: CampAuditAction,
    pub camp_id: String,
    pub source_camp_ids: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub before: Option<Camp>,
    #[serde(default)]
    pub after: Option<Camp>,
}

/// Audit action kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampAuditAction {
    RefreshInsert,
    RefreshUpdate,
    RefreshDelete,
    OverrideMerge,
    OverrideSplit,
}

impl CampOverrideFile {
    pub fn load_default() -> Result<Self> {
        let path = default_override_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read camp override file {}", path.display()))?;
        let overrides = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse camp override file {}", path.display()))?;
        Ok(overrides)
    }

    pub fn fingerprint(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(self).context("Failed to serialize camp overrides")?;
        Ok(sha256_hex(yaml.as_bytes()))
    }
}

/// SQLite-backed camp catalog.
pub struct CampDatabase {
    conn: Connection,
}

impl CampDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create camp DB directory {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open camp DB {}", path.display()))?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_default() -> Result<Self> {
        Self::open(default_db_path())
    }

    pub fn load_base_camps(&self) -> Result<Vec<Camp>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT camp_id,
                   zone_id,
                   zone_short_name,
                   spawngroup_ids_json,
                   centroid_x,
                   centroid_y,
                   centroid_z,
                   level_min,
                   level_max,
                   mob_families_json,
                   source
            FROM camps
            ORDER BY zone_id, zone_short_name, camp_id
            "#,
        )?;

        let camps = stmt
            .query_map([], camp_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(camps)
    }

    pub fn load_effective_camps(&mut self) -> Result<Vec<Camp>> {
        let base_camps = self.load_base_camps()?;
        let overrides = CampOverrideFile::load_default()?;
        let override_fingerprint = overrides.fingerprint()?;
        let stored_fingerprint = self.meta_value("override_fingerprint")?;
        let (effective, audit_entries) = apply_overrides(base_camps, &overrides)?;

        if stored_fingerprint.as_deref() != Some(override_fingerprint.as_str()) {
            self.record_audit_entries(&audit_entries)?;
            self.set_meta("override_fingerprint", &override_fingerprint)?;
        }

        Ok(effective)
    }

    pub fn get_effective_camp(&mut self, camp_id: &str) -> Result<Option<Camp>> {
        let camps = self.load_effective_camps()?;
        Ok(camps
            .into_iter()
            .find(|camp| camp.camp_id.eq_ignore_ascii_case(camp_id)))
    }

    pub fn refresh_from_snapshot(&mut self, snapshot: &PeqSnapshot) -> Result<CampRefreshReport> {
        let new_camps = build_camps(snapshot);
        let old_camps = self.load_base_camps()?;
        let diff = CampRefreshDiff::from_sets(&old_camps, &new_camps);

        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM camps", [])?;
        for camp in &new_camps {
            insert_camp_row(&tx, camp)?;
        }
        tx.commit()?;

        self.record_audit_entries(&diff.to_audit_entries())?;

        Ok(CampRefreshReport {
            inserted: diff.inserted.len(),
            updated: diff.updated.len(),
            deleted: diff.deleted.len(),
            total: new_camps.len(),
        })
    }

    pub fn list_with_zone_filter(&mut self, zone: Option<&str>) -> Result<Vec<Camp>> {
        let camps = self.load_effective_camps()?;
        Ok(match zone {
            Some(zone) => camps
                .into_iter()
                .filter(|camp| camp_matches_zone(camp, zone))
                .collect(),
            None => camps,
        })
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS camps (
                camp_id TEXT PRIMARY KEY NOT NULL,
                zone_id INTEGER NOT NULL,
                zone_short_name TEXT NOT NULL,
                spawngroup_ids_json TEXT NOT NULL,
                centroid_x REAL NOT NULL,
                centroid_y REAL NOT NULL,
                centroid_z REAL NOT NULL,
                level_min INTEGER,
                level_max INTEGER,
                mob_families_json TEXT NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_camps_zone_short_name
                ON camps(zone_short_name);

            CREATE TABLE IF NOT EXISTS camp_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                camp_id TEXT NOT NULL,
                source_camp_ids_json TEXT NOT NULL,
                before_json TEXT,
                after_json TEXT,
                note TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS camp_meta (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    fn record_audit_entries(&mut self, entries: &[CampAuditEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let tx = self.conn.transaction()?;
        for entry in entries {
            tx.execute(
                r#"
                INSERT INTO camp_audit_log (
                    action,
                    camp_id,
                    source_camp_ids_json,
                    before_json,
                    after_json,
                    note
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    entry.action.as_str(),
                    &entry.camp_id,
                    serde_json::to_string(&entry.source_camp_ids)?,
                    entry.before.as_ref().map(serde_json::to_string).transpose()?,
                    entry.after.as_ref().map(serde_json::to_string).transpose()?,
                    entry.note.clone(),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn meta_value(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM camp_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn set_meta(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO camp_meta (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![key, value],
        )?;
        Ok(())
    }
}

/// Summary returned after refreshing the camp table from a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampRefreshReport {
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
    pub total: usize,
}

#[derive(Debug, Clone)]
struct CampRefreshDiff {
    inserted: Vec<Camp>,
    updated: Vec<(Camp, Camp)>,
    deleted: Vec<Camp>,
}

impl CampRefreshDiff {
    fn from_sets(old: &[Camp], new: &[Camp]) -> Self {
        let old_map: BTreeMap<String, Camp> = old
            .iter()
            .cloned()
            .map(|camp| (camp.camp_id.clone(), camp))
            .collect();
        let new_map: BTreeMap<String, Camp> = new
            .iter()
            .cloned()
            .map(|camp| (camp.camp_id.clone(), camp))
            .collect();

        let inserted = new_map
            .iter()
            .filter_map(|(camp_id, camp)| {
                if old_map.contains_key(camp_id) {
                    None
                } else {
                    Some(camp.clone())
                }
            })
            .collect();

        let updated = new_map
            .iter()
            .filter_map(|(camp_id, camp)| {
                old_map.get(camp_id).and_then(|old_camp| {
                    if old_camp != camp {
                        Some((old_camp.clone(), camp.clone()))
                    } else {
                        None
                    }
                })
            })
            .collect();

        let deleted = old_map
            .iter()
            .filter_map(|(camp_id, camp)| {
                if new_map.contains_key(camp_id) {
                    None
                } else {
                    Some(camp.clone())
                }
            })
            .collect();

        Self {
            inserted,
            updated,
            deleted,
        }
    }

    fn to_audit_entries(&self) -> Vec<CampAuditEntry> {
        let mut entries = Vec::new();

        entries.extend(self.inserted.iter().map(|camp| CampAuditEntry {
            action: CampAuditAction::RefreshInsert,
            camp_id: camp.camp_id.clone(),
            source_camp_ids: camp.spawngroup_ids.iter().map(|id| id.to_string()).collect(),
            note: Some("camp refresh insert".to_string()),
            before: None,
            after: Some(camp.clone()),
        }));

        entries.extend(self.updated.iter().map(|(before, after)| CampAuditEntry {
            action: CampAuditAction::RefreshUpdate,
            camp_id: after.camp_id.clone(),
            source_camp_ids: after.spawngroup_ids.iter().map(|id| id.to_string()).collect(),
            note: Some("camp refresh update".to_string()),
            before: Some(before.clone()),
            after: Some(after.clone()),
        }));

        entries.extend(self.deleted.iter().map(|camp| CampAuditEntry {
            action: CampAuditAction::RefreshDelete,
            camp_id: camp.camp_id.clone(),
            source_camp_ids: camp.spawngroup_ids.iter().map(|id| id.to_string()).collect(),
            note: Some("camp refresh delete".to_string()),
            before: Some(camp.clone()),
            after: None,
        }));

        entries
    }
}

#[derive(Debug, Clone)]
struct GroupSummary {
    zone_id: i32,
    zone_short_name: String,
    spawngroup_id: i32,
    centroid_xyz: [f32; 3],
    spawn_count: usize,
}

impl GroupSummary {
    fn distance_to(&self, other: &Self) -> f32 {
        let dx = self.centroid_xyz[0] - other.centroid_xyz[0];
        let dy = self.centroid_xyz[1] - other.centroid_xyz[1];
        let dz = self.centroid_xyz[2] - other.centroid_xyz[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

pub fn build_camps(snapshot: &PeqSnapshot) -> Vec<Camp> {
    let spawnentries_by_group = spawnentries_by_group(&snapshot.spawnentries);
    let groups = summarize_groups(snapshot);
    let mut grouped_by_zone: BTreeMap<(i32, String), Vec<GroupSummary>> = BTreeMap::new();

    for group in groups {
        grouped_by_zone
            .entry((group.zone_id, group.zone_short_name.clone()))
            .or_default()
            .push(group);
    }

    let mut camps = Vec::new();
    for ((zone_id, zone_short_name), mut zone_groups) in grouped_by_zone {
        zone_groups.sort_by_key(|group| group.spawngroup_id);
        let clusters = dbscan_clusters(&zone_groups, DEFAULT_CAMP_CLUSTER_EPS, DEFAULT_CAMP_CLUSTER_MIN_SAMPLES);

        for cluster in clusters {
            let mut spawngroup_ids: Vec<i32> = cluster
                .iter()
                .map(|group| group.spawngroup_id)
                .collect();
            spawngroup_ids.sort_unstable();
            spawngroup_ids.dedup();

            let camp_centroid = weighted_cluster_centroid(&cluster);
            let mut mob_families = BTreeSet::new();
            let mut levels = Vec::new();

            for spawngroup_id in &spawngroup_ids {
                if let Some(entries) = spawnentries_by_group.get(spawngroup_id) {
                    for entry in entries {
                        mob_families.insert(entry.family_label());
                        if let Some(level) = entry.level {
                            levels.push(level);
                        }
                    }
                }
            }

            camps.push(Camp {
                camp_id: camp_id(&zone_short_name, &spawngroup_ids),
                zone_id,
                zone_short_name: zone_short_name.clone(),
                spawngroup_ids,
                centroid_xyz: camp_centroid,
                level_min: levels.iter().min().copied(),
                level_max: levels.iter().max().copied(),
                mob_families: mob_families.into_iter().collect(),
                source: CampSource::PeqDerived,
            });
        }
    }

    camps.sort_by(|left, right| {
        left.zone_id
            .cmp(&right.zone_id)
            .then_with(|| left.zone_short_name.cmp(&right.zone_short_name))
            .then_with(|| left.spawngroup_ids.cmp(&right.spawngroup_ids))
            .then_with(|| left.camp_id.cmp(&right.camp_id))
    });

    camps
}

pub fn camp_id(zone_short_name: &str, spawngroup_ids: &[i32]) -> String {
    let mut ids = spawngroup_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();

    let normalized_zone = normalize_zone_short_name(zone_short_name);
    let signature = format!(
        "{}|{}",
        normalized_zone,
        ids.iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );

    sha256_hex(signature.as_bytes())
}

pub fn apply_overrides(
    base_camps: Vec<Camp>,
    overrides: &CampOverrideFile,
) -> Result<(Vec<Camp>, Vec<CampAuditEntry>)> {
    let mut camps: BTreeMap<String, Camp> = base_camps
        .into_iter()
        .map(|camp| (camp.camp_id.clone(), camp))
        .collect();
    let mut audit_entries = Vec::new();

    for split in &overrides.split {
        let source = camps
            .remove(&split.camp_id)
            .with_context(|| format!("Unknown camp_id in split override: {}", split.camp_id))?;
        let source_ids: BTreeSet<i32> = source.spawngroup_ids.iter().copied().collect();
        let mut seen = BTreeSet::new();
        let mut replacements = Vec::new();

        for partition in &split.partitions {
            let mut ids = partition.spawngroup_ids.clone();
            ids.sort_unstable();
            ids.dedup();
            if ids.is_empty() {
                bail!("Split override for {} contains an empty partition", split.camp_id);
            }

            let partition_ids: BTreeSet<i32> = ids.iter().copied().collect();
            if !partition_ids.is_subset(&source_ids) {
                bail!(
                    "Split override for {} includes spawngroup_ids outside the source camp",
                    split.camp_id
                );
            }
            if !seen.is_disjoint(&partition_ids) {
                bail!("Split override for {} reuses a spawngroup_id across partitions", split.camp_id);
            }
            seen.extend(partition_ids.iter().copied());

            let camp = Camp {
                camp_id: camp_id(&source.zone_short_name, &ids),
                zone_id: source.zone_id,
                zone_short_name: source.zone_short_name.clone(),
                spawngroup_ids: ids.clone(),
                centroid_xyz: source.centroid_xyz,
                level_min: source.level_min,
                level_max: source.level_max,
                mob_families: source.mob_families.clone(),
                source: CampSource::OperatorOverride,
            };

            replacements.push(camp.clone());
            audit_entries.push(CampAuditEntry {
                action: CampAuditAction::OverrideSplit,
                camp_id: camp.camp_id.clone(),
                source_camp_ids: vec![split.camp_id.clone()],
                note: partition.note.clone().or_else(|| split.note.clone()),
                before: Some(source.clone()),
                after: Some(camp),
            });
        }

        if seen != source_ids {
            bail!(
                "Split override for {} must account for every spawngroup_id in the source camp",
                split.camp_id
            );
        }

        for camp in replacements {
            camps.insert(camp.camp_id.clone(), camp);
        }
    }

    for merge in &overrides.merge {
        let mut source_ids = Vec::new();
        let mut source_camps = Vec::new();
        for camp_id in &merge.camp_ids {
            let camp = camps
                .remove(camp_id)
                .with_context(|| format!("Unknown camp_id in merge override: {camp_id}"))?;
            source_ids.push(camp_id.clone());
            source_camps.push(camp);
        }

        if source_camps.is_empty() {
            bail!("Merge override must reference at least one camp");
        }

        let zone_id = source_camps[0].zone_id;
        let zone_short_name = source_camps[0].zone_short_name.clone();
        if source_camps
            .iter()
            .any(|camp| camp.zone_id != zone_id || camp.zone_short_name != zone_short_name)
        {
            bail!("Merge override camps must all come from the same zone");
        }

        let mut spawngroup_ids = BTreeSet::new();
        let mut mob_families = BTreeSet::new();
        let mut level_min = None;
        let mut level_max = None;
        for camp in &source_camps {
            spawngroup_ids.extend(camp.spawngroup_ids.iter().copied());
            mob_families.extend(camp.mob_families.iter().cloned());
            level_min = match (level_min, camp.level_min) {
                (Some(existing), Some(next)) => Some(existing.min(next)),
                (None, Some(next)) => Some(next),
                (value, None) => value,
            };
            level_max = match (level_max, camp.level_max) {
                (Some(existing), Some(next)) => Some(existing.max(next)),
                (None, Some(next)) => Some(next),
                (value, None) => value,
            };
        }

        let spawngroup_ids: Vec<i32> = spawngroup_ids.into_iter().collect();
        let camp = Camp {
            camp_id: camp_id(&zone_short_name, &spawngroup_ids),
            zone_id,
            zone_short_name,
            spawngroup_ids,
            centroid_xyz: weighted_merge_centroid(&source_camps),
            level_min,
            level_max,
            mob_families: mob_families.into_iter().collect(),
            source: CampSource::OperatorOverride,
        };

        audit_entries.push(CampAuditEntry {
            action: CampAuditAction::OverrideMerge,
            camp_id: camp.camp_id.clone(),
            source_camp_ids: source_ids,
            note: merge.note.clone(),
            before: None,
            after: Some(camp.clone()),
        });

        camps.insert(camp.camp_id.clone(), camp);
    }

    let mut camps: Vec<Camp> = camps.into_values().collect();
    camps.sort_by(|left, right| {
        left.zone_id
            .cmp(&right.zone_id)
            .then_with(|| left.zone_short_name.cmp(&right.zone_short_name))
            .then_with(|| left.spawngroup_ids.cmp(&right.spawngroup_ids))
            .then_with(|| left.camp_id.cmp(&right.camp_id))
    });

    Ok((camps, audit_entries))
}

pub fn default_db_path() -> PathBuf {
    paths::data_dir().join(DEFAULT_CAMP_DB_FILENAME)
}

pub fn default_override_path() -> PathBuf {
    if let Some(home) = home_dir() {
        return home.join(DEFAULT_CAMP_OVERRIDE_PATH);
    }
    PathBuf::from(DEFAULT_CAMP_OVERRIDE_PATH)
}

pub fn camp_matches_zone(camp: &Camp, zone: &str) -> bool {
    if let Ok(zone_id) = zone.parse::<i32>() {
        camp.zone_id == zone_id
    } else {
        camp.zone_short_name.eq_ignore_ascii_case(zone)
    }
}

fn insert_camp_row(conn: &Connection, camp: &Camp) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO camps (
            camp_id,
            zone_id,
            zone_short_name,
            spawngroup_ids_json,
            centroid_x,
            centroid_y,
            centroid_z,
            level_min,
            level_max,
            mob_families_json,
            source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        params![
            &camp.camp_id,
            camp.zone_id,
            &camp.zone_short_name,
            serde_json::to_string(&camp.spawngroup_ids)?,
            camp.centroid_xyz[0],
            camp.centroid_xyz[1],
            camp.centroid_xyz[2],
            camp.level_min.map(i64::from),
            camp.level_max.map(i64::from),
            serde_json::to_string(&camp.mob_families)?,
            camp.source.as_str(),
        ],
    )?;
    Ok(())
}

fn camp_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Camp> {
    let spawngroup_ids_json: String = row.get("spawngroup_ids_json")?;
    let mob_families_json: String = row.get("mob_families_json")?;
    let level_min: Option<i64> = row.get("level_min")?;
    let level_max: Option<i64> = row.get("level_max")?;
    let source: String = row.get("source")?;

    let spawngroup_ids = serde_json::from_str::<Vec<i32>>(&spawngroup_ids_json).map_err(
        |error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        },
    )?;
    let mob_families = serde_json::from_str::<Vec<String>>(&mob_families_json).map_err(
        |error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        },
    )?;

    Ok(Camp {
        camp_id: row.get("camp_id")?,
        zone_id: row.get("zone_id")?,
        zone_short_name: row.get("zone_short_name")?,
        spawngroup_ids,
        centroid_xyz: [row.get("centroid_x")?, row.get("centroid_y")?, row.get("centroid_z")?],
        level_min: level_min.map(|value| value as u8),
        level_max: level_max.map(|value| value as u8),
        mob_families,
        source: CampSource::from_str(&source).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
    })
}

fn summarize_groups(snapshot: &PeqSnapshot) -> Vec<GroupSummary> {
    let mut groups: BTreeMap<(i32, String, i32), Vec<&Spawn2Row>> = BTreeMap::new();

    for spawn in &snapshot.spawn2 {
        groups
            .entry((
                spawn.zone_id,
                normalize_zone_short_name(&spawn.zone_short_name),
                spawn.spawngroup_id,
            ))
            .or_default()
            .push(spawn);
    }

    groups
        .into_iter()
        .map(|((zone_id, zone_short_name, spawngroup_id), spawns)| GroupSummary {
            zone_id,
            zone_short_name,
            spawngroup_id,
            centroid_xyz: weighted_spawn_centroid(&spawns),
            spawn_count: spawns.len(),
        })
        .collect()
}

fn dbscan_clusters(
    groups: &[GroupSummary],
    eps: f32,
    min_samples: usize,
) -> Vec<Vec<GroupSummary>> {
    let mut visited = vec![false; groups.len()];
    let mut assigned = vec![false; groups.len()];
    let mut clusters = Vec::new();

    for idx in 0..groups.len() {
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let mut neighbors = region_query(groups, idx, eps);
        if neighbors.len() + 1 < min_samples {
            continue;
        }

        let mut cluster_indices = Vec::new();
        cluster_indices.push(idx);
        assigned[idx] = true;

        neighbors.sort_unstable();
        neighbors.dedup();
        let mut queue = neighbors;

        while let Some(next_idx) = queue.pop() {
            if !visited[next_idx] {
                visited[next_idx] = true;
                let mut next_neighbors = region_query(groups, next_idx, eps);
                if next_neighbors.len() + 1 >= min_samples {
                    queue.extend(next_neighbors);
                    queue.sort_unstable();
                    queue.dedup();
                }
            }

            if !assigned[next_idx] {
                assigned[next_idx] = true;
                cluster_indices.push(next_idx);
            }
        }

        cluster_indices.sort_unstable();
        clusters.push(
            cluster_indices
                .into_iter()
                .map(|index| groups[index].clone())
                .collect(),
        );
    }

    clusters
}

fn region_query(groups: &[GroupSummary], idx: usize, eps: f32) -> Vec<usize> {
    let mut neighbors = Vec::new();
    for (other_idx, other) in groups.iter().enumerate() {
        if other_idx == idx {
            continue;
        }
        if groups[idx].distance_to(other) <= eps {
            neighbors.push(other_idx);
        }
    }
    neighbors
}

fn weighted_spawn_centroid(spawns: &[&Spawn2Row]) -> [f32; 3] {
    let count = spawns.len() as f32;
    let sum_x = spawns.iter().map(|spawn| spawn.x).sum::<f32>();
    let sum_y = spawns.iter().map(|spawn| spawn.y).sum::<f32>();
    let sum_z = spawns.iter().map(|spawn| spawn.z).sum::<f32>();
    [sum_x / count, sum_y / count, sum_z / count]
}

fn weighted_cluster_centroid(groups: &[GroupSummary]) -> [f32; 3] {
    let total_spawns = groups.iter().map(|group| group.spawn_count).sum::<usize>() as f32;
    let sum_x = groups
        .iter()
        .map(|group| group.centroid_xyz[0] * group.spawn_count as f32)
        .sum::<f32>();
    let sum_y = groups
        .iter()
        .map(|group| group.centroid_xyz[1] * group.spawn_count as f32)
        .sum::<f32>();
    let sum_z = groups
        .iter()
        .map(|group| group.centroid_xyz[2] * group.spawn_count as f32)
        .sum::<f32>();
    [sum_x / total_spawns, sum_y / total_spawns, sum_z / total_spawns]
}

fn weighted_merge_centroid(camps: &[Camp]) -> [f32; 3] {
    let total_groups = camps.iter().map(|camp| camp.spawngroup_ids.len()).sum::<usize>() as f32;
    let sum_x = camps
        .iter()
        .map(|camp| camp.centroid_xyz[0] * camp.spawngroup_ids.len() as f32)
        .sum::<f32>();
    let sum_y = camps
        .iter()
        .map(|camp| camp.centroid_xyz[1] * camp.spawngroup_ids.len() as f32)
        .sum::<f32>();
    let sum_z = camps
        .iter()
        .map(|camp| camp.centroid_xyz[2] * camp.spawngroup_ids.len() as f32)
        .sum::<f32>();
    [sum_x / total_groups, sum_y / total_groups, sum_z / total_groups]
}

fn spawnentries_by_group(
    spawnentries: &[SpawnentryRow],
) -> BTreeMap<i32, Vec<SpawnentryRow>> {
    let mut map: BTreeMap<i32, Vec<SpawnentryRow>> = BTreeMap::new();
    for entry in spawnentries {
        map.entry(entry.spawngroup_id)
            .or_default()
            .push(entry.clone());
    }
    for entries in map.values_mut() {
        entries.sort_by(|left, right| {
            left.npc_name
                .cmp(&right.npc_name)
                .then_with(|| left.npc_id.cmp(&right.npc_id))
                .then_with(|| left.level.cmp(&right.level))
        });
    }
    map
}

fn normalize_zone_short_name(zone_short_name: &str) -> String {
    zone_short_name.trim().to_ascii_lowercase()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        return Some(PathBuf::from(home));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(home));
    }
    None
}

impl CampAuditAction {
    fn as_str(&self) -> &'static str {
        match self {
            Self::RefreshInsert => "refresh_insert",
            Self::RefreshUpdate => "refresh_update",
            Self::RefreshDelete => "refresh_delete",
            Self::OverrideMerge => "override_merge",
            Self::OverrideSplit => "override_split",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn camp_id_is_stable_and_order_insensitive() {
        let a = camp_id("solb", &[2, 1]);
        let b = camp_id("SOLB", &[1, 2, 2]);

        assert_eq!(a, "8e91f70d02bf1c6d227d3ed537f7105b734213e1690156b00978b770b3188654");
        assert_eq!(a, b);
    }

    #[test]
    fn build_camps_clusters_nearby_spawngroups() {
        let snapshot = PeqSnapshot {
            spawn2: vec![
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 1,
                    x: 0.0,
                    y: 20.0,
                    z: 0.0,
                },
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 2,
                    x: 100.0,
                    y: 0.0,
                    z: 0.0,
                },
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 3,
                    x: 900.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            spawngroups: vec![
                SpawngroupRow {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 1,
                },
                SpawngroupRow {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 2,
                },
                SpawngroupRow {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 3,
                },
            ],
            spawnentries: vec![
                SpawnentryRow {
                    spawngroup_id: 1,
                    npc_id: 101,
                    npc_name: Some("efreeti".to_string()),
                    level: Some(50),
                },
                SpawnentryRow {
                    spawngroup_id: 2,
                    npc_id: 102,
                    npc_name: Some("efreeti".to_string()),
                    level: Some(52),
                },
                SpawnentryRow {
                    spawngroup_id: 3,
                    npc_id: 103,
                    npc_name: Some("giant".to_string()),
                    level: Some(40),
                },
            ],
        };

        let camps = build_camps(&snapshot);

        assert_eq!(camps.len(), 1);
        assert_eq!(camps[0].zone_short_name, "solb");
        assert_eq!(camps[0].spawngroup_ids, vec![1, 2]);
        assert_eq!(camps[0].level_min, Some(50));
        assert_eq!(camps[0].level_max, Some(52));
        assert_eq!(camps[0].mob_families, vec!["efreeti".to_string()]);
        assert_eq!(
            camps[0].camp_id,
            "8e91f70d02bf1c6d227d3ed537f7105b734213e1690156b00978b770b3188654"
        );
    }

    #[test]
    fn overrides_merge_and_split_cleanly() {
        let base = vec![
            Camp {
                camp_id: camp_id("solb", &[1, 2]),
                zone_id: 50,
                zone_short_name: "solb".to_string(),
                spawngroup_ids: vec![1, 2],
                centroid_xyz: [0.0, 0.0, 0.0],
                level_min: Some(50),
                level_max: Some(52),
                mob_families: vec!["efreeti".to_string()],
                source: CampSource::PeqDerived,
            },
            Camp {
                camp_id: camp_id("solb", &[3, 4]),
                zone_id: 50,
                zone_short_name: "solb".to_string(),
                spawngroup_ids: vec![3, 4],
                centroid_xyz: [300.0, 0.0, 0.0],
                level_min: Some(40),
                level_max: Some(41),
                mob_families: vec!["giant".to_string()],
                source: CampSource::PeqDerived,
            },
        ];

        let overrides = CampOverrideFile {
            merge: vec![CampMergeOverride {
                camp_ids: vec![camp_id("solb", &[1, 2]), camp_id("solb", &[3, 4])],
                note: Some("combine the two adjacent halls".to_string()),
            }],
            split: vec![],
        };

        let (merged, audit) = apply_overrides(base.clone(), &overrides).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(audit.len(), 1);
        assert_eq!(merged[0].spawngroup_ids, vec![1, 2, 3, 4]);

        let split_overrides = CampOverrideFile {
            merge: vec![],
            split: vec![CampSplitOverride {
                camp_id: camp_id("solb", &[1, 2]),
                partitions: vec![
                    CampSplitPartition {
                        spawngroup_ids: vec![1],
                        note: Some("north".to_string()),
                    },
                    CampSplitPartition {
                        spawngroup_ids: vec![2],
                        note: Some("south".to_string()),
                    },
                ],
                note: Some("split the hub".to_string()),
            }],
        };

        let (split, audit) = apply_overrides(vec![base[0].clone()], &split_overrides).unwrap();
        assert_eq!(split.len(), 2);
        assert_eq!(audit.len(), 2);
        assert_eq!(split[0].source, CampSource::OperatorOverride);
        assert_eq!(split[1].source, CampSource::OperatorOverride);
    }

    #[test]
    fn camp_database_persists_and_loads_effective_camps() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("eq_data.db");
        let mut db = CampDatabase::open(&db_path).unwrap();

        db.refresh_from_snapshot(&PeqSnapshot {
            spawn2: vec![
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Spawn2Row {
                    zone_id: 50,
                    zone_short_name: "solb".to_string(),
                    spawngroup_id: 2,
                    x: 100.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            spawngroups: vec![],
            spawnentries: vec![],
        })
        .unwrap();

        let camps = db.load_base_camps().unwrap();
        assert_eq!(camps.len(), 1);
        assert_eq!(camps[0].spawngroup_ids, vec![1, 2]);
    }
}
