//! Session replay recorder for the injected DLL.
//!
//! The recorder captures local PCs plus engaged NPCs into a zstd-compressed
//! NDJSON stream. The file format is intentionally simple:
//! - first line: JSON header object
//! - subsequent lines: `[t, code, payload]`
//!
//! This module is best-effort. If file creation or compression setup fails, the
//! DLL continues running and the recorder stays disabled.

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use textquest_common::{
    combat::{BuffCategory, BuffInfo, CombatStatus},
    types::{ClientId, SharedStateFrame, SpawnData},
};

const REPLAY_VERSION: u32 = 2;
const KEYFRAME_INTERVAL_MS: u64 = 10_000;
const DELTA_INTERVAL_MS: u64 = 250;
const FLUSH_INTERVAL_MS: u64 = 1_000;
const SAMPLE_WINDOW_MS: u64 = 10 * 60 * 1_000;
const LIVE_ZSTD_LEVEL: i32 = 3;
const COLD_ZSTD_LEVEL: i32 = 19;
const DICTIONARY_SIZE: usize = 112 * 1024;
const MAX_DICTIONARY_SAMPLES: usize = 24_000;

static RECORDER: OnceLock<Mutex<ReplayRecorder>> = OnceLock::new();

pub type CharId = u32;
pub type SpawnId = u32;
pub type ZoneId = String;
pub type BitSet = BTreeSet<u32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PcMode {
    Tank,
    Dps,
    Heal,
    Support,
    Idle,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitStand {
    Standing,
    Sitting,
    Ducking,
    Feigning,
    Dead,
    Unknown { raw: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuffSlot {
    pub spell_id: u32,
    pub ticks_remaining: i32,
    pub caster: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcState {
    pub character_id: CharId,
    pub hp: u32,
    pub mana: u32,
    pub end: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub zone: ZoneId,
    pub target_id: Option<SpawnId>,
    pub buffs: Vec<BuffSlot>,
    pub debuffs: Vec<BuffSlot>,
    pub aa_ready: BitSet,
    pub gcd_remaining_ms: u32,
    pub autoattack: bool,
    pub mode: PcMode,
    pub sit_stand: SitStand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcState {
    pub spawn_id: SpawnId,
    pub name: String,
    pub hp: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub zone: ZoneId,
    pub target_id: Option<SpawnId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub version: u32,
    pub session_id: String,
    pub party: Vec<String>,
    pub start_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplayMeta {
    version: u32,
    session_id: String,
    character: String,
    party: Vec<String>,
    zones: Vec<String>,
    start_ts: u64,
    end_ts: Option<u64>,
    keyframes: u64,
    deltas: u64,
    spawn_events: u64,
    dictionary_id: Option<String>,
    dictionary_bytes: Option<usize>,
}

#[derive(Debug, Default, Clone)]
struct CadenceState {
    last_keyframe_ms: Option<u64>,
    last_delta_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CadenceDecision {
    Keyframe,
    Delta,
    None,
}

impl CadenceState {
    fn observe(&mut self, now_ms: u64, changed: bool) -> CadenceDecision {
        if self
            .last_keyframe_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= KEYFRAME_INTERVAL_MS)
        {
            self.last_keyframe_ms = Some(now_ms);
            self.last_delta_ms = Some(now_ms);
            return CadenceDecision::Keyframe;
        }

        if changed
            && self
                .last_delta_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= DELTA_INTERVAL_MS)
        {
            self.last_delta_ms = Some(now_ms);
            return CadenceDecision::Delta;
        }

        CadenceDecision::None
    }
}

#[derive(Debug, Clone)]
struct TrackedPc {
    state: PcState,
    cadence: CadenceState,
    last_seen_ms: u64,
}

#[derive(Debug, Clone)]
struct TrackedNpc {
    state: NpcState,
    cadence: CadenceState,
    last_seen_ms: u64,
}

struct EventWriter {
    encoder: zstd::stream::write::Encoder<'static, File>,
}

impl std::fmt::Debug for EventWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventWriter").finish_non_exhaustive()
    }
}

impl EventWriter {
    fn from_file(file: File, level: i32, dictionary: Option<&[u8]>) -> io::Result<Self> {
        let encoder = match dictionary {
            Some(dictionary) => {
                zstd::stream::write::Encoder::with_dictionary(file, level, dictionary)
                    .map_err(io::Error::other)?
            }
            None => zstd::stream::write::Encoder::new(file, level).map_err(io::Error::other)?,
        };
        Ok(Self { encoder })
    }

    fn open(path: &Path, level: i32, dictionary: Option<&[u8]>) -> io::Result<Self> {
        let file = secure_create_file(path)?;
        Self::from_file(file, level, dictionary)
    }

    fn finish(self) -> io::Result<File> {
        self.encoder.finish().map_err(io::Error::other)
    }

    fn write_header(&mut self, header: &ReplayHeader) -> io::Result<()> {
        let line = serde_json::to_vec(header).map_err(io::Error::other)?;
        self.encoder.write_all(&line)?;
        self.encoder.write_all(b"\n")?;
        Ok(())
    }

    fn write_event<P: Serialize>(
        &mut self,
        timestamp_ms: u64,
        code: char,
        payload: &P,
    ) -> io::Result<Vec<u8>> {
        let timestamp = timestamp_ms as f64 / 1000.0;
        let line = serde_json::to_vec(&(timestamp, code, payload)).map_err(io::Error::other)?;
        self.encoder.write_all(&line)?;
        self.encoder.write_all(b"\n")?;
        Ok(line)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush()?;
        self.encoder.get_mut().sync_all()
    }
}

#[derive(Debug)]
struct ReplayRecorder {
    client_id: ClientId,
    session_id: String,
    root_dir: PathBuf,
    stream_path: PathBuf,
    meta_path: PathBuf,
    dictionary_path: PathBuf,
    writer: Option<EventWriter>,
    metadata: Option<ReplayMeta>,
    started_at_ms: Option<u64>,
    sample_window_end_ms: Option<u64>,
    dictionary: Option<Vec<u8>>,
    sample_lines: Vec<Vec<u8>>,
    party: BTreeSet<String>,
    zones: BTreeSet<String>,
    tracked_pcs: HashMap<CharId, TrackedPc>,
    tracked_npcs: HashMap<SpawnId, TrackedNpc>,
    cached_spawns: Vec<SpawnData>,
    last_sync_ms: u64,
}

impl ReplayRecorder {
    fn new(client_id: ClientId, session_id: u64) -> Self {
        let session_id = format!("{session_id:x}");
        let root_dir = replay_root_dir();
        Self {
            client_id,
            session_id,
            root_dir: root_dir.clone(),
            stream_path: root_dir.join("game_state.ndjson.zst"),
            meta_path: root_dir.join("meta.json"),
            dictionary_path: root_dir.join("dictionary.zstd"),
            writer: None,
            metadata: None,
            started_at_ms: None,
            sample_window_end_ms: None,
            dictionary: None,
            sample_lines: Vec::new(),
            party: BTreeSet::new(),
            zones: BTreeSet::new(),
            tracked_pcs: HashMap::new(),
            tracked_npcs: HashMap::new(),
            cached_spawns: Vec::new(),
            last_sync_ms: 0,
        }
    }

    fn ensure_started(&mut self, frame: &SharedStateFrame) -> io::Result<()> {
        if self.writer.is_some() {
            return Ok(());
        }

        let Some(local_player) = frame.local_player.as_ref() else {
            return Ok(());
        };

        let character = sanitize_component(&display_name(local_player));
        let root_dir = replay_root_dir().join(&character).join(&self.session_id);
        ensure_secure_replay_dir(&root_dir)?;

        self.root_dir = root_dir.clone();
        self.stream_path = root_dir.join("game_state.ndjson.zst");
        self.meta_path = root_dir.join("meta.json");
        self.dictionary_path = root_dir.join("dictionary.zstd");

        let mut writer = EventWriter::open(&self.stream_path, LIVE_ZSTD_LEVEL, None)?;
        let party = self.party_from_frame(frame);
        let header = ReplayHeader {
            version: REPLAY_VERSION,
            session_id: self.session_id.clone(),
            party: party.iter().cloned().collect(),
            start_ts: frame.timestamp_ms,
        };
        writer.write_header(&header)?;

        self.party = party;
        self.zones.insert(frame.zone_short_name.clone());
        self.metadata = Some(ReplayMeta {
            version: REPLAY_VERSION,
            session_id: self.session_id.clone(),
            character,
            party: self.party.iter().cloned().collect(),
            zones: self.zones.iter().cloned().collect(),
            start_ts: frame.timestamp_ms,
            end_ts: None,
            keyframes: 0,
            deltas: 0,
            spawn_events: 0,
            dictionary_id: None,
            dictionary_bytes: None,
        });
        self.started_at_ms = Some(frame.timestamp_ms);
        self.sample_window_end_ms = Some(frame.timestamp_ms.saturating_add(SAMPLE_WINDOW_MS));
        self.last_sync_ms = frame.timestamp_ms;
        self.writer = Some(writer);
        self.persist_meta()?;
        Ok(())
    }

    fn record_frame(&mut self, frame: &SharedStateFrame) {
        if frame.local_player.is_none() {
            return;
        }

        if let Err(error) = self.ensure_started(frame) {
            tracing::warn!(client_id = self.client_id, error = %error, "replay recorder start failed");
            self.writer = None;
            return;
        }

        let Some(mut writer) = self.writer.take() else {
            return;
        };

        if let Some(spawns) = frame.nearby_spawns.as_ref() {
            self.cached_spawns = spawns.clone();
        }
        self.zones.insert(frame.zone_short_name.clone());
        self.merge_party(frame);

        let now_ms = frame.timestamp_ms;
        let target_id = frame.target.as_ref().map(|spawn| spawn.spawn_id);
        let engaged_npcs = self.current_engaged_npc_ids(frame, target_id);
        let current_pcs = self.collect_player_spawns(frame);
        let current_npcs = self.collect_engaged_npc_spawns(frame, &engaged_npcs);

        let current_pc_ids: BTreeSet<CharId> =
            current_pcs.iter().map(|spawn| spawn.spawn_id).collect();
        let current_npc_ids: BTreeSet<SpawnId> =
            current_npcs.iter().map(|spawn| spawn.spawn_id).collect();

        let mut events: Vec<(char, Value)> = Vec::new();
        for spawn in current_pcs {
            if let Some(event) = self.observe_pc(now_ms, spawn, frame, target_id) {
                events.push(event);
            }
        }

        for spawn in current_npcs {
            events.extend(self.observe_npc(now_ms, spawn, frame, target_id));
        }

        events.extend(self.reap_missing_npcs(now_ms, &current_npc_ids));
        self.reap_missing_pcs(now_ms, &current_pc_ids);

        if !events.is_empty() {
            for (code, payload) in events {
                if let Err(error) = write_event_record(
                    &mut writer,
                    &mut self.sample_lines,
                    self.sample_window_end_ms,
                    &mut self.metadata,
                    now_ms,
                    code,
                    &payload,
                ) {
                    tracing::warn!(client_id = self.client_id, error = %error, "replay write failed");
                    self.writer = None;
                    return;
                }
            }
            if now_ms.saturating_sub(self.last_sync_ms) >= FLUSH_INTERVAL_MS {
                if let Err(error) = writer.flush() {
                    tracing::warn!(client_id = self.client_id, error = %error, "replay flush failed");
                }
                self.last_sync_ms = now_ms;
            }
        }

        self.writer = Some(writer);

        if self
            .sample_window_end_ms
            .is_some_and(|deadline| now_ms >= deadline)
            && self.dictionary.is_none()
        {
            self.sample_window_end_ms = None;
            if let Err(error) = self.train_dictionary() {
                tracing::warn!(client_id = self.client_id, error = %error, "replay dictionary training failed");
            }
        }
    }

    fn stop(&mut self) {
        if let Some(mut meta) = self.metadata.take() {
            meta.end_ts = Some(
                self.last_sync_ms
                    .max(self.started_at_ms.unwrap_or(self.last_sync_ms)),
            );
            meta.party = self.party.iter().cloned().collect();
            meta.zones = self.zones.iter().cloned().collect();
            self.metadata = Some(meta);
            let _ = self.persist_meta();
        }

        if let Some(writer) = self.writer.take() {
            let _ = writer.finish();
        }
    }

    fn party_from_frame(&self, frame: &SharedStateFrame) -> BTreeSet<String> {
        let mut party = BTreeSet::new();
        if let Some(local_player) = frame.local_player.as_ref() {
            party.insert(display_name(local_player));
        }
        if let Some(spawns) = frame.nearby_spawns.as_ref() {
            for spawn in spawns.iter().filter(|spawn| spawn.spawn_type == 0) {
                party.insert(display_name(spawn));
            }
        }
        party
    }

    fn merge_party(&mut self, frame: &SharedStateFrame) {
        self.party.extend(self.party_from_frame(frame));
        if let Some(meta) = self.metadata.as_mut() {
            meta.party = self.party.iter().cloned().collect();
            meta.zones = self.zones.iter().cloned().collect();
        }
    }

    fn collect_player_spawns(&self, frame: &SharedStateFrame) -> Vec<SpawnData> {
        let mut pcs = Vec::new();
        if let Some(local_player) = frame.local_player.clone() {
            pcs.push(local_player);
        }
        if let Some(spawns) = frame.nearby_spawns.as_ref() {
            for spawn in spawns {
                if spawn.spawn_type == 0 && !pcs.iter().any(|pc| pc.spawn_id == spawn.spawn_id) {
                    pcs.push(spawn.clone());
                }
            }
        }
        pcs
    }

    fn collect_engaged_npc_spawns(
        &self,
        frame: &SharedStateFrame,
        engaged_ids: &BTreeSet<SpawnId>,
    ) -> Vec<SpawnData> {
        let mut npcs = Vec::new();
        if let Some(spawns) = frame.nearby_spawns.as_ref() {
            for spawn in spawns {
                if spawn.spawn_type != 0
                    && (engaged_ids.contains(&spawn.spawn_id)
                        || self.tracked_npcs.contains_key(&spawn.spawn_id))
                {
                    npcs.push(spawn.clone());
                }
            }
        }
        npcs
    }

    fn current_engaged_npc_ids(
        &self,
        frame: &SharedStateFrame,
        target_id: Option<SpawnId>,
    ) -> BTreeSet<SpawnId> {
        let mut ids = BTreeSet::new();
        if let Some(target_id) = target_id {
            ids.insert(target_id);
        }

        #[cfg(windows)]
        {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            if eq_base != 0 {
                if let Some(targets) =
                    unsafe { crate::combat::xtarget::read_extended_targets(eq_base) }
                {
                    ids.extend(targets.hater_spawn_ids());
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = frame;
        }

        ids
    }

    fn observe_pc(
        &mut self,
        now_ms: u64,
        spawn: SpawnData,
        frame: &SharedStateFrame,
        target_id: Option<SpawnId>,
    ) -> Option<(char, Value)> {
        let state = self.make_pc_state(&spawn, frame, target_id);
        let tracked = self
            .tracked_pcs
            .entry(state.character_id)
            .or_insert_with(|| TrackedPc {
                state: state.clone(),
                cadence: CadenceState::default(),
                last_seen_ms: now_ms,
            });
        let previous = tracked.state.clone();
        tracked.last_seen_ms = now_ms;
        let decision = tracked.cadence.observe(now_ms, previous != state);
        tracked.state = state.clone();

        match decision {
            CadenceDecision::Keyframe => Some(('k', serde_json::to_value(state).ok()?)),
            CadenceDecision::Delta => Some(('d', delta_payload(&previous, &state, "character_id"))),
            CadenceDecision::None => None,
        }
    }

    fn observe_npc(
        &mut self,
        now_ms: u64,
        spawn: SpawnData,
        frame: &SharedStateFrame,
        target_id: Option<SpawnId>,
    ) -> Vec<(char, Value)> {
        let state = self.make_npc_state(&spawn, frame, target_id);
        let was_new = !self.tracked_npcs.contains_key(&state.spawn_id);
        let tracked = self
            .tracked_npcs
            .entry(state.spawn_id)
            .or_insert_with(|| TrackedNpc {
                state: state.clone(),
                cadence: CadenceState::default(),
                last_seen_ms: now_ms,
            });
        let previous = tracked.state.clone();
        tracked.last_seen_ms = now_ms;
        let decision = tracked.cadence.observe(now_ms, previous != state);
        tracked.state = state.clone();

        let mut events = Vec::new();
        if was_new {
            if let Ok(payload) = serde_json::to_value(&state) {
                events.push(('s', payload));
            }
        }
        match decision {
            CadenceDecision::Keyframe => {
                if let Ok(payload) = serde_json::to_value(&state) {
                    events.push(('k', payload));
                }
            }
            CadenceDecision::Delta => {
                events.push(('d', delta_payload(&previous, &state, "spawn_id")));
            }
            CadenceDecision::None => {}
        }
        events
    }

    fn reap_missing_npcs(
        &mut self,
        now_ms: u64,
        current_npc_ids: &BTreeSet<SpawnId>,
    ) -> Vec<(char, Value)> {
        let mut events = Vec::new();
        let stale_ids: Vec<SpawnId> = self
            .tracked_npcs
            .iter()
            .filter_map(|(&spawn_id, tracked)| {
                (!current_npc_ids.contains(&spawn_id)
                    && now_ms.saturating_sub(tracked.last_seen_ms) >= DELTA_INTERVAL_MS)
                    .then_some(spawn_id)
            })
            .collect();

        for spawn_id in stale_ids {
            if let Some(tracked) = self.tracked_npcs.remove(&spawn_id) {
                let payload = serde_json::json!({
                    "spawn_id": tracked.state.spawn_id,
                    "name": tracked.state.name,
                });
                events.push(('x', payload));
            }
        }

        events
    }

    fn reap_missing_pcs(&mut self, now_ms: u64, current_pc_ids: &BTreeSet<CharId>) {
        let stale_ids: Vec<CharId> = self
            .tracked_pcs
            .iter()
            .filter_map(|(&character_id, tracked)| {
                (!current_pc_ids.contains(&character_id)
                    && now_ms.saturating_sub(tracked.last_seen_ms) >= DELTA_INTERVAL_MS)
                    .then_some(character_id)
            })
            .collect();

        for character_id in stale_ids {
            self.tracked_pcs.remove(&character_id);
        }
    }

    fn make_pc_state(
        &self,
        spawn: &SpawnData,
        frame: &SharedStateFrame,
        target_id: Option<SpawnId>,
    ) -> PcState {
        let local_player_id = frame.local_player.as_ref().map(|player| player.spawn_id);
        let local = local_player_id == Some(spawn.spawn_id);
        PcState {
            character_id: spawn.spawn_id,
            hp: clamp_to_u32(spawn.hp_current),
            mana: clamp_to_u32(spawn.mana_current as i64),
            end: clamp_to_u32(spawn.endurance_current as i64),
            x: spawn.x,
            y: spawn.y,
            z: spawn.z,
            heading: spawn.heading,
            zone: frame.zone_short_name.clone(),
            target_id: if local { target_id } else { None },
            buffs: if local {
                self.active_buffs(frame)
            } else {
                Vec::new()
            },
            debuffs: Vec::new(),
            aa_ready: BTreeSet::new(),
            gcd_remaining_ms: if local && matches!(frame.combat_status, CombatStatus::OnGcd) {
                1_500
            } else {
                0
            },
            autoattack: local
                && matches!(
                    frame.combat_status,
                    CombatStatus::Engaging { .. } | CombatStatus::Pulling { .. }
                ),
            mode: mode_for_class(spawn.class_id),
            sit_stand: sit_stand_from_raw(spawn.stand_state),
        }
    }

    fn make_npc_state(
        &self,
        spawn: &SpawnData,
        frame: &SharedStateFrame,
        target_id: Option<SpawnId>,
    ) -> NpcState {
        NpcState {
            spawn_id: spawn.spawn_id,
            name: display_name(spawn),
            hp: clamp_to_u32(spawn.hp_current),
            x: spawn.x,
            y: spawn.y,
            z: spawn.z,
            heading: spawn.heading,
            zone: frame.zone_short_name.clone(),
            target_id: (target_id == Some(spawn.spawn_id)).then_some(spawn.spawn_id),
        }
    }

    fn active_buffs(&self, frame: &SharedStateFrame) -> Vec<BuffSlot> {
        frame
            .active_buffs
            .iter()
            .filter(|buff| {
                matches!(
                    buff.category,
                    BuffCategory::LongBuff | BuffCategory::ShortBuff
                )
            })
            .map(buff_slot_from_info)
            .collect()
    }

    fn persist_meta(&self) -> io::Result<()> {
        if let Some(meta) = self.metadata.as_ref() {
            secure_replace_file(
                &self.meta_path,
                &serde_json::to_vec_pretty(meta).map_err(io::Error::other)?,
            )
        } else {
            Ok(())
        }
    }

    fn train_dictionary(&mut self) -> io::Result<()> {
        if self.sample_lines.len() < 64 {
            return Ok(());
        }

        let samples: Vec<&[u8]> = self.sample_lines.iter().map(Vec::as_slice).collect();
        let dict = zstd::dict::from_samples(&samples, DICTIONARY_SIZE).map_err(io::Error::other)?;
        secure_create_and_write_file(&self.dictionary_path, &dict)?;

        if let Some(writer) = self.writer.take() {
            let file = writer.finish()?;
            self.writer = Some(EventWriter::from_file(file, COLD_ZSTD_LEVEL, Some(&dict))?);
        }

        self.dictionary = Some(dict.clone());
        if let Some(meta) = self.metadata.as_mut() {
            meta.dictionary_id = Some(hash_bytes(&dict));
            meta.dictionary_bytes = Some(dict.len());
        }
        self.persist_meta()?;
        self.sample_lines.clear();
        Ok(())
    }
}

fn ensure_secure_replay_dir(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(io::Error::other(format!(
                        "refusing to write replay data through symlinked directory: {}",
                        current.display()
                    )));
                }
                if !meta.is_dir() {
                    return Err(io::Error::other(format!(
                        "replay path component is not a directory: {}",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                builder.mode(0o700);
                builder.create(&current)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn secure_create_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

fn secure_create_and_write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = secure_create_file(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn secure_replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(io::Error::other(format!(
                "refusing to overwrite symlinked replay file: {}",
                path.display()
            )));
        }
        if !meta.is_file() {
            return Err(io::Error::other(format!(
                "replay metadata path is not a regular file: {}",
                path.display()
            )));
        }
        fs::remove_file(path)?;
    }
    secure_create_and_write_file(path, bytes)
}

fn replay_root_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        PathBuf::from(home).join(".textquest").join("replays")
    } else {
        PathBuf::from(".textquest").join("replays")
    }
}

fn sanitize_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        String::from("unknown")
    } else {
        out
    }
}

fn display_name(spawn: &SpawnData) -> String {
    if spawn.displayed_name.is_empty() {
        spawn.name.clone()
    } else {
        spawn.displayed_name.clone()
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn clamp_to_u32(value: i64) -> u32 {
    value.max(0).min(u32::MAX as i64) as u32
}

fn sit_stand_from_raw(raw: u8) -> SitStand {
    match raw {
        0 => SitStand::Standing,
        3 => SitStand::Sitting,
        4 => SitStand::Ducking,
        110 => SitStand::Feigning,
        111 => SitStand::Dead,
        raw => SitStand::Unknown { raw },
    }
}

fn mode_for_class(class_id: u8) -> PcMode {
    match class_id {
        1 | 3 | 5 | 16 => PcMode::Tank,
        2 | 6 | 10 => PcMode::Heal,
        8 | 14 | 25 | 26 => PcMode::Support,
        4 | 7 | 9 | 11 | 12 | 13 | 15 => PcMode::Dps,
        _ => PcMode::Unknown,
    }
}

fn buff_slot_from_info(info: &BuffInfo) -> BuffSlot {
    BuffSlot {
        spell_id: info.spell_id.max(0) as u32,
        ticks_remaining: info.duration_ticks.max(0),
        caster: Some(format!("lvl{}", info.caster_level)),
    }
}

fn delta_payload<T: Serialize>(previous: &T, current: &T, id_key: &str) -> Value {
    let previous = serde_json::to_value(previous).unwrap_or(Value::Null);
    let current = serde_json::to_value(current).unwrap_or(Value::Null);
    let mut payload = Map::new();

    if let Value::Object(current_map) = current {
        if let Some(id) = current_map.get(id_key).cloned() {
            payload.insert(id_key.to_string(), id);
        }

        if let Value::Object(previous_map) = previous {
            for (key, current_value) in current_map {
                if key == id_key {
                    continue;
                }
                if previous_map.get(&key) != Some(&current_value) {
                    payload.insert(key, current_value);
                }
            }
        }
    }

    Value::Object(payload)
}

fn write_event_record<P: Serialize>(
    writer: &mut EventWriter,
    sample_lines: &mut Vec<Vec<u8>>,
    sample_window_end_ms: Option<u64>,
    metadata: &mut Option<ReplayMeta>,
    now_ms: u64,
    code: char,
    payload: &P,
) -> io::Result<()> {
    let line = writer.write_event(now_ms, code, payload)?;
    if sample_window_end_ms.is_some_and(|deadline| now_ms <= deadline)
        && sample_lines.len() < MAX_DICTIONARY_SAMPLES
    {
        sample_lines.push(line);
    }
    if let Some(meta) = metadata.as_mut() {
        match code {
            'k' => meta.keyframes += 1,
            'd' => meta.deltas += 1,
            's' | 'x' => meta.spawn_events += 1,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn start(client_id: ClientId, session_id: u64) -> io::Result<()> {
    let recorder = RECORDER.get_or_init(|| Mutex::new(ReplayRecorder::new(client_id, session_id)));
    let mut guard = recorder
        .lock()
        .map_err(|_| io::Error::other("replay recorder mutex poisoned"))?;
    *guard = ReplayRecorder::new(client_id, session_id);
    Ok(())
}

#[cfg(not(windows))]
pub fn start(_client_id: ClientId, _session_id: u64) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn record_state(frame: &SharedStateFrame) {
    let Some(recorder) = RECORDER.get() else {
        return;
    };
    let Ok(mut guard) = recorder.lock() else {
        tracing::error!("replay recorder mutex poisoned");
        return;
    };
    guard.record_frame(frame);
}

#[cfg(not(windows))]
pub fn record_state(_frame: &SharedStateFrame) {}

#[cfg(windows)]
pub fn stop() {
    let Some(recorder) = RECORDER.get() else {
        return;
    };
    let Ok(mut guard) = recorder.lock() else {
        tracing::error!("replay recorder mutex poisoned during stop");
        return;
    };
    guard.stop();
}

#[cfg(not(windows))]
pub fn stop() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pc_state(character_id: u32, hp: u32, mana: u32, end: u32) -> PcState {
        PcState {
            character_id,
            hp,
            mana,
            end,
            x: 1.0,
            y: 2.0,
            z: 3.0,
            heading: 4.0,
            zone: String::from("testzone"),
            target_id: Some(42),
            buffs: vec![BuffSlot {
                spell_id: 123,
                ticks_remaining: 5,
                caster: Some(String::from("lvl60")),
            }],
            debuffs: Vec::new(),
            aa_ready: BTreeSet::new(),
            gcd_remaining_ms: 0,
            autoattack: false,
            mode: PcMode::Dps,
            sit_stand: SitStand::Standing,
        }
    }

    #[test]
    fn cadence_emits_keyframes_and_deltas_on_schedule() {
        let mut cadence = CadenceState::default();
        let mut keyframes = 0usize;
        let mut deltas = 0usize;

        for tick in 0..14_400u64 {
            let now_ms = tick * DELTA_INTERVAL_MS;
            match cadence.observe(now_ms, true) {
                CadenceDecision::Keyframe => keyframes += 1,
                CadenceDecision::Delta => deltas += 1,
                CadenceDecision::None => {}
            }
        }

        assert_eq!(keyframes, 360);
        assert_eq!(deltas, 14_040);
    }

    #[test]
    fn delta_payload_only_contains_changed_fields() {
        let previous = make_pc_state(7, 100, 50, 25);
        let mut current = previous.clone();
        current.hp = 99;
        current.autoattack = true;
        let payload = delta_payload(&previous, &current, "character_id");
        let fields = payload.as_object().expect("object payload");
        assert_eq!(fields.get("character_id").and_then(Value::as_u64), Some(7));
        assert!(fields.contains_key("hp"));
        assert!(fields.contains_key("autoattack"));
        assert!(!fields.contains_key("mana"));
    }

    #[test]
    fn replay_header_serializes_as_single_json_object() {
        let header = ReplayHeader {
            version: REPLAY_VERSION,
            session_id: String::from("abc123"),
            party: vec![String::from("Tank"), String::from("Cleric")],
            start_ts: 123,
        };
        let json = serde_json::to_string(&header).expect("serialize header");
        let back: ReplayHeader = serde_json::from_str(&json).expect("deserialize header");
        assert_eq!(back.session_id, "abc123");
        assert_eq!(back.party.len(), 2);
    }

    #[test]
    fn sitstand_and_mode_mappings_cover_expected_variants() {
        assert!(matches!(sit_stand_from_raw(0), SitStand::Standing));
        assert!(matches!(sit_stand_from_raw(111), SitStand::Dead));
        assert!(matches!(mode_for_class(2), PcMode::Heal));
        assert!(matches!(mode_for_class(12), PcMode::Dps));
    }
}
