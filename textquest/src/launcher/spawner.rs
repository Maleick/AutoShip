use anyhow::Result;
use std::path::Path;

/// Result of spawning an EQ client process.
pub struct SpawnedProcess {
    /// OS process ID of the newly launched EQ client.
    pub pid: u32,
}

/// Launch an EQ client process with login and server args.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn spawn_eq_client(
    eq_path: &Path,
    account: &str,
    server: &str,
    extra_args: &[String],
) -> Result<SpawnedProcess> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
    };

    // MQ2 syntax: eqgame.exe patchme /login:username
    // No /server: flag — server selection is handled post-login by the automation.
    let cmd = format!(
        "\"{}\\eqgame.exe\" patchme /login:{}{}",
        eq_path.display(),
        account,
        if extra_args.is_empty() {
            String::new()
        } else {
            format!(" {}", extra_args.join(" "))
        }
    );

    let mut cmd_wide: Vec<u16> = OsString::from(&cmd).encode_wide().chain(Some(0)).collect();
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    // EQ must run from its own directory (loads DLLs relative to CWD).
    let eq_dir: Vec<u16> = OsString::from(eq_path.as_os_str())
        .encode_wide()
        .chain(Some(0))
        .collect();

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
            windows::core::PCWSTR(eq_dir.as_ptr()),
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

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn spawn_eq_client_stub_returns_platform_error() {
        let result = spawn_eq_client(
            Path::new("/tmp/eq"),
            "Frostreaver",
            "Teek",
            &[String::from("/nomusic")],
        );
        let error = result.err().expect("stub should return an error");

        assert!(
            format!("{error:#}").contains("only available on Windows"),
            "stub error should explain the platform restriction"
        );
    }
}
