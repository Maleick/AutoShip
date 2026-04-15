//! Shared memory WRITER (DLL side).
//!
//! Creates a named shared memory region and publishes game state snapshots for
//! the orchestrator to read. On non-Windows platforms this is a compile-only
//! stub.

use anyhow::Result;
use std::sync::LazyLock;
use textquest_common::types::{ClientId, SharedStateFrame};

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
    #[cfg(windows)]
    encode_buffer: Vec<u8>,
}

// SAFETY: SharedStateWriter is only accessed from the game loop thread (single
// writer). The sequence number uses AtomicU64 with Release ordering as a write
// fence. The reader side uses Acquire ordering to observe the complete payload.
#[cfg(windows)]
unsafe impl Send for SharedStateWriter {}
#[cfg(windows)]
unsafe impl Sync for SharedStateWriter {}

impl SharedStateWriter {
    /// Create the named shared memory region for `client_id`.
    ///
    /// Memory name: `textquest_state_{client_id}`
    pub fn new(client_id: ClientId, session_id: u64) -> Result<Self> {
        #[cfg(windows)]
        {
            use textquest_common::ipc::SHARED_MEMORY_SIZE;
            use windows::{
                Win32::{
                    Foundation::INVALID_HANDLE_VALUE,
                    System::Memory::{
                        CreateFileMappingW, FILE_MAP_WRITE, MapViewOfFile, PAGE_READWRITE,
                    },
                },
                core::PCWSTR,
            };

            let name: Vec<u16> = format!(
                "{}\0",
                textquest_common::ipc::shared_memory_name(session_id, client_id)
            )
            .encode_utf16()
            .collect();

            // Restrict shared memory access to the current user via an explicit DACL.
            // Fail closed: if DACL creation fails, abort rather than using default (open)
            // security. sa_setup must be kept alive until after
            // CreateFileMappingW returns.
            let sa_setup = create_current_user_security_attributes().ok_or_else(|| {
                anyhow::anyhow!(
                    "DACL creation failed for client {client_id} — refusing to create shared \
                     memory with default security"
                )
            })?;
            let sa_ptr = Some(sa_setup.sa_ptr());

            // SAFETY: CreateFileMappingW with INVALID_HANDLE_VALUE creates a
            // page-file-backed mapping. sa_ptr points to a valid SECURITY_ATTRIBUTES
            // (or None for default security). name is a null-terminated UTF-16 string.
            // The resulting handle is either valid or an error is returned.
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    sa_ptr,
                    PAGE_READWRITE,
                    0,
                    SHARED_MEMORY_SIZE as u32,
                    PCWSTR(name.as_ptr()),
                )
            }?;
            drop(sa_setup); // buffers no longer needed after CreateFileMappingW

            // SAFETY: handle is a valid file mapping handle from CreateFileMappingW
            // above. MapViewOfFile maps it into our address space with write access.
            // The returned pointer is valid for SHARED_MEMORY_SIZE bytes until
            // UnmapViewOfFile is called (in Drop). Null check follows immediately.
            let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, SHARED_MEMORY_SIZE) };
            if ptr.Value.is_null() {
                anyhow::bail!("MapViewOfFile returned null for client {client_id}");
            }

            Ok(Self {
                client_id,
                _handle: handle,
                ptr: ptr.Value as *mut u8,
                size: SHARED_MEMORY_SIZE,
                sequence: 0,
                encode_buffer: Vec::with_capacity(8 * 1024),
            })
        }

        #[cfg(not(windows))]
        {
            let _ = (client_id, session_id);
            Ok(Self { client_id })
        }
    }

    /// Publish updated game state.
    ///
    /// Layout in shared memory:
    /// ```text
    /// [sequence: u64 LE][payload_len: u32 LE][payload: bincode bytes]
    /// ```
    pub fn write(&mut self, frame: &SharedStateFrame) -> Result<()> {
        #[cfg(windows)]
        {
            use std::{
                sync::atomic::{AtomicU64, Ordering},
                time::Instant,
            };

            let perf_start = if *PERF_TRACE_ENABLED {
                Some(Instant::now())
            } else {
                None
            };
            let payload_len = encode_frame_into_buffer(frame, &mut self.encode_buffer, self.size)?;

            let base = self.ptr;
            // SAFETY: base points to the start of a mapped shared memory region
            // of SHARED_MEMORY_SIZE bytes (validated non-null in new()). Casting
            // to AtomicU64 is valid because the mapping is at least 8-byte aligned
            // (OS guarantees page-aligned mappings) and we are the sole writer.
            // The reader uses Acquire ordering to observe complete payloads.
            let seq = unsafe { &*(base as *const AtomicU64) };

            // Mark write-in-progress: increment sequence to make it odd
            self.sequence += 1;
            seq.store(self.sequence, Ordering::Release);

            // SAFETY: base is a valid mapped pointer. The size check above ensures
            // payload.len() + 12 <= self.size, so base+8 (4 bytes) and base+12
            // (payload.len() bytes) are within the mapped region. The copies are
            // non-overlapping because source is stack/heap and dest is shared memory.
            let len_bytes = (payload_len as u32).to_le_bytes();
            unsafe {
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), base.add(8), 4);
                std::ptr::copy_nonoverlapping(
                    self.encode_buffer.as_ptr(),
                    base.add(12),
                    payload_len,
                );
            }

            // Mark write-complete: increment sequence to make it even
            self.sequence += 1;
            seq.store(self.sequence, Ordering::Release);

            if let Some(start) = perf_start {
                tracing::info!(
                    target: "textquest::perf",
                    client_id = self.client_id,
                    payload_len,
                    has_spawn_snapshot = frame.nearby_spawns.is_some(),
                    spawn_epoch = frame.spawn_epoch,
                    elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                    "Shared memory frame published"
                );
            }

            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = (self.client_id, frame);
            Ok(())
        }
    }
}

