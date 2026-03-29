//! DLL-side login automation — drives EQ's login UI autonomously.
//!
//! The orchestrator sends credentials once via `StartLogin`; the FSM handles
//! all UI steps (credential entry, server select, character select, enter world)
//! and reports progress back via `LoginPhaseUpdate` IPC responses.

pub mod eqmain;
pub mod widgets;

use std::sync::Mutex;
use std::time::Instant;

use dmft_common::login::{LoginError, LoginPhase};

/// Global login FSM instance, one per injected DLL.
static LOGIN_FSM: Mutex<Option<LoginFsm>> = Mutex::new(None);

/// Initialize the login FSM. Called once during DLL setup.
pub fn init() {
    let mut guard = LOGIN_FSM.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(LoginFsm::new());
    tracing::info!("Login FSM initialized");
}

/// Run one login tick. Call from `on_game_tick()` when local_player is None.
pub fn tick() -> Option<LoginPhase> {
    let mut guard = LOGIN_FSM.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(fsm) = guard.as_mut() {
        fsm.tick()
    } else {
        None
    }
}

/// Store credentials received from the orchestrator's StartLogin command.
pub fn start_login(
    account_name: String,
    password: String,
    server_name: String,
    character_name: String,
) {
    let mut guard = LOGIN_FSM.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(fsm) = guard.as_mut() {
        fsm.store_credentials(account_name, password, server_name, character_name);
    } else {
        // Auto-init if not yet created
        let mut fsm = LoginFsm::new();
        fsm.store_credentials(account_name, password, server_name, character_name);
        *guard = Some(fsm);
    }
}

/// Get current login phase for status queries.
pub fn phase() -> LoginPhase {
    let guard = LOGIN_FSM.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|fsm| fsm.phase.clone())
        .unwrap_or(LoginPhase::NotStarted)
}

/// Internal states for the login FSM — more granular than the IPC-facing LoginPhase.
#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    WaitForLoginScreen,
    EnteringCredentials,
    WaitForServerSelect,
    SelectingServer,
    WaitForCharSelect,
    SelectingCharacter,
    WaitForWorld,
    InWorld,
    Error(LoginError),
}

/// Credentials stored temporarily in memory, zeroized after use.
struct Credentials {
    account_name: String,
    password: String,
    server_name: String,
    character_name: String,
}

impl Drop for Credentials {
    fn drop(&mut self) {
        // Zeroize password bytes in place
        // Safety: we're overwriting the String's buffer before it's freed
        unsafe {
            let bytes = self.password.as_bytes_mut();
            for b in bytes.iter_mut() {
                *b = 0;
            }
        }
    }
}

/// The login state machine. Drives EQ's login UI from credential entry to in-world.
pub struct LoginFsm {
    state: State,
    /// Current IPC-facing phase (sent to orchestrator).
    phase: LoginPhase,
    /// Stored credentials — zeroized after credential entry.
    credentials: Option<Credentials>,
    /// Server name — kept after credential zeroization for server selection.
    server_name: String,
    /// Character name — kept after credential zeroization for character selection.
    character_name: String,
    /// Timestamp when we entered the current state (for timeout detection).
    state_entered_at: Instant,
    /// Number of retry attempts for the current phase.
    retries: u32,
    /// Maximum retries before giving up.
    max_retries: u32,
    /// Timeout per state in seconds.
    state_timeout_secs: u64,
    /// Cached eqmain.dll base address (0 = not resolved yet).
    eqmain_base: u64,
}

