//! Shared memory READER (orchestrator side).
//!
//! Reads game state published by the injected DLL via a named shared memory
//! region. On Windows this uses CreateFileMappingW / MapViewOfFile; on other
//! platforms it returns an empty stub so the project compiles.

use dmft_common::types::{ClientId, GameState};
use anyhow::Result;

/// Reads game state from shared memory for a specific client.
pub struct SharedStateReader {
    client_id: ClientId,
    #[cfg(windows)]
    _handle: windows::Win32::Foundation::HANDLE,
    #[cfg(windows)]
    _ptr: *mut u8,
    #[cfg(windows)]
    _size: usize,
}

// SAFETY: SharedStateReader is only accessed from the orchestrator's poll thread (single reader).
// The sequence number uses AtomicU64 with Acquire ordering as a read fence.
// The writer side uses Release ordering to ensure the complete payload is visible.
#[cfg(windows)]
unsafe impl Send for SharedStateReader {}
#[cfg(windows)]
unsafe impl Sync for SharedStateReader {}

impl SharedStateReader {
    /// Open (or create) the named shared memory region for `client_id`.
    ///
    /// Memory name: `dmft_state_{client_id}`
    pub fn new(client_id: ClientId) -> Result<Self> {
        #[cfg(windows)]
        {
            use dmft_common::ipc::SHARED_MEMORY_SIZE;
            use windows::core::PCWSTR;
            use windows::Win32::System::Memory::{
                OpenFileMappingW, CreateFileMappingW, MapViewOfFile,
                FILE_MAP_READ, PAGE_READWRITE,
            };
            use windows::Win32::Foundation::INVALID_HANDLE_VALUE;

            let name: Vec<u16> = format!("dmft_state_{}\0", client_id)
                .encode_utf16()
                .collect();

            // Try to open existing shared memory with read-only access first (C2 audit fix).
            // Fall back to CreateFileMappingW if the DLL hasn't created it yet.
            let handle = unsafe {
                OpenFileMappingW(
                    FILE_MAP_READ.0,
                    false,
                    PCWSTR(name.as_ptr()),
                )
            }.unwrap_or_else(|_| {
                // DLL hasn't created the shared memory yet — create it ourselves.
                // This is fine; the DLL will open the existing one when it starts.
                unsafe {
                    CreateFileMappingW(
                        INVALID_HANDLE_VALUE,
                        None,
                        PAGE_READWRITE,
                        0,
                        SHARED_MEMORY_SIZE as u32,
                        PCWSTR(name.as_ptr()),
                    )
                }.expect("Failed to create shared memory")
            });

            let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, SHARED_MEMORY_SIZE) };
            if ptr.Value.is_null() {
                anyhow::bail!("MapViewOfFile returned null for client {}", client_id);
            }

            Ok(Self {
                client_id,
                _handle: handle,
                _ptr: ptr.Value as *mut u8,
                _size: SHARED_MEMORY_SIZE,
            })
        }

        #[cfg(not(windows))]
        {
            Ok(Self { client_id })
        }
    }

    /// Read the latest game state. Returns `None` if no data is available yet.
    ///
    /// The shared memory layout is:
    /// ```text
    /// [sequence: u64 LE][payload_len: u32 LE][payload: bincode bytes]
    /// ```
    /// A zero sequence number means the DLL hasn't written yet.
    pub fn read(&self) -> Option<GameState> {
        #[cfg(windows)]
        {
            use std::sync::atomic::{AtomicU64, Ordering};

            let base = self._ptr;
            let seq = unsafe { &*(base as *const AtomicU64) };

            // 1. Load sequence number with Acquire ordering
            let seq_before = seq.load(Ordering::Acquire);

            // 2. Sequence 0 means no data yet; odd means write in progress — retry
            if seq_before == 0 || seq_before % 2 != 0 {
                return None;
            }

            // 3. Read payload length
            let len_bytes: [u8; 4] = unsafe {
                std::ptr::read(base.add(8) as *const [u8; 4])
            };
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
            let (state, _): (GameState, _) = bincode::serde::decode_from_slice(
                &payload_copy,
                bincode::config::standard(),
            )
            .ok()?;

            Some(state)
        }

        #[cfg(not(windows))]
        {
            let _ = self.client_id;
            None
        }
    }
}

impl Drop for SharedStateReader {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::System::Memory::UnmapViewOfFile;
            use windows::Win32::Foundation::CloseHandle;

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
