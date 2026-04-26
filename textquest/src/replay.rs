//! Replay bundle storage, integrity, and retention utilities.
//!
//! Bundle layout:
//!
//! ```text
//! <root>/<character>/<session_id>/
//!   ├── meta.json
//!   ├── game_state.ndjson.zst
//!   ├── events.ndjson.zst
//!   ├── operator.ndjson.zst
//!   ├── orchestrator.ndjson.zst
//!   ├── dictionary.zstd
//!   └── tui.cast.zst
//! ```
//!
//! The `meta.json` file records the replay session metadata and BLAKE3 hashes
//! for every stored stream.  The `content_hash` is computed from the logical
//! replay streams in a fixed order and is stable across hot/warm/cold
//! recompression.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const REPLAY_LAYOUT_VERSION: u32 = 1;
pub const HOT_TIER_DAYS: u64 = 14;
pub const WARM_TIER_DAYS: u64 = 90;
pub const HOT_ZSTD_LEVEL: i32 = 3;
pub const WARM_ZSTD_LEVEL: i32 = 19;
pub const COLD_ZSTD_LEVEL: i32 = 19;
pub const DEFAULT_DICT_BYTES: usize = 96 * 1024;

const META_FILE: &str = "meta.json";
const GAME_STATE_FILE: &str = "game_state.ndjson.zst";
const EVENTS_FILE: &str = "events.ndjson.zst";
const OPERATOR_FILE: &str = "operator.ndjson.zst";
const ORCHESTRATOR_FILE: &str = "orchestrator.ndjson.zst";
const DICTIONARY_FILE: &str = "dictionary.zstd";
const TUI_CAST_FILE: &str = "tui.cast.zst";

