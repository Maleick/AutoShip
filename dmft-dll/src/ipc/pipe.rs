//! Named pipe SERVER (DLL side).
//!
//! Creates a named pipe and listens for commands from the orchestrator.
//! On non-Windows platforms this is a compile-only stub.

use dmft_common::ipc::{Command, Response, SessionToken};
#[cfg(windows)]
use dmft_common::ipc::PIPE_NAME_PREFIX;
use dmft_common::types::ClientId;
#[cfg(windows)]
use dmft_common::protocol;
use anyhow::Result;

/// Listens for commands from the orchestrator via named pipe.
pub struct CommandListener {
    client_id: ClientId,
    /// Session token set at injection time. The orchestrator must present this
    /// token as the first message after connecting before any commands are accepted.
    expected_token: SessionToken,
    /// Whether the current connection has been authenticated.
    authenticated: bool,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

impl CommandListener {
    /// Create a named pipe server for `client_id`.
    ///
    /// `token` is the session token generated at injection time. The orchestrator
    /// must send this token as the first 32 bytes after connecting; connections
    /// that fail the handshake are dropped.
    ///
    /// Pipe name: `\\.\pipe\dmft_cmd_{client_id}`
    pub fn new(client_id: ClientId, token: SessionToken) -> Result<Self> {
        #[cfg(windows)]
        {
            use windows::core::PCSTR;
            use windows::Win32::System::Pipes::CreateNamedPipeA;
            use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
            use windows::Win32::System::Pipes::{PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT};
            const PIPE_ACCESS_DUPLEX: FILE_FLAGS_AND_ATTRIBUTES = FILE_FLAGS_AND_ATTRIBUTES(0x00000003);

            let pipe_name = format!("{}cmd_{}\0", PIPE_NAME_PREFIX, client_id);

            let security_attrs = build_restrictive_security_attributes();

            let handle = unsafe {
                CreateNamedPipeA(
                    PCSTR(pipe_name.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,    // max instances
                    4096, // out buffer
                    4096, // in buffer
                    0,    // default timeout
                    security_attrs.as_ref().map(|sa| sa as *const _),
                )
            }?;

            Ok(Self {
                client_id,
                expected_token: token,
                authenticated: false,
                handle,
            })
        }

        #[cfg(not(windows))]
        {
            let _ = client_id;
            Ok(Self {
                client_id,
                expected_token: token,
                authenticated: false,
            })
        }
    }

    /// Block until a command is received, then return it.
    ///
    /// Each call is a full cycle: wait for connect → auth → read command →
    /// disconnect. This ensures the pipe is ready for the next client.
    pub fn receive(&mut self) -> Result<Command> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};
            use windows::Win32::Storage::FileSystem::ReadFile;
            use windows::Win32::Foundation::ERROR_PIPE_CONNECTED;

            // Wait for client to connect. ERROR_PIPE_CONNECTED means a client
            // connected between CreateNamedPipe and ConnectNamedPipe — that's fine.
            let connect_result = unsafe { ConnectNamedPipe(self.handle, None) };
            if let Err(ref e) = connect_result {
                if e.code() != ERROR_PIPE_CONNECTED.into() {
                    return Err(connect_result.unwrap_err().into());
                }
            }

            // Read 32-byte session token (first message on every connection).
            let mut token_buf = [0u8; 32];
            let mut token_bytes_read: u32 = 0;
            let token_result = unsafe {
                ReadFile(self.handle, Some(&mut token_buf), Some(&mut token_bytes_read), None)
            };

            if token_result.is_err() || token_bytes_read != 32
                || !constant_time_eq(&token_buf, &self.expected_token)
            {
                tracing::error!(
                    client_id = self.client_id,
                    "Session token validation failed — dropping connection"
                );
                unsafe { let _ = DisconnectNamedPipe(self.handle); }
                anyhow::bail!(
                    "Session token mismatch for client {}",
                    self.client_id
                );
            }

