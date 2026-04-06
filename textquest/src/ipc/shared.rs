//! Shared memory READER (orchestrator side).
//!
//! Reads game state published by the injected DLL via a named shared memory
//! region. Uses `OpenFileMappingW` with `FILE_MAP_READ` (read-only, least privilege).
//! On non-Windows platforms it returns an empty stub so the project compiles.

use anyhow::Result;
use std::sync::LazyLock;
use textquest_common::types::{ClientId, GameState, SharedStateFrame, SpawnData};

static PERF_TRACE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var(textquest_common::ipc::PERF_TRACE_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
});

/// Reads game state from shared memory for a specific client.
pub struct SharedStateReader {
    #[allow(dead_code)] // Used for diagnostics and future per-client filtering
    client_id: ClientId,
    #[cfg(windows)]
    _handle: windows::Win32::Foundation::HANDLE,
    #[cfg(windows)]
    _ptr: *mut u8,
    #[cfg(windows)]
    _size: usize,
    cached_spawns: Option<(u64, Vec<SpawnData>)>,
}

// SAFETY: SharedStateReader is only accessed from the orchestrator's poll thread (single reader).
// The sequence number uses AtomicU64 with Acquire ordering as a read fence.
// The writer side uses Release ordering to ensure the complete payload is visible.
#[cfg(windows)]
unsafe impl Send for SharedStateReader {}
#[cfg(windows)]
unsafe impl Sync for SharedStateReader {}

impl SharedStateReader {
    /// Open an existing named shared memory region for `client_id`.
    ///
    /// The DLL (writer) creates the region; this opens it **read-only**.
    /// Returns an error if the DLL has not yet created the mapping — callers
    /// should retry on the next poll cycle.
    ///
    /// Memory name: `textquest_state_{client_id}`
    ///
    /// Uses `OpenFileMappingW` + `FILE_MAP_READ` — the orchestrator has no need
    /// for write access to the DLL-owned mapping.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(client_id: ClientId, session_id: u64) -> Result<Self> {
        #[cfg(windows)]
        {
            use textquest_common::ipc::SHARED_MEMORY_SIZE;
            use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile, OpenFileMappingW};
            use windows::core::PCWSTR;

            let name: Vec<u16> = format!(
                "{}\0",
                textquest_common::ipc::shared_memory_name(session_id, client_id)
            )
            .encode_utf16()
            .collect();

            // Open the mapping created by the DLL with read-only access.
            // OpenFileMappingW (not CreateFileMappingW) ensures the orchestrator
            // can never accidentally write to shared memory; PAGE_READWRITE is
            // not needed or requested here.
            let handle =
                unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(name.as_ptr())) }?;

            let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, SHARED_MEMORY_SIZE) };
            if ptr.Value.is_null() {
                anyhow::bail!("MapViewOfFile returned null for client {client_id}");
            }

            Ok(Self {
                client_id,
                _handle: handle,
                _ptr: ptr.Value as *mut u8,
                _size: SHARED_MEMORY_SIZE,
                cached_spawns: None,
            })
        }

        #[cfg(not(windows))]
        {
            let _ = session_id;
            Ok(Self {
                client_id,
                cached_spawns: None,
            })
        }
    }

    /// Read the latest game state. Returns `None` if no data is available yet.
    ///
    /// The shared memory layout is:
    /// ```text
    /// [sequence: u64 LE][payload_len: u32 LE][payload: bincode bytes]
    /// ```
    /// A zero sequence number means the DLL hasn't written yet.
    #[must_use]
    pub fn read(&mut self) -> Option<GameState> {
        #[cfg(windows)]
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            use std::time::Instant;

            let perf_start = if *PERF_TRACE_ENABLED {
                Some(Instant::now())
            } else {
                None
            };

            let base = self._ptr;
            let seq = unsafe { &*(base as *const AtomicU64) };

            // 1. Load sequence number with Acquire ordering
            let seq_before = seq.load(Ordering::Acquire);

            // 2. Sequence 0 means no data yet; odd means write in progress — retry
            if seq_before == 0 || seq_before % 2 != 0 {
                return None;
            }

            // 3. Read payload length
            let len_bytes: [u8; 4] = unsafe { std::ptr::read(base.add(8) as *const [u8; 4]) };
            let payload_len = u32::from_le_bytes(len_bytes) as usize;

            if payload_len == 0 || payload_len > self._size - 12 {
                return None;
            }

            // 4. Copy payload into a local buffer to avoid referencing shared memory during decode
            let mut payload_copy = vec![0u8; payload_len];
            unsafe {
                std::ptr::copy_nonoverlapping(base.add(12), payload_copy.as_mut_ptr(), payload_len);
            }

            // 5. Re-check sequence number — if it changed, data may be torn
            let seq_after = seq.load(Ordering::Acquire);
            if seq_after != seq_before {
                return None;
            }

            // 6. Decode only if both sequence reads match and are even
            let (frame, _): (SharedStateFrame, _) =
                bincode::serde::decode_from_slice(&payload_copy, bincode::config::standard())
                    .ok()?;

            let state = reconstruct_game_state(frame, &mut self.cached_spawns)?;

            if let Some(start) = perf_start {
                tracing::info!(
                    target: "textquest::perf",
                    client_id = self.client_id,
                    nearby_spawns = state.nearby_spawns.len(),
                    cached_spawn_epoch = self.cached_spawns.as_ref().map(|(epoch, _)| *epoch),
                    elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                    "Shared memory frame consumed"
                );
            }

            Some(state)
        }

        #[cfg(not(windows))]
        {
            let _ = self.client_id;
            None
        }
    }
}

