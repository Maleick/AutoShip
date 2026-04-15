//! Named pipe SERVER (DLL side).
//!
//! Creates a named pipe and listens for commands from the orchestrator.
//! On non-Windows platforms this is a compile-only stub.

use anyhow::Result;
#[cfg(windows)]
use textquest_common::protocol;
use textquest_common::{
    ipc::{Command, IpcCommand, IpcResponse, SessionToken},
    types::ClientId,
};

/// Listens for commands from the orchestrator via named pipe.
///
/// Supports persistent connections: the orchestrator authenticates once per
/// connection, then sends multiple commands without reconnecting. If the
/// connection drops, the listener waits for a new one.
pub struct CommandListener {
    client_id: ClientId,
    /// Session token set at injection time. The orchestrator must present this
    /// token as the first message after connecting before any commands are
    /// accepted.
    expected_token: SessionToken,
    /// Whether the current connection has been authenticated.
    connected: bool,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

impl CommandListener {
    /// Create a named pipe server for `client_id`.
    ///
    /// `token` is the session token generated at injection time. The
    /// orchestrator must send this token as the first 32 bytes after
    /// connecting; connections that fail the handshake are dropped.
    ///
    /// Pipe name: `\\.\pipe\textquest_cmd_{client_id}`
    pub fn new(client_id: ClientId, token: SessionToken) -> Result<Self> {
        #[cfg(windows)]
        {
            use windows::{
                Win32::{
                    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
                    System::Pipes::{
                        CreateNamedPipeA, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
                    },
                },
                core::PCSTR,
            };
            const PIPE_ACCESS_DUPLEX: FILE_FLAGS_AND_ATTRIBUTES =
                FILE_FLAGS_AND_ATTRIBUTES(0x0000_0003);

            let session_id = textquest_common::ipc::session_id_from_token(&token);
            let pipe_name = format!(
                "{}\0",
                textquest_common::ipc::pipe_name(session_id, client_id)
            );

            let security_setup = build_restrictive_security_attributes().map_err(|e| {
                anyhow::anyhow!(
                    "Pipe DACL creation failed — refusing to create pipe with default security: \
                     {e}"
                )
            })?;

            // SAFETY: CreateNamedPipeA creates a named pipe server with the
            // given name and security attributes. pipe_name is null-terminated.
            // security_setup.sa is a valid SECURITY_ATTRIBUTES with a DACL
            // restricting access to the current user. The handle is stored in
            // self and closed in Drop.
            let handle = unsafe {
                CreateNamedPipeA(
                    PCSTR(pipe_name.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,    // max instances
                    4096, // out buffer
                    4096, // in buffer
                    0,    // default timeout
                    Some(&security_setup.sa as *const _),
                )
            }?;
            drop(security_setup);

            Ok(Self {
                client_id,
                expected_token: token,
                connected: false,
                handle,
            })
        }

        #[cfg(not(windows))]
        {
            let _ = client_id;
            Ok(Self {
                client_id,
                expected_token: token,
                connected: false,
            })
        }
    }

    /// Block until a command is received, then return it.
    ///
    /// On the first call (or after a disconnection), waits for the orchestrator
    /// to connect and authenticate with a 32-byte session token. Subsequent
    /// calls read commands from the same connection without re-authenticating.
    ///
    /// If the read fails (orchestrator disconnected), the pipe is reset and the
    /// next call will wait for a new connection.
    ///
    /// The returned `IpcCommand` contains both the command payload and the
    /// optional correlation ID sent by the orchestrator. Callers should echo
    /// the correlation ID back in the corresponding `IpcResponse`.
    pub fn receive(&mut self) -> Result<IpcCommand> {
        #[cfg(windows)]
        {
            use windows::Win32::{
                Foundation::ERROR_PIPE_CONNECTED,
                Storage::FileSystem::ReadFile,
                System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe},
            };

            // If not connected, wait for a new connection + authenticate.
            if !self.connected {
                // SAFETY: self.handle is a valid named pipe handle from
                // CreateNamedPipeA. ConnectNamedPipe blocks until a client connects.
                // ERROR_PIPE_CONNECTED means a client connected before we called this.
                let connect_result = unsafe { ConnectNamedPipe(self.handle, None) };
                if let Err(ref e) = connect_result {
                    if e.code() != ERROR_PIPE_CONNECTED.into() {
                        return Err(connect_result.unwrap_err().into());
                    }
                }

                // Read 32-byte session token (once per connection).
                let mut token_buf = [0u8; 32];
                let mut token_bytes_read: u32 = 0;
                // SAFETY: self.handle is a connected named pipe. token_buf is
                // a stack-allocated 32-byte buffer. ReadFile writes at most 32
                // bytes. token_bytes_read receives the actual count.
                let token_result = unsafe {
                    ReadFile(
                        self.handle,
                        Some(&mut token_buf),
                        Some(&mut token_bytes_read),
                        None,
                    )
                };

                if token_result.is_err()
                    || token_bytes_read != 32
                    || !constant_time_eq(&token_buf, &self.expected_token)
                {
                    tracing::error!(
                        client_id = self.client_id,
                        "Session token validation failed — dropping connection"
                    );
                    // SAFETY: self.handle is a valid pipe handle. DisconnectNamedPipe
                    // drops the client connection so the pipe can accept a new one.
                    unsafe {
                        let _ = DisconnectNamedPipe(self.handle);
                    }
                    anyhow::bail!("Session token mismatch for client {}", self.client_id);
                }

                tracing::debug!(client_id = self.client_id, "Pipe session authenticated");
                self.connected = true;
            }

            // Read the next command from the connected pipe.
            let mut buf = vec![
                0u8;
                textquest_common::protocol::MAX_MESSAGE_SIZE as usize
                    + textquest_common::protocol::FRAME_HEADER_SIZE
            ];
            let mut bytes_read: u32 = 0;
            // SAFETY: self.handle is a connected pipe. buf is a heap-allocated
            // 4096-byte buffer. ReadFile writes at most buf.len() bytes.
            let read_result =
                unsafe { ReadFile(self.handle, Some(&mut buf), Some(&mut bytes_read), None) };

            if let Err(e) = read_result {
                // Orchestrator disconnected — reset for next connection.
                tracing::debug!(
                    client_id = self.client_id,
                    error = %e,
                    "Pipe read failed — orchestrator likely disconnected"
                );
                self.disconnect();
                return Err(e.into());
            }

            let (ipc_cmd, _) = protocol::decode_frame::<IpcCommand>(&buf[..bytes_read as usize])
                .map_err(|error| {
                    anyhow::anyhow!(
                        "Protocol error decoding command for client {}: {error}",
                        self.client_id
                    )
                })?
                .ok_or_else(|| {
                    anyhow::anyhow!("Incomplete command frame for client {}", self.client_id)
                })?;

            if !validate_command(&ipc_cmd.command) {
                anyhow::bail!("Command validation failed for client {}", self.client_id);
            }

            Ok(ipc_cmd)
        }

        #[cfg(not(windows))]
        {
            let _ = self.client_id;
            anyhow::bail!("Not implemented (non-Windows stub)")
        }
    }

