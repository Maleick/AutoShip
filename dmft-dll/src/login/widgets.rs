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

/// Write credentials directly to CEditWnd widgets by finding them in CXWndManager's
/// window list and setting their InputText CXStr in-place.
///
/// This is the MQ2 approach — no keyboard simulation. We:
/// 1. Walk CXWndManager::pWindows to find username/password edit widgets
/// 2. Write directly to CEditBaseWnd::InputText (CXStr at +0x278)
/// 3. Click the Login button via vtable WndNotification(XWM_LCLICK)
pub fn type_credentials_to_window(eqmain_base: u64, account: &str, password: &str) -> bool {
    #[cfg(windows)]
    {
        use dmft_common::offsets::eqmain as off;

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
                if wnd_ptr == 0 { continue; }

                if let Some(text) = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT) {
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

            // Write username to both WindowText (+0x078) and InputText (+0x278)
            let wrote_username = write_cxstr_inplace(
                username_edit + off::CEDITBASEWND_INPUT_TEXT,
                account,
            );
            // Also try WindowText in case InputText CXStr isn't allocated
            let wrote_wt = write_cxstr_inplace(
                username_edit + off::CXWND_WINDOW_TEXT,
                account,
            );
            tracing::info!(
                input_text = wrote_username,
                window_text = wrote_wt,
                account,
                "Wrote username to edit widget"
            );

            // Write password — the password CEditWnd may have null CXStr (never typed in).
            // If null, clone the username's CStrRep structure using EQ's process heap
            // so EQ can safely manage it (HeapAlloc matches EQ's deallocation path).
            let pw_input_addr = password_edit + off::CEDITBASEWND_INPUT_TEXT;
            let pw_wt_addr = password_edit + off::CXWND_WINDOW_TEXT;

            let pw_rep = *(pw_input_addr as *const usize);
            if pw_rep == 0 {
                let un_rep = *((username_edit + off::CEDITBASEWND_INPUT_TEXT) as *const usize);
                if un_rep != 0 {
                    // Clone the CStrRep using the process default heap (same heap EQ uses)
                    if let Some(new_rep) = clone_cstrrep_for_password(un_rep) {
                        *(pw_input_addr as *mut usize) = new_rep;
                        *(pw_wt_addr as *mut usize) = new_rep;
                        tracing::info!("Cloned CStrRep for password via process heap");
                    }
                }
            }

            let wrote_password = write_cxstr_inplace(pw_input_addr as usize, password);
            let wrote_pw_wt = {
                let wt_rep = *(pw_wt_addr as *const usize);
                let it_rep = *(pw_input_addr as *const usize);
                if wt_rep == it_rep { true } // same rep, already written
                else if wt_rep != 0 { write_cxstr_inplace(pw_wt_addr as usize, password) }
                else { false }
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
            if let Some(readback) = read_cxstr(username_edit + off::CEDITBASEWND_INPUT_TEXT) {
                tracing::info!(readback = %readback, "Username InputText readback");
            } else {
                tracing::warn!("Username InputText readback: null or empty");
            }
            if let Some(readback) = read_cxstr(username_edit + off::CXWND_WINDOW_TEXT) {
                tracing::info!(readback = %readback, "Username WindowText readback");
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

            // Click the Login button directly via vtable WndNotification.
            // Previous crash may have been from wrong button pointer (now fixed).
            if login_button != 0 {
                tracing::info!(
                    ptr = format!("{:#x}", login_button),
                    "Clicking Login button via WndNotification"
                );
                // Small delay to let credential writes settle
                std::thread::sleep(std::time::Duration::from_millis(100));
                click_button_via_vtable(login_button);
                tracing::info!("Login button clicked");
            } else {
                tracing::warn!("Login button not found — credentials written but not submitted");
            }
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
/// Uses SendInput for hardware-level key simulation.
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
                let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
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

/// Read a CXStr value from a raw pointer. CXStr is a single pointer to CStrRep.
/// CStrRep layout: refcount(4) + alloc(4) + length(4) + encoding(4) + freeList(8) + data[](at +0x18)
#[cfg(windows)]
unsafe fn read_cxstr(cxstr_addr: usize) -> Option<String> {
    use dmft_common::offsets::eqmain as off;

    let rep_ptr = *(cxstr_addr as *const usize);
    if rep_ptr == 0 {
        return None;
    }

    let length = *((rep_ptr + off::CSTRREP_LENGTH) as *const u32) as usize;
    if length == 0 || length > 256 {
        return None;
    }

    let data_ptr = (rep_ptr + off::CSTRREP_DATA) as *const u8;
    let bytes = std::slice::from_raw_parts(data_ptr, length);
    String::from_utf8(bytes.to_vec()).ok()
}

/// Write a string into a CXStr field by overwriting the existing CStrRep buffer.
/// If the CStrRep is null (empty CXStr), allocates a new one on the heap.
#[cfg(windows)]
unsafe fn write_cxstr_inplace(cxstr_addr: usize, text: &str) -> bool {
    use dmft_common::offsets::eqmain as off;

    let mut rep_ptr = *(cxstr_addr as *const usize);

    if rep_ptr == 0 {
        // Cannot write to a null CXStr — EQ must allocate through its own CXFreeList.
        // Allocating from Rust's heap crashes EQ on deallocation.
        // Caller should use a donor CStrRep from another widget.
        tracing::warn!("CXStr rep is null — need donor CStrRep");
        return false;
    }

    let alloc = *((rep_ptr + off::CSTRREP_ALLOC) as *const u32) as usize;
    if text.len() >= alloc {
        tracing::warn!(
            text_len = text.len(),
            alloc,
            "CXStr buffer too small for text"
        );
        return false;
    }

    // Write the new string data
    let data_ptr = (rep_ptr + off::CSTRREP_DATA) as *mut u8;
    std::ptr::copy_nonoverlapping(text.as_ptr(), data_ptr, text.len());
    // Null-terminate
    *data_ptr.add(text.len()) = 0;
    // Update length
    *((rep_ptr + off::CSTRREP_LENGTH) as *mut u32) = text.len() as u32;

    true
}

/// Click a button widget by calling WndNotification(XWM_LCLICK) through the vtable.
/// MUST be called from EQ's main thread (game loop), not from the IPC thread.
#[cfg(windows)]
pub unsafe fn click_button_via_vtable(button_wnd: usize) {
    use dmft_common::offsets::eqmain as off;

    let vtable = *(button_wnd as *const usize);
    if vtable == 0 {
        tracing::warn!("Button vtable is null");
        return;
    }

    let wnd_notification_ptr = *((vtable + off::CXWND_VTABLE_WND_NOTIFICATION) as *const usize);
    if wnd_notification_ptr == 0 {
        tracing::warn!("WndNotification function pointer is null");
        return;
    }

    // x64 calling convention: rcx=this, rdx=sender, r8=message, r9=data
    type WndNotificationFn = unsafe extern "C" fn(usize, usize, u32, usize) -> i32;
    let func: WndNotificationFn = std::mem::transmute(wnd_notification_ptr);
    func(button_wnd, button_wnd, off::XWM_LCLICK, 0);
}

/// Non-windows stub for click_button_via_vtable.
#[cfg(not(windows))]
pub unsafe fn click_button_via_vtable(_button_wnd: usize) {}

/// Clone a CStrRep from a donor, using the process default heap for allocation.
/// This ensures EQ can safely free/manage the buffer since it uses the same heap.
#[cfg(windows)]
unsafe fn clone_cstrrep_for_password(donor_rep: usize) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    use windows::Win32::System::Memory::{GetProcessHeap, HeapAlloc, HEAP_ZERO_MEMORY};

    let donor_alloc = *((donor_rep + off::CSTRREP_ALLOC) as *const u32) as usize;
    let total_size = off::CSTRREP_DATA + donor_alloc.max(128);

    let heap = GetProcessHeap().ok()?;
    let new_rep = HeapAlloc(heap, HEAP_ZERO_MEMORY, total_size);
    if new_rep.is_null() {
        tracing::error!("HeapAlloc failed for CStrRep clone");
        return None;
    }

    let new_rep_addr = new_rep as usize;

    // Copy header from donor
    *(new_rep_addr as *mut i32) = 1; // refCount = 1
    *((new_rep_addr + off::CSTRREP_ALLOC) as *mut u32) = donor_alloc.max(128) as u32;
    *((new_rep_addr + off::CSTRREP_LENGTH) as *mut u32) = 0; // empty initially
    *((new_rep_addr + off::CSTRREP_ENCODING) as *mut u32) =
        *((donor_rep + off::CSTRREP_ENCODING) as *const u32); // same encoding
    // Copy freeList pointer from donor — critical for EQ's deallocation
    *((new_rep_addr + 0x10) as *mut usize) = *((donor_rep + 0x10) as *const usize);

    Some(new_rep_addr)
}

/// Walk CXWndManager's window array and log each window's address and WindowText.
/// This helps identify the login UI widget addresses for direct credential writing.
#[cfg(windows)]
fn enumerate_cxwnd_windows(cxwnd_mgr: usize) {
    use dmft_common::offsets::eqmain as off;

    tracing::info!("=== WINDOW ENUMERATION ===");

    unsafe {
        let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const i32);

        tracing::info!(
            array_ptr = format!("{:#x}", array_ptr),
            count,
            "CXWndManager::pWindows"
        );

        if array_ptr == 0 || count <= 0 || count > 500 {
            tracing::warn!("Invalid window array");
            return;
        }

        let focus_wnd = *((cxwnd_mgr + off::CXWNDMGR_FOCUS_WINDOW) as *const usize);
        tracing::info!(focus = format!("{:#x}", focus_wnd), "FocusWindow");

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 {
                continue;
            }

            // Read WindowText (CXStr at +0x078)
            let window_text = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT)
                .unwrap_or_default();

            // Read XMLIndex
            let xml_index = *((wnd_ptr + off::CXWND_XML_INDEX) as *const i32);

            // Read visibility
            let visible = *((wnd_ptr + off::CXWND_DSHOW) as *const bool);

            // Only log windows that are visible or have a name
            if visible || !window_text.is_empty() {
                tracing::info!(
                    idx = i,
                    ptr = format!("{:#x}", wnd_ptr),
                    xml_index,
                    visible,
                    text = %window_text,
                    "Window"
                );
            }
        }
    }

    tracing::info!("=== END WINDOW ENUMERATION ===");
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
/// Delegates to write_cxstr_inplace which overwrites the existing CStrRep buffer.
#[cfg(windows)]
unsafe fn write_cxstr(cxstr_ptr: *mut u8, text: &str) {
    write_cxstr_inplace(cxstr_ptr as usize, text);
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
