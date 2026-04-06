//! UI widget manipulation helpers for EQ's login system.
//!
//! Uses direct memory writes to `EQLogin`'s fixed char arrays for credential entry,
//! bypassing CXStr/SIDL widget navigation entirely. For other UI interactions
//! (splash dismiss, error dialogs), falls back to SIDL window lookup.
//!
//! Core widget primitives (`CXStr` read/write, button click, window find) live in
//! `crate::eq::widgets` — this module re-exports and composes them for login-specific flows.
//!
//! All functions are no-ops on non-Windows platforms.

use textquest_common::login::LoginError;

// ─── Widget XML names (stable across EQ patches) ───

pub const LOGIN_USERNAME_EDIT: &str = "LOGIN_UsernameEdit";
pub const LOGIN_PASSWORD_EDIT: &str = "LOGIN_PasswordEdit";
pub const LOGIN_CONNECT_BUTTON: &str = "LOGIN_ConnectButton";
pub const SERVERSELECT_SERVER_LIST: &str = "SERVERSELECT_ServerList";
pub const CHARACTER_LIST: &str = "Character_List";
pub const OK_DIALOG: &str = "okdialog";
pub const DBG_SPLASH: &str = "dbgsplash";
pub const SOE_SPLASH: &str = "soesplash";

// ─── MQ2 AutoLogin SIDL window names (from StateMachine.cpp) ───
// These are CSidlScreenWnd::SidlText values, stable across patches.
// Used for state detection: which screen is EQ showing right now?

/// Login screen — the main connect/credential entry screen (eqmain context).
pub const SIDL_CONNECT: &str = "connect";
/// Server select screen (eqmain context).
pub const SIDL_SERVER_SELECT: &str = "serverselect";
/// Yes/No confirmation dialog (e.g., "already logged in — kick?").
pub const SIDL_YES_NO_DIALOG: &str = "yesnodialog";
/// OK dialog (error messages, server full, etc.).
pub const SIDL_OK_DIALOG: &str = "okdialog";
/// Character select screen (eqgame context — eqmain.dll is unloaded).
pub const SIDL_CHARACTER_LIST_WND: &str = "CharacterListWnd";

// ─── SIDL child widget names ───
pub const SIDL_YESNO_YES_BUTTON: &str = "YESNO_YesButton";
pub const SIDL_YESNO_NO_BUTTON: &str = "YESNO_NoButton";
pub const SIDL_YESNO_DISPLAY: &str = "YESNO_Display";

// ─── Pre-login prompt screens (from MQ2AutoLogin) ───
// These are (parent_window_text, button_text) pairs for screens that must be
// dismissed before reaching the login form. Matched by WindowText substring.
const PRE_LOGIN_PROMPTS: &[(&str, &str)] = &[
    ("EULA", "I Accept"),     // End User License Agreement
    ("Order", "Decline"),     // OrderWindow upsell
    ("Expansion", "Decline"), // OrderExpansionWindow upsell
    ("seizure", "OK"),        // Seizure / photosensitivity warning
    ("news", "OK"),           // News / patch notes
];

/// Check if a named window is visible in the UI (by `WindowText` + dShow flag).
pub fn is_window_visible(eqmain_base: u64, window_name: &str) -> bool {
    #[cfg(windows)]
    {
        if eqmain_base == 0 {
            return false;
        }
        let Some(cxwnd_mgr) = super::eqmain::resolve_cxwnd_manager(eqmain_base) else {
            return false;
        };
        unsafe { crate::eq::widgets::find_visible_window_by_name(cxwnd_mgr, window_name) }.is_some()
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, window_name);
        false
    }
}

/// Check if a SIDL-named window is visible in the eqmain.dll `CXWndManager`.
///
/// Uses `CSidlScreenWnd::SidlText` (+0x270 in eqmain) for matching and checks
/// the dShow visibility flag. This is the MQ2 `AutoLogin` approach.
pub fn is_sidl_window_visible(eqmain_base: u64, sidl_name: &str) -> bool {
    #[cfg(windows)]
    {
        find_visible_sidl_window(eqmain_base, sidl_name).is_some()
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, sidl_name);
        false
    }
}

/// Find a visible SIDL-named window in eqmain.dll's `CXWndManager`.
///
/// Returns the `CXWnd` pointer if found and visible, None otherwise.
/// Uses eqmain.dll offsets for `CXWndManager` and `SidlText`.
pub fn find_visible_sidl_window(eqmain_base: u64, sidl_name: &str) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain as off;

        if eqmain_base == 0 {
            return None;
        }

        let cxwnd_mgr = super::eqmain::resolve_cxwnd_manager(eqmain_base)?;

        // eqmain.dll uses SidlText at the same offset as eqgame's CSIDL_SCREEN_WND_SIDL_TEXT
        // (0x270), but CXWndManager layout differs (array at +0x010, count at +0x018).
        unsafe {
            crate::eq::widgets::find_visible_window_by_sidl_name(
                cxwnd_mgr,
                sidl_name,
                textquest_common::offsets::eqgame::CSIDL_SCREEN_WND_SIDL_TEXT,
                off::CXWNDMGR_WINDOWS_ARRAY,
                off::CXWNDMGR_WINDOWS_COUNT,
            )
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, sidl_name);
        None
    }
}

/// Find a visible child window by its SIDL name within a parent window.
///
/// Walks the parent's child `TList` and checks `SidlText` + dShow.
pub fn find_visible_child_by_sidl(parent_wnd: usize, sidl_name: &str) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqgame as eqg;
        use textquest_common::offsets::eqmain as off;

        if parent_wnd == 0 {
            return None;
        }

        unsafe {
            let mut child = *((parent_wnd + off::CXWND_FIRST_NODE) as *const usize);
            let mut count = 0u32;

            while child != 0 && count < 200 {
                count += 1;

                if crate::eq::widgets::is_visible(child) {
                    if let Some(text) =
                        crate::eq::widgets::read_cxstr(child + eqg::CSIDL_SCREEN_WND_SIDL_TEXT)
                    {
                        if text.eq_ignore_ascii_case(sidl_name) {
                            return Some(child);
                        }
                    }
                }

                child = *((child + off::CXWND_NEXT) as *const usize);
            }
        }
        None
    }

    #[cfg(not(windows))]
    {
        let _ = (parent_wnd, sidl_name);
        None
    }
}

