//! Named pipe SERVER (DLL side).
//!
//! Creates a named pipe and listens for commands from the orchestrator.
//! On non-Windows platforms this is a compile-only stub.

use dmft_common::ipc::{Command, Response};
#[cfg(windows)]
use dmft_common::ipc::PIPE_NAME_PREFIX;
use dmft_common::types::ClientId;
#[cfg(windows)]
use dmft_common::protocol;
use anyhow::Result;

/// Listens for commands from the orchestrator via named pipe.
pub struct CommandListener {
    client_id: ClientId,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

impl CommandListener {
    /// Create a named pipe server for `client_id`.
    ///
    /// Pipe name: `\\.\pipe\dmft_cmd_{client_id}`
    pub fn new(client_id: ClientId) -> Result<Self> {
        #[cfg(windows)]
        {
            use windows::core::PCSTR;
            use windows::Win32::System::Pipes::CreateNamedPipeA;
            use windows::Win32::System::Pipes::{PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT};

            let pipe_name = format!("{}cmd_{}\0", PIPE_NAME_PREFIX, client_id);

            let handle = unsafe {
                CreateNamedPipeA(
                    PCSTR(pipe_name.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,    // max instances
                    4096, // out buffer
                    4096, // in buffer
                    0,    // default timeout
                    None, // default security
                )
            }?;

            Ok(Self {
                client_id,
                handle,
            })
        }

        #[cfg(not(windows))]
        {
            let _ = client_id;
            Ok(Self { client_id })
        }
    }

    /// Block until a command is received, then return it.
    pub fn receive(&self) -> Result<Command> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Pipes::ConnectNamedPipe;
            use windows::Win32::Storage::FileSystem::ReadFile;

            // Wait for client to connect
            unsafe {
                ConnectNamedPipe(self.handle, None)?;
            }

            let mut buf = vec![0u8; 4096];
            let mut bytes_read: u32 = 0;
            unsafe {
                ReadFile(self.handle, Some(&mut buf), Some(&mut bytes_read), None)?;
            }

            let (cmd, _) = protocol::decode::<Command>(&buf[..bytes_read as usize])
                .ok_or_else(|| anyhow::anyhow!("Failed to decode command for client {}", self.client_id))?;

            Ok(cmd)
        }

        #[cfg(not(windows))]
        {
            let _ = self.client_id;
            anyhow::bail!("Not implemented (non-Windows stub)")
        }
    }

    /// Send a response back to the orchestrator.
    pub fn respond(&self, response: &Response) -> Result<()> {
        #[cfg(windows)]
        {
            use windows::Win32::Storage::FileSystem::WriteFile;

            let data = protocol::encode(response);
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