fn reconstruct_game_state(
    frame: SharedStateFrame,
    cached_spawns: &mut Option<(u64, Vec<SpawnData>)>,
) -> Option<GameState> {
    if let Some(spawns) = frame.nearby_spawns.as_ref() {
        *cached_spawns = Some((frame.spawn_epoch, spawns.clone()));
    }

    let spawns = cached_spawns.as_ref().map(|(_, spawns)| spawns.clone())?;
    Some(frame.into_game_state(spawns))
}

impl Drop for SharedStateReader {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Memory::UnmapViewOfFile;

            unsafe {
                let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self._ptr as *mut _,
                };
                let _ = UnmapViewOfFile(view);
                let _ = CloseHandle(self._handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spawn(id: u32) -> SpawnData {
        SpawnData {
            spawn_id: id,
            name: format!("spawn_{id}"),
            displayed_name: format!("Spawn {id}"),
            spawn_type: 1,
            level: 60,
            class_id: 1,
            x: id as f32,
            y: id as f32,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 50,
            mana_max: 50,
            endurance_current: 25,
            endurance_max: 25,
            speed_run: 0.0,
            stand_state: 0,
        }
    }

    fn make_frame(epoch: u64, nearby_spawns: Option<Vec<SpawnData>>) -> SharedStateFrame {
        SharedStateFrame {
            client_id: 42,
            local_player: Some(make_spawn(1)),
            target: Some(make_spawn(2)),
            nearby_spawns,
            timestamp_ms: 1234,
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
            zone_short_name: "qeynos".into(),
            zone_long_name: "South Qeynos".into(),
            spawn_epoch: epoch,
        }
    }

    #[test]
    fn incremental_frame_before_first_spawn_snapshot_returns_none() {
        let mut cached = None;

        let state = reconstruct_game_state(make_frame(0, None), &mut cached);

        assert!(state.is_none());
        assert!(cached.is_none());
    }

    #[test]
    fn incremental_frame_reuses_cached_spawns() {
        let mut cached = None;
        let full = reconstruct_game_state(
            make_frame(1, Some(vec![make_spawn(10), make_spawn(20)])),
            &mut cached,
        )
        .expect("full frame should decode");
        let incremental = reconstruct_game_state(make_frame(1, None), &mut cached)
            .expect("hot frame should decode");

        assert_eq!(full.nearby_spawns.len(), 2);
        assert_eq!(incremental.nearby_spawns, full.nearby_spawns);
    }

    #[test]
    fn new_spawn_epoch_replaces_cached_spawns() {
        let mut cached = None;
        let _ = reconstruct_game_state(
            make_frame(1, Some(vec![make_spawn(10), make_spawn(20)])),
            &mut cached,
        );
        let updated =
            reconstruct_game_state(make_frame(2, Some(vec![make_spawn(99)])), &mut cached)
                .expect("updated frame should decode");

        assert_eq!(updated.nearby_spawns.len(), 1);
        assert_eq!(updated.nearby_spawns[0].spawn_id, 99);
        assert_eq!(cached.as_ref().map(|(epoch, _)| *epoch), Some(2));
    }
}
