//! Shared memory WRITER (DLL side).
//!
//! Creates a named shared memory region and publishes game state snapshots for
//! the orchestrator to read. On non-Windows platforms this is a compile-only stub.

use dmft_common::types::{ClientId, GameState};
use anyhow::Result;

/// Writes game state to shared memory for the orchestrator to read.
pub struct SharedStateWriter {
    client_id: ClientId,
    #[cfg(windows)]
    _handle: windows::Win32::Foundation::HANDLE,
    #[cfg(windows)]
    ptr: *mut u8,
    #[cfg(windows)]
    size: usize,
    #[cfg(windows)]
    sequence: u64,
}

// SAFETY: SharedStateWriter is only accessed from the game loop thread (single writer).
// The sequence number uses AtomicU64 with Release ordering as a write fence.
// The reader side uses Acquire ordering to observe the complete payload.
#[cfg(windows)]
unsafe impl Send for SharedStateWriter {}
#[cfg(windows)]
unsafe impl Sync for SharedStateWriter {}

impl SharedStateWriter {
    /// Create the named shared memory region for `client_id`.
    ///
    /// Memory name: `dmft_state_{client_id}`
    pub fn new(client_id: ClientId) -> Result<Self> {
        #[cfg(windows)]
        {
            use dmft_common::ipc::SHARED_MEMORY_SIZE;
            use windows::core::PCWSTR;
            use windows::Win32::System::Memory::{
                CreateFileMappingW, MapViewOfFile, FILE_MAP_WRITE, PAGE_READWRITE,
            };
            use windows::Win32::Foundation::INVALID_HANDLE_VALUE;

            let name: Vec<u16> = format!("dmft_state_{}\0", client_id)
                .encode_utf16()
                .collect();

            // Create shared memory with restrictive security attributes.
            // Only the current user can access it (prevents other processes from
            // reading game state or injecting corrupt data).
            let sa = create_current_user_security_attributes();
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    sa.as_ref().map(|s| s as *const _ as *const _),
                    PAGE_READWRITE,
                    0,
                    SHARED_MEMORY_SIZE as u32,
                    PCWSTR(name.as_ptr()),
                )
            }?;

            let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, SHARED_MEMORY_SIZE) };
            if ptr.Value.is_null() {
                anyhow::bail!("MapViewOfFile returned null for client {}", client_id);
            }

            Ok(Self {
                client_id,
                _handle: handle,
                ptr: ptr.Value as *mut u8,
                size: SHARED_MEMORY_SIZE,
                sequence: 0,
            })
        }

        #[cfg(not(windows))]
        {
            let _ = client_id;
            Ok(Self { client_id })
        }
    }

    /// Publish updated game state.
    ///
    /// Layout in shared memory:
    /// ```text
    /// [sequence: u64 LE][payload_len: u32 LE][payload: bincode bytes]
    /// ```
    pub fn write(&mut self, state: &GameState) -> Result<()> {
        #[cfg(windows)]
        {
            use std::sync::atomic::{AtomicU64, Ordering};

            let payload = bincode::serde::encode_to_vec(state, bincode::config::standard())
                .map_err(|e| anyhow::anyhow!("bincode encode failed: {}", e))?;

            if payload.len() + 12 > self.size {
                anyhow::bail!(
                    "GameState too large for shared memory: {} bytes (max {})",
                    payload.len(),
                    self.size - 12
                );
            }

            let base = self.ptr;
            let seq = unsafe { &*(base as *const AtomicU64) };

            // Mark write-in-progress: increment sequence to make it odd
            self.sequence += 1;
            seq.store(self.sequence, Ordering::Release);

            // Write payload
            let len_bytes = (payload.len() as u32).to_le_bytes();
            unsafe {
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), base.add(8), 4);
                std::ptr::copy_nonoverlapping(payload.as_ptr(), base.add(12), payload.len());
            }

            // Mark write-complete: increment sequence to make it even
            self.sequence += 1;
            seq.store(self.sequence, Ordering::Release);

            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = (self.client_id, state);
            Ok(())
        }
    }
}

/// Create SECURITY_ATTRIBUTES with a DACL that only allows the current user.
/// Returns None if security setup fails (falls back to default DACL).
///
/// TODO(security-C1): Implement proper DACL using PSECURITY_DESCRIPTOR wrapper.
/// The windows 0.54 crate requires PSECURITY_DESCRIPTOR type instead of raw pointers.
/// For now, returns None (default DACL) to avoid Windows build breaks.
/// This is tracked as a known security gap in the audit report.
#[cfg(windows)]
fn create_current_user_security_attributes() -> Option<windows::Win32::Security::SECURITY_ATTRIBUTES> {
    // Placeholder — returns None until PSECURITY_DESCRIPTOR wrapping is implemented.
    // The CreateFileMappingW call uses `sa.as_ref().map(...)` which falls back to
    // None (default DACL) when this returns None.
    None
}

impl Drop for SharedStateWriter {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::System::Memory::UnmapViewOfFile;
            use windows::Win32::Foundation::CloseHandle;

            unsafe {
                let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut _,
                };
                let _ = UnmapViewOfFile(view);
                let _ = CloseHandle(self._handle);
            }
        }
    }
}