/// Read the display text from a `YesNo` dialog's `YESNO_Display` child window.
/// Returns the dialog message text, or None if not found.
pub fn read_yesno_dialog_text(dialog_wnd: usize) -> Option<String> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqgame as eqg;
        use textquest_common::offsets::eqmain as off;

        if dialog_wnd == 0 {
            return None;
        }

        unsafe {
            // Find YESNO_Display child by SidlText
            let mut child = *((dialog_wnd + off::CXWND_FIRST_NODE) as *const usize);
            let mut count = 0u32;

            while child != 0 && count < 200 {
                count += 1;

                if let Some(sidl_text) =
                    crate::eq::widgets::read_cxstr(child + eqg::CSIDL_SCREEN_WND_SIDL_TEXT)
                {
                    if sidl_text.eq_ignore_ascii_case(SIDL_YESNO_DISPLAY) {
                        // Read the WindowText of the display child
                        return crate::eq::widgets::read_cxstr(child + off::CXWND_WINDOW_TEXT);
                    }
                }

                child = *((child + off::CXWND_NEXT) as *const usize);
            }
        }
        None
    }

    #[cfg(not(windows))]
    {
        let _ = dialog_wnd;
        None
    }
}

/// Click the Yes button in a `YesNo` dialog by finding the `YESNO_YesButton` child.
pub fn click_yesno_yes(dialog_wnd: usize) -> bool {
    #[cfg(windows)]
    {
        if dialog_wnd == 0 {
            return false;
        }
        if let Some(yes_btn) = find_visible_child_by_sidl(dialog_wnd, SIDL_YESNO_YES_BUTTON) {
            unsafe {
                crate::eq::widgets::click_button_via_vtable(yes_btn);
            }
            true
        } else {
            // Fallback: try finding by WindowText
            unsafe {
                if let Some(btn) = crate::eq::widgets::find_child_button_by_text(dialog_wnd, "Yes")
                {
                    crate::eq::widgets::click_button_via_vtable(btn);
                    return true;
                }
            }
            false
        }
    }

    #[cfg(not(windows))]
    {
        let _ = dialog_wnd;
        false
    }
}

/// Click the No button in a `YesNo` dialog by finding the `YESNO_NoButton` child.
pub fn click_yesno_no(dialog_wnd: usize) -> bool {
    #[cfg(windows)]
    {
        if dialog_wnd == 0 {
            return false;
        }
        if let Some(no_btn) = find_visible_child_by_sidl(dialog_wnd, SIDL_YESNO_NO_BUTTON) {
            unsafe {
                crate::eq::widgets::click_button_via_vtable(no_btn);
            }
            true
        } else {
            // Fallback: try finding by WindowText
            unsafe {
                if let Some(btn) = crate::eq::widgets::find_child_button_by_text(dialog_wnd, "No") {
                    crate::eq::widgets::click_button_via_vtable(btn);
                    return true;
                }
            }
            false
        }
    }

    #[cfg(not(windows))]
    {
        let _ = dialog_wnd;
        false
    }
}

/// Click the OK button in an OK dialog.
pub fn click_ok_dialog(dialog_wnd: usize) -> bool {
    #[cfg(windows)]
    {
        if dialog_wnd == 0 {
            return false;
        }
        // Try clicking the dialog itself (it may be the button)
        unsafe {
            crate::eq::widgets::click_button_via_vtable(dialog_wnd);
        }
        true
    }

    #[cfg(not(windows))]
    {
        let _ = dialog_wnd;
        false
    }
}

/// Find a SIDL window by its XML name. Resolves `CXWndManager` from `eqmain_base`,
/// then delegates to `crate::eq::widgets::find_window_by_name()`.
#[cfg(windows)]
fn find_window_by_name(eqmain_base: u64, name: &str) -> Option<usize> {
    let cxwnd_mgr = super::eqmain::resolve_cxwnd_manager(eqmain_base)?;
    unsafe { crate::eq::widgets::find_window_by_name(cxwnd_mgr, name) }
}

/// Find a window whose `WindowText` contains the given substring (case-insensitive).
#[cfg(windows)]
fn find_window_by_text_contains(eqmain_base: u64, substring: &str) -> Option<usize> {
    let cxwnd_mgr = super::eqmain::resolve_cxwnd_manager(eqmain_base)?;
    unsafe { crate::eq::widgets::find_window_by_text_contains(cxwnd_mgr, substring) }
}

/// Walk a parent window's child list looking for a button whose `WindowText`
/// contains the given substring.
#[cfg(windows)]
fn find_child_button_by_text(parent_wnd: usize, button_text: &str) -> Option<usize> {
    unsafe { crate::eq::widgets::find_child_button_by_text(parent_wnd, button_text) }
}

/// Write login credentials directly to `EQLogin`'s fixed char arrays.
///
/// This bypasses SIDL widget navigation and `CXStr` entirely — `EQLogin` has
/// plain `char[0x80]` arrays for Login and PW that we can write directly.
///
/// Path: `eqmain_base` → pinstLoginClient → deref → `LoginClient`
///       → +0x010 (pLoginData) → deref → `EQLogin` → write Login/PW.
pub fn write_login_credentials(eqmain_base: u64, account: &str, password: &str) -> bool {
    #[cfg(windows)]
    {
        use super::eqmain;
        use textquest_common::offsets::eqmain as eqmain_offsets;

        let Some(eqlogin) = eqmain::resolve_eqlogin(eqmain_base) else {
            tracing::warn!("Cannot write credentials — EQLogin not resolved");
            return false;
        };

        // SAFETY: eqlogin is a validated non-null EQLogin* resolved through
        // the LoginClient pointer chain. EQLOGIN_USERNAME and EQLOGIN_PASSWORD
        // are char[0x80] fields at known offsets within the EQLogin struct.
        // write_bytes(0, 0x80) zeroes the entire buffer, then copy_nonoverlapping
        // writes the credential bytes (clamped to EQLOGIN_FIELD_MAX = 0x7F to
        // preserve the null terminator). The buffers are within committed eqmain
        // memory. If eqlogin were freed, resolve_eqlogin would have returned None.
        unsafe {
            // Write username: zero buffer, then copy bytes (max 0x7F to leave null terminator)
            let username_addr = (eqlogin + eqmain_offsets::EQLOGIN_USERNAME) as *mut u8;
            std::ptr::write_bytes(username_addr, 0, 0x80);
            let username_len = account.len().min(eqmain_offsets::EQLOGIN_FIELD_MAX);
            std::ptr::copy_nonoverlapping(account.as_ptr(), username_addr, username_len);

            // Write password: zero buffer, then copy bytes
            let password_addr = (eqlogin + eqmain_offsets::EQLOGIN_PASSWORD) as *mut u8;
            std::ptr::write_bytes(password_addr, 0, 0x80);
            let password_len = password.len().min(eqmain_offsets::EQLOGIN_FIELD_MAX);
            std::ptr::copy_nonoverlapping(password.as_ptr(), password_addr, password_len);
        }

        tracing::debug!(
            account_len = account.len(),
            eqlogin = format!("{:#x}", eqlogin),
            "Wrote credentials to EQLogin char arrays (account & password redacted)"
        );
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, account, password);
        false
    }
}

