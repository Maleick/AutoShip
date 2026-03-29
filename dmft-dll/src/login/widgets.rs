//! UI widget manipulation helpers for EQ's SIDL-based UI system.
//!
//! These functions find windows by XML name, set text fields, click buttons,
//! and read list items — used by the login FSM to drive the login UI.
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

/// Set text in a CEditWnd (username/password fields).
pub fn set_edit_text(eqmain_base: u64, window_name: &str, text: &str) -> bool {
    #[cfg(windows)]
    {
        let Some(edit_wnd) = find_window_by_name(eqmain_base, window_name) else {
            return false;
        };

        unsafe {
            // CEditBaseWnd::InputText is a CXStr at offset 0x278
            let input_text_ptr = edit_wnd.add(dmft_common::offsets::eqmain::CEDITBASEWND_INPUT_TEXT);
            write_cxstr(input_text_ptr, text);
        }

        tracing::debug!(window = window_name, "Set edit text");
        true
    }

    #[cfg(not(windows))]
    {
        let _ = (eqmain_base, window_name, text);
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

        // Resolve JoinServer function address
        let Some(join_server_addr) = eqmain_offsets::rebase(eqmain_offsets::JOIN_SERVER, eqmain_base) else {
            tracing::warn!("Failed to rebase JoinServer address");
            return false;
        };

        // TODO: Find server ID by iterating LoginClient::ServerList at offset 0x178
        // For now, we need to match server_name to a server ID.
        // This requires reading the server list from LoginServerAPI.
        // Placeholder: attempt server ID 0 (will be replaced with actual lookup)
        let _ = (login_api, join_server_addr, server_name);
        tracing::info!(server = server_name, "JoinServer lookup not yet implemented — will resolve on live client");
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
    fn widget_names_are_consistent() {
        assert_eq!(LOGIN_USERNAME_EDIT, "LOGIN_UsernameEdit");
        assert_eq!(LOGIN_PASSWORD_EDIT, "LOGIN_PasswordEdit");
        assert_eq!(LOGIN_CONNECT_BUTTON, "LOGIN_ConnectButton");
        assert_eq!(SERVERSELECT_SERVER_LIST, "SERVERSELECT_ServerList");
        assert_eq!(CHARACTER_LIST, "Character_List");
    }
}
