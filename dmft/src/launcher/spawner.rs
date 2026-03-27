use anyhow::Result;
use std::path::Path;

pub struct SpawnedProcess {
    pub pid: u32,
}

/// Launch an EQ client process with login and server args.
#[cfg(windows)]
pub fn spawn_eq_client(
    eq_path: &Path,
    account: &str,
    server: &str,
    extra_args: &[String],
) -> Result<SpawnedProcess> {
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let cmd = format!(
        "\"{}\" patchme /login:{} /server:{}{}",
        eq_path.display(),
        account,
        server,
        if extra_args.is_empty() {
            String::new()
        } else {
            format!(" {}", extra_args.join(" "))
        }
    );

    let mut cmd_wide: Vec<u16> = OsString::from(&cmd).encode_wide().chain(Some(0)).collect();
    let mut si = STARTUPINFOW::default();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi = PROCESS_INFORMATION::default();

    // ACCEPTED RISK (security-H3): Account name is visible in process command line
    // via Task Manager. This is required by EQ's patchme launcher (/login: flag).
    // Mitigation: do not log the full command line. Consider PEB scrubbing
    // post-launch in a future security hardening pass.
    unsafe {
        CreateProcessW(
            None,
            windows::core::PWSTR(cmd_wide.as_mut_ptr()),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            None,
            &si,
            &mut pi,
        )?;

        // Close thread handle immediately; we only need the process handle
        let _ = CloseHandle(pi.hThread);
        let pid = pi.dwProcessId;
        let _ = CloseHandle(pi.hProcess);

        tracing::info!(pid, account = "[redacted]", server, "Launched EQ client");
        Ok(SpawnedProcess { pid })
    }
}

#[cfg(not(windows))]
pub fn spawn_eq_client(
    eq_path: &Path,
    _account: &str,
    server: &str,
    _extra_args: &[String],
) -> Result<SpawnedProcess> {
    tracing::warn!(
        path = %eq_path.display(),
        account = "[redacted]",
        server,
        "spawn_eq_client not available on this platform (stub)"
    );
    anyhow::bail!("EQ client launching is only available on Windows")
}
