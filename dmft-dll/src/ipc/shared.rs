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

            // Restrict shared memory access to the current user via an explicit DACL.
            // Falls back to the default DACL (with a warning) if DACL setup fails.
            // sa_setup must be kept alive until after CreateFileMappingW returns.
            let sa_setup = create_current_user_security_attributes();
            if sa_setup.is_none() {
                tracing::warn!(
                    client_id,
                    "DACL creation failed — shared memory will use the default DACL"
                );
            }
            let sa_ptr = sa_setup.as_ref().map(SecuritySetup::sa_ptr);

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

/// Owns the SECURITY_ATTRIBUTES and its backing buffers (absolute security
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

/// Build SECURITY_ATTRIBUTES with a DACL granting only the current user
/// `FILE_MAP_ALL_ACCESS` to the shared memory region.
///
/// Returns `None` on any API failure; the caller falls back to the default
/// DACL and logs a warning.  The returned `SecuritySetup` must be kept alive
/// for the duration of the `CreateFileMappingW` call.
#[cfg(windows)]
fn create_current_user_security_attributes() -> Option<SecuritySetup> {
    use std::mem;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        ACE_REVISION, ACL, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
        TOKEN_QUERY, TOKEN_USER,
        AddAccessAllowedAce, GetLengthSid, GetTokenInformation, InitializeAcl,
        InitializeSecurityDescriptor, SetSecurityDescriptorDacl, TokenUser,
    };
    use windows::Win32::System::Memory::FILE_MAP_ALL_ACCESS;
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        // 1. Open the current process token (read-only query — no write needed).
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;

        // 2. Two-pass GetTokenInformation to obtain the user SID.
        //    First call returns ERROR_INSUFFICIENT_BUFFER with the required size.
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

        // 3. Build an ACL with one ACCESS_ALLOWED_ACE for the current user.
        //    Layout: ACL header (8 B) + ACE_HEADER+ACCESS_MASK (8 B) + SID bytes.
        let sid_len = GetLengthSid(sid) as usize;
        let ace_size = 8usize + sid_len; // sizeof(ACE_HEADER) + sizeof(ACCESS_MASK) + SID
        let acl_size = mem::size_of::<ACL>() + ace_size;
        let mut acl_buf = vec![0u8; acl_size];
        InitializeAcl(
            acl_buf.as_mut_ptr() as *mut ACL,
            acl_size as u32,
            ACE_REVISION(2), // ACL_REVISION = 2
        ).ok()?;
        AddAccessAllowedAce(
            acl_buf.as_mut_ptr() as *mut ACL,
            ACE_REVISION(2), // ACL_REVISION = 2
            FILE_MAP_ALL_ACCESS.0,
            sid,
        ).ok()?;

        // 4. Build an absolute SECURITY_DESCRIPTOR pointing to the ACL.
        let mut sd_buf = vec![0u8; mem::size_of::<SECURITY_DESCRIPTOR>()];
        let sd_ptr = PSECURITY_DESCRIPTOR(sd_buf.as_mut_ptr() as *mut _);
        InitializeSecurityDescriptor(
            sd_ptr,
            1, // SECURITY_DESCRIPTOR_REVISION
        ).ok()?;
        SetSecurityDescriptorDacl(
            sd_ptr,
            true,  // bDaclPresent — our explicit DACL applies
            Some(acl_buf.as_mut_ptr() as *mut ACL),
            false, // bDaclDefaulted — DACL was set explicitly, not inherited
        ).ok()?;

        // 5. Assemble SECURITY_ATTRIBUTES.
        //    lpSecurityDescriptor points into sd_buf's heap allocation, which is
        //    stable as long as SecuritySetup (and therefore sd_buf) is alive.
        let sa = SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd_buf.as_mut_ptr() as *mut _,
            bInheritHandle: false.into(),
        };

        Some(SecuritySetup { _sd_buf: sd_buf, _acl_buf: acl_buf, sa })
    }
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
