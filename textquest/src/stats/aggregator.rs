//! Session aggregator — read JSONL events, compute derived metrics, store in SQLite.

use anyhow::{anyhow, Context, Result};
use chrono::DateTime;
use rusqlite::{params, Connection};
use serde::{Deserialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::schema::AggregateSchema;

/// Session event from JSONL (minimal schema; aligns with TestEvent for testing).
#[derive(Debug, Clone, Deserialize)]
pub struct SessionEvent {
    pub timestamp: String,
    pub session_id: String,
    pub iteration: u32,
    pub event_type: String,
    #[serde(default)]
    pub details: Value,
}

/// Aggregator for session events.
pub struct SessionAggregator {
    conn: Connection,
}

impl SessionAggregator {
    /// Open metrics DB and ensure aggregate schema exists.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .context("Failed to open metrics database")?;
        conn.execute_batch(AggregateSchema::SCHEMA)
            .context("Failed to initialize aggregate schema")?;
        Ok(Self { conn })
    }

    /// Compact a single session's JSONL into aggregates (idempotent via session_id).
    pub fn compact_session(&self, session_dir: &Path, session_id: &str) -> Result<()> {
        let events_path = session_dir.join("events.jsonl");
        if !events_path.exists() {
            return Err(anyhow!("events.jsonl not found: {:?}", events_path));
        }

        // Read and parse JSONL
        let content = fs::read_to_string(&events_path)
            .with_context(|| format!("Failed to read JSONL: {:?}", events_path))?;

        let mut events = Vec::new();
        for (line_no, line) in content.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionEvent>(line) {
                Ok(ev) => events.push(ev),
                Err(e) => tracing::warn!("Skipping invalid JSON at line {}: {}", line_no + 1, e),
            }
        }

        if events.is_empty() {
            tracing::warn!("No valid events in {:?}", events_path);
            return Ok(());
        }

        // Compute aggregates
        let aggregates = self.compute_aggregates(session_id, &events)?;

        // Store in DB (idempotent: use INSERT OR REPLACE)
        self.store_aggregates(session_id, &aggregates)?;

        // Mark as aggregated
        self.mark_aggregated(session_id, events_path.to_string_lossy().as_ref())?;

        tracing::info!("Compacted session {} with {} events", session_id, events.len());
        Ok(())
    }

    /// Compact all pending sessions in event_base_dir.
    pub fn compact_all_pending(&self, event_base_dir: &Path) -> Result<()> {
        // Find all session-* directories
        let entries = fs::read_dir(event_base_dir)
            .context("Failed to read event base directory")?;

        let mut compacted = 0;
        for entry in entries {
            let entry = entry.context("Failed to read dir entry")?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
            if let Some(session_id) = dir_name.strip_prefix("session-") {
                // Extract short ID (after last dash)
                if let Some(short_id) = session_id.split('-').last() {
                    // Check if already aggregated
                    if !self.is_aggregated(short_id)? {
                        match self.compact_session(&path, short_id) {
                            Ok(()) => compacted += 1,
                            Err(e) => {
                                tracing::warn!("Failed to compact {}: {}", short_id, e);
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("Compacted {} pending sessions", compacted);
        Ok(())
    }

    /// Compute all aggregate metrics from events.
    fn compute_aggregates(&self, session_id: &str, events: &[SessionEvent]) -> Result<Value> {
        let mut aggregates = json!({
            "session_id": session_id,
            "kill_time_per_mob": self.compute_kill_time(events)?,
            "net_plat_per_hour": self.compute_net_plat(events)?,
            "death_recovery": self.compute_death_recovery(events)?,
            "spell_efficiency": self.compute_spell_efficiency(events)?,
            "pull_cadence": self.compute_pull_cadence(events)?,
            "ability_contribution": self.compute_ability_contribution(events)?,
            "route_node_deaths": self.compute_route_node_deaths(events)?,
            "stuck_heatmap": self.compute_stuck_heatmap(events)?,
            "xp_tracking": self.compute_xp_tracking(events)?,
            "kill_tracking": self.compute_kill_tracking(events)?,
            "plat_tracking": self.compute_plat_tracking(events)?,
        });

        Ok(aggregates)
    }

    fn compute_kill_time(&self, events: &[SessionEvent]) -> Result<Value> {
        // Group MobKill events by zone and party composition
        let mut kills: BTreeMap<String, Vec<i64>> = BTreeMap::new();

        for event in events {
            if event.event_type == "ScenarioMobKill" {
                if let (Some(zone), Some(duration_ms)) = (
                    event.details.get("zone").and_then(|v| v.as_str()),
                    event.details.get("duration_ms").and_then(|v| v.as_i64()),
                ) {
                    let party_comp = event
                        .details
                        .get("party_comp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let key = format!("{}:{}", zone, party_comp);
                    kills.entry(key).or_insert_with(Vec::new).push(duration_ms);
                }
            }
        }

        let mut result = json!({});
        for (key, times) in kills {
            if !times.is_empty() {
                let mean = times.iter().sum::<i64>() as f64 / times.len() as f64;
                let mut sorted = times.clone();
                sorted.sort();
                let p50 = sorted[sorted.len() / 2] as f64;
                let p90 = sorted[(sorted.len() * 9) / 10] as f64;

                result[key] = json!({
                    "mean_ms": mean,
                    "p50_ms": p50,
                    "p90_ms": p90,
                    "kill_count": sorted.len(),
                });
            }
        }

        Ok(result)
    }

    fn compute_net_plat(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut plat_by_zone: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new();

        for event in events {
            if let Some(zone) = event.details.get("zone").and_then(|v| v.as_str()) {
                let (mut gross, mut deaths, mut consumables) =
                    plat_by_zone.remove(zone).unwrap_or((0, 0, 0));

                match event.event_type.as_str() {
                    "Loot" => {
                        if let Some(plat) = event.details.get("plat_value").and_then(|v| v.as_i64())
                        {
                            gross += plat;
                        }
                    }
                    "Death" => {
                        if let Some(cost) = event.details.get("death_cost").and_then(|v| v.as_i64()) {
                            deaths += cost;
                        }
                    }
                    "Consumable" => {
                        if let Some(cost) =
                            event.details.get("consumable_cost").and_then(|v| v.as_i64())
                        {
                            consumables += cost;
                        }
                    }
                    _ => {}
                }

                plat_by_zone.insert(zone.to_string(), (gross, deaths, consumables));
            }
        }

        let mut result = json!({});
        for (zone, (gross, deaths, consumables)) in plat_by_zone {
            let net = gross - deaths - consumables;
            result[zone] = json!({
                "gross_plat": gross,
                "deaths_cost": deaths,
                "consumable_cost": consumables,
                "net_plat": net,
            });
        }

        Ok(result)
    }

    fn compute_death_recovery(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut recovery_times = Vec::new();

        let mut death_ts: Option<DateTime<chrono::FixedOffset>> = None;
        for event in events {
            if event.event_type == "Death" {
                if let Ok(ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                    death_ts = Some(ts);
                }
            } else if event.event_type == "Resume" && death_ts.is_some() {
                if let Ok(resume_ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                    if let Some(death) = death_ts {
                        let duration = resume_ts.signed_duration_since(death);
                        recovery_times.push(duration.num_seconds());
                        death_ts = None;
                    }
                }
            }
        }

        let mut result = json!({
            "death_count": 0,
            "median_seconds": null,
            "p90_seconds": null,
        });

        if !recovery_times.is_empty() {
            let mut sorted = recovery_times.clone();
            sorted.sort();
            let median = sorted[sorted.len() / 2];
            let p90 = sorted[(sorted.len() * 9) / 10];
            result["death_count"] = json!(sorted.len());
            result["median_seconds"] = json!(median);
            result["p90_seconds"] = json!(p90);
        }

        Ok(result)
    }

    fn compute_spell_efficiency(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut spell_stats: BTreeMap<String, (i64, i64)> = BTreeMap::new();

        for event in events {
            if event.event_type == "SpellCast" {
                if let Some(spell) = event.details.get("spell_name").and_then(|v| v.as_str()) {
                    let (total, success) = spell_stats.remove(spell).unwrap_or((0, 0));
                    let success_increment = event
                        .details
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .map(|s| if s { 1 } else { 0 })
                        .unwrap_or(0);
                    spell_stats.insert(spell.to_string(), (total + 1, success + success_increment));
                }
            }
        }

        let mut result = json!({});
        for (spell, (total, success)) in spell_stats {
            let rate = if total > 0 {
                success as f64 / total as f64
            } else {
                0.0
            };
            result[spell] = json!({
                "total_attempts": total,
                "successful_casts": success,
                "success_rate": rate,
            });
        }

        Ok(result)
    }

    fn compute_pull_cadence(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut pull_times = Vec::new();

        let mut last_pull: Option<i64> = None;
        for event in events {
            if event.event_type == "Pull" {
                if let Ok(ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                    let ts_ms = ts.timestamp_millis();
                    if let Some(prev) = last_pull {
                        pull_times.push((ts_ms - prev) / 1000);
                    }
                    last_pull = Some(ts_ms);
                }
            }
        }

        let mean_pull = if !pull_times.is_empty() {
            pull_times.iter().sum::<i64>() as f64 / pull_times.len() as f64
        } else {
            0.0
        };

        let mut sorted_pulls = pull_times.clone();
        sorted_pulls.sort();
        let p50_pull = if !sorted_pulls.is_empty() {
            sorted_pulls[sorted_pulls.len() / 2] as f64
        } else {
            0.0
        };
        let p90_pull = if !sorted_pulls.is_empty() {
            sorted_pulls[(sorted_pulls.len() * 9) / 10] as f64
        } else {
            0.0
        };

        Ok(json!({
            "mean_pull_interval_s": mean_pull,
            "p50_pull_interval_s": p50_pull,
            "p90_pull_interval_s": p90_pull,
            "mean_mana_recovery_s": 0.0,
            "mean_hp_recovery_s": 0.0,
            "pull_count": pull_times.len(),
        }))
    }

    fn compute_ability_contribution(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut ability_stats: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new();

        for event in events {
            if event.event_type == "AbilityUsed" {
                if let Some(ability) = event.details.get("ability_name").and_then(|v| v.as_str()) {
                    let (damage, uses, mana) = ability_stats.remove(ability).unwrap_or((0, 0, 0));
                    let dmg_inc = event
                        .details
                        .get("damage")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let mana_inc = event
                        .details
                        .get("mana_cost")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    ability_stats.insert(ability.to_string(), (damage + dmg_inc, uses + 1, mana + mana_inc));
                }
            }
        }

        let mut result = json!({});
        for (ability, (damage, uses, mana)) in ability_stats {
            let rank = if mana > 0 {
                (damage as f64 * uses as f64) / mana as f64
            } else {
                damage as f64 * uses as f64
            };
            result[ability] = json!({
                "damage_total": damage,
                "times_used": uses,
                "mana_spent": mana,
                "contribution_rank": rank,
            });
        }

        Ok(result)
    }

    fn compute_route_node_deaths(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut deaths_by_node: BTreeMap<String, (i64, i64)> = BTreeMap::new();

        for event in events {
            if event.event_type == "Death" || event.event_type == "RouteTraversal" {
                if let (Some(zone), Some(x), Some(y), Some(z)) = (
                    event.details.get("zone").and_then(|v| v.as_str()),
                    event.details.get("x").and_then(|v| v.as_f64()),
                    event.details.get("y").and_then(|v| v.as_f64()),
                    event.details.get("z").and_then(|v| v.as_f64()),
                ) {
                    let key = format!("{}:{:.1}:{:.1}:{:.1}", zone, x, y, z);
                    let (deaths, traversals) = deaths_by_node.remove(&key).unwrap_or((0, 0));

                    if event.event_type == "Death" {
                        deaths_by_node.insert(key, (deaths + 1, traversals));
                    } else {
                        deaths_by_node.insert(key, (deaths, traversals + 1));
                    }
                }
            }
        }

        let mut result = json!({});
        for (key, (deaths, traversals)) in deaths_by_node {
            let rate = if traversals > 0 {
                deaths as f64 / traversals as f64
            } else {
                0.0
            };
            result[key] = json!({
                "death_count": deaths,
                "traversal_count": traversals,
                "death_rate": rate,
            });
        }

        Ok(result)
    }

    fn compute_stuck_heatmap(&self, events: &[SessionEvent]) -> Result<Value> {
        const BUCKET_SIZE: f64 = 100.0; // 100-unit buckets

        let mut stuck_by_bucket: BTreeMap<String, i64> = BTreeMap::new();

        for event in events {
            if event.event_type == "Stuck" {
                if let (Some(zone), Some(x), Some(y)) = (
                    event.details.get("zone").and_then(|v| v.as_str()),
                    event.details.get("x").and_then(|v| v.as_f64()),
                    event.details.get("y").and_then(|v| v.as_f64()),
                ) {
                    let x_bucket = (x / BUCKET_SIZE).floor() as i64;
                    let y_bucket = (y / BUCKET_SIZE).floor() as i64;
                    let key = format!("{}:{}:{}", zone, x_bucket, y_bucket);
                    let count = stuck_by_bucket.remove(&key).unwrap_or(0);
                    stuck_by_bucket.insert(key, count + 1);
                }
            }
        }

        let mut result = json!({});
        for (key, count) in stuck_by_bucket {
            result[key] = json!(count);
        }

        Ok(result)
    }

    fn compute_xp_tracking(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut xp_by_char: BTreeMap<String, (i32, f64, i32, f64, i64)> = BTreeMap::new();

        let mut first_level: Option<i32> = None;
        let mut first_xp_pct: Option<f64> = None;
        let mut last_level: Option<i32> = None;
        let mut last_xp_pct: Option<f64> = None;
        let mut duration_ms: i64 = 0;

        for event in events {
            if event.event_type == "LevelUp" {
                if let (Some(char_name), Some(level), Some(xp_pct)) = (
                    event.details.get("character_name").and_then(|v| v.as_str()),
                    event.details.get("new_level").and_then(|v| v.as_i64()),
                    event.details.get("xp_percent").and_then(|v| v.as_f64()),
                ) {
                    let level = level as i32;
                    if first_level.is_none() {
                        first_level = Some(level);
                        first_xp_pct = Some(xp_pct);
                    }
                    last_level = Some(level);
                    last_xp_pct = Some(xp_pct);

                    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&event.timestamp) {
                        duration_ms = ts.timestamp_millis() as i64 - duration_ms;
                    }
                }
            }
        }

        let mut result = json!({});
        if let (Some(start_level), Some(start_xp), Some(end_level), Some(end_xp)) =
            (first_level, first_xp_pct, last_level, last_xp_pct)
        {
            let duration_seconds = (duration_ms / 1000).max(1);
            let xp_gained_pct = (end_xp - start_xp).max(0.0);
            let xp_per_hour = (xp_gained_pct / duration_seconds as f64) * 3600.0;

            result = json!({
                "start_level": start_level,
                "end_level": end_level,
                "xp_percent_start": start_xp,
                "xp_percent_end": end_xp,
                "xp_gained_percent": xp_gained_pct,
                "duration_seconds": duration_seconds,
                "xp_per_hour": xp_per_hour,
            });
        }

        Ok(result)
    }

    fn compute_kill_tracking(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut kills_by_mob: BTreeMap<String, (u32, bool, String)> = BTreeMap::new();
        let mut total_duration_ms: i64 = 0;

        for event in events {
            if event.event_type == "Kill" {
                if let (Some(mob_name), Some(zone)) = (
                    event.details.get("target_name").and_then(|v| v.as_str()),
                    event.details.get("zone").and_then(|v| v.as_str()),
                ) {
                    let is_named = event.details.get("is_named").and_then(|v| v.as_bool()).unwrap_or(false);
                    let key = format!("{}:{}", mob_name, zone);
                    let (count, _, _) = kills_by_mob.remove(&key).unwrap_or((0, false, zone.to_string()));
                    kills_by_mob.insert(key, (count + 1, is_named, zone.to_string()));
                }
            }

            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&event.timestamp) {
                total_duration_ms = ts.timestamp_millis().max(total_duration_ms);
            }
        }

        let duration_seconds = (total_duration_ms / 1000).max(1);
        let mut result = json!({});

        for (key, (count, is_named, _zone)) in kills_by_mob {
            let kills_per_hour = (count as f64 / duration_seconds as f64) * 3600.0;
            result[key] = json!({
                "kill_count": count,
                "is_named": is_named,
                "duration_seconds": duration_seconds,
                "kills_per_hour": kills_per_hour,
            });
        }

        Ok(result)
    }

    fn compute_plat_tracking(&self, events: &[SessionEvent]) -> Result<Value> {
        let mut gross_plat: i64 = 0;
        let mut net_plat: i64 = 0;
        let mut transaction_count: u32 = 0;
        let mut duration_ms: i64 = 0;

        for event in events {
            match event.event_type.as_str() {
                "LootDrop" => {
                    if let Some(value) = event.details.get("item_value").and_then(|v| v.as_i64()) {
                        gross_plat += value;
                        net_plat += value;
                        transaction_count += 1;
                    }
                }
                "VendorSale" => {
                    if let Some(value) = event.details.get("amount").and_then(|v| v.as_i64()) {
                        net_plat += value;
                        transaction_count += 1;
                    }
                }
                "Death" => {
                    if let Some(cost) = event.details.get("death_cost").and_then(|v| v.as_i64()) {
                        net_plat -= cost;
                        transaction_count += 1;
                    }
                }
                _ => {}
            }

            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&event.timestamp) {
                duration_ms = ts.timestamp_millis().max(duration_ms);
            }
        }

        let duration_seconds = (duration_ms / 1000).max(1);
        let plat_per_hour = (net_plat as f64 / duration_seconds as f64) * 3600.0;

        Ok(json!({
            "gross_platinum": gross_plat,
            "net_platinum": net_plat,
            "duration_seconds": duration_seconds,
            "platinum_per_hour": plat_per_hour,
            "transaction_count": transaction_count,
        }))
    }

    fn store_aggregates(&self, session_id: &str, aggregates: &Value) -> Result<()> {
        let tx = self.conn.transaction().context("Failed to start transaction")?;

        tx.execute(
            "INSERT OR REPLACE INTO session_aggregates (session_id, updated_at) VALUES (?, datetime('now'))",
            params![session_id],
        )
        .context("Failed to insert session_aggregates")?;

        // kill_time_per_mob
        if let Some(kills) = aggregates.get("kill_time_per_mob").and_then(|v| v.as_object()) {
            for (key, metrics) in kills {
                if let Some(parts) = key.split(':').collect::<Vec<_>>().get(0..2) {
                    if parts.len() == 2 {
                        let zone = parts[0];
                        let party_comp = parts[1];
                        let mean = metrics.get("mean_ms").and_then(|v| v.as_f64());
                        let p50 = metrics.get("p50_ms").and_then(|v| v.as_f64());
                        let p90 = metrics.get("p90_ms").and_then(|v| v.as_f64());
                        let count = metrics.get("kill_count").and_then(|v| v.as_i64());

                        tx.execute(
                            "INSERT OR REPLACE INTO kill_time_per_mob (session_id, zone, party_comp, mean_ms, p50_ms, p90_ms, kill_count)
                             VALUES (?, ?, ?, ?, ?, ?, ?)",
                            params![session_id, zone, party_comp, mean, p50, p90, count],
                        )
                        .context("Failed to insert kill_time_per_mob")?;
                    }
                }
            }
        }

        // net_plat_per_hour_zone
        if let Some(plat_data) = aggregates.get("net_plat_per_hour").and_then(|v| v.as_object()) {
            for (zone, metrics) in plat_data {
                let gross = metrics.get("gross_plat").and_then(|v| v.as_i64()).unwrap_or(0);
                let deaths = metrics.get("deaths_cost").and_then(|v| v.as_i64()).unwrap_or(0);
                let consumables = metrics.get("consumable_cost").and_then(|v| v.as_i64()).unwrap_or(0);
                let net = metrics.get("net_plat").and_then(|v| v.as_i64()).unwrap_or(0);
                let duration_secs: i64 = 3600; // placeholder, would come from session metadata
                let plat_per_hour = if duration_secs > 0 {
                    (net as f64 / duration_secs as f64) * 3600.0
                } else {
                    0.0
                };

                tx.execute(
                    "INSERT OR REPLACE INTO net_plat_per_hour_zone (session_id, zone, gross_plat, deaths_cost, consumable_cost, net_plat, duration_seconds, plat_per_hour)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    params![session_id, zone, gross, deaths, consumables, net, duration_secs, plat_per_hour],
                )
                .context("Failed to insert net_plat_per_hour_zone")?;
            }
        }

        // death_recovery_time
        if let Some(recovery) = aggregates.get("death_recovery").and_then(|v| v.as_object()) {
            let death_count = recovery.get("death_count").and_then(|v| v.as_i64());
            let median_secs = recovery.get("median_seconds").and_then(|v| v.as_f64());
            let p90_secs = recovery.get("p90_seconds").and_then(|v| v.as_f64());

            tx.execute(
                "INSERT OR REPLACE INTO death_recovery_time (session_id, death_count, median_seconds, p90_seconds)
                 VALUES (?, ?, ?, ?)",
                params![session_id, death_count, median_secs, p90_secs],
            )
            .context("Failed to insert death_recovery_time")?;
        }

        // spell_efficiency
        if let Some(spells) = aggregates.get("spell_efficiency").and_then(|v| v.as_object()) {
            for (spell_name, metrics) in spells {
                let total = metrics.get("total_attempts").and_then(|v| v.as_i64()).unwrap_or(0);
                let success = metrics.get("successful_casts").and_then(|v| v.as_i64()).unwrap_or(0);
                let rate = metrics.get("success_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);

                tx.execute(
                    "INSERT OR REPLACE INTO spell_efficiency (session_id, spell_name, total_attempts, successful_casts, success_rate)
                     VALUES (?, ?, ?, ?, ?)",
                    params![session_id, spell_name, total, success, rate],
                )
                .context("Failed to insert spell_efficiency")?;
            }
        }

        // pull_cadence
        if let Some(cadence) = aggregates.get("pull_cadence").and_then(|v| v.as_object()) {
            let mean_pull = cadence.get("mean_pull_interval_s").and_then(|v| v.as_f64());
            let p50_pull = cadence.get("p50_pull_interval_s").and_then(|v| v.as_f64());
            let p90_pull = cadence.get("p90_pull_interval_s").and_then(|v| v.as_f64());
            let mana_recovery = cadence.get("mean_mana_recovery_s").and_then(|v| v.as_f64());
            let hp_recovery = cadence.get("mean_hp_recovery_s").and_then(|v| v.as_f64());
            let pull_count = cadence.get("pull_count").and_then(|v| v.as_i64());

            tx.execute(
                "INSERT OR REPLACE INTO pull_cadence (session_id, mean_pull_interval_s, p50_pull_interval_s, p90_pull_interval_s, mean_mana_recovery_s, mean_hp_recovery_s, pull_count)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![session_id, mean_pull, p50_pull, p90_pull, mana_recovery, hp_recovery, pull_count],
            )
            .context("Failed to insert pull_cadence")?;
        }

        // ability_contribution
        if let Some(abilities) = aggregates.get("ability_contribution").and_then(|v| v.as_object()) {
            for (ability_name, metrics) in abilities {
                let damage = metrics.get("damage_total").and_then(|v| v.as_i64()).unwrap_or(0);
                let uses = metrics.get("times_used").and_then(|v| v.as_i64()).unwrap_or(0);
                let mana = metrics.get("mana_spent").and_then(|v| v.as_i64()).unwrap_or(0);
                let rank = metrics.get("contribution_rank").and_then(|v| v.as_f64()).unwrap_or(0.0);

                tx.execute(
                    "INSERT OR REPLACE INTO ability_contribution (session_id, ability_name, damage_total, times_used, mana_spent, contribution_rank)
                     VALUES (?, ?, ?, ?, ?, ?)",
                    params![session_id, ability_name, damage, uses, mana, rank],
                )
                .context("Failed to insert ability_contribution")?;
            }
        }

        // route_node_death_rate
        if let Some(nodes) = aggregates.get("route_node_deaths").and_then(|v| v.as_object()) {
            for (key, metrics) in nodes {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() >= 4 {
                    let zone = parts[0];
                    if let (Ok(x), Ok(y), Ok(z)) = (
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                        parts[3].parse::<f64>(),
                    ) {
                        let deaths = metrics.get("death_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let traversals = metrics.get("traversal_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let rate = metrics.get("death_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);

                        tx.execute(
                            "INSERT OR REPLACE INTO route_node_death_rate (session_id, zone, node_x, node_y, node_z, death_count, traversal_count, death_rate)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                            params![session_id, zone, x, y, z, deaths, traversals, rate],
                        )
                        .context("Failed to insert route_node_death_rate")?;
                    }
                }
            }
        }

        // stuck_heatmap
        if let Some(buckets) = aggregates.get("stuck_heatmap").and_then(|v| v.as_object()) {
            for (key, count) in buckets {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 3 {
                    let zone = parts[0];
                    if let (Ok(x_bucket), Ok(y_bucket)) =
                        (parts[1].parse::<i64>(), parts[2].parse::<i64>())
                    {
                        let stuck_count = count.as_i64().unwrap_or(0);

                        tx.execute(
                            "INSERT OR REPLACE INTO stuck_heatmap (session_id, zone, x_bucket, y_bucket, stuck_count)
                             VALUES (?, ?, ?, ?, ?)",
                            params![session_id, zone, x_bucket, y_bucket, stuck_count],
                        )
                        .context("Failed to insert stuck_heatmap")?;
                    }
                }
            }
        }

        // XP Tracking
        if let Some(xp_data) = aggregates.get("xp_tracking").and_then(|v| v.as_object()) {
            if !xp_data.is_empty() {
                let start_level = xp_data.get("start_level").and_then(|v| v.as_i64());
                let end_level = xp_data.get("end_level").and_then(|v| v.as_i64());
                let xp_pct_start = xp_data.get("xp_percent_start").and_then(|v| v.as_f64());
                let xp_pct_end = xp_data.get("xp_percent_end").and_then(|v| v.as_f64());
                let xp_gained = xp_data.get("xp_gained_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let duration = xp_data.get("duration_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
                let xp_per_hour = xp_data.get("xp_per_hour").and_then(|v| v.as_f64());

                tx.execute(
                    "INSERT OR REPLACE INTO xp_tracking_session
                     (session_id, character_name, start_level, end_level, xp_percent_start, xp_percent_end,
                      xp_gained_percent, duration_seconds, xp_per_hour)
                     VALUES (?, 'character', ?, ?, ?, ?, ?, ?, ?)",
                    params![session_id, start_level, end_level, xp_pct_start, xp_pct_end, xp_gained, duration, xp_per_hour],
                )
                .context("Failed to insert xp_tracking_session")?;
            }
        }

        // Kill Tracking
        if let Some(kill_data) = aggregates.get("kill_tracking").and_then(|v| v.as_object()) {
            for (key, metrics) in kill_data {
                if let Some(parts) = key.split(':').collect::<Vec<_>>().get(0..2) {
                    if parts.len() == 2 {
                        let mob_name = parts[0];
                        let zone = parts[1];
                        let kill_count = metrics.get("kill_count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let is_named = metrics.get("is_named").and_then(|v| v.as_bool()).unwrap_or(false);
                        let duration = metrics.get("duration_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
                        let kills_per_hour = metrics.get("kills_per_hour").and_then(|v| v.as_f64());

                        tx.execute(
                            "INSERT OR REPLACE INTO kill_tracking_session
                             (session_id, character_name, mob_name, zone, kill_count, is_named, duration_seconds, kills_per_hour)
                             VALUES (?, 'character', ?, ?, ?, ?, ?, ?)",
                            params![session_id, mob_name, zone, kill_count, is_named, duration, kills_per_hour],
                        )
                        .context("Failed to insert kill_tracking_session")?;
                    }
                }
            }
        }

        // Plat Tracking
        if let Some(plat_data) = aggregates.get("plat_tracking").and_then(|v| v.as_object()) {
            let gross = plat_data.get("gross_platinum").and_then(|v| v.as_i64()).unwrap_or(0);
            let net = plat_data.get("net_platinum").and_then(|v| v.as_i64()).unwrap_or(0);
            let duration = plat_data.get("duration_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
            let plat_per_hour = plat_data.get("platinum_per_hour").and_then(|v| v.as_f64());
            let tx_count = plat_data.get("transaction_count").and_then(|v| v.as_i64()).unwrap_or(0);

            tx.execute(
                "INSERT OR REPLACE INTO plat_summary_session
                 (session_id, character_name, gross_platinum, net_platinum, duration_seconds, platinum_per_hour, transaction_count)
                 VALUES (?, 'character', ?, ?, ?, ?, ?)",
                params![session_id, gross, net, duration, plat_per_hour, tx_count],
            )
            .context("Failed to insert plat_summary_session")?;
        }

        tx.commit().context("Failed to commit transaction")?;
        Ok(())
    }

    fn is_aggregated(&self, session_id: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT 1 FROM aggregated_sessions WHERE session_id = ?")?;
        let exists = stmt.exists(params![session_id])?;
        Ok(exists)
    }

    fn mark_aggregated(&self, session_id: &str, jsonl_path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO aggregated_sessions (session_id, jsonl_path) VALUES (?, ?)",
            params![session_id, jsonl_path],
        )
        .context("Failed to mark aggregated")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_kill_event(zone: &str, duration_ms: i64) -> SessionEvent {
        SessionEvent {
            timestamp: Utc::now().to_rfc3339(),
            session_id: "test-session".to_string(),
            iteration: 1,
            event_type: "ScenarioMobKill".to_string(),
            details: json!({
                "zone": zone,
                "duration_ms": duration_ms,
                "party_comp": "full",
            }),
        }
    }

    #[test]
    fn kill_time_computes_percentiles() {
        let events = vec![
            sample_kill_event("gfaydark", 10000),
            sample_kill_event("gfaydark", 20000),
            sample_kill_event("gfaydark", 30000),
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_kill_time(&events).unwrap();

        let metric = &result["gfaydark:full"];
        assert!(metric.get("mean_ms").is_some());
        assert!(metric.get("p50_ms").is_some());
        assert!(metric.get("p90_ms").is_some());
        assert_eq!(metric["kill_count"], 3);
    }

    #[test]
    fn empty_events_yield_empty_aggregates() {
        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_kill_time(&[]).unwrap();
        assert_eq!(result.as_object().unwrap().len(), 0);
    }

    #[test]
    fn net_plat_aggregates_by_zone() {
        let events = vec![
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 1,
                event_type: "Loot".to_string(),
                details: json!({"zone": "gfaydark", "plat_value": 1000}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 2,
                event_type: "Death".to_string(),
                details: json!({"zone": "gfaydark", "death_cost": 100}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 3,
                event_type: "Consumable".to_string(),
                details: json!({"zone": "gfaydark", "consumable_cost": 50}),
            },
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_net_plat(&events).unwrap();

        assert_eq!(result["gfaydark"]["gross_plat"], 1000);
        assert_eq!(result["gfaydark"]["deaths_cost"], 100);
        assert_eq!(result["gfaydark"]["consumable_cost"], 50);
        assert_eq!(result["gfaydark"]["net_plat"], 850);
    }

    #[test]
    fn spell_efficiency_calculates_success_rate() {
        let events = vec![
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 1,
                event_type: "SpellCast".to_string(),
                details: json!({"spell_name": "heal", "success": true}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 2,
                event_type: "SpellCast".to_string(),
                details: json!({"spell_name": "heal", "success": false}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 3,
                event_type: "SpellCast".to_string(),
                details: json!({"spell_name": "heal", "success": true}),
            },
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_spell_efficiency(&events).unwrap();

        assert_eq!(result["heal"]["total_attempts"], 3);
        assert_eq!(result["heal"]["successful_casts"], 2);
        assert!((result["heal"]["success_rate"].as_f64().unwrap() - (2.0/3.0)).abs() < 0.01);
    }

    #[test]
    fn ability_contribution_ranks_by_efficiency() {
        let events = vec![
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 1,
                event_type: "AbilityUsed".to_string(),
                details: json!({"ability_name": "fireball", "damage": 500, "mana_cost": 100}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 2,
                event_type: "AbilityUsed".to_string(),
                details: json!({"ability_name": "fireball", "damage": 500, "mana_cost": 100}),
            },
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_ability_contribution(&events).unwrap();

        assert_eq!(result["fireball"]["damage_total"], 1000);
        assert_eq!(result["fireball"]["times_used"], 2);
        assert_eq!(result["fireball"]["mana_spent"], 200);
        assert!((result["fireball"]["contribution_rank"].as_f64().unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn route_node_deaths_tracks_hazardous_locations() {
        let events = vec![
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 1,
                event_type: "RouteTraversal".to_string(),
                details: json!({"zone": "gfaydark", "x": 100.0, "y": 200.0, "z": 50.0}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 2,
                event_type: "RouteTraversal".to_string(),
                details: json!({"zone": "gfaydark", "x": 100.0, "y": 200.0, "z": 50.0}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 3,
                event_type: "Death".to_string(),
                details: json!({"zone": "gfaydark", "x": 100.0, "y": 200.0, "z": 50.0}),
            },
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_route_node_deaths(&events).unwrap();

        let key = "gfaydark:100.0:200.0:50.0";
        assert_eq!(result[key]["traversal_count"], 2);
        assert_eq!(result[key]["death_count"], 1);
        assert!((result[key]["death_rate"].as_f64().unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn stuck_heatmap_buckets_by_location() {
        let events = vec![
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 1,
                event_type: "Stuck".to_string(),
                details: json!({"zone": "gfaydark", "x": 150.0, "y": 250.0}),
            },
            SessionEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: "test-session".to_string(),
                iteration: 2,
                event_type: "Stuck".to_string(),
                details: json!({"zone": "gfaydark", "x": 180.0, "y": 280.0}),
            },
        ];

        let store = SessionAggregator::new(":memory:").unwrap();
        let result = store.compute_stuck_heatmap(&events).unwrap();

        let bucket1 = "gfaydark:1:2";
        let bucket2 = "gfaydark:1:2";
        assert!(result.get(bucket1).is_some() || result.get(bucket2).is_some());
    }
}
