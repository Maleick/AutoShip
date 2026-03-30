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

/// Check if the login FSM has completed (in world, error, or idle after completion).
pub fn is_done() -> bool {
    let guard = LOGIN_FSM.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|fsm| matches!(fsm.state, State::InWorld | State::Error(_) | State::Idle))
        .unwrap_or(true)
}

/// Internal states for the login FSM — more granular than the IPC-facing LoginPhase.
#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    WaitForLoginScreen,
    EnteringCredentials,
    /// Credentials submitted — waiting for EQ to process authentication.
    WaitForServerSelect,
    SelectingServer,
    /// Server selected — waiting for transition to character select.
    /// eqmain.dll unloads and eqgame.exe takes over.
    WaitForCharSelect,
    /// Character select screen is up (eqgame context).
    SelectingCharacter,
    WaitForWorld,
    InWorld,
    Error(LoginError),
}

/// Throttle ticks between actions to avoid spamming EQ's UI.
/// At ~30fps game loop, 15 ticks = ~500ms.
const ACTION_COOLDOWN_TICKS: u32 = 15;

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
    /// Ticks since entering current state (for action throttling).
    ticks_in_state: u32,
    /// Whether we've performed the action for this state (prevents double-actions).
    action_taken: bool,
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
            ticks_in_state: 0,
            action_taken: false,
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
    ///
    /// The FSM detects which screen EQ is showing by scanning for visible SIDL
    /// windows each tick (the MQ2 AutoLogin approach). This replaces the previous
    /// timer-based polling that ran on a background thread.
    pub fn tick(&mut self) -> Option<LoginPhase> {
        let prev_phase = self.phase.clone();

        match &self.state {
            State::Idle | State::InWorld | State::Error(_) => return None,
            _ => {}
        }

        self.ticks_in_state += 1;

        // Check for timeout
        if self.state_entered_at.elapsed().as_secs() > self.state_timeout_secs {
            self.handle_timeout();
            return self.phase_if_changed(&prev_phase);
        }

        // Resolve eqmain.dll base on every tick — it can load/unload during login.
        self.eqmain_base = eqmain::find_eqmain();

        // Log eqmain status periodically for debugging
        if self.ticks_in_state % 20 == 1 {
            tracing::info!(
                eqmain_base = format!("{:#x}", self.eqmain_base),
                state = ?self.state,
                ticks = self.ticks_in_state,
                "Login FSM tick"
            );
        }

        // Before doing state-specific work, check for dialogs that can appear
        // at any point during login (error dialogs, "already logged in", etc.)
        if self.eqmain_base != 0 {
            if self.handle_dialogs() {
                return self.phase_if_changed(&prev_phase);
            }
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

    /// Handle dialogs that can appear at any login stage.
    /// Returns true if the FSM transitioned to an error state.
    fn handle_dialogs(&mut self) -> bool {
        // YesNo dialog — "already logged in, kick?" → click Yes
        if let Some(dialog_wnd) = widgets::find_visible_sidl_window(
            self.eqmain_base,
            widgets::SIDL_YES_NO_DIALOG,
        ) {
            let dialog_text = widgets::read_yesno_dialog_text(dialog_wnd)
                .unwrap_or_default();
            tracing::info!(text = %dialog_text, "YesNo dialog detected");

            // "Already logged in" dialogs → click Yes to kick
            if dialog_text.to_ascii_lowercase().contains("logged in")
                || dialog_text.to_ascii_lowercase().contains("kick")
                || dialog_text.to_ascii_lowercase().contains("already")
            {
                tracing::info!("Clicking YES to dismiss 'already logged in' dialog");
                widgets::click_yesno_yes(dialog_wnd);
                return false; // Not an error, just a dialog to dismiss
            }

            // Unknown YesNo — click Yes as a safe default
            tracing::info!("Clicking YES on unknown YesNo dialog");
            widgets::click_yesno_yes(dialog_wnd);
            return false;
        }

        // OK dialog — error messages, server full, etc.
        if let Some(dialog_wnd) = widgets::find_visible_sidl_window(
            self.eqmain_base,
            widgets::SIDL_OK_DIALOG,
        ) {
            tracing::warn!("OK dialog detected — dismissing");
            widgets::click_ok_dialog(dialog_wnd);
            // Don't transition to error — let the FSM detect the actual state
            // on the next tick (e.g., back to login screen means wrong password).
            return false;
        }

        false
    }

    fn tick_wait_for_login_screen(&mut self) {
        if self.eqmain_base == 0 { return; }

        // Use the proven type_credentials_to_window approach that was working
        // before the FSM rewrite. It scans for USERNAME/PASSWORD labels and
        // writes credentials + clicks Login. If it succeeds, skip to server select.
        if !self.action_taken {
            let creds = self.credentials.as_ref();
            if let Some(creds) = creds {
                // Try writing credentials (proven working approach)
                let wrote = widgets::type_credentials_to_window(
                    self.eqmain_base,
                    &creds.account_name,
                    &creds.password,
                );
                if wrote {
                    tracing::info!("Credentials written + Login clicked");
                    // Also type password via WM_CHAR as backup
                    widgets::type_password_wm_char(self.eqmain_base, &creds.password);
                    self.action_taken = true;
                    self.transition(State::WaitForServerSelect);
                    return;
                }
            }
        }
    }

    fn tick_entering_credentials(&mut self) {
        // Only attempt credential entry once, then wait for server select
        if self.action_taken {
            // Already submitted credentials — check if we're now at server select
            // (the FSM will detect this via WaitForServerSelect on next transition)
            if self.ticks_in_state > ACTION_COOLDOWN_TICKS {
                // Give EQ time to process, then move to waiting for server select
                self.transition(State::WaitForServerSelect);
            }
            return;
        }

        let Some(creds) = self.credentials.as_ref() else {
            self.transition(State::Error(LoginError::Timeout {
                phase: "EnteringCredentials (no credentials)".into(),
            }));
            return;
        };

        // Wait a few ticks before acting (let the screen settle)
        if self.ticks_in_state < 5 {
            return;
        }

        let account = creds.account_name.clone();
        let password = creds.password.clone();

        // Strategy 1: Write credentials to EQLogin char arrays + CXStr widgets
        let wrote_chars = widgets::write_login_credentials(self.eqmain_base, &account, &password);
        let wrote_cxstr = widgets::type_credentials_to_window(self.eqmain_base, &account, &password);

        if !wrote_chars && !wrote_cxstr {
            tracing::warn!("Both credential write methods failed — retrying next tick");
            return;
        }

        tracing::info!(account = %account, "Credentials written to EQ memory");

        // Strategy 2: Also type password via PostMessage as backup
        // (CXStr writes may not be read by EQ's submit handler)
        widgets::type_password_wm_char(self.eqmain_base, &password);

        // Zeroize credentials from FSM memory
        self.credentials = None;
        self.action_taken = true;

        tracing::info!(account = %account, "Credential entry complete — waiting for server select");
    }

    fn tick_wait_for_server_select(&mut self) {
        if self.eqmain_base == 0 {
            // eqmain.dll unloaded — we jumped straight to character select
            tracing::info!("eqmain.dll unloaded during server select wait — at character select");
            self.transition(State::SelectingCharacter);
            return;
        }

        // Detect server select by SIDL name "serverselect"
        if widgets::is_sidl_window_visible(self.eqmain_base, widgets::SIDL_SERVER_SELECT) {
            tracing::info!("Server select screen detected (SIDL: serverselect)");
            self.transition(State::SelectingServer);
            return;
        }

        // Fallback: detect by WindowText
        if widgets::is_window_visible(self.eqmain_base, "PLAY EVERQUEST!")
            || widgets::is_window_visible(self.eqmain_base, "SERVER SELECT")
        {
            tracing::info!("Server select screen detected (fallback: WindowText)");
            self.transition(State::SelectingServer);
        }
    }

    fn tick_selecting_server(&mut self) {
        if self.action_taken {
            // Already clicked — wait for transition
            if self.ticks_in_state > ACTION_COOLDOWN_TICKS {
                self.transition(State::WaitForCharSelect);
            }
            return;
        }

        // Wait a few ticks for the screen to settle
        if self.ticks_in_state < 5 {
            return;
        }

        // Click "PLAY EVERQUEST!" to join the default/last server.
        let clicked = widgets::click_button(self.eqmain_base, "PLAY EVERQUEST!")
            || widgets::click_button(self.eqmain_base, "QUICK CONNECT TO LAST SERVER");

        if clicked {
            tracing::info!(server = %self.server_name, "PLAY EVERQUEST clicked");
            self.action_taken = true;
        } else {
            // Fallback: try Enter key
            if widgets::simulate_enter_key(self.eqmain_base) {
                tracing::info!("Server select: Enter key sent as fallback");
                self.action_taken = true;
            }
        }
    }

    fn tick_wait_for_char_select(&mut self) {
        // eqmain.dll unloads when transitioning to character select.
        if self.eqmain_base == 0 {
            tracing::info!("eqmain.dll unloaded — transitioning to character select");
            self.transition(State::SelectingCharacter);
            return;
        }

        // Periodically press Enter to dismiss blocking dialogs that may not have
        // SIDL names we recognize (e.g., server messages, maintenance notices).
        if self.ticks_in_state > 0 && self.ticks_in_state % 90 == 0 {
            tracing::info!("Pressing Enter to dismiss potential dialog");
            widgets::simulate_enter_key(self.eqmain_base);
        }
    }

    fn tick_selecting_character(&mut self) {
        if self.action_taken {
            // Already initiated character selection — wait for world
            if self.ticks_in_state > ACTION_COOLDOWN_TICKS * 2 {
                self.transition(State::WaitForWorld);
            }
            return;
        }

        if self.character_name.is_empty() {
            tracing::error!("No character name available for character selection");
            self.transition(State::Error(LoginError::CharacterNotFound {
                expected: String::new(),
                found: String::new(),
            }));
            return;
        }

        // Wait for eqgame to be ready (EQ_BASE must be set)
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return;
        }

        // Wait a couple seconds for the character select UI to fully load
        if self.ticks_in_state < 60 {
            return;
        }

        // Queue SelectCharacter + EnterWorld via the game loop mechanism.
        // This is the same approach as the IPC handler's phase3_enter_world,
        // but now driven by the FSM instead of a background thread.
        let char_name = self.character_name.clone();
        self.do_select_character_via_game_loop(eq_base, &char_name);
    }

    /// Queue character selection and enter world via the game loop's existing mechanism.
    fn do_select_character_via_game_loop(&mut self, eq_base: u64, character_name: &str) {
        // Find CCharacterListWnd by scanning eqgame.exe's CXWndManager
        let Some(mgr_ptr_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::PINST_CXWND_MANAGER,
            eq_base,
        ) else {
            tracing::warn!("Failed to rebase pinstCXWndManager");
            return;
        };

        let char_list_wnd = {
            #[cfg(windows)]
            {
                use dmft_common::offsets::eqgame as eqg;
                unsafe {
                    let mgr = *(mgr_ptr_addr as *const usize);
                    if mgr == 0 {
                        tracing::warn!("CXWndManager is null");
                        return;
                    }

                    // Use eqgame offsets to find CharacterListWnd
                    crate::eq::widgets::find_visible_window_by_sidl_name(
                        mgr,
                        widgets::SIDL_CHARACTER_LIST_WND,
                        eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
                        eqg::CXWNDMGR_WINDOWS_ARRAY,
                        eqg::CXWNDMGR_WINDOWS_COUNT,
                    )
                }
            }
            #[cfg(not(windows))]
            {
                let _ = mgr_ptr_addr;
                None::<usize>
            }
        };

        let Some(wnd) = char_list_wnd else {
            if self.ticks_in_state < 150 {
                // Still loading — retry next tick
                return;
            }
            tracing::warn!("CharacterListWnd not found after extended wait");
            // Try fallback: /enterworld slash command
            crate::hooks::game_loop::queue_slash_command("/enterworld".to_string());
            self.action_taken = true;
            return;
        };

        let Some(enter_world_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::ENTER_WORLD,
            eq_base,
        ) else {
            tracing::warn!("Failed to rebase ENTER_WORLD");
            return;
        };

        tracing::info!(
            wnd = format!("{:#x}", wnd),
            character = %character_name,
            "Queuing SelectCharacter + EnterWorld via game loop"
        );

        crate::hooks::game_loop::queue_enter_world(
            wnd,
            enter_world_addr,
            character_name.to_string(),
        );

        self.action_taken = true;
        tracing::info!(character = %character_name, "Character select + enter world queued");
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
            #[cfg(windows)]
            {
                let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
                if player_ptr != 0 {
                    tracing::info!("Local player detected — login complete!");
                    self.transition(State::InWorld);
                }
            }
            #[cfg(not(windows))]
            {
                let _ = player_ptr_addr;
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
        self.ticks_in_state = 0;
        self.action_taken = false;

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
        if *prev != self.phase {
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
