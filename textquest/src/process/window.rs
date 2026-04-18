use anyhow::Result;

/// Window handle wrapper for sending input to EQ clients.
/// Milestone 1 only defines the structure — actual input dispatch is M2.
pub struct WindowHandle {
    #[cfg(windows)]
    #[allow(dead_code)] // Needed for PostMessage input dispatch in M2+
    /// Win32 window handle for this EQ client.
    pub hwnd: windows::Win32::Foundation::HWND,
    /// Window title text.
    pub title: String,
    /// Process ID that owns this window.
    pub pid: u32,
}

/// Get the PID of the foreground window among a set of known PIDs.
/// Returns `None` if no known PID matches the foreground window.
#[cfg(windows)]
pub fn get_foreground_pid(known_pids: &[u32]) -> Option<u32> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };

    unsafe {
        let fg: HWND = GetForegroundWindow();
        if fg.is_invalid() {
            return None;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(fg, Some(&mut pid));

        if pid != 0 && known_pids.contains(&pid) {
            Some(pid)
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn get_foreground_pid(_known_pids: &[u32]) -> Option<u32> {
    None
}

/// Find all windows matching a title substring (case-insensitive).
/// Returns (HWND, title, PID) tuples.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn find_windows_by_title(substring: &str) -> Result<Vec<WindowHandle>> {
    use std::sync::Mutex;
    use windows::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId},
    };

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = &*(lparam.0 as *const (String, *const Mutex<Vec<WindowHandle>>));
            let substring = &data.0;
            let results = &*data.1;

            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len as usize]);
                if title.to_lowercase().contains(substring) {
                    let mut pid: u32 = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    if let Ok(mut r) = results.lock() {
                        r.push(WindowHandle { hwnd, title, pid });
                    }
                }
            }
            BOOL(1) // continue enumeration
        }
    }

    let substring_lower = substring.to_lowercase();
    let results: Mutex<Vec<WindowHandle>> = Mutex::new(Vec::new());
    let data = (substring_lower, &results as *const _);
    unsafe { EnumWindows(Some(enum_callback), LPARAM(&data as *const _ as isize)) }.ok();

    Ok(results.into_inner().unwrap_or_default())
}

#[cfg(not(windows))]
pub fn find_windows_by_title(substring: &str) -> Result<Vec<WindowHandle>> {
    tracing::warn!(
        substring,
        "find_windows_by_title called on non-Windows platform (stub)"
    );
    Ok(Vec::new())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn find_windows_by_title_stub_returns_empty_results() {
        let windows = find_windows_by_title("eqgame").expect("stub search should succeed");
        assert!(
            windows.is_empty(),
            "non-Windows stub should not report windows"
        );
    }

    #[test]
    fn get_foreground_pid_stub_returns_none() {
        let pids = vec![100, 200, 300];
        assert!(get_foreground_pid(&pids).is_none());
    }
}