/// Set text in a `CEditWnd` (username/password fields).
///
/// For login fields (`LOGIN_UsernameEdit`, `LOGIN_PasswordEdit`), this uses
/// direct memory writes to `EQLogin`'s char arrays instead of `CXStr` manipulation.
/// For other edit widgets, falls back to the SIDL-based approach.
pub fn set_edit_text(eqmain_base: u64, window_name: &str, text: &str) -> bool {
    #[cfg(windows)]
    {
        // For login credential fields, use direct-write path (bypasses CXStr entirely)
        if window_name == LOGIN_USERNAME_EDIT || window_name == LOGIN_PASSWORD_EDIT {
            // The direct-write path writes both fields at once via write_login_credentials().
            // Individual field writes aren't meaningful since both must be set before login.
            // Return true if EQLogin is accessible (the FSM calls write_login_credentials
            // separately before clicking connect).
            tracing::debug!(
                window = window_name,
                "set_edit_text for login field — use write_login_credentials() instead"
            );
            return super::eqmain::resolve_eqlogin(eqmain_base).is_some();
        }

        // Fallback: SIDL widget path for non-login edit fields
        let Some(edit_wnd) = find_window_by_name(eqmain_base, window_name) else {
            return false;
        };

        unsafe {
            let input_text_addr =
                edit_wnd + textquest_common::offsets::eqmain::CEDITBASEWND_INPUT_TEXT;
            crate::eq::widgets::write_cxstr_inplace(input_text_addr, text);
        }

        tracing::debug!(window = window_name, "Set edit text via SIDL");
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, window_name, text);
        false
    }
}