impl LoginFsm {
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            phase: LoginPhase::NotStarted,
            credentials: None,
            server_name: String::new(),
            character_name: String::new(),
            state_entered_at: Instant::now(),
            retries: 0,
            max_retries: 3,
            state_timeout_secs: 60,
            eqmain_base: 0,
        }
    }

    /// Store credentials and begin the login sequence.
    pub fn store_credentials(
        &mut self,
        account_name: String,
        password: String,
        server_name: String,
        character_name: String,
    ) {
        tracing::info!(
            account = %account_name,
            server = %server_name,
            character = %character_name,
            "Login credentials received (password redacted)"
        );
        // Keep server/character names separately — they're needed after password zeroization
        self.server_name = server_name.clone();
        self.character_name = character_name.clone();
        self.credentials = Some(Credentials {
            account_name,
            password,
            server_name,
            character_name,
        });
        self.transition(State::WaitForLoginScreen);
    }

    /// Advance the FSM by one tick. Returns Some(phase) when the phase changes.
    pub fn tick(&mut self) -> Option<LoginPhase> {
        let prev_phase = self.phase.clone();

        match &self.state {
            State::Idle | State::InWorld | State::Error(_) => return None,
            _ => {}
        }

        // Check for timeout
        if self.state_entered_at.elapsed().as_secs() > self.state_timeout_secs {
            self.handle_timeout();
            return self.phase_if_changed(&prev_phase);
        }

        // Ensure eqmain.dll base is resolved
        if self.eqmain_base == 0 {
            self.eqmain_base = eqmain::find_eqmain();
            if self.eqmain_base == 0 {
                return None; // eqmain.dll not loaded yet
            }
            tracing::info!(base = format!("{:#x}", self.eqmain_base), "eqmain.dll resolved");
        }

        match self.state.clone() {
            State::WaitForLoginScreen => self.tick_wait_for_login_screen(),
            State::EnteringCredentials => self.tick_entering_credentials(),
            State::WaitForServerSelect => self.tick_wait_for_server_select(),
            State::SelectingServer => self.tick_selecting_server(),
            State::WaitForCharSelect => self.tick_wait_for_char_select(),
            State::SelectingCharacter => self.tick_selecting_character(),
            State::WaitForWorld => self.tick_wait_for_world(),
            _ => {}
        }

        self.phase_if_changed(&prev_phase)
    }

    fn tick_wait_for_login_screen(&mut self) {
        // Dismiss splash screens if present
        widgets::dismiss_splash(self.eqmain_base);

        // Check if login screen is visible (LOGIN_ConnectButton exists and visible)
        if widgets::is_window_visible(self.eqmain_base, "LOGIN_ConnectButton") {
            tracing::info!("Login screen detected");
            self.transition(State::EnteringCredentials);
        }
    }

    fn tick_entering_credentials(&mut self) {
        let Some(creds) = self.credentials.as_ref() else {
            self.transition(State::Error(LoginError::Timeout {
                phase: "EnteringCredentials (no credentials)".into(),
            }));
            return;
        };

        let account = creds.account_name.clone();
        let password = creds.password.clone();

        // Set username field
        if !widgets::set_edit_text(self.eqmain_base, "LOGIN_UsernameEdit", &account) {
            tracing::warn!("Failed to set username field");
            return;
        }

        // Set password field
        if !widgets::set_edit_text(self.eqmain_base, "LOGIN_PasswordEdit", &password) {
            tracing::warn!("Failed to set password field");
            return;
        }

        // Click the connect button
        if !widgets::click_button(self.eqmain_base, "LOGIN_ConnectButton") {
            tracing::warn!("Failed to click connect button");
            return;
        }

        tracing::info!(account = %account, "Credentials entered, clicking connect");

        // Zeroize credentials — drop the Credentials struct which zeros the password
        self.credentials = None;

        self.transition(State::WaitForServerSelect);
    }

    fn tick_wait_for_server_select(&mut self) {
        // Check for error dialogs (wrong password, account locked, etc.)
        if let Some(error) = widgets::check_error_dialog(self.eqmain_base) {
            self.transition(State::Error(error));
            return;
        }

        // Check if server list is visible
        if widgets::is_window_visible(self.eqmain_base, "SERVERSELECT_ServerList") {
            tracing::info!("Server select screen detected");
            self.transition(State::SelectingServer);
        }
    }

    fn tick_selecting_server(&mut self) {
        if self.server_name.is_empty() {
            tracing::error!("No server name available for server selection");
            self.transition(State::Error(LoginError::Timeout {
                phase: "SelectingServer (no server name)".into(),
            }));
            return;
        }

        let server_name = self.server_name.clone();
        self.do_select_server(&server_name);
    }

    fn do_select_server(&mut self, server_name: &str) {
        // Use JoinServer API directly (bypasses UI list)
        if widgets::join_server(self.eqmain_base, server_name) {
            tracing::info!(server = %server_name, "Server join requested");
            self.transition(State::WaitForCharSelect);
        } else {
            tracing::warn!(server = %server_name, "Failed to join server — retrying next tick");
        }
    }

    fn tick_wait_for_char_select(&mut self) {
        // Check for error dialogs
        if let Some(error) = widgets::check_error_dialog(self.eqmain_base) {
            self.transition(State::Error(error));
            return;
        }

        // Check for "already logged in" dialog
        if widgets::is_window_visible(self.eqmain_base, "yesnodialog") {
            tracing::info!("'Already logged in' dialog detected — clicking Yes");
            widgets::click_button(self.eqmain_base, "yesnodialog");
            return;
        }

        // Check if character list is visible
        if widgets::is_window_visible(self.eqmain_base, "Character_List") {
            tracing::info!("Character select screen detected");
            self.transition(State::SelectingCharacter);
        }
    }

    fn tick_selecting_character(&mut self) {
        if self.character_name.is_empty() {
            tracing::error!("No character name available for character selection");
            self.transition(State::Error(LoginError::CharacterNotFound {
                expected: String::new(),
                found: String::new(),
            }));
            return;
        }

        let char_name = self.character_name.clone();
        self.do_select_character(&char_name);
    }

    fn do_select_character(&mut self, character_name: &str) {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);

        if widgets::select_character(self.eqmain_base, eq_base, character_name) {
            tracing::info!(character = %character_name, "Character selected, entering world");
            self.transition(State::WaitForWorld);
        } else {
            tracing::warn!(
                character = %character_name,
                "Character not found in list"
            );
            self.transition(State::Error(LoginError::CharacterNotFound {
                expected: character_name.to_string(),
                found: String::new(),
            }));
        }
    }

    fn tick_wait_for_world(&mut self) {
        // Check if local player pointer is non-null (= in world)
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return;
        }

        if let Some(player_ptr_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::PINST_LOCAL_PLAYER,
            eq_base,
        ) {
            let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
            if player_ptr != 0 {
                tracing::info!("Local player detected — login complete!");
                self.transition(State::InWorld);
            }
        }
    }

    fn handle_timeout(&mut self) {
        let phase_name = format!("{:?}", self.state);
        if self.retries < self.max_retries {
            self.retries += 1;
            tracing::warn!(
                phase = %phase_name,
                retry = self.retries,
                "Login phase timed out, retrying"
            );
            // Reset the timer for the retry
            self.state_entered_at = Instant::now();
        } else {
            tracing::error!(
                phase = %phase_name,
                "Login phase timed out after {} retries",
                self.max_retries
            );
            self.transition(State::Error(LoginError::Timeout { phase: phase_name }));
        }
    }

    fn transition(&mut self, new_state: State) {
        tracing::info!(from = ?self.state, to = ?new_state, "Login FSM transition");
        self.state = new_state;
        self.state_entered_at = Instant::now();
        self.retries = 0;

        // Update IPC-facing phase
        self.phase = match &self.state {
            State::Idle => LoginPhase::NotStarted,
            State::WaitForLoginScreen => LoginPhase::AtLoginScreen,
            State::EnteringCredentials => LoginPhase::EnteringCredentials,
            State::WaitForServerSelect | State::SelectingServer => LoginPhase::ServerSelecting,
            State::WaitForCharSelect | State::SelectingCharacter => LoginPhase::CharacterSelecting,
            State::WaitForWorld => LoginPhase::Zoning,
            State::InWorld => LoginPhase::InWorld,
            State::Error(e) => LoginPhase::Failed {
                reason: e.clone(),
            },
        };
    }

    fn phase_if_changed(&self, prev: &LoginPhase) -> Option<LoginPhase> {
        // Use Debug format for comparison since LoginPhase doesn't impl PartialEq
        let prev_dbg = format!("{:?}", prev);
        let curr_dbg = format!("{:?}", self.phase);
        if prev_dbg != curr_dbg {
            Some(self.phase.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_fsm_is_idle() {
        let fsm = LoginFsm::new();
        assert_eq!(fsm.state, State::Idle);
        assert!(matches!(fsm.phase, LoginPhase::NotStarted));
    }

    #[test]
    fn test_store_credentials_transitions_to_wait_for_login() {
        let mut fsm = LoginFsm::new();
        fsm.store_credentials(
            "testaccount".into(),
            "testpass".into(),
            "TestServer".into(),
            "TestChar".into(),
        );
        assert_eq!(fsm.state, State::WaitForLoginScreen);
        assert!(matches!(fsm.phase, LoginPhase::AtLoginScreen));
        assert!(fsm.credentials.is_some());
    }

    #[test]
    fn test_idle_tick_returns_none() {
        let mut fsm = LoginFsm::new();
        assert!(fsm.tick().is_none());
    }

    #[test]
    fn test_error_state_tick_returns_none() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::Error(LoginError::WrongPassword));
        assert!(fsm.tick().is_none());
    }

    #[test]
    fn test_in_world_tick_returns_none() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::InWorld);
        assert!(fsm.tick().is_none());
    }

    #[test]
    fn test_transition_updates_phase() {
        let mut fsm = LoginFsm::new();

        fsm.transition(State::WaitForLoginScreen);
        assert!(matches!(fsm.phase, LoginPhase::AtLoginScreen));

        fsm.transition(State::EnteringCredentials);
        assert!(matches!(fsm.phase, LoginPhase::EnteringCredentials));

        fsm.transition(State::WaitForServerSelect);
        assert!(matches!(fsm.phase, LoginPhase::ServerSelecting));

        fsm.transition(State::SelectingServer);
        assert!(matches!(fsm.phase, LoginPhase::ServerSelecting));

        fsm.transition(State::WaitForCharSelect);
        assert!(matches!(fsm.phase, LoginPhase::CharacterSelecting));

        fsm.transition(State::SelectingCharacter);
        assert!(matches!(fsm.phase, LoginPhase::CharacterSelecting));

        fsm.transition(State::WaitForWorld);
        assert!(matches!(fsm.phase, LoginPhase::Zoning));

        fsm.transition(State::InWorld);
        assert!(matches!(fsm.phase, LoginPhase::InWorld));

        fsm.transition(State::Error(LoginError::AccountLocked));
        assert!(matches!(fsm.phase, LoginPhase::Failed { .. }));
    }

    #[test]
    fn test_happy_path_state_sequence() {
        let mut fsm = LoginFsm::new();

        // Start: Idle
        assert_eq!(fsm.state, State::Idle);

        // Receive credentials → WaitForLoginScreen
        fsm.store_credentials(
            "account".into(),
            "pass".into(),
            "Server".into(),
            "Char".into(),
        );
        assert_eq!(fsm.state, State::WaitForLoginScreen);

        // Simulate progression through each state
        fsm.transition(State::EnteringCredentials);
        assert_eq!(fsm.state, State::EnteringCredentials);

        fsm.transition(State::WaitForServerSelect);
        assert_eq!(fsm.state, State::WaitForServerSelect);

        fsm.transition(State::SelectingServer);
        assert_eq!(fsm.state, State::SelectingServer);

        fsm.transition(State::WaitForCharSelect);
        assert_eq!(fsm.state, State::WaitForCharSelect);

        fsm.transition(State::SelectingCharacter);
        assert_eq!(fsm.state, State::SelectingCharacter);

        fsm.transition(State::WaitForWorld);
        assert_eq!(fsm.state, State::WaitForWorld);

        fsm.transition(State::InWorld);
        assert_eq!(fsm.state, State::InWorld);
        assert!(matches!(fsm.phase, LoginPhase::InWorld));
    }

    #[test]
    fn test_timeout_retries_then_errors() {
        let mut fsm = LoginFsm::new();
        fsm.max_retries = 2;
        fsm.transition(State::WaitForLoginScreen);

        // First timeout — retry
        fsm.handle_timeout();
        assert_eq!(fsm.state, State::WaitForLoginScreen);
        assert_eq!(fsm.retries, 1);

        // Second timeout — retry
        fsm.handle_timeout();
        assert_eq!(fsm.state, State::WaitForLoginScreen);
        assert_eq!(fsm.retries, 2);

        // Third timeout — error
        fsm.handle_timeout();
        assert!(matches!(fsm.state, State::Error(LoginError::Timeout { .. })));
    }

    #[test]
    fn test_credentials_password_zeroized_on_drop() {
        let creds = Credentials {
            account_name: "test".into(),
            password: "secret123".into(),
            server_name: "srv".into(),
            character_name: "chr".into(),
        };

        drop(creds);

        // Verify the Drop impl runs without panicking.
        // We can't safely read freed memory to verify zeroing,
        // but the Drop impl zeros the password buffer before deallocation.
    }

    #[test]
    fn test_phase_if_changed_detects_changes() {
        let mut fsm = LoginFsm::new();
        let prev = fsm.phase.clone();

        // No change
        assert!(fsm.phase_if_changed(&prev).is_none());

        // Change
        fsm.transition(State::WaitForLoginScreen);
        assert!(fsm.phase_if_changed(&prev).is_some());
    }
}
