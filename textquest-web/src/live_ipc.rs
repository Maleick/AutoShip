use textquest_common::ipc::{AutoAcceptSettings, MerchantWindowSnapshot};

#[derive(Debug, Default, Clone)]
pub struct LiveApplySummary {
    pub attempted: usize,
    pub applied: usize,
    pub failures: Vec<(u32, String)>,
}

#[derive(Debug, Default, Clone)]
pub struct LiveMerchantQueryResult {
    pub pid: u32,
    pub windows: Vec<MerchantWindowSnapshot>,
}

pub fn apply_auto_accept_settings(settings: &AutoAcceptSettings) -> LiveApplySummary {
    imp::apply_auto_accept_settings(settings)
}

pub fn query_merchant_windows() -> Vec<LiveMerchantQueryResult> {
    imp::query_merchant_windows()
}

#[cfg(not(windows))]
mod imp {
    use super::{AutoAcceptSettings, LiveApplySummary, LiveMerchantQueryResult};

    pub fn apply_auto_accept_settings(_settings: &AutoAcceptSettings) -> LiveApplySummary {
        LiveApplySummary::default()
    }

    pub fn query_merchant_windows() -> Vec<LiveMerchantQueryResult> {
        Vec::new()
    }
}

#[cfg(windows)]
mod imp {
    use std::{ffi::CString, path::Path};

    use anyhow::{Context, Result};
    use textquest_common::{
        ipc::{
            AutoAcceptSettings, Command, IpcCommand, IpcResponse, MerchantQuery,
            MerchantWindowSnapshot, Response, load_session_token, pipe_name, session_id_from_token,
        },
        protocol,
    };
    use windows::{
        Win32::{
            Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, STILL_ACTIVE},
            Storage::FileSystem::{
                CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_EXISTING, ReadFile,
                WriteFile,
            },
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        core::PCSTR,
    };

    use super::{LiveApplySummary, LiveMerchantQueryResult};

    const PIPE_READ_CHUNK_SIZE: usize = 4096;

    struct PipeClient {
        handle: HANDLE,
    }

    impl PipeClient {
        fn connect(pid: u32, session_id: u64) -> Result<Self> {
            let pipe_name = pipe_name(session_id, pid);
            let c_string = CString::new(pipe_name).context("invalid pipe name")?;
            let handle = unsafe {
                CreateFileA(
                    PCSTR(c_string.as_ptr().cast()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .with_context(|| format!("connect pipe for PID {pid}"))?;

            Ok(Self { handle })
        }

        fn write_all(&self, bytes: &[u8]) -> Result<()> {
            let mut offset = 0usize;
            while offset < bytes.len() {
                let mut written = 0u32;
                unsafe {
                    WriteFile(
                        self.handle,
                        Some(&bytes[offset..]),
                        Some(&mut written),
                        None,
                    )
                }
                .context("write to named pipe")?;
                if written == 0 {
                    anyhow::bail!("named pipe write returned zero bytes");
                }
                offset += written as usize;
            }
            Ok(())
        }

        fn send_token(&self, token: &[u8; 32]) -> Result<()> {
            self.write_all(token)
        }

        fn send_async(&self, command: &Command) -> Result<()> {
            let frame = protocol::encode(&IpcCommand::new(command.clone()))
                .context("encode IPC command")?;
            self.write_all(&frame)
        }

        fn send_sync(&self, command: &Command) -> Result<Response> {
            let frame = protocol::encode(&IpcCommand::new(command.clone()))
                .context("encode IPC command")?;
            self.write_all(&frame)?;

            let max_frame_size = protocol::MAX_MESSAGE_SIZE as usize + protocol::FRAME_HEADER_SIZE;
            let mut buf = Vec::with_capacity(max_frame_size);

            loop {
                if buf.len() >= max_frame_size {
                    anyhow::bail!("IPC response exceeded max frame size");
                }

                let mut chunk =
                    vec![0u8; PIPE_READ_CHUNK_SIZE.min(max_frame_size.saturating_sub(buf.len()))];
                let mut bytes_read = 0u32;
                unsafe { ReadFile(self.handle, Some(&mut chunk), Some(&mut bytes_read), None) }
                    .context("read from named pipe")?;
                if bytes_read == 0 {
                    anyhow::bail!("named pipe read returned zero bytes");
                }

                buf.extend_from_slice(&chunk[..bytes_read as usize]);

                match protocol::decode_frame::<IpcResponse>(&buf) {
                    Ok(Some((ipc_response, _))) => return Ok(ipc_response.response),
                    Ok(None) => continue,
                    Err(error) => {
                        return Err(anyhow::Error::new(error)).context("decode IPC response");
                    }
                }
            }
        }
    }

    impl Drop for PipeClient {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    fn token_pid(path: &Path) -> Option<u32> {
        let file_name = path.file_name()?.to_str()?;
        let digits = file_name
            .strip_prefix("login_token_")?
            .strip_suffix(".bin")?;
        digits.parse().ok()
    }

    fn is_process_alive(pid: u32) -> bool {
        let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        else {
            return false;
        };
        let mut exit_code = 0u32;
        let alive = unsafe { GetExitCodeProcess(handle, &mut exit_code).is_ok() }
            && exit_code == STILL_ACTIVE.0 as u32;
        unsafe {
            let _ = CloseHandle(handle);
        }
        alive
    }

    fn for_each_live_session<F>(mut visit: F)
    where
        F: FnMut(u32),
    {
        let token_dir = std::env::temp_dir().join("textquest");
        let Ok(entries) = std::fs::read_dir(&token_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(pid) = token_pid(&path) else {
                continue;
            };
            if !is_process_alive(pid) {
                continue;
            }
            visit(pid);
        }
    }

    pub fn apply_auto_accept_settings(settings: &AutoAcceptSettings) -> LiveApplySummary {
        let mut summary = LiveApplySummary::default();
        for_each_live_session(|pid| {
            summary.attempted += 1;

            let apply_result = (|| -> Result<()> {
                let token =
                    load_session_token(pid).with_context(|| format!("load token for PID {pid}"))?;
                let session_id = session_id_from_token(&token);
                let pipe = PipeClient::connect(pid, session_id)?;
                pipe.send_token(&token)?;
                pipe.send_async(&Command::SetAutoAcceptSettings {
                    settings: settings.clone(),
                })?;
                Ok(())
            })();

            match apply_result {
                Ok(()) => summary.applied += 1,
                Err(error) => summary.failures.push((pid, error.to_string())),
            }
        });

        summary
    }

    pub fn query_merchant_windows() -> Vec<LiveMerchantQueryResult> {
        let mut results = Vec::new();

        for_each_live_session(|pid| {
            let query_result = (|| -> Result<Vec<MerchantWindowSnapshot>> {
                let token =
                    load_session_token(pid).with_context(|| format!("load token for PID {pid}"))?;
                let session_id = session_id_from_token(&token);
                let pipe = PipeClient::connect(pid, session_id)?;
                pipe.send_token(&token)?;
                match pipe.send_sync(&Command::QueryMerchantItems {
                    filter: MerchantQuery {
                        text_contains: None,
                        max_rows: Some(256),
                    },
                })? {
                    Response::MerchantItems { windows } => Ok(windows),
                    other => anyhow::bail!("Unexpected IPC response: {other:?}"),
                }
            })();

            match query_result {
                Ok(windows) => results.push(LiveMerchantQueryResult { pid, windows }),
                Err(error) => {
                    tracing::debug!(pid, error = %error, "Failed to query merchant windows");
                }
            }
        });

        results
    }
}