/// Write credentials directly to `CEditWnd` widgets by finding them in `CXWndManager`'s
/// window list and setting their `InputText` `CXStr` in-place.
///
/// This is the MQ2 approach — no keyboard simulation. We:
/// 1. Walk `CXWndManager::pWindows` to find username/password edit widgets
/// 2. Write directly to `CEditBaseWnd::InputText` (`CXStr` at +0x278)
/// 3. Click the Login button via vtable WndNotification(XWM_LCLICK)
pub fn type_credentials_to_window(eqmain_base: u64, account: &str, password: &str) -> bool {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain as off;

        let Some(cxwnd_mgr) = super::eqmain::resolve_cxwnd_manager(eqmain_base) else {
            tracing::warn!("Cannot write credentials — CXWndManager not resolved");
            return false;
        };

        unsafe {
            let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
            let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

            if array_ptr == 0 || count == 0 || count > 500 {
                tracing::warn!(count, "Invalid CXWndManager window array");
                return false;
            }

            // Find username and password edit widgets by scanning for the
            // "USERNAME" and "PASSWORD" label windows. The edit fields are
            // the windows immediately before their labels in the array.
            let mut username_edit: usize = 0;
            let mut password_edit: usize = 0;
            let mut login_button: usize = 0;
            let mut prev_wnd: usize = 0;
            let mut prev_prev_wnd: usize = 0;
            // Collect all "LOGIN" buttons — the login form submit button appears
            // BEFORE the credential fields in the window array
            let mut login_candidates: Vec<usize> = Vec::new();

            for i in 0..count as usize {
                let wnd_ptr = *((array_ptr + i * 8) as *const usize);
                if wnd_ptr == 0 {
                    continue;
                }

                if let Some(text) = crate::eq::widgets::read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT)
                {
                    if text == "USERNAME" && prev_prev_wnd != 0 {
                        username_edit = prev_prev_wnd;
                        tracing::info!(
                            ptr = format!("{:#x}", username_edit),
                            "Found username edit widget (2 before USERNAME label)"
                        );
                    }
                    if text == "PASSWORD" && prev_prev_wnd != 0 {
                        password_edit = prev_prev_wnd;
                        tracing::info!(
                            ptr = format!("{:#x}", password_edit),
                            "Found password edit widget (2 before PASSWORD label)"
                        );
                    }
                    if text == "LOGIN" {
                        login_candidates.push(wnd_ptr);
                    }
                }

                prev_prev_wnd = prev_wnd;
                prev_wnd = wnd_ptr;

                // Stop scanning once we have both edit widgets + at least two login
                // candidates (menu tab + form submit button). If we only find one,
                // keep scanning — the form submit is typically the second "LOGIN" widget.
                // Continuing to scan past both can crash on bad window pointers.
                if username_edit != 0 && password_edit != 0 && login_candidates.len() >= 2 {
                    break;
                }
            }

            // The login form submit button is typically the second "LOGIN" in the list
            // (idx=12 is the main menu LOGIN tab, idx=18 is the form submit button)
            if login_candidates.len() >= 2 {
                login_button = login_candidates[1]; // Form submit button
            } else if login_candidates.len() == 1 {
                login_button = login_candidates[0];
            }
            if login_button != 0 {
                tracing::info!(
                    ptr = format!("{:#x}", login_button),
                    candidates = login_candidates.len(),
                    "Found Login button"
                );
            }

            if username_edit == 0 || password_edit == 0 {
                tracing::warn!("Could not find username/password edit widgets");
                return false;
            }

            // Find a valid CStrRep donor from ANY field on the username widget.
            // The /login: flag inconsistently populates InputText vs WindowText.
            let un_input_addr = username_edit + off::CEDITBASEWND_INPUT_TEXT;
            let un_wt_addr = username_edit + off::CXWND_WINDOW_TEXT;
            let donor_rep = {
                let it = *(un_input_addr as *const usize);
                let wt = *(un_wt_addr as *const usize);
                if it != 0 {
                    it
                } else {
                    wt
                }
            };

            // If username InputText is null, allocate a CStrRep for it
            if *(un_input_addr as *const usize) == 0 && donor_rep != 0 {
                if let Some(new_rep) = crate::eq::widgets::clone_cstrrep(donor_rep) {
                    *(un_input_addr as *mut usize) = new_rep;
                    tracing::info!("Allocated CStrRep for username InputText");
                }
            }

            // Write username to both InputText (+0x278) and WindowText (+0x078)
            let wrote_username = crate::eq::widgets::write_cxstr_inplace(un_input_addr, account);
            let wrote_wt = crate::eq::widgets::write_cxstr_inplace(un_wt_addr, account);
            tracing::info!(
                input_text = wrote_username,
                window_text = wrote_wt,
                account_len = account.len(),
                "Wrote username to edit widget (account redacted)"
            );

            // Write password — CEditWnd may have null CXStr (never typed in).
            // Clone from the donor CStrRep (username's InputText or WindowText).
            let pw_input_addr = password_edit + off::CEDITBASEWND_INPUT_TEXT;
            let pw_wt_addr = password_edit + off::CXWND_WINDOW_TEXT;

            let pw_rep = *(pw_input_addr as *const usize);
            if pw_rep == 0 && donor_rep != 0 {
                if let Some(new_rep) = crate::eq::widgets::clone_cstrrep(donor_rep) {
                    *(pw_input_addr as *mut usize) = new_rep;
                    *(pw_wt_addr as *mut usize) = new_rep;
                    tracing::info!("Cloned CStrRep for password via process heap");
                }
            }

            let wrote_password = crate::eq::widgets::write_cxstr_inplace(pw_input_addr, password);
            let wrote_pw_wt = {
                let wt_rep = *(pw_wt_addr as *const usize);
                let it_rep = *(pw_input_addr as *const usize);
                if wt_rep == it_rep {
                    true
                }
                // same rep, already written
                else if wt_rep != 0 {
                    crate::eq::widgets::write_cxstr_inplace(pw_wt_addr, password)
                } else {
                    false
                }
            };
            tracing::info!(
                input_text = wrote_password,
                window_text = wrote_pw_wt,
                "Wrote password to edit widget (content redacted)"
            );

            if !wrote_username && !wrote_wt {
                tracing::error!("Failed to write username to any CXStr field");
                return false;
            }

            // Read back to verify writes took effect
            if let Some(readback) =
                crate::eq::widgets::read_cxstr(username_edit + off::CEDITBASEWND_INPUT_TEXT)
            {
                tracing::info!(
                    readback_len = readback.len(),
                    "Username InputText readback (account redacted)"
                );
            } else {
                tracing::warn!("Username InputText readback: null or empty");
            }
            if let Some(readback) =
                crate::eq::widgets::read_cxstr(username_edit + off::CXWND_WINDOW_TEXT)
            {
                tracing::info!(
                    readback_len = readback.len(),
                    "Username WindowText readback (account redacted)"
                );
            }

            // Hex dump the edit widget around the CXStr fields to verify layout
            tracing::info!("=== USERNAME EDIT WIDGET HEX DUMP ===");
            for row_off in [0x070usize, 0x078, 0x080, 0x270, 0x278, 0x280] {
                let addr = username_edit + row_off;
                let val = *(addr as *const usize);
                tracing::info!(
                    offset = format!("+{:#05x}", row_off),
                    val = format!("{:#018x}", val),
                    "EditWnd field"
                );
            }
            tracing::info!("=== END HEX DUMP ===");

            // Click the Login button using phase-aware helper.
            // eqmain is loaded → direct vtable click (game loop not active yet).
            if login_button != 0 {
                std::thread::sleep(std::time::Duration::from_millis(150));
                tracing::info!(
                    ptr = format!("{:#x}", login_button),
                    "Phase 1: Clicking Login button (eqmain context)"
                );
                click_button_for_phase(login_button, true); // eqmain = true
                tracing::info!("Phase 1: Login button clicked");
            } else {
                tracing::warn!("Phase 1: Login button not found — will try Enter key fallback");
            }
            // Always send Enter via PostMessage as the reliable submit path.
            // During eqmain, the game loop hook isn't active so queued button
            // clicks won't execute. PostMessage delivers directly to the window.
            simulate_enter_key(eqmain_base);
            tracing::info!("Enter key sent via PostMessage as login submit");
        }

        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, account, password);
        false
    }
}

/// Simulate pressing Enter on the EQ window to submit login credentials.
/// Type the password into EQ's focused password field using PostMessageW(WM_CHAR).
/// This works even when EQ is not the foreground window. The username should
/// already be filled by the /login: command-line flag.
///
/// Flow: click password field → type password char-by-char → press Enter.
pub fn type_password_wm_char(eqmain_base: u64, password: &str) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

        let Some(hwnd_val) = super::eqmain::resolve_eq_hwnd(eqmain_base) else {
            tracing::warn!("Cannot type password — EQ HWND not resolved");
            return false;
        };

        let hwnd = HWND(hwnd_val as isize);
        const WM_CHAR: u32 = 0x0102;
        const WM_KEYDOWN: u32 = 0x0100;
        const WM_KEYUP: u32 = 0x0101;
        const VK_TAB: u16 = 0x09;
        const VK_RETURN: u16 = 0x0D;

        // SAFETY: hwnd was resolved from EQLogin::hEQWnd — a valid HWND stored
        // by EQ itself. PostMessageW is safe to call with any HWND (returns
        // failure if invalid). WM_KEYDOWN/WM_KEYUP/WM_CHAR are standard Win32
        // keyboard messages. Character values are ASCII-safe (login credentials).
        unsafe {
            // Tab to move focus from username to password field
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_TAB as usize), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(50));
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_TAB as usize), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Type each character of the password via WM_CHAR
            for ch in password.chars() {
                let _ = PostMessageW(hwnd, WM_CHAR, WPARAM(ch as usize), LPARAM(0));
                std::thread::sleep(std::time::Duration::from_millis(15));
            }

            std::thread::sleep(std::time::Duration::from_millis(100));

            // Press Enter to submit
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_RETURN as usize), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(30));
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_RETURN as usize), LPARAM(0));
        }

        tracing::info!(
            hwnd = format!("{:#x}", hwnd_val),
            pw_len = password.len(),
            "Typed password via WM_CHAR + Enter"
        );
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, password);
        false
    }
}

