use anyhow::Result;

/// Window handle wrapper for sending input to EQ clients.
/// Milestone 1 only defines the structure — actual input dispatch is M2.
pub struct WindowHandle {
    #[cfg(windows)]
    pub hwnd: windows::Win32::Foundation::HWND,
    pub title: String,
    pub pid: u32,
}

/// Find all windows matching a title substring (case-insensitive).
/// Returns (HWND, title, PID) tuples.
#[cfg(windows)]
pub fn find_windows_by_title(substring: &str) -> Result<Vec<WindowHandle>> {
    use std::sync::Mutex;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };

    let substring_lower = substring.to_lowercase();
    let results: Mutex<Vec<WindowHandle>> = Mutex::new(Vec::new());

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