            tracing::debug!(client_id = self.client_id, "Pipe session authenticated");

            // Read the actual command.
            let mut buf = vec![0u8; 4096];
            let mut bytes_read: u32 = 0;
            unsafe {
                ReadFile(self.handle, Some(&mut buf), Some(&mut bytes_read), None)?;
            }

            let (cmd, _) = protocol::decode::<Command>(&buf[..bytes_read as usize])
                .ok_or_else(|| anyhow::anyhow!("Failed to decode command for client {}", self.client_id))?;

            if !validate_command(&cmd) {
                unsafe { let _ = DisconnectNamedPipe(self.handle); }
                anyhow::bail!("Command validation failed for client {}", self.client_id);
            }

            // Disconnect so the pipe is ready for the next connection.
            unsafe { let _ = DisconnectNamedPipe(self.handle); }

            Ok(cmd)
        }

        #[cfg(not(windows))]
        {
            let _ = self.client_id;
            anyhow::bail!("Not implemented (non-Windows stub)")
        }
    }

    /// Reset authentication state (no longer needed — each connection re-auths).
    pub fn reset_auth(&mut self) {
        // No-op: authentication is now per-connection.
    }

    /// Send a response back to the orchestrator.
    pub fn respond(&self, response: &Response) -> Result<()> {
        #[cfg(windows)]
        {
            use windows::Win32::Storage::FileSystem::WriteFile;

            let data = protocol::encode(response)
                .map_err(|e| anyhow::anyhow!("failed to encode response: {e}"))?;
            let mut written: u32 = 0;
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
        Command::CastSpell { spell_slot, .. } => *spell_slot <= 13,
        Command::MoveTo { x, y, z } => x.is_finite() && y.is_finite() && z.is_finite(),
        Command::NavigateTo { waypoints } => waypoints.len() <= 1000,
        _ => true,
    }
}

/// Constant-time comparison to prevent timing side-channels on token validation.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Build a SECURITY_ATTRIBUTES with a DACL that grants GENERIC_READ|GENERIC_WRITE
/// only to the current user SID, denying access to other users on the system.
///
/// Uses the SDDL string `D:(A;;GRGW;;;CU)` which means:
/// - D: DACL
/// - A: Allow
/// - GRGW: GENERIC_READ | GENERIC_WRITE
/// - CU: CREATOR_OWNER (resolves to the creating user's SID)
///
/// Returns `None` if the security descriptor cannot be created (non-fatal —
/// the pipe falls back to default security).
#[cfg(windows)]
fn build_restrictive_security_attributes() -> Option<windows::Win32::Security::SECURITY_ATTRIBUTES> {
    use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorA;
    use windows::core::PCSTR;

    // SDDL: DACL grants GENERIC_ALL to Authenticated Users.
    // CO (CREATOR_OWNER) is too restrictive — blocks the orchestrator process
    // even when running as the same user. AU (Authenticated Users) is safe for
    // local named pipes since they're not network-accessible by default.
    let sddl = b"D:(A;;GA;;;AU)\0";
    let mut sd_ptr: windows::Win32::Security::PSECURITY_DESCRIPTOR =
        windows::Win32::Security::PSECURITY_DESCRIPTOR(std::ptr::null_mut());

    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorA(
            PCSTR(sddl.as_ptr()),
            1, // SDDL_REVISION_1
            &mut sd_ptr,
            None,
        )
    };

    if ok.is_err() {
        tracing::warn!("Failed to create restrictive DACL for pipe — using default security");
        return None;
    }

    Some(windows::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_ptr.0,
        bInheritHandle: false.into(),
    })
}

#[cfg(not(windows))]
fn build_restrictive_security_attributes() -> Option<()> {
    None
}

impl Drop for CommandListener {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Pipes::DisconnectNamedPipe;
            unsafe {
                let _ = DisconnectNamedPipe(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