/// Simulate pressing Enter via `PostMessageW` — works even when EQ is not foreground.
pub fn simulate_enter_key(eqmain_base: u64) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

        let Some(hwnd_val) = super::eqmain::resolve_eq_hwnd(eqmain_base) else {
            tracing::warn!("Cannot simulate Enter — EQ HWND not resolved");
            return false;
        };

        let hwnd = HWND(hwnd_val as isize);
        const WM_KEYDOWN: u32 = 0x0100;
        const WM_KEYUP: u32 = 0x0101;
        const VK_RETURN: u16 = 0x0D;

        // SAFETY: hwnd was resolved from EQLogin::hEQWnd. PostMessageW is safe
        // to call with any HWND. WM_KEYDOWN/WM_KEYUP with VK_RETURN simulates
        // pressing Enter to submit/dismiss dialogs.
        unsafe {
            // PostMessage sends directly to the HWND — works even when EQ
            // is not the foreground window (unlike SendInput).
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_RETURN as usize), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(50));
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_RETURN as usize), LPARAM(0));
        }

        tracing::debug!(
            hwnd = format!("{:#x}", hwnd_val),
            "Simulated Enter key via PostMessage"
        );
        true
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        false
    }
}

/// Click a button widget by sending `XWM_LCLICK` notification.
/// Delegates to `crate::eq::widgets::click_button_via_vtable` which uses the
/// named vtable offset (`CXWND_VTABLE_WND_NOTIFICATION`) rather than a hardcoded index.
pub fn click_button(eqmain_base: u64, window_name: &str) -> bool {
    #[cfg(windows)]
    {
        let Some(button_wnd) = find_window_by_name(eqmain_base, window_name) else {
            return false;
        };

        unsafe {
            crate::eq::widgets::click_button_via_vtable(button_wnd);
        }

        tracing::debug!(window = window_name, "Clicked button");
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, window_name);
        false
    }
}

/// Click a button, choosing the right mechanism for the current login phase.
///
/// During **eqmain** (login screen, server select), `ProcessGameEvents` is NOT
/// hooked — `queue_button_click()` queues a pointer that never drains. So we
/// call `click_button_via_vtable()` directly on the IPC/background thread.
///
/// During **eqgame** (character select, in-world), the game loop hook IS active,
/// so we queue the click for the next game tick to stay on the correct thread.
///
/// The `in_eqmain` flag indicates which context we're in:
/// - `true` = eqmain.dll is loaded (login screen or server select)
/// - `false` = eqmain.dll is unloaded (character select or in-game)
pub fn click_button_for_phase(button_wnd: usize, in_eqmain: bool) {
    if button_wnd == 0 {
        return;
    }

    if in_eqmain {
        // eqmain context: direct vtable click (game loop hook not active)
        tracing::debug!(
            ptr = format!("{:#x}", button_wnd),
            "click_button_for_phase: direct vtable click (eqmain)"
        );
        unsafe { crate::eq::widgets::click_button_via_vtable(button_wnd) };
    } else {
        // eqgame context: queue for game loop thread
        tracing::debug!(
            ptr = format!("{:#x}", button_wnd),
            "click_button_for_phase: queued for game loop (eqgame)"
        );
        crate::hooks::game_loop::queue_button_click(button_wnd);
    }
}

