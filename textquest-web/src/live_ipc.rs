use textquest_common::ipc::AutoAcceptSettings;

#[derive(Debug, Default, Clone)]
pub struct LiveApplySummary {
    pub attempted: usize,
    pub applied: usize,
    pub failures: Vec<(u32, String)>,
}

pub fn apply_auto_accept_settings(settings: &AutoAcceptSettings) -> LiveApplySummary {
    imp::apply_auto_accept_settings(settings)
}

#[cfg(not(windows))]
mod imp {
    use super::{AutoAcceptSettings, LiveApplySummary};

    pub fn apply_auto_accept_settings(_settings: &AutoAcceptSettings) -> LiveApplySummary {
        LiveApplySummary::default()
    }
}

#[cfg(windows)]
mod imp {
    use std::{ffi::CString, path::Path};

    use anyhow::{Context, Result};
    use textquest_common::{
        ipc::{
            AutoAcceptSettings, Command, IpcCommand, load_session_token, pipe_name,
            session_id_from_token,
        },
        protocol,
    };
    use windows::{
        Win32::{
            Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, STILL_ACTIVE},
            Storage::FileSystem::{
                CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_EXISTING, WriteFile,
            },
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        core::PCSTR,
    };

    use super::LiveApplySummary;

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

    pub fn apply_auto_accept_settings(settings: &AutoAcceptSettings) -> LiveApplySummary {
        let mut summary = LiveApplySummary::default();
        let token_dir = std::env::temp_dir().join("textquest");
        let Ok(entries) = std::fs::read_dir(&token_dir) else {
            return summary;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(pid) = token_pid(&path) else {
                continue;
            };

            if !is_process_alive(pid) {
                continue;
            }

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
        }

        summary
    }
}