fn encode_frame_into_buffer(
    frame: &SharedStateFrame,
    buffer: &mut Vec<u8>,
    mapping_size: usize,
) -> Result<usize> {
    buffer.clear();
    let payload_len =
        bincode::serde::encode_into_std_write(frame, buffer, bincode::config::standard())
            .map_err(|e| anyhow::anyhow!("bincode encode failed: {e}"))?;
    if payload_len + 12 > mapping_size {
        anyhow::bail!(
            "SharedStateFrame too large for shared memory: {} bytes (max {})",
            payload_len,
            mapping_size - 12
        );
    }
    Ok(payload_len)
}

/// Owns the `SECURITY_ATTRIBUTES` and its backing buffers (absolute security
/// descriptor + ACL).  Both buffers must outlive any Windows API call that
/// reads the SA, because the kernel dereferences them synchronously before
/// returning.  Moving this struct is safe: Vec stores its data on the heap,
/// so the raw pointers inside `sa` remain valid across moves.
#[cfg(windows)]
struct SecuritySetup {
    _sd_buf: Vec<u8>,
    _acl_buf: Vec<u8>,
    sa: windows::Win32::Security::SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
impl SecuritySetup {
    fn sa_ptr(&self) -> *const windows::Win32::Security::SECURITY_ATTRIBUTES {
        &self.sa
    }
}

/// Build `SECURITY_ATTRIBUTES` with a DACL granting only the current user
/// `FILE_MAP_ALL_ACCESS` to the shared memory region.
///
/// Returns `None` on any API failure; the caller falls back to the default
/// DACL and logs a warning.  The returned `SecuritySetup` must be kept alive
/// for the duration of the `CreateFileMappingW` call.
#[cfg(windows)]
fn create_current_user_security_attributes() -> Option<SecuritySetup> {
    use std::mem;
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{
            ACE_REVISION, ACL, AddAccessAllowedAce, GetLengthSid, GetTokenInformation,
            InitializeAcl, InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            SECURITY_DESCRIPTOR, SetSecurityDescriptorDacl, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::{
            Memory::FILE_MAP_ALL_ACCESS,
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    unsafe {
        // 1. Open the current process token (read-only query — no write needed).
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;

        // 2. Two-pass GetTokenInformation to obtain the user SID. First call returns
        //    ERROR_INSUFFICIENT_BUFFER with the required size.
        let mut info_size = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut info_size);
        let mut user_buf = vec![0u8; info_size as usize];
        let result = GetTokenInformation(
            token,
            TokenUser,
            Some(user_buf.as_mut_ptr() as *mut _),
            info_size,
            &mut info_size,
        );
        let _ = CloseHandle(token);
        result.ok()?;

        let token_user = &*(user_buf.as_ptr() as *const TOKEN_USER);
        let sid = token_user.User.Sid; // type inferred from TOKEN_USER.User.Sid

        // 3. Build an ACL with one ACCESS_ALLOWED_ACE for the current user. Layout: ACL
        //    header (8 B) + ACE_HEADER+ACCESS_MASK (8 B) + SID bytes.
        let sid_len = GetLengthSid(sid) as usize;
        let ace_size = 8usize + sid_len; // sizeof(ACE_HEADER) + sizeof(ACCESS_MASK) + SID
        let acl_size = mem::size_of::<ACL>() + ace_size;
        let mut acl_buf = vec![0u8; acl_size];
        InitializeAcl(
            acl_buf.as_mut_ptr() as *mut ACL,
            acl_size as u32,
            ACE_REVISION(2), // ACL_REVISION = 2
        )
        .ok()?;
        AddAccessAllowedAce(
            acl_buf.as_mut_ptr() as *mut ACL,
            ACE_REVISION(2), // ACL_REVISION = 2
            FILE_MAP_ALL_ACCESS.0,
            sid,
        )
        .ok()?;

        // 4. Build an absolute SECURITY_DESCRIPTOR pointing to the ACL.
        let mut sd_buf = vec![0u8; mem::size_of::<SECURITY_DESCRIPTOR>()];
        let sd_ptr = PSECURITY_DESCRIPTOR(sd_buf.as_mut_ptr() as *mut _);
        InitializeSecurityDescriptor(
            sd_ptr, 1, // SECURITY_DESCRIPTOR_REVISION
        )
        .ok()?;
        SetSecurityDescriptorDacl(
            sd_ptr,
            true, // bDaclPresent — our explicit DACL applies
            Some(acl_buf.as_mut_ptr() as *mut ACL),
            false, // bDaclDefaulted — DACL was set explicitly, not inherited
        )
        .ok()?;

        // 5. Assemble SECURITY_ATTRIBUTES. lpSecurityDescriptor points into sd_buf's
        //    heap allocation, which is stable as long as SecuritySetup (and therefore
        //    sd_buf) is alive.
        let sa = SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd_buf.as_mut_ptr() as *mut _,
            bInheritHandle: false.into(),
        };

        Some(SecuritySetup {
            _sd_buf: sd_buf,
            _acl_buf: acl_buf,
            sa,
        })
    }
}

impl Drop for SharedStateWriter {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::{Foundation::CloseHandle, System::Memory::UnmapViewOfFile};