/// Dismiss splash screens and pre-login prompts (EULA, order windows, seizure
/// warning, news) if visible. `MQ2AutoLogin` clicks through 6+ screens before
/// the login form appears — we do the same.
pub fn dismiss_splash(eqmain_base: u64) {
    #[cfg(windows)]
    {
        // Classic splash screens — click anywhere to dismiss
        for name in &[DBG_SPLASH, SOE_SPLASH] {
            if is_window_visible(eqmain_base, name) {
                click_button(eqmain_base, name);
                tracing::info!(splash = name, "Dismissed splash screen");
            }
        }

        // Pre-login prompt screens — find by parent text, click matching button
        for &(parent_text, button_text) in PRE_LOGIN_PROMPTS {
            if let Some(parent_wnd) = find_window_by_text_contains(eqmain_base, parent_text) {
                // Found a window matching the parent — now find the button
                if let Some(button_wnd) = find_child_button_by_text(parent_wnd, button_text) {
                    unsafe {
                        crate::eq::widgets::click_button_via_vtable(button_wnd);
                    }
                    tracing::info!(
                        parent = parent_text,
                        button = button_text,
                        "Dismissed pre-login prompt"
                    );
                } else {
                    // Fallback: click the parent window itself
                    unsafe {
                        crate::eq::widgets::click_button_via_vtable(parent_wnd);
                    }
                    tracing::info!(
                        parent = parent_text,
                        "Dismissed pre-login prompt (clicked parent, button not found)"
                    );
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
    }
}

/// Check for error dialogs (okdialog) and return the appropriate `LoginError`.
///
/// # Current behavior (partial stub)
///
/// Detects whether an OK dialog is visible and dismisses it, but always returns
/// `LoginError::WrongPassword` regardless of the actual dialog content. This is
/// because reading the dialog's `CStmlWnd` text requires `CXStr` pointer chasing
/// that has not been validated on the target client.
///
/// # Intended behavior (when fully implemented)
///
/// 1. Detect the visible `okdialog` window.
/// 2. Read the `CStmlWnd` text content (the dialog message body).
/// 3. Pattern-match the text to return a specific `LoginError` variant:
///    - "password" / "invalid" -> `WrongPassword`
///    - "suspended" / "banned" -> `AccountLocked`
///    - "server" / "full" -> `ServerFull`
///    - "timeout" / "connection" -> `Timeout`
/// 4. Dismiss the dialog by clicking OK.
///
/// # Returns
///
/// `Some(LoginError::WrongPassword)` if any OK dialog is visible (always the same
/// variant until text parsing is implemented). `None` if no dialog is visible.
///
// NOTE: Dialog text parsing (CStmlWnd → CXStr) is not yet implemented. All error
// dialogs are treated as WrongPassword. Refining this requires validating the CXStr
// struct layout on a live client, then pattern-matching the message text.
pub fn check_error_dialog(eqmain_base: u64) -> Option<LoginError> {
    #[cfg(windows)]
    {
        if !is_window_visible(eqmain_base, OK_DIALOG) {
            return None;
        }

        // Dismiss the dialog and report a generic error. Refining error
        // classification requires CStmlWnd text parsing (CXStr layout unvalidated).
        click_button(eqmain_base, OK_DIALOG);
        tracing::warn!("Error dialog detected and dismissed");

        // Default to WrongPassword — will be refined with text parsing
        Some(LoginError::WrongPassword)
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Join a specific server by name using `LoginServerAPI::JoinServer`.
///
/// # Intended behavior (when fully implemented)
///
/// 1. Resolve the `LoginServerAPI` pointer from eqmain.dll globals.
/// 2. Iterate `LoginClient::ServerList` (a `DoublyLinkedList<EQClientServerData*>` at
///    offset `0x178`) to find the entry whose `ServerName` (`CXStr` at `+0x08`) matches
///    `server_name`.
/// 3. Extract the `ServerID` (at `+0x00`) from the matching entry.
/// 4. Call `LoginServerAPI::JoinServer(api, server_id, nullptr, 10)` to initiate
///    the server connection.
///
/// # Why this is a stub
///
/// The `LoginServerAPI` pointer and `JoinServer` function address are successfully
/// resolved, but the server-list iteration is not yet implemented. We need to:
/// - Validate the `EQClientServerData` struct layout via a calibration dump on a live
///   client (field offsets come from MQ2's `LoginFrontend.h` but are unverified).
/// - Confirm the `JoinServer` calling convention (`extern "C"`, 4 args) matches the
///   target client build.
///
/// The current login FSM bypasses this entirely by clicking "PLAY EVERQUEST!" which
/// joins the last-used server. Named server selection requires this function.
///
/// # Returns
///
/// Always returns `false` — the stub has not yet performed any server join.
/// When implemented, returns `true` if the join request was successfully dispatched.
///
/// # Related
///
/// - `calibrate_login_dump()` in this module dumps `LoginServerAPI` addresses for
///   reverse-engineering the server list.
/// - `LoginFsm::tick_selecting_server()` in `mod.rs` uses the "PLAY EVERQUEST!"
///   button click as a workaround.
///
// STUB: Server-list iteration and JoinServer call not yet implemented. The "PLAY
// EVERQUEST!" button workaround is sufficient for single-server setups. Named server
// selection (needed for multi-server TLP configs) requires iterating LoginClient::ServerList
// at offset 0x178 and calling LoginServerAPI::JoinServer with the resolved ServerID.
pub fn join_server(eqmain_base: u64, server_name: &str) -> bool {
    #[cfg(windows)]
    {
        use super::eqmain;
        use textquest_common::offsets::eqmain as eqmain_offsets;

        let Some(login_api) = eqmain::resolve_login_server_api(eqmain_base) else {
            tracing::warn!("LoginServerAPI not resolved");
            return false;
        };

        let Some(join_server_addr) =
            eqmain_offsets::rebase(eqmain_offsets::JOIN_SERVER, eqmain_base)
        else {
            tracing::warn!("Failed to rebase JoinServer address");
            return false;
        };

        // Server ID lookup requires iterating LoginClient::ServerList (DoublyLinkedList<EQClientServerData*> at offset 0x178).
        // EQClientServerData layout: ServerID at 0x00, ServerName (CXStr) at 0x08. Needs calibration dump on live client.
        tracing::info!(
            server = server_name,
            login_api = format!("{:#x}", login_api),
            join_server_fn = format!("{:#x}", join_server_addr),
            "JoinServer: API resolved but server ID lookup not yet implemented. \
             Use CalibrateLogin to dump server list."
        );

        // When server ID is known, the call will be:
        // type JoinServerFn = unsafe extern "C" fn(*mut u8, i32, *mut u8, i32) -> u32;
        // let func: JoinServerFn = std::mem::transmute(join_server_addr);
        // func(login_api as *mut u8, server_id, std::ptr::null_mut(), 10);

        false
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, server_name);
        false
    }
}

/// Select a character by name from the character select screen and enter world.
///
/// # Intended behavior (when fully implemented)
///
/// 1. Find the `Character_List` `CListWnd` in eqgame.exe's `CXWndManager`.
/// 2. Iterate the list items to find the row matching `character_name`.
/// 3. Call `CCharacterListWnd::SelectCharacter(index)` to highlight the character.
/// 4. Call `CCharacterListWnd::EnterWorld()` to zone into the game.
///
/// Both `SelectCharacter` and `EnterWorld` are eqgame.exe functions (not eqmain.dll),
/// since eqmain.dll unloads during the transition from server select to character select.
///
/// # Why this is a stub
///
/// Reading `CListWnd` item text requires understanding the `CListWnd` vtable layout
/// to call `GetItemText(row, col)`, which has not been reverse-engineered yet. The
/// function addresses for `SelectCharacter` and `EnterWorld` are resolved successfully,
/// but we cannot determine which list row corresponds to the desired character without
/// the item-text accessor.
///
/// The login FSM works around this via `do_select_character_via_game_loop()` in
/// `mod.rs`, which uses `queue_enter_world()` from the game loop hook to select
/// the character by name through a different code path (scanning `CXWndManager`
/// by `SidlText`).
///
/// # Returns
///
/// Always returns `false` — the stub has not performed character selection.
/// When implemented, returns `true` if `SelectCharacter` + `EnterWorld` were
/// successfully called.
///
/// # Related
///
/// - `LoginFsm::do_select_character_via_game_loop()` in `mod.rs` is the working
///   alternative that bypasses this function entirely.
/// - `crate::hooks::game_loop::queue_enter_world()` is the mechanism used by the FSM.
///
// STUB: Direct CListWnd item iteration and SelectCharacter/EnterWorld calls not yet
// implemented. The game-loop-based workaround (`queue_enter_world`) handles character
// selection reliably. Direct calls would be cleaner but require reading CListWnd items
// via GetItemText vtable call (unverified) or by walking the ItemsArray (see eq::widgets).
pub fn select_character(eqmain_base: u64, eq_base: u64, character_name: &str) -> bool {
    #[cfg(windows)]
    {
        // Character selection uses eqgame.exe functions, not eqmain.dll
        let Some(select_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::SELECT_CHARACTER, eq_base)
        else {
            tracing::warn!("Failed to rebase SELECT_CHARACTER");
            return false;
        };

        let Some(enter_world_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::ENTER_WORLD, eq_base)
        else {
            tracing::warn!("Failed to rebase ENTER_WORLD");
            return false;
        };

        // Character index lookup requires CListWnd item iteration (GetItemText vtable).
        // Once implemented: call SelectCharacter(index) then EnterWorld().
        let _ = (eqmain_base, select_addr, enter_world_addr, character_name);
        tracing::info!(
            character = character_name,
            "Character selection not yet implemented — will resolve on live client"
        );
        false
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, eq_base, character_name);
        false
    }
}

/// Log all window texts visible in the `CXWndManager` array.
/// Used for calibration — helps identify EULA and pre-login screen widget names.
pub fn log_all_window_texts(eqmain_base: u64) {
    #[cfg(windows)]
    {
        let Some(cxwnd_mgr) = super::eqmain::resolve_cxwnd_manager(eqmain_base) else {
            return;
        };

        tracing::info!("=== WINDOW TEXT DUMP (pre-login calibration) ===");
        unsafe {
            crate::eq::widgets::for_each_window(cxwnd_mgr, |i, wnd_ptr, text| {
                if let Some(text) = text {
                    let visible = crate::eq::widgets::is_visible(wnd_ptr);
                    tracing::info!(
                        idx = i,
                        ptr = format!("{:#x}", wnd_ptr),
                        text = %text,
                        visible,
                        "Window"
                    );
                }
                true
            });
        }
        tracing::info!("=== END WINDOW TEXT DUMP ===");
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
    }
}

/// Dump all login-related pointer addresses to the log for calibration.
/// This is called when the DLL receives a `CalibrateLogin` command.
pub fn calibrate_login_dump(eqmain_base: u64) {
    use super::eqmain;
    use textquest_common::offsets::eqmain as eqmain_offsets;

    tracing::info!("=== LOGIN CALIBRATION DUMP ===");
    tracing::info!(eqmain_base = format!("{:#x}", eqmain_base));

    // LoginClient pointer
    if let Some(addr) = eqmain_offsets::rebase(eqmain_offsets::PINST_LOGIN_CLIENT, eqmain_base) {
        tracing::info!(
            pinst_login_client_addr = format!("{:#x}", addr),
            "pinstLoginClient address"
        );

        #[cfg(windows)]
        {
            let login_client = unsafe { *(addr as *const usize) };
            tracing::info!(
                login_client_ptr = format!("{:#x}", login_client),
                "LoginClient*"
            );

            if login_client != 0 {
                let eqlogin_ptr = unsafe {
                    *((login_client + eqmain_offsets::LOGINCLIENT_LOGIN_DATA) as *const usize)
                };
                tracing::info!(
                    eqlogin_ptr = format!("{:#x}", eqlogin_ptr),
                    "EQLogin* (pLoginData)"
                );

                if eqlogin_ptr != 0 {
                    // Dump HWND
                    let hwnd =
                        unsafe { *((eqlogin_ptr + eqmain_offsets::EQLOGIN_HWND) as *const usize) };
                    tracing::info!(hwnd = format!("{:#x}", hwnd), "EQLogin::hEQWnd");

                    // Dump username field (first 32 bytes)
                    let username_addr =
                        (eqlogin_ptr + eqmain_offsets::EQLOGIN_USERNAME) as *const u8;
                    let username_bytes = unsafe { std::slice::from_raw_parts(username_addr, 32) };
                    let username = String::from_utf8_lossy(
                        &username_bytes
                            [..username_bytes.iter().position(|&b| b == 0).unwrap_or(32)],
                    );
                    tracing::info!(
                        username = %username,
                        username_addr = format!("{:#x}", username_addr as usize),
                        "EQLogin::Login"
                    );

                    // Dump password field presence (don't log actual password)
                    let pw_addr = (eqlogin_ptr + eqmain_offsets::EQLOGIN_PASSWORD) as *const u8;
                    let pw_first = unsafe { *pw_addr };
                    tracing::info!(
                        has_password = pw_first != 0,
                        pw_addr = format!("{:#x}", pw_addr as usize),
                        "EQLogin::PW (content redacted)"
                    );

                    // Dump ReturnCode
                    let return_code = unsafe { *((eqlogin_ptr + 0x410) as *const i32) };
                    tracing::info!(return_code, "EQLogin::ReturnCode");
                }
            }
        }
    }

    // LoginServerAPI
    if let Some(login_api) = eqmain::resolve_login_server_api(eqmain_base) {
        tracing::info!(
            login_server_api = format!("{:#x}", login_api),
            "LoginServerAPI*"
        );
    } else {
        tracing::info!("LoginServerAPI: not resolved (null or eqmain not loaded)");
    }

    // CSidlManager
    if let Some(sidl) = eqmain::resolve_sidl_manager(eqmain_base) {
        tracing::info!(sidl_manager = format!("{:#x}", sidl), "CSidlManager*");
    } else {
        tracing::info!("CSidlManager: not resolved");
    }

    // CXWndManager
    if let Some(cxwnd) = eqmain::resolve_cxwnd_manager(eqmain_base) {
        tracing::info!(cxwnd_manager = format!("{:#x}", cxwnd), "CXWndManager*");
    } else {
        tracing::info!("CXWndManager: not resolved");
    }

    // LoginController
    if let Some(addr) = eqmain_offsets::rebase(eqmain_offsets::PINST_LOGIN_CONTROLLER, eqmain_base)
    {
        tracing::info!(
            pinst_login_controller_addr = format!("{:#x}", addr),
            "pinstLoginController address"
        );
        #[cfg(windows)]
        {
            let controller_ptr = unsafe { *(addr as *const usize) };
            tracing::info!(
                login_controller = format!("{:#x}", controller_ptr),
                "LoginController*"
            );
        }
    }

    // Hex dump CXWndManager to discover actual struct layout (eqmain vs eqgame offsets differ)
    #[cfg(windows)]
    if let Some(cxwnd_mgr) = eqmain::resolve_cxwnd_manager(eqmain_base) {
        tracing::info!("=== CXWNDMANAGER HEX DUMP ===");
        unsafe {
            // Dump first 0x200 bytes to find ArrayClass<CXWnd*> pWindows
            for row in 0..32u64 {
                let offset = row * 16;
                let addr = cxwnd_mgr + offset as usize;
                let bytes: [u8; 16] = std::ptr::read(addr as *const [u8; 16]);
                let hex: String = bytes
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                // Also interpret as usize pairs (pointers)
                let ptr1 = *(addr as *const usize);
                let ptr2 = *((addr + 8) as *const usize);
                tracing::info!(
                    offset = format!("+{:#05x}", offset),
                    hex = %hex,
                    p1 = format!("{:#018x}", ptr1),
                    p2 = format!("{:#018x}", ptr2),
                    "CXWndMgr"
                );
            }
        }
        tracing::info!("=== END CXWNDMANAGER HEX DUMP ===");
        enumerate_cxwnd_windows(cxwnd_mgr);
    }

    tracing::info!("=== END LOGIN CALIBRATION DUMP ===");
}

/// Walk `CXWndManager`'s window array and log each window for calibration.
///
/// NOTE: Do not log raw `WindowText` here because edit controls can contain
/// sensitive user-entered values (e.g. credentials). Only log metadata.
#[cfg(windows)]
fn enumerate_cxwnd_windows(cxwnd_mgr: usize) {
    use textquest_common::offsets::eqmain as off;

    tracing::info!("=== WINDOW ENUMERATION ===");

    unsafe {
        let focus_wnd = *((cxwnd_mgr + off::CXWNDMGR_FOCUS_WINDOW) as *const usize);
        tracing::info!(focus = format!("{:#x}", focus_wnd), "FocusWindow");

        crate::eq::widgets::for_each_window(cxwnd_mgr, |i, wnd_ptr, text| {
            let window_text = text.unwrap_or_default();
            let visible = crate::eq::widgets::is_visible(wnd_ptr);
            let xml_idx = crate::eq::widgets::xml_index(wnd_ptr);
            let has_text = !window_text.is_empty();
            let text_len = window_text.chars().count();

            if visible || has_text {
                tracing::info!(
                    idx = i,
                    ptr = format!("{:#x}", wnd_ptr),
                    xml_index = xml_idx,
                    visible,
                    has_text,
                    text_len,
                    "Window"
                );
            }
            true // continue iteration
        });
    }

    tracing::info!("=== END WINDOW ENUMERATION ===");
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn is_window_visible_returns_false_on_macos() {
        assert!(!is_window_visible(0, "LOGIN_ConnectButton"));
    }

    #[test]
    fn set_edit_text_returns_false_on_macos() {
        assert!(!set_edit_text(0, "LOGIN_UsernameEdit", "test"));
    }

    #[test]
    fn click_button_returns_false_on_macos() {
        assert!(!click_button(0, "LOGIN_ConnectButton"));
    }

    #[test]
    fn dismiss_splash_noop_on_macos() {
        dismiss_splash(0); // Should not panic
    }

    #[test]
    fn check_error_dialog_returns_none_on_macos() {
        assert!(check_error_dialog(0).is_none());
    }

    #[test]
    fn join_server_returns_false_on_macos() {
        assert!(!join_server(0, "TestServer"));
    }

    #[test]
    fn select_character_returns_false_on_macos() {
        assert!(!select_character(0, 0, "TestChar"));
    }

    #[test]
    fn write_login_credentials_returns_false_on_macos() {
        assert!(!write_login_credentials(0, "testaccount", "testpass"));
    }

    #[test]
    fn simulate_enter_key_returns_false_on_macos() {
        assert!(!simulate_enter_key(0));
    }

    #[test]
    fn calibrate_login_dump_noop_on_macos() {
        calibrate_login_dump(0); // Should not panic
    }

    #[test]
    fn widget_names_are_consistent() {
        assert_eq!(LOGIN_USERNAME_EDIT, "LOGIN_UsernameEdit");
        assert_eq!(LOGIN_PASSWORD_EDIT, "LOGIN_PasswordEdit");
        assert_eq!(LOGIN_CONNECT_BUTTON, "LOGIN_ConnectButton");
        assert_eq!(SERVERSELECT_SERVER_LIST, "SERVERSELECT_ServerList");
        assert_eq!(CHARACTER_LIST, "Character_List");
    }

    #[test]
    fn sidl_names_are_consistent() {
        assert_eq!(SIDL_CONNECT, "connect");
        assert_eq!(SIDL_SERVER_SELECT, "serverselect");
        assert_eq!(SIDL_YES_NO_DIALOG, "yesnodialog");
        assert_eq!(SIDL_OK_DIALOG, "okdialog");
        assert_eq!(SIDL_CHARACTER_LIST_WND, "CharacterListWnd");
    }

    #[test]
    fn is_sidl_window_visible_returns_false_on_macos() {
        assert!(!is_sidl_window_visible(0, "connect"));
    }

    #[test]
    fn find_visible_sidl_window_returns_none_on_macos() {
        assert!(find_visible_sidl_window(0, "connect").is_none());
    }

    #[test]
    fn find_visible_child_by_sidl_returns_none_on_macos() {
        assert!(find_visible_child_by_sidl(0, "YESNO_YesButton").is_none());
    }

    #[test]
    fn read_yesno_dialog_text_returns_none_on_macos() {
        assert!(read_yesno_dialog_text(0).is_none());
    }

    #[test]
    fn click_yesno_yes_returns_false_on_macos() {
        assert!(!click_yesno_yes(0));
    }

    #[test]
    fn click_yesno_no_returns_false_on_macos() {
        assert!(!click_yesno_no(0));
    }

    #[test]
    fn click_ok_dialog_returns_false_on_macos() {
        assert!(!click_ok_dialog(0));
    }
}
