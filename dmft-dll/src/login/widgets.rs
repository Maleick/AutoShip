//! UI widget manipulation helpers for EQ's login system.
//!
//! Uses direct memory writes to EQLogin's fixed char arrays for credential entry,
//! bypassing CXStr/SIDL widget navigation entirely. For other UI interactions
//! (splash dismiss, error dialogs), falls back to SIDL window lookup.
//!
//! All functions are no-ops on non-Windows platforms.

use dmft_common::login::LoginError;

// ─── Widget XML names (stable across EQ patches) ───

pub const LOGIN_USERNAME_EDIT: &str = "LOGIN_UsernameEdit";
pub const LOGIN_PASSWORD_EDIT: &str = "LOGIN_PasswordEdit";
pub const LOGIN_CONNECT_BUTTON: &str = "LOGIN_ConnectButton";
pub const SERVERSELECT_SERVER_LIST: &str = "SERVERSELECT_ServerList";
pub const CHARACTER_LIST: &str = "Character_List";
pub const OK_DIALOG: &str = "okdialog";
pub const YES_NO_DIALOG: &str = "yesnodialog";
pub const DBG_SPLASH: &str = "dbgsplash";
pub const SOE_SPLASH: &str = "soesplash";

/// Check if a named window is visible in the UI.
pub fn is_window_visible(eqmain_base: u64, window_name: &str) -> bool {
    #[cfg(windows)]
    {
        let Some(wnd) = find_window_by_name(eqmain_base, window_name) else {
            return false;
        };
        // CXWnd visibility flag is at a known offset in the vtable/struct.
        // For now, a non-null window pointer means it exists in the SIDL tree.
        // TODO: Check CXWnd::IsVisible() or dShow flag at runtime.
        !wnd.is_null()
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, window_name);
        false
    }
}

/// Find a SIDL window by its XML name.
/// Returns a raw pointer to the CXWnd, or None if not found.
#[cfg(windows)]
fn find_window_by_name(eqmain_base: u64, name: &str) -> Option<*mut u8> {
    use super::eqmain;

    let sidl_mgr = eqmain::resolve_sidl_manager(eqmain_base)?;

    // CSidlManager maintains a hash map of window name → CXWnd*.
    // We need to walk this to find windows by name.
    // For the initial implementation, we use CSidlManager::FindScreenPieceTemplate
    // or iterate the window list.
    //
    // TODO: Implement CSidlManager window lookup once we validate the struct layout
    // on the live client. For now, return None to let the FSM retry on next tick.
    let _ = (sidl_mgr, name);
    tracing::trace!(name, "Window lookup not yet implemented — will resolve on live client");
    None
}

/// Write login credentials directly to EQLogin's fixed char arrays.
///
/// This bypasses SIDL widget navigation and CXStr entirely — EQLogin has
/// plain `char[0x80]` arrays for Login and PW that we can write directly.
///
/// Path: eqmain_base → pinstLoginClient → deref → LoginClient
///       → +0x010 (pLoginData) → deref → EQLogin → write Login/PW.
pub fn write_login_credentials(eqmain_base: u64, account: &str, password: &str) -> bool {
    #[cfg(windows)]
    {
        use dmft_common::offsets::eqmain as eqmain_offsets;
        use super::eqmain;

        let Some(eqlogin) = eqmain::resolve_eqlogin(eqmain_base) else {
            tracing::warn!("Cannot write credentials — EQLogin not resolved");
            return false;
        };

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
            account = %account,
            eqlogin = format!("{:#x}", eqlogin),
            "Wrote credentials to EQLogin char arrays (password redacted)"
        );
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, account, password);
        false
    }
}

/// Set text in a CEditWnd (username/password fields).
///
/// For login fields (LOGIN_UsernameEdit, LOGIN_PasswordEdit), this uses
/// direct memory writes to EQLogin's char arrays instead of CXStr manipulation.
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
            let input_text_ptr = edit_wnd.add(dmft_common::offsets::eqmain::CEDITBASEWND_INPUT_TEXT);
            write_cxstr(input_text_ptr, text);
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