    /// Disconnect the current client and prepare for a new connection.
    pub fn disconnect(&mut self) {
        self.connected = false;
        #[cfg(windows)]
        {
            use windows::Win32::System::Pipes::DisconnectNamedPipe;
            // SAFETY: self.handle is a valid pipe handle. DisconnectNamedPipe
            // drops the current client connection.
            unsafe {
                let _ = DisconnectNamedPipe(self.handle);
            }
        }
    }

    /// Send a response back to the orchestrator.
    pub fn respond(&self, response: &IpcResponse) -> Result<()> {
        #[cfg(windows)]
        {
            use windows::Win32::Storage::FileSystem::WriteFile;

            let data = protocol::encode(response)
                .map_err(|e| anyhow::anyhow!("failed to encode response: {e}"))?;
            let mut written: u32 = 0;
            // SAFETY: self.handle is a connected pipe. data is a valid byte
            // slice from bincode encoding. WriteFile writes at most data.len() bytes.
            unsafe {
                WriteFile(self.handle, Some(&data), Some(&mut written), None)?;
            }
            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = (self.client_id, response);
            Ok(())
        }
    }
}

/// Validate that command parameters are within acceptable bounds.
pub fn validate_command(cmd: &Command) -> bool {
    match cmd {
        Command::CastSpell { spell_slot, .. } => {
            // Slot must be 1-13. `recast` is already bounded by u8; 0 means
            // single-cast, non-zero enables the recast loop.
            *spell_slot >= 1
                && *spell_slot <= 13
                && !matches!(
                    cmd,
                    Command::CastSpell {
                        kill: true,
                        recast: 1..,
                        ..
                    }
                )
        }
        Command::MoveTo { x, y, z } => x.is_finite() && y.is_finite() && z.is_finite(),
        Command::NavigateTo { waypoints } => waypoints.len() <= 1000,
        Command::NavLoc { x, y, z } => x.is_finite() && y.is_finite() && z.is_finite(),
        Command::NavWaypointSave { name }
        | Command::NavWaypointRecall { name }
        | Command::NavWaypointDelete { name } => !name.is_empty() && name.len() <= 64,
        Command::QueryContainerSlots { filter } => {
            filter
                .location
                .as_ref()
                .is_none_or(|value| value.len() <= 32)
                && filter.top_slot.is_none_or(|slot| slot >= -1)
                && filter.bag_slot.is_none_or(|slot| slot >= -1)
                && filter
                    .item_name_contains
                    .as_ref()
                    .is_none_or(|value| !value.is_empty() && value.len() <= 128)
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            account_name.len() <= 128
                && password.len() <= 128
                && server_name.len() <= 64
                && character_name.len() <= 64
        }
        _ => true,
    }
}

/// Constant-time comparison to prevent timing side-channels on token
/// validation.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Owns the `SECURITY_ATTRIBUTES` and its backing buffers (absolute security
/// descriptor + ACL).  Both buffers must outlive any Windows API call that
/// reads the SA, because the kernel dereferences them synchronously.
#[cfg(windows)]
struct PipeSecuritySetup {
    _sd_buf: Vec<u8>,
    _acl_buf: Vec<u8>,
    sa: windows::Win32::Security::SECURITY_ATTRIBUTES,
}

/// Build `SECURITY_ATTRIBUTES` with a DACL granting only the current user
/// full access to the named pipe.
///
/// Returns `Err` on any API failure — the caller must abort pipe creation
/// rather than falling back to a default (open) security descriptor.
#[cfg(windows)]
fn build_restrictive_security_attributes() -> Result<PipeSecuritySetup> {
    use std::mem;
    use windows::Win32::{
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Security::{
            ACE_REVISION, ACL, AddAccessAllowedAce, GetLengthSid, GetTokenInformation,
            InitializeAcl, InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            SECURITY_DESCRIPTOR, SetSecurityDescriptorDacl, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    unsafe {
        // 1. Open the current process token.
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| anyhow::anyhow!("OpenProcessToken failed: {e}"))?;

        // 2. Two-pass GetTokenInformation to obtain the user SID.
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
        result.map_err(|e| anyhow::anyhow!("GetTokenInformation failed: {e}"))?;

        let token_user = &*(user_buf.as_ptr() as *const TOKEN_USER);
        let sid = token_user.User.Sid;

        // 3. Build an ACL with one ACCESS_ALLOWED_ACE for the current user.
        let sid_len = GetLengthSid(sid) as usize;
        let ace_size = 8usize + sid_len;
        let acl_size = mem::size_of::<ACL>() + ace_size;
        let mut acl_buf = vec![0u8; acl_size];
        InitializeAcl(
            acl_buf.as_mut_ptr() as *mut ACL,
            acl_size as u32,
            ACE_REVISION(2),
        )
        .map_err(|e| anyhow::anyhow!("InitializeAcl failed: {e}"))?;
        AddAccessAllowedAce(
            acl_buf.as_mut_ptr() as *mut ACL,
            ACE_REVISION(2),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            sid,
        )
        .map_err(|e| anyhow::anyhow!("AddAccessAllowedAce failed: {e}"))?;

        // 4. Build an absolute SECURITY_DESCRIPTOR pointing to the ACL.
        let mut sd_buf = vec![0u8; mem::size_of::<SECURITY_DESCRIPTOR>()];
        let sd_ptr = PSECURITY_DESCRIPTOR(sd_buf.as_mut_ptr() as *mut _);
        InitializeSecurityDescriptor(
            sd_ptr, 1, // SECURITY_DESCRIPTOR_REVISION
        )
        .map_err(|e| anyhow::anyhow!("InitializeSecurityDescriptor failed: {e}"))?;
        SetSecurityDescriptorDacl(sd_ptr, true, Some(acl_buf.as_mut_ptr() as *mut ACL), false)
            .map_err(|e| anyhow::anyhow!("SetSecurityDescriptorDacl failed: {e}"))?;

        // 5. Assemble SECURITY_ATTRIBUTES.
        let sa = SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd_buf.as_mut_ptr() as *mut _,
            bInheritHandle: false.into(),
        };

        Ok(PipeSecuritySetup {
            _sd_buf: sd_buf,
            _acl_buf: acl_buf,
            sa,
        })
    }
}

#[cfg(not(windows))]
fn build_restrictive_security_attributes() -> Result<()> {
    Ok(())
}

impl Drop for CommandListener {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::{Foundation::CloseHandle, System::Pipes::DisconnectNamedPipe};
            // SAFETY: self.handle is a valid pipe handle created in new().
            // DisconnectNamedPipe + CloseHandle are called exactly once in Drop.
            // After this, the handle is invalid and must not be used.
            unsafe {
                let _ = DisconnectNamedPipe(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_command;
    use textquest_common::ipc::Command;

    #[test]
    fn validate_cast_spell_valid_slot() {
        assert!(validate_command(&Command::CastSpell {
            spell_slot: 1,
            target_id: None,
            kill: false,
            recast: 0,
        }));
        assert!(validate_command(&Command::CastSpell {
            spell_slot: 13,
            target_id: Some(42),
            kill: false,
            recast: 0,
        }));
    }

    #[test]
    fn validate_cast_spell_invalid_slot_zero() {
        // Slot 0 is invalid (1-based indexing).
        assert!(!validate_command(&Command::CastSpell {
            spell_slot: 0,
            target_id: None,
            kill: false,
            recast: 0,
        }));
    }

    #[test]
    fn validate_cast_spell_invalid_slot_too_high() {
        assert!(!validate_command(&Command::CastSpell {
            spell_slot: 14,
            target_id: None,
            kill: false,
            recast: 0,
        }));
    }

    #[test]
    fn validate_cast_spell_kill_flag_valid() {
        assert!(validate_command(&Command::CastSpell {
            spell_slot: 5,
            target_id: Some(99),
            kill: true,
            recast: 0,
        }));
    }

    #[test]
    fn validate_cast_spell_recast_flag_valid() {
        assert!(validate_command(&Command::CastSpell {
            spell_slot: 5,
            target_id: None,
            kill: false,
            recast: 10,
        }));
    }

    #[test]
    fn validate_cast_spell_kill_and_recast_rejected() {
        // Combining kill=true and recast>0 is invalid (ambiguous intent).
        assert!(!validate_command(&Command::CastSpell {
            spell_slot: 5,
            target_id: None,
            kill: true,
            recast: 3,
        }));
    }

    #[test]
    fn validate_cancel_cast_loop_always_valid() {
        assert!(validate_command(&Command::CancelCastLoop));
    }
}