            // SAFETY: self.ptr is a valid mapped view from MapViewOfFile (validated
            // non-null in new()). self._handle is a valid file mapping handle from
            // CreateFileMappingW. Both are cleaned up exactly once in Drop. After
            // this, the shared memory region is no longer accessible.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spawn(id: u32) -> textquest_common::types::SpawnData {
        textquest_common::types::SpawnData {
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
            is_gm: false,
        }
    }

    fn make_frame(with_spawns: bool) -> SharedStateFrame {
        SharedStateFrame {
            client_id: 42,
            local_player: Some(make_spawn(1)),
            target: Some(make_spawn(2)),
            nearby_spawns: with_spawns.then(|| (0..32).map(make_spawn).collect()),
            timestamp_ms: 1234,
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
            zone_short_name: "qeynos".into(),
            zone_long_name: "South Qeynos".into(),
            spawn_epoch: 7,
            actual_version: None,
        }
    }

    #[test]
    fn encode_frame_respects_shared_memory_limit() {
        let frame = make_frame(true);
        let mut buffer = Vec::new();

        let payload_len = encode_frame_into_buffer(
            &frame,
            &mut buffer,
            textquest_common::ipc::SHARED_MEMORY_SIZE,
        )
        .expect("frame should encode");

        assert_eq!(payload_len, buffer.len());
        assert!(payload_len + 12 <= textquest_common::ipc::SHARED_MEMORY_SIZE);
    }

    #[test]
    fn encode_frame_reuses_buffer_capacity() {
        let mut buffer = Vec::with_capacity(64);
        let initial_capacity = buffer.capacity();

        let full_len = encode_frame_into_buffer(&make_frame(true), &mut buffer, 64 * 1024)
            .expect("full frame encodes");
        let grown_capacity = buffer.capacity();
        let hot_len = encode_frame_into_buffer(&make_frame(false), &mut buffer, 64 * 1024)
            .expect("hot frame encodes");

        assert!(grown_capacity >= initial_capacity);
        assert_eq!(buffer.len(), hot_len);
        assert!(full_len > hot_len);
    }

    #[test]
    fn encode_frame_fails_when_mapping_too_small() {
        let frame = make_frame(true);
        let mut buffer = Vec::new();
        // Provide a mapping_size smaller than any encoded frame could fit
        let result = encode_frame_into_buffer(&frame, &mut buffer, 16);
        assert!(
            result.is_err(),
            "should fail when mapping_size is too small"
        );
    }

    #[test]
    fn encode_frame_no_spawns_is_smaller() {
        let with_spawns = make_frame(true);
        let without_spawns = make_frame(false);

        let mut buf1 = Vec::new();
        let mut buf2 = Vec::new();

        let len_with = encode_frame_into_buffer(
            &with_spawns,
            &mut buf1,
            textquest_common::ipc::SHARED_MEMORY_SIZE,
        )
        .expect("with spawns encodes");
        let len_without = encode_frame_into_buffer(
            &without_spawns,
            &mut buf2,
            textquest_common::ipc::SHARED_MEMORY_SIZE,
        )
        .expect("without spawns encodes");

        assert!(
            len_with > len_without,
            "frame with spawns should encode to more bytes"
        );
    }

    #[test]
    fn encode_frame_clears_buffer_between_calls() {
        let frame = make_frame(false);
        let mut buffer = Vec::new();

        encode_frame_into_buffer(
            &frame,
            &mut buffer,
            textquest_common::ipc::SHARED_MEMORY_SIZE,
        )
        .expect("first encode");
        let first_len = buffer.len();

        encode_frame_into_buffer(
            &frame,
            &mut buffer,
            textquest_common::ipc::SHARED_MEMORY_SIZE,
        )
        .expect("second encode");
        let second_len = buffer.len();

        assert_eq!(
            first_len, second_len,
            "repeated encodes should produce same length"
        );
    }
}