/// Type text into the EQ login window using SendInput (hardware-level keyboard simulation).
///
/// SendInput injects keystrokes at the OS level — the foreground window receives them
/// exactly as if the user typed them. This works with EQ's custom CXWnd/CSidlWnd UI
/// where PostMessageW(WM_CHAR) does not reach the focused edit widget.
///
/// Sequence: SetForegroundWindow → clear field → type username → Tab → clear → type password.
pub fn type_credentials_to_window(eqmain_base: u64, account: &str, password: &str) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_UNICODE, KEYEVENTF_KEYUP,
            VK_BACK, VK_TAB, VK_RETURN,
        };
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        use windows::Win32::Foundation::HWND;

        let Some(hwnd_val) = super::eqmain::resolve_eq_hwnd(eqmain_base) else {
            tracing::warn!("Cannot type credentials — EQ HWND not resolved");
            return false;
        };

        let hwnd = HWND(hwnd_val as isize);

        unsafe {
            // Bring EQ window to foreground so SendInput targets it
            let _ = SetForegroundWindow(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Helper: send a virtual key press (down + up)
            let send_vk = |vk: u16| {
                let down = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                            wScan: 0,
                            dwFlags: Default::default(),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                let up = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32);
                std::thread::sleep(std::time::Duration::from_millis(5));
            };

            // Helper: send a unicode character via SendInput
            let send_char = |ch: u16| {
                let down = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                let up = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32);
                std::thread::sleep(std::time::Duration::from_millis(10));
            };

            // Clear username field with backspaces
            for _ in 0..128 {
                send_vk(VK_BACK.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Type account name
            for ch in account.encode_utf16() {
                send_char(ch);
            }
            tracing::info!("Typed account name ({} chars) via SendInput", account.len());

            // Tab to password field
            std::thread::sleep(std::time::Duration::from_millis(100));
            send_vk(VK_TAB.0);
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Clear password field with backspaces
            for _ in 0..128 {
                send_vk(VK_BACK.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Type password
            for ch in password.encode_utf16() {
                send_char(ch);
            }
            tracing::info!("Typed password ({} chars) via SendInput", password.len());
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
/// Uses SendInput for hardware-level key simulation (matches type_credentials_to_window).
pub fn simulate_enter_key(eqmain_base: u64) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VK_RETURN,
        };

        let Some(hwnd_val) = super::eqmain::resolve_eq_hwnd(eqmain_base) else {
            tracing::warn!("Cannot simulate Enter — EQ HWND not resolved");
            return false;
        };

        unsafe {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0x1C,
                        dwFlags: Default::default(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0x1C,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32);
        }

        tracing::debug!(hwnd = format!("{:#x}", hwnd_val), "Simulated Enter key via SendInput");
        true
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        false
    }
}

/// Click a button widget by sending XWM_LCLICK notification.
pub fn click_button(eqmain_base: u64, window_name: &str) -> bool {
    #[cfg(windows)]
    {
        let Some(button_wnd) = find_window_by_name(eqmain_base, window_name) else {
            return false;
        };

        unsafe {
            // Read vtable pointer
            let vftable = *(button_wnd as *const *const usize);
            // WndNotification is typically at vtable index ~30-40 (varies by class).
            // TODO: Validate exact vtable index on live client.
            // For now, use a placeholder index that will be calibrated.
            const WNDNOTIFICATION_VFUNC_INDEX: usize = 34;
            let wnd_notification_addr = *vftable.add(WNDNOTIFICATION_VFUNC_INDEX);

            type WndNotificationFn =
                unsafe extern "C" fn(*mut u8, *mut u8, u32, *mut u8);
            let func: WndNotificationFn = std::mem::transmute(wnd_notification_addr);
            func(
                button_wnd,
                button_wnd,
                dmft_common::offsets::eqmain::XWM_LCLICK,
                std::ptr::null_mut(),
            );
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

/// Dismiss splash screens (dbgsplash, soesplash) if visible.
pub fn dismiss_splash(eqmain_base: u64) {
    #[cfg(windows)]
    {
        for name in &[DBG_SPLASH, SOE_SPLASH] {
            if is_window_visible(eqmain_base, name) {
                click_button(eqmain_base, name);
                tracing::debug!(splash = name, "Dismissed splash screen");
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
    }
}

/// Check for error dialogs (okdialog) and return the appropriate LoginError.
pub fn check_error_dialog(eqmain_base: u64) -> Option<LoginError> {
    #[cfg(windows)]
    {
        if !is_window_visible(eqmain_base, OK_DIALOG) {
            return None;
        }

        // Read the dialog text to determine error type.
        // TODO: Read CStmlWnd text content once CXStr layout is validated.
        // For now, dismiss the dialog and report a generic error.
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

/// Join a server by name using LoginServerAPI::JoinServer directly.
pub fn join_server(eqmain_base: u64, server_name: &str) -> bool {
    #[cfg(windows)]
    {
        use super::eqmain;
        use dmft_common::offsets::eqmain as eqmain_offsets;

        let Some(login_api) = eqmain::resolve_login_server_api(eqmain_base) else {
            tracing::warn!("LoginServerAPI not resolved");
            return false;
        };

        let Some(join_server_addr) = eqmain_offsets::rebase(eqmain_offsets::JOIN_SERVER, eqmain_base) else {
            tracing::warn!("Failed to rebase JoinServer address");
            return false;
        };

        // TODO: Find server ID by iterating LoginClient::ServerList at offset 0x178.
        // The ServerList is a DoublyLinkedList<EQClientServerData*>.
        // EQClientServerData has ServerName (CXStr) at offset 0x08 and ID (ServerID) at 0x00.
        // For now, log what we have and return false — need calibration dump to discover
        // the actual server ID for the target TLP.
        tracing::info!(
            server = server_name,
            login_api = format!("{:#x}", login_api),
            join_server_fn = format!("{:#x}", join_server_addr),
            "JoinServer: API resolved but server ID lookup not yet implemented. \
             Use CalibrateLogin to dump server list."
        );

        // TODO: Once we know the server ID, call:
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

/// Select a character by name and enter world.
/// Uses eqgame.exe's SelectCharacter + EnterWorld functions.
pub fn select_character(eqmain_base: u64, eq_base: u64, character_name: &str) -> bool {
    #[cfg(windows)]
    {
        // Character selection uses eqgame.exe functions, not eqmain.dll
        let Some(select_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::SELECT_CHARACTER,
            eq_base,
        ) else {
            tracing::warn!("Failed to rebase SELECT_CHARACTER");
            return false;
        };

        let Some(enter_world_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::ENTER_WORLD,
            eq_base,
        ) else {
            tracing::warn!("Failed to rebase ENTER_WORLD");
            return false;
        };

        // TODO: Find character index in Character_List CListWnd by name,
        // then call SelectCharacter(index) followed by EnterWorld().
        // Requires reading CListWnd items to match character_name.
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

/// Dump all login-related pointer addresses to the log for calibration.
/// This is called when the DLL receives a CalibrateLogin command.
pub fn calibrate_login_dump(eqmain_base: u64) {
    use super::eqmain;
    use dmft_common::offsets::eqmain as eqmain_offsets;

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
            tracing::info!(login_client_ptr = format!("{:#x}", login_client), "LoginClient*");

            if login_client != 0 {
                let eqlogin_ptr = unsafe {
                    *((login_client + eqmain_offsets::LOGINCLIENT_LOGIN_DATA) as *const usize)
                };
                tracing::info!(eqlogin_ptr = format!("{:#x}", eqlogin_ptr), "EQLogin* (pLoginData)");

                if eqlogin_ptr != 0 {
                    // Dump HWND
                    let hwnd = unsafe {
                        *((eqlogin_ptr + eqmain_offsets::EQLOGIN_HWND) as *const usize)
                    };
                    tracing::info!(hwnd = format!("{:#x}", hwnd), "EQLogin::hEQWnd");

                    // Dump username field (first 32 bytes)
                    let username_addr = (eqlogin_ptr + eqmain_offsets::EQLOGIN_USERNAME) as *const u8;
                    let username_bytes = unsafe { std::slice::from_raw_parts(username_addr, 32) };
                    let username = String::from_utf8_lossy(
                        &username_bytes[..username_bytes.iter().position(|&b| b == 0).unwrap_or(32)]
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
                    let return_code = unsafe {
                        *((eqlogin_ptr + 0x410) as *const i32)
                    };
                    tracing::info!(return_code, "EQLogin::ReturnCode");
                }
            }
        }
    }

    // LoginServerAPI
    if let Some(login_api) = eqmain::resolve_login_server_api(eqmain_base) {
        tracing::info!(login_server_api = format!("{:#x}", login_api), "LoginServerAPI*");
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
    if let Some(addr) = eqmain_offsets::rebase(eqmain_offsets::PINST_LOGIN_CONTROLLER, eqmain_base) {
        tracing::info!(
            pinst_login_controller_addr = format!("{:#x}", addr),
            "pinstLoginController address"
        );
        #[cfg(windows)]
        {
            let controller_ptr = unsafe { *(addr as *const usize) };
            tracing::info!(login_controller = format!("{:#x}", controller_ptr), "LoginController*");
        }
    }

    tracing::info!("=== END LOGIN CALIBRATION DUMP ===");
}

/// Read a list item from a CListWnd at the given row and column.
#[cfg(windows)]
#[allow(dead_code)]
pub fn read_list_item(
    _eqmain_base: u64,
    _list_wnd: *mut u8,
    _row: usize,
    _col: usize,
) -> Option<String> {
    // TODO: Implement CListWnd item reading once struct layout is validated.
    // CListWnd stores items in a nested structure that varies by EQ version.
    None
}

/// Write a Rust string into an EQ CXStr field.
/// CXStr is a pointer to CStrRep; CStrRep has UTF-8 data at offset 0x18.
#[cfg(windows)]
unsafe fn write_cxstr(cxstr_ptr: *mut u8, text: &str) {
    // CXStr layout: pointer to CStrRep, which has the string data.
    // For initial implementation, we write directly — this will need
    // refinement once CXStr allocation patterns are validated on live.
    //
    // TODO: Use EQ's CXStr allocation functions to properly create/set strings.
    // Direct memory writes risk heap corruption if the string grows beyond
    // the existing buffer. Safe approach: call CXStr::operator=() or
    // SetWindowText equivalent.
    let _ = (cxstr_ptr, text);
    tracing::trace!("CXStr write stub — will implement with validated layout");
}

#[cfg(test)]
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
}