#[allow(dead_code)]
const STREAM_ORDER: [&str; 5] = [
    GAME_STATE_FILE,
    EVENTS_FILE,
    OPERATOR_FILE,
    ORCHESTRATOR_FILE,
    TUI_CAST_FILE,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReplayTier {
    #[default]
    Hot,
    Warm,
    Cold,
    EternalAggregate,
}

impl ReplayTier {
    pub fn zstd_level(self) -> i32 {
        match self {
            Self::Hot => HOT_ZSTD_LEVEL,
            Self::Warm | Self::Cold => WARM_ZSTD_LEVEL,
            Self::EternalAggregate => HOT_ZSTD_LEVEL,
        }
    }

    pub fn downsample_state(self) -> bool {
        matches!(self, Self::Cold)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayMeta {
    pub schema_version: u32,
    pub session_id: String,
    pub character: String,
    pub party: Vec<String>,
    pub zones: Vec<String>,
    pub dictionary_id: Option<String>,
    pub content_hash: String,
    pub policy_sha: String,
    pub config_hash: String,
    pub stream_hashes: BTreeMap<String, String>,
    pub tier: ReplayTier,
    pub created_unix_seconds: u64,
}

impl ReplayMeta {
    pub fn new(
        session_id: impl Into<String>,
        character: impl Into<String>,
        party: Vec<String>,
        zones: Vec<String>,
        policy_sha: impl Into<String>,
        config_hash: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: REPLAY_LAYOUT_VERSION,
            session_id: session_id.into(),
            character: character.into(),
            party,
            zones,
            dictionary_id: None,
            content_hash: String::new(),
            policy_sha: policy_sha.into(),
            config_hash: config_hash.into(),
            stream_hashes: BTreeMap::new(),
            tier: ReplayTier::Hot,
            created_unix_seconds: unix_seconds(SystemTime::now()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayStreams {
    pub game_state: Vec<u8>,
    pub events: Vec<u8>,
    pub operator: Vec<u8>,
    pub orchestrator: Vec<u8>,
    pub tui_cast: Vec<u8>,
}

impl ReplayStreams {
    pub fn new(
        game_state: impl Into<Vec<u8>>,
        events: impl Into<Vec<u8>>,
        operator: impl Into<Vec<u8>>,
        orchestrator: impl Into<Vec<u8>>,
        tui_cast: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            game_state: game_state.into(),
            events: events.into(),
            operator: operator.into(),
            orchestrator: orchestrator.into(),
            tui_cast: tui_cast.into(),
        }
    }

    fn as_ordered_slices(&self) -> [(&'static str, &[u8]); 5] {
        [
            (GAME_STATE_FILE, &self.game_state),
            (EVENTS_FILE, &self.events),
            (OPERATOR_FILE, &self.operator),
            (ORCHESTRATOR_FILE, &self.orchestrator),
            (TUI_CAST_FILE, &self.tui_cast),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayBundle {
    pub meta: ReplayMeta,
    pub streams: ReplayStreams,
    pub dictionary: Option<Vec<u8>>,
}

impl ReplayBundle {
    pub fn new(meta: ReplayMeta, streams: ReplayStreams, dictionary: Option<Vec<u8>>) -> Self {
        Self {
            meta,
            streams,
            dictionary,
        }
    }

    pub fn update_hashes(&mut self) {
        let (stream_hashes, content_hash) = compute_hashes(&self.streams);
        self.meta.stream_hashes = stream_hashes;
        self.meta.content_hash = content_hash;
        self.meta.dictionary_id = self.dictionary.as_deref().map(dictionary_id);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayVerification {
    pub ok: bool,
    pub mismatches: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedReplayBundle {
    pub path: PathBuf,
    pub bundle: ReplayBundle,
    pub verification: ReplayVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplaySummary {
    pub session_id: String,
    pub character: String,
    pub zones: Vec<String>,
    pub party: Vec<String>,
    pub tier: ReplayTier,
    pub created_unix_seconds: u64,
    pub path: PathBuf,
    pub content_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayListFilters {
    pub character: Option<String>,
    pub zone: Option<String>,
    pub since: Option<Duration>,
}

pub fn replay_root() -> PathBuf {
    if let Some(root) = std::env::var_os("TEXTQUEST_REPLAY_DIR") {
        return PathBuf::from(root);
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".textquest").join("replays")
}

pub fn bundle_dir(root: impl AsRef<Path>, character: &str, session_id: &str) -> PathBuf {
    root.as_ref().join(character).join(session_id)
}

pub fn read_meta(path: impl AsRef<Path>) -> Result<ReplayMeta> {
    let meta_path = path.as_ref().join(META_FILE);
    let data = fs::read(&meta_path)
        .with_context(|| format!("failed to read replay meta {}", meta_path.display()))?;
    let meta = serde_json::from_slice(&data)
        .with_context(|| format!("failed to parse replay meta {}", meta_path.display()))?;
    Ok(meta)
}

pub fn write_bundle(dir: impl AsRef<Path>, bundle: &mut ReplayBundle) -> Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create replay bundle dir {}", dir.display()))?;

    let dictionary = bundle
        .dictionary
        .clone()
        .unwrap_or_else(|| build_dictionary(&bundle.streams));
    let compression_dictionary = match bundle.meta.tier {
        ReplayTier::Hot => None,
        ReplayTier::Warm | ReplayTier::Cold | ReplayTier::EternalAggregate => {
            Some(dictionary.as_slice())
        }
    };

    bundle.dictionary = Some(dictionary.clone());
    bundle.update_hashes();

    write_stream(
        dir.join(GAME_STATE_FILE),
        &bundle.streams.game_state,
        bundle.meta.tier.zstd_level(),
        compression_dictionary,
    )?;
    write_stream(
        dir.join(EVENTS_FILE),
        &bundle.streams.events,
        bundle.meta.tier.zstd_level(),
        compression_dictionary,
    )?;
    write_stream(
        dir.join(OPERATOR_FILE),
        &bundle.streams.operator,
        bundle.meta.tier.zstd_level(),
        compression_dictionary,
    )?;
    write_stream(
        dir.join(ORCHESTRATOR_FILE),
        &bundle.streams.orchestrator,
        bundle.meta.tier.zstd_level(),
        compression_dictionary,
    )?;
    write_stream(
        dir.join(TUI_CAST_FILE),
        &bundle.streams.tui_cast,
        bundle.meta.tier.zstd_level(),
        compression_dictionary,
    )?;

    fs::write(dir.join(DICTIONARY_FILE), &dictionary)
        .with_context(|| format!("failed to write replay dictionary {}", dir.display()))?;
    bundle.meta.dictionary_id = bundle.dictionary.as_deref().map(dictionary_id);

    let meta_bytes = serde_json::to_vec_pretty(&bundle.meta)
        .context("failed to serialize replay metadata")?;
    fs::write(dir.join(META_FILE), meta_bytes)
        .with_context(|| format!("failed to write replay meta {}", dir.display()))?;
    Ok(())
}

pub fn write_bundle_with_tier(
    dir: impl AsRef<Path>,
    mut bundle: ReplayBundle,
    tier: ReplayTier,
) -> Result<()> {
    bundle.meta.tier = tier;
    if tier.downsample_state() {
        bundle.streams.game_state = downsample_game_state(&bundle.streams.game_state)?;
    }
    write_bundle(dir, &mut bundle)
}

pub fn load_bundle(dir: impl AsRef<Path>) -> Result<LoadedReplayBundle> {
    let dir = dir.as_ref();
    let meta = read_meta(dir)?;
    let dictionary_path = dir.join(DICTIONARY_FILE);
    let dictionary = if dictionary_path.exists() {
        Some(fs::read(&dictionary_path).with_context(|| {
            format!(
                "failed to read replay dictionary {}",
                dictionary_path.display()
            )
        })?)
    } else {
        None
    };

    let use_dictionary = match meta.tier {
        ReplayTier::Hot => None,
        ReplayTier::Warm | ReplayTier::Cold | ReplayTier::EternalAggregate => {
            dictionary.as_deref()
        }
    };

    let streams = ReplayStreams {
        game_state: read_stream(dir.join(GAME_STATE_FILE), use_dictionary)?,
        events: read_stream(dir.join(EVENTS_FILE), use_dictionary)?,
        operator: read_stream(dir.join(OPERATOR_FILE), use_dictionary)?,
        orchestrator: read_stream(dir.join(ORCHESTRATOR_FILE), use_dictionary)?,
        tui_cast: read_stream(dir.join(TUI_CAST_FILE), use_dictionary)?,
    };
    let bundle = ReplayBundle::new(meta, streams, dictionary);
    let verification = verify_bundle(&bundle);
    Ok(LoadedReplayBundle {
        path: dir.to_path_buf(),
        bundle,
        verification,
    })
}

pub fn verify_bundle(bundle: &ReplayBundle) -> ReplayVerification {
    let (stream_hashes, content_hash) = compute_hashes(&bundle.streams);
    let mut mismatches = Vec::new();

    for (file, hash) in stream_hashes {
        match bundle.meta.stream_hashes.get(&file) {
            Some(expected) if expected == &hash => {}
            Some(expected) => mismatches.push(format!("{file}: expected {expected}, got {hash}")),
            None => mismatches.push(format!("{file}: missing from meta.json")),
        }
    }

    if bundle.meta.content_hash != content_hash {
        mismatches.push(format!(
            "content_hash: expected {}, got {}",
            bundle.meta.content_hash, content_hash
        ));
    }

    ReplayVerification {
        ok: mismatches.is_empty(),
        mismatches,
    }
}

pub fn list_bundles(root: impl AsRef<Path>, filters: &ReplayListFilters) -> Result<Vec<ReplaySummary>> {
    let root = root.as_ref();
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut summaries = Vec::new();
    let cutoff = filters
        .since
        .and_then(|since| SystemTime::now().checked_sub(since));

    for character_entry in fs::read_dir(root)
        .with_context(|| format!("failed to read replay root {}", root.display()))?
    {
        let character_entry = match character_entry {
            Ok(entry) => entry,
            Err(err) => {
                tracing::warn!("replay list: skipping unreadable character entry: {err}");
                continue;
            }
        };
        if !character_entry.path().is_dir() {
            continue;
        }

        let character = character_entry.file_name().to_string_lossy().to_string();
        if let Some(filter_character) = filters.character.as_deref()
            && filter_character != character
        {
            continue;
        }

        for session_entry in match fs::read_dir(character_entry.path()) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::warn!(
                    "replay list: skipping unreadable character dir {}: {err}",
                    character_entry.path().display()
                );
                continue;
            }
        } {
            let session_entry = match session_entry {
                Ok(entry) => entry,
                Err(err) => {
                    tracing::warn!("replay list: skipping unreadable session entry: {err}");
                    continue;
                }
            };
            if !session_entry.path().is_dir() {
                continue;
            }

            let meta = match read_meta(session_entry.path()) {
                Ok(meta) => meta,
                Err(err) => {
                    tracing::warn!(
                        "replay list: skipping bundle {}: {err}",
                        session_entry.path().display()
                    );
                    continue;
                }
            };

            if let Some(zone_filter) = filters.zone.as_deref()
                && !meta.zones.iter().any(|zone| zone == zone_filter)
            {
                continue;
            }

            if let Some(cutoff) = cutoff
                && let Some(created) = UNIX_EPOCH.checked_add(Duration::from_secs(meta.created_unix_seconds))
                && created < cutoff
            {
                continue;
            }

            summaries.push(ReplaySummary {
                session_id: meta.session_id.clone(),
                character: meta.character.clone(),
                zones: meta.zones.clone(),
                party: meta.party.clone(),
                tier: meta.tier,
                created_unix_seconds: meta.created_unix_seconds,
                path: session_entry.path(),
                content_hash: meta.content_hash.clone(),
            });
        }
    }

    summaries.sort_by(|a, b| {
        a.character
            .cmp(&b.character)
            .then(a.created_unix_seconds.cmp(&b.created_unix_seconds))
            .then(a.session_id.cmp(&b.session_id))
    });
    Ok(summaries)
}

pub fn compact_bundle(dir: impl AsRef<Path>) -> Result<ReplayMeta> {
    let loaded = load_bundle(dir.as_ref())?;
    let mut bundle = loaded.bundle;
    let age = bundle_age(&bundle.meta);

    bundle.meta.tier = if age < Duration::from_secs(HOT_TIER_DAYS * 86_400) {
        ReplayTier::Hot
    } else if age < Duration::from_secs(WARM_TIER_DAYS * 86_400) {
        ReplayTier::Warm
    } else {
        ReplayTier::Cold
    };

    if bundle.meta.tier.downsample_state() {
        bundle.streams.game_state = downsample_game_state(&bundle.streams.game_state)?;
    }

    write_bundle(loaded.path, &mut bundle)?;
    Ok(bundle.meta)
}

pub fn compact_root(root: impl AsRef<Path>) -> Result<Vec<ReplayMeta>> {
    let root = root.as_ref();
    let mut updated = Vec::new();

    if !root.exists() {
        return Ok(updated);
    }

    for character_entry in fs::read_dir(root)
        .with_context(|| format!("failed to read replay root {}", root.display()))?
    {
        let character_entry = match character_entry {
            Ok(entry) => entry,
            Err(err) => {
                tracing::warn!("replay compact: skipping unreadable character entry: {err}");
                continue;
            }
        };
        if !character_entry.path().is_dir() {
            continue;
        }

        for session_entry in match fs::read_dir(character_entry.path()) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::warn!(
                    "replay compact: skipping unreadable character dir {}: {err}",
                    character_entry.path().display()
                );
                continue;
            }
        } {
            let session_entry = match session_entry {
                Ok(entry) => entry,
                Err(err) => {
                    tracing::warn!("replay compact: skipping unreadable session entry: {err}");
                    continue;
                }
            };
            if !session_entry.path().is_dir() {
                continue;
            }
            match compact_bundle(session_entry.path()) {
                Ok(meta) => updated.push(meta),
                Err(err) => tracing::warn!(
                    "replay compact: failed to compact {}: {err}",
                    session_entry.path().display()
                ),
            }
        }
    }

    Ok(updated)
}

pub fn export_bundle_to_tqreplay(
    bundle_dir: impl AsRef<Path>,
    output: impl AsRef<Path>,
    redacted: bool,
) -> Result<()> {
    let bundle_dir = bundle_dir.as_ref();
    let output = output.as_ref();
    let file = fs::File::create(output)
        .with_context(|| format!("failed to create replay export {}", output.display()))?;
    let encoder = zstd::stream::write::Encoder::new(file, WARM_ZSTD_LEVEL)
        .context("failed to create zstd encoder for replay export")?;
    let mut builder = tar::Builder::new(encoder);

    let mut entries = vec![META_FILE, GAME_STATE_FILE, EVENTS_FILE, OPERATOR_FILE, ORCHESTRATOR_FILE, DICTIONARY_FILE, TUI_CAST_FILE];
    if redacted {
        entries.retain(|entry| *entry != OPERATOR_FILE);
    }

    for entry in entries {
        let path = bundle_dir.join(entry);
        if !path.exists() {
            continue;
        }
        builder
            .append_path_with_name(&path, entry)
            .with_context(|| format!("failed to append {} to replay export", path.display()))?;
    }

    let encoder = builder
        .into_inner()
        .context("failed to finish tar archive for replay export")?;
    encoder.finish().context("failed to finish replay export")?;
    Ok(())
}

pub fn import_tqreplay(
    input: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> Result<PathBuf> {
    let input = input.as_ref();
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let file = fs::File::open(input)
        .with_context(|| format!("failed to open replay export {}", input.display()))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .context("failed to open replay export zstd stream")?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive
        .entries()
        .context("failed to read replay export entries")?
    {
        let mut entry = entry.context("failed to read replay export entry")?;
        let header = entry.header();
        if header.entry_type().is_symlink() || header.entry_type().is_hard_link() {
            bail!("replay export contains unsupported link entry");
        }

        let entry_path = entry
            .path()
            .context("failed to read replay export entry path")?
            .into_owned();
        if entry_path.is_absolute()
            || entry_path
                .components()
                .any(|component| component == Component::ParentDir)
        {
            bail!(
                "replay export contains invalid entry path: {}",
                entry_path.display()
            );
        }

        let unpacked = entry.unpack_in(output_dir).with_context(|| {
            format!(
                "failed to unpack replay export entry {}",
                entry_path.display()
            )
        })?;
        if !unpacked {
            bail!(
                "replay export entry escapes destination directory: {}",
                entry_path.display()
            );
        }
    }
    Ok(output_dir.to_path_buf())
}

fn read_stream(path: impl AsRef<Path>, dictionary: Option<&[u8]>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let data = fs::read(path)
        .with_context(|| format!("failed to read replay stream {}", path.display()))?;
    decode_stream(&data, dictionary).with_context(|| format!("failed to decode {}", path.display()))
}

fn write_stream(
    path: impl AsRef<Path>,
    bytes: &[u8],
    level: i32,
    dictionary: Option<&[u8]>,
) -> Result<()> {
    let path = path.as_ref();
    let encoded = encode_stream(bytes, level, dictionary)
        .with_context(|| format!("failed to encode replay stream {}", path.display()))?;
    fs::write(path, encoded).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn encode_stream(bytes: &[u8], level: i32, dictionary: Option<&[u8]>) -> Result<Vec<u8>> {
    if let Some(dictionary) = dictionary {
        let mut encoder = zstd::stream::Encoder::with_dictionary(Vec::new(), level, dictionary)
            .context("failed to create dictionary zstd encoder")?;
        encoder.write_all(bytes)?;
        let encoded = encoder.finish()?;
        Ok(encoded)
    } else {
        zstd::stream::encode_all(Cursor::new(bytes), level).context("failed to zstd-compress stream")
    }
}

fn decode_stream(bytes: &[u8], dictionary: Option<&[u8]>) -> Result<Vec<u8>> {
    if let Some(dictionary) = dictionary {
        let mut decoder = zstd::stream::Decoder::with_dictionary(Cursor::new(bytes), dictionary)
            .context("failed to create dictionary zstd decoder")?;
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        Ok(out)
    } else {
        zstd::stream::decode_all(Cursor::new(bytes)).context("failed to zstd-decompress stream")
    }
}

fn compute_hashes(streams: &ReplayStreams) -> (BTreeMap<String, String>, String) {
    let mut hashes = BTreeMap::new();
    let mut content_hasher = blake3::Hasher::new();
    for (name, bytes) in streams.as_ordered_slices() {
        let hash = hash_bytes(bytes);
        hashes.insert(name.to_string(), hash.clone());
        content_hasher.update(bytes);
    }
    (
        hashes,
        content_hasher.finalize().to_hex().to_string(),
    )
}

fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn dictionary_id(bytes: &[u8]) -> String {
    format!("dict-{}", hash_bytes(bytes))
}

fn build_dictionary(streams: &ReplayStreams) -> Vec<u8> {
    let mut dictionary = Vec::new();
    for bytes in [
        &streams.game_state,
        &streams.events,
        &streams.operator,
        &streams.orchestrator,
        &streams.tui_cast,
    ] {
        let take = DEFAULT_DICT_BYTES.saturating_sub(dictionary.len());
        if take == 0 {
            break;
        }
        dictionary.extend(bytes.iter().copied().take(take));
    }
    if dictionary.is_empty() {
        dictionary.extend_from_slice(b"textquest-replay-dictionary");
    }
    dictionary
}

fn downsample_game_state(bytes: &[u8]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(bytes)
        .context("game_state.ndjson stream is not valid UTF-8 for cold downsampling")?;
    let mut last_second: Option<i64> = None;
    let mut out = String::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match ndjson_second(line) {
            Some(second) if last_second == Some(second) => continue,
            Some(second) => {
                last_second = Some(second);
                out.push_str(line);
                out.push('\n');
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }

    Ok(out.into_bytes())
}

fn ndjson_second(line: &str) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let candidate = value
        .get("timestamp")
        .or_else(|| value.get("ts"))
        .or_else(|| value.get("time"))
        .or_else(|| value.get("at"))?;

    if let Some(second) = candidate.as_i64() {
        return Some(second);
    }
    if let Some(float) = candidate.as_f64() {
        return Some(float.floor() as i64);
    }
    if let Some(text) = candidate.as_str() {
        if let Ok(second) = text.parse::<i64>() {
            return Some(second);
        }
        if let Some((seconds, _fraction)) = text.split_once('.')
            && let Ok(second) = seconds.parse::<i64>()
        {
            return Some(second);
        }
    }
    None
}

fn bundle_age(meta: &ReplayMeta) -> Duration {
    let created = UNIX_EPOCH
        .checked_add(Duration::from_secs(meta.created_unix_seconds))
        .unwrap_or(UNIX_EPOCH);
    SystemTime::now()
        .duration_since(created)
        .unwrap_or_else(|_| Duration::from_secs(0))
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tar::{Builder as TarBuilder, Header};

    fn sample_bundle() -> ReplayBundle {
        let mut meta = ReplayMeta::new(
            "session-001",
            "ranger",
            vec!["ranger".into(), "cleric".into()],
            vec!["oot".into(), "karnor".into()],
            "policy-sha",
            "config-hash",
        );
        meta.tier = ReplayTier::Hot;
        let streams = ReplayStreams::new(
            br#"{"ts": 1, "state": "a"}
{"ts": 1, "state": "b"}
{"ts": 2, "state": "c"}
"#,
            b"{\"event\":1}\n{\"event\":2}\n",
            b"{\"operator\":1}\n",
            b"{\"orchestrator\":1}\n",
            b"C 1 80 24\n",
        );
        ReplayBundle::new(meta, streams, None)
    }

    fn temp_bundle_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn write_and_load_round_trip() {
        let dir = temp_bundle_dir();
        let bundle_dir = dir.path().join("ranger").join("session-001");
        let mut bundle = sample_bundle();
        write_bundle(&bundle_dir, &mut bundle).expect("write bundle");

        let loaded = load_bundle(&bundle_dir).expect("load bundle");
        assert!(loaded.verification.ok, "{:?}", loaded.verification.mismatches);
        assert_eq!(loaded.bundle.meta.session_id, "session-001");
        assert_eq!(loaded.bundle.streams.events, b"{\"event\":1}\n{\"event\":2}\n");
    }

    #[test]
    fn verify_detects_tamper() {
        let dir = temp_bundle_dir();
        let bundle_dir = dir.path().join("ranger").join("session-001");
        let mut bundle = sample_bundle();
        write_bundle(&bundle_dir, &mut bundle).expect("write bundle");

        let mut bytes = fs::read(bundle_dir.join(EVENTS_FILE)).expect("read events");
        bytes[0] ^= 0xff;
        fs::write(bundle_dir.join(EVENTS_FILE), bytes).expect("tamper events");

        let loaded = load_bundle(&bundle_dir).expect("load bundle");
        assert!(!loaded.verification.ok);
        assert!(!loaded.verification.mismatches.is_empty());
    }

    #[test]
    fn cold_downsample_keeps_one_state_per_second() {
        let mut bundle = sample_bundle();
        bundle.streams.game_state = br#"{"ts": 10, "state": "a"}
{"ts": 10, "state": "b"}
{"ts": 11, "state": "c"}
{"ts": 11, "state": "d"}
{"ts": 12, "state": "e"}
"#
        .to_vec();
        let downsampled = downsample_game_state(&bundle.streams.game_state).expect("downsample");
        let lines: Vec<&str> = std::str::from_utf8(&downsampled)
            .expect("utf8")
            .lines()
            .collect();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().any(|line| line.contains("\"state\": \"a\"")));
        assert!(lines.iter().any(|line| line.contains("\"state\": \"c\"")));
        assert!(lines.iter().any(|line| line.contains("\"state\": \"e\"")));
    }

    #[test]
    fn export_and_import_round_trip() {
        let dir = temp_bundle_dir();
        let bundle_dir = dir.path().join("ranger").join("session-001");
        let mut bundle = sample_bundle();
        write_bundle(&bundle_dir, &mut bundle).expect("write bundle");

        let export_path = dir.path().join("session-001.tqreplay");
        export_bundle_to_tqreplay(&bundle_dir, &export_path, false).expect("export");

        let import_dir = dir.path().join("imported");
        let unpacked = import_tqreplay(&export_path, &import_dir).expect("import");
        let loaded = load_bundle(unpacked.join("ranger").join("session-001")).expect("load");
        assert!(loaded.verification.ok);
        assert_eq!(loaded.bundle.meta.content_hash, bundle.meta.content_hash);
    }

    #[test]
    fn compact_switches_to_cold_for_old_bundles() {
        let dir = temp_bundle_dir();
        let bundle_dir = dir.path().join("ranger").join("session-001");
        let mut bundle = sample_bundle();
        bundle.meta.created_unix_seconds = unix_seconds(SystemTime::now() - Duration::from_secs(100 * 86_400));
        write_bundle(&bundle_dir, &mut bundle).expect("write bundle");

        let meta = compact_bundle(&bundle_dir).expect("compact");
        assert_eq!(meta.tier, ReplayTier::Cold);
        let loaded = load_bundle(&bundle_dir).expect("load compacted bundle");
        assert!(String::from_utf8_lossy(&loaded.bundle.streams.game_state).lines().count() <= 3);
    }

    #[test]
    fn import_rejects_parent_dir_paths() {
        let dir = temp_bundle_dir();
        let export_path = dir.path().join("malicious.tqreplay");
        let file = fs::File::create(&export_path).expect("create export");
        let encoder = zstd::stream::write::Encoder::new(file, WARM_ZSTD_LEVEL).expect("encoder");
        let mut builder = TarBuilder::new(encoder);

        let payload = b"owned";
        let mut header = Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "../escape.txt", Cursor::new(payload))
            .expect("append");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish zstd");

        let import_dir = dir.path().join("imported");
        let err = import_tqreplay(&export_path, &import_dir).expect_err("reject traversal");
        assert!(err.to_string().contains("invalid entry path"));
    }

    #[test]
    fn import_rejects_symlink_entries() {
        let dir = temp_bundle_dir();
        let export_path = dir.path().join("malicious-link.tqreplay");
        let file = fs::File::create(&export_path).expect("create export");
        let encoder = zstd::stream::write::Encoder::new(file, WARM_ZSTD_LEVEL).expect("encoder");
        let mut builder = TarBuilder::new(encoder);

        let mut header = Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder
            .append_link(&mut header, "pivot", "../outside")
            .expect("append link");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish zstd");

        let import_dir = dir.path().join("imported");
        let err = import_tqreplay(&export_path, &import_dir).expect_err("reject symlink");
        assert!(err.to_string().contains("unsupported link entry"));
    }
}
