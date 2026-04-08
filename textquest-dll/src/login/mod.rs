//! DLL-side login automation — drives EQ's login UI autonomously.
//!
//! The orchestrator sends credentials once via `StartLogin`; the FSM handles
//! all UI steps (credential entry, server select, character select, enter world)
//! and reports progress back via `LoginPhaseUpdate` IPC responses.

pub mod eqmain;
pub mod widgets;

use std::sync::Mutex;
use std::time::Instant;

use textquest_common::login::{LoginError, LoginPhase, RelogConfig, RelogPhase, RetryState};
use zeroize::Zeroizing;

/// Global login FSM instance, one per injected DLL.
static LOGIN_FSM: Mutex<Option<LoginFsm>> = Mutex::new(None);

/// Initialize the login FSM. Called once during DLL setup.
pub fn init() {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(LoginFsm::new());
    tracing::info!("Login FSM initialized");
}

/// Run one login tick. Call from `on_game_tick()` when `local_player` is None.
pub fn tick() -> Option<LoginPhase> {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.tick()
    } else {
        None
    }
}

/// Store credentials received from the orchestrator's `StartLogin` command.
pub fn start_login(
    account_name: String,
    password: String,
    server_name: String,
    character_name: String,
) {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.store_credentials(account_name, password, server_name, character_name);
    } else {
        // Auto-init if not yet created
        let mut fsm = LoginFsm::new();
        fsm.store_credentials(account_name, password, server_name, character_name);
        *guard = Some(fsm);
    }
}

/// Advance the FSM to WaitForServerSelect, skipping credential entry.
/// Called by the GiveTime hook after it has already written credentials
/// on the main thread — prevents the FSM from double-writing them.
pub fn advance_to_server_select() {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        if fsm.state == State::WaitForLoginScreen || fsm.state == State::EnteringCredentials {
            fsm.action_taken = true;
            fsm.transition(State::WaitForServerSelect);
            tracing::info!(
                "FSM advanced to WaitForServerSelect (credentials already written by GiveTime hook)"
            );
        }
    }
}

/// Get current login phase for status queries.
pub fn phase() -> LoginPhase {
    let guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .as_ref()
        .map_or(LoginPhase::NotStarted, |fsm| fsm.phase.clone())
}

/// Start a camp-relog cycle: camp → logout → reconnect with backoff.
pub fn start_relog(
    account_name: String,
    password: String,
    server_name: String,
    character_name: String,
    config: RelogConfig,
) {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.start_relog(account_name, password, server_name, character_name, config);
    } else {
        let mut fsm = LoginFsm::new();
        fsm.start_relog(account_name, password, server_name, character_name, config);
        *guard = Some(fsm);
    }
}

/// Cancel an in-progress relog. The client stays wherever it currently is.
pub fn cancel_relog() {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.cancel_relog();
    }
}

/// Switch to a different server. Issues `/camp desktop` then re-authenticates.
pub fn switch_server(
    server_name: String,
    character_name: String,
    account_name: String,
    password: String,
) {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.start_switch_server(server_name, character_name, account_name, password);
    } else {
        let mut fsm = LoginFsm::new();
        fsm.start_switch_server(server_name, character_name, account_name, password);
        *guard = Some(fsm);
    }
}

/// Switch to a different character on the current server. Issues `/camp` then
/// selects the new character at character select.
pub fn switch_character(character_name: String) {
    let mut guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(fsm) = guard.as_mut() {
        fsm.start_switch_character(character_name);
    } else {
        let mut fsm = LoginFsm::new();
        fsm.start_switch_character(character_name);
        *guard = Some(fsm);
    }
}

/// Check if the login FSM has completed (in world, error, or idle after completion).
pub fn is_done() -> bool {
    let guard = LOGIN_FSM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .as_ref()
        .is_none_or(|fsm| matches!(fsm.state, State::InWorld | State::Error(_) | State::Idle))
}

/// Internal states for the login FSM — more granular than the IPC-facing `LoginPhase`.
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
    // ── Relog states ──
    /// `/camp desktop` issued, waiting for the camp timer to complete.
    RelogCamping,
    /// Camp complete, waiting for backoff delay before reconnecting.
    RelogWaitingBackoff,
    /// Re-entering the login flow after backoff.
    RelogReconnecting,
    // ── Switch states ──
    /// `/camp desktop` issued for server switch, waiting for camp timer.
    SwitchServerCamping,
    /// `/camp` issued for character switch, waiting for character select.
    SwitchCharacterCamping,
}

/// Throttle ticks between actions to avoid spamming EQ's UI.
/// At ~30fps game loop, 15 ticks = ~500ms.
const ACTION_COOLDOWN_TICKS: u32 = 15;

#[derive(Debug, Clone, PartialEq)]
enum ConflictDialog {
    KickActiveCharacter,
    OfflineTrader,
}

/// Credentials stored temporarily in memory, zeroized after use.
///
/// The `password` field uses `Zeroizing<String>` which overwrites the heap buffer
/// with volatile zeroes on drop — preventing compiler optimization from eliding the
/// zeroing and covering prior heap reallocations that a manual loop would miss.
struct Credentials {
    account_name: String,
    password: Zeroizing<String>,
    server_name: String,
    character_name: String,
}

/// Issue a slash command through the game loop's command queue.
fn issue_slash_command(cmd: &str) {
    crate::hooks::game_loop::queue_slash_command(cmd.to_string());
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
    // ── Relog state ──
    /// Active relog configuration (set when a relog is in progress).
    relog_config: Option<RelogConfig>,
    /// Retry state for the current relog sequence.
    relog_retry: RetryState,
    /// Credentials stashed for relog reconnection (re-used across retries).
    relog_credentials: Option<Credentials>,
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
            relog_config: None,
            relog_retry: RetryState::default(),
            relog_credentials: None,
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
            password: Zeroizing::new(password),
            server_name,
            character_name,
        });
        self.transition(State::WaitForLoginScreen);
    }

    /// Advance the FSM by one tick. Returns Some(phase) when the phase changes.
    ///
    /// The FSM detects which screen EQ is showing by scanning for visible SIDL
    /// windows each tick (the MQ2 `AutoLogin` approach). This replaces the previous
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
        if self.eqmain_base != 0 && self.handle_dialogs() {
            return self.phase_if_changed(&prev_phase);
        }

        match self.state.clone() {
            State::WaitForLoginScreen => self.tick_wait_for_login_screen(),
            State::EnteringCredentials => self.tick_entering_credentials(),
            State::WaitForServerSelect => self.tick_wait_for_server_select(),
            State::SelectingServer => self.tick_selecting_server(),
            State::WaitForCharSelect => self.tick_wait_for_char_select(),
            State::SelectingCharacter => self.tick_selecting_character(),
            State::WaitForWorld => self.tick_wait_for_world(),
            State::RelogCamping => self.tick_relog_camping(),
            State::RelogWaitingBackoff => self.tick_relog_waiting_backoff(),
            State::RelogReconnecting => self.tick_relog_reconnecting(),
            State::SwitchServerCamping => self.tick_switch_server_camping(),
            State::SwitchCharacterCamping => self.tick_switch_character_camping(),
            _ => {}
        }

        self.phase_if_changed(&prev_phase)
    }

    /// Handle dialogs that can appear at any login stage.
    /// Returns true if the FSM transitioned to an error state.
    fn handle_dialogs(&mut self) -> bool {
        // YesNo dialog — "already logged in, kick?" → click Yes
        if let Some(dialog_wnd) =
            widgets::find_visible_sidl_window(self.eqmain_base, widgets::SIDL_YES_NO_DIALOG)
        {
            let dialog_text = widgets::read_yesno_dialog_text(dialog_wnd).unwrap_or_default();
            tracing::info!(text = %dialog_text, "YesNo dialog detected");
            return self.handle_yesno_dialog(dialog_wnd, &dialog_text);
        }

        // OK dialog — error messages, server full, etc.
        // Read dialog text, classify the error, and apply recovery logic.
        if let Some(error) = widgets::check_error_dialog(self.eqmain_base) {
            let action = widgets::recovery_action(&error);
            match action {
                widgets::RecoveryAction::Abort => {
                    tracing::error!(error = ?error, "Login error — aborting");
                    self.transition(State::Error(error));
                    return true;
                }
                widgets::RecoveryAction::Retry => {
                    tracing::warn!(error = ?error, "Transient error — resetting for retry");
                    self.state_entered_at = Instant::now();
                }
                widgets::RecoveryAction::WaitAndRetry => {
                    tracing::warn!(error = ?error, "Server-side error — retry with grace period");
                    self.state_entered_at = Instant::now();
                    // Give an extra retry attempt for server-side issues
                    self.retries = self.retries.saturating_sub(1);
                }
            }
        }

        false
    }

    fn handle_yesno_dialog(&mut self, dialog_wnd: usize, dialog_text: &str) -> bool {
        match Self::classify_conflict_dialog(dialog_text) {
            Some(ConflictDialog::KickActiveCharacter) => {
                tracing::warn!("KickActiveCharacter dialog detected — clicking YES to continue");
                widgets::click_yesno_yes(dialog_wnd);
                // Reset timeout/retry window after kicking the other session
                self.state_entered_at = Instant::now();
                self.retries = 0;
                false
            }
            Some(ConflictDialog::OfflineTrader) => {
                tracing::error!(
                    text = dialog_text,
                    "Offline trader conflict detected — stopping login"
                );
                widgets::click_yesno_no(dialog_wnd);
                self.transition(State::Error(LoginError::OfflineTrader));
                true
            }
            None => {
                tracing::info!("Clicking YES on unknown YesNo dialog");
                widgets::click_yesno_yes(dialog_wnd);
                false
            }
        }
    }

    fn classify_conflict_dialog(text: &str) -> Option<ConflictDialog> {
        let normalized = text.to_ascii_lowercase();
        if normalized.contains("kickactivecharacter")
            || normalized.contains("kick active character")
            || normalized.contains("already logged in")
            || normalized.contains("logged in elsewhere")
            || normalized.contains("active character")
        {
            Some(ConflictDialog::KickActiveCharacter)
        } else if normalized.contains("offline trader")
            || normalized.contains("offline mode")
            || normalized.contains("trader mode")
            || normalized.contains("bazaar trader")
        {
            Some(ConflictDialog::OfflineTrader)
        } else {
            None
        }
    }

    fn tick_wait_for_login_screen(&mut self) {
        if self.eqmain_base == 0 {
            return;
        }

        // Use the proven type_credentials_to_window approach that was working
        // before the FSM rewrite. It scans for USERNAME/PASSWORD labels and
        // writes credentials + clicks Login. If it succeeds, skip to server select.
        if !self.action_taken {
            if let Some(creds) = self.credentials.take() {
                // Try writing credentials (proven working approach)
                let wrote = widgets::type_credentials_to_window(
                    self.eqmain_base,
                    &creds.account_name,
                    &creds.password,
                );
                if wrote {
                    tracing::info!("Credentials written + Login clicked");
                    self.action_taken = true;
                    self.transition(State::WaitForServerSelect);
                } else {
                    // Keep credentials for retry if credential entry failed.
                    self.credentials = Some(creds);
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
        let wrote_cxstr =
            widgets::type_credentials_to_window(self.eqmain_base, &account, &password);

        if !wrote_chars && !wrote_cxstr {
            tracing::warn!("Both credential write methods failed — retrying next tick");
            return;
        }

        tracing::info!(account = %account, "Credentials written to EQ memory");

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

        // Click "PLAY EVERQUEST!" via phase-aware helper.
        // eqmain_base != 0 means we're still in eqmain context → direct vtable click.
        let in_eqmain = self.eqmain_base != 0;
        let clicked = Self::try_click_button(self.eqmain_base, "PLAY EVERQUEST!", in_eqmain)
            || Self::try_click_button(self.eqmain_base, "QUICK CONNECT TO LAST SERVER", in_eqmain);

        if clicked {
            tracing::info!(server = %self.server_name, in_eqmain, "PLAY EVERQUEST clicked");
            self.action_taken = true;
        } else {
            // Fallback: try Enter key
            if widgets::simulate_enter_key(self.eqmain_base) {
                tracing::info!("Server select: Enter key sent as fallback");
                self.action_taken = true;
            }
        }
    }

    /// Find a button by name and click it using the phase-aware helper.
    fn try_click_button(eqmain_base: u64, window_name: &str, in_eqmain: bool) -> bool {
        #[cfg(windows)]
        {
            let Some(cxwnd_mgr) = eqmain::resolve_cxwnd_manager(eqmain_base) else {
                return false;
            };
            let Some(button_wnd) =
                (unsafe { crate::eq::widgets::find_window_by_name(cxwnd_mgr, window_name) })
            else {
                return false;
            };
            widgets::click_button_for_phase(button_wnd, in_eqmain);
            tracing::debug!(window = window_name, in_eqmain, "FSM clicked button");
            true
        }
        #[cfg(not(windows))]
        {
            let _ = (eqmain_base, window_name, in_eqmain);
            false
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
        if self.ticks_in_state > 0 && self.ticks_in_state.is_multiple_of(90) {
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
        // Find CCharacterListWnd by scanning eqgame.exe's CXWndManager.
        // We use rescan_char_list_wnd() from the game loop module — it scans by
        // SidlText WITHOUT checking dShow visibility, which is critical because
        // the eqmain dShow offset (0x06c) may not match eqgame's CXWnd layout.
        let char_list_wnd = crate::hooks::game_loop::rescan_char_list_wnd();

        let Some(wnd) = char_list_wnd else {
            if self.ticks_in_state < 150 {
                // Still loading — retry next tick
                return;
            }
            tracing::warn!("CharacterListWnd not found after extended wait — trying Enter key");
            // Fallback: send Enter key to click the Enter World button.
            // (Slash commands require local player, which is null at char select.)
            crate::hooks::game_loop::send_enter_to_eq();
            self.action_taken = true;
            return;
        };

        // Pre-validate: scan the character list to confirm the target exists.
        // This provides a clear error with available character names instead of
        // silently selecting index 0 when the character isn't found.
        match widgets::find_character_in_list(wnd, character_name) {
            Ok(index) => {
                tracing::info!(
                    character = %character_name,
                    index,
                    wnd = format!("{:#x}", wnd),
                    "Character found in list — proceeding to enter world"
                );
            }
            Err(available) => {
                let found_str = if available.is_empty() {
                    "none (list empty or unreadable)".to_string()
                } else {
                    available.join(", ")
                };
                tracing::error!(
                    expected = %character_name,
                    available = %found_str,
                    "Character not found in character list"
                );
                self.transition(State::Error(LoginError::CharacterNotFound {
                    expected: character_name.to_string(),
                    found: found_str,
                }));
                return;
            }
        }

        let Some(enter_world_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::ENTER_WORLD, eq_base)
        else {
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

        if let Some(player_ptr_addr) = textquest_common::offsets::rebase(
            textquest_common::offsets::PINST_LOCAL_PLAYER,
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

    // ── Relog methods ──────────────────────────────────────────────────

    /// Begin a camp-relog cycle from in-world or any state.
    fn start_relog(
        &mut self,
        account_name: String,
        password: String,
        server_name: String,
        character_name: String,
        config: RelogConfig,
    ) {
        tracing::info!(
            account = %account_name,
            server = %server_name,
            character = %character_name,
            camp_first = config.camp_before_relog,
            "Starting relog sequence"
        );
        self.server_name = server_name.clone();
        self.character_name = character_name.clone();
        self.relog_retry.reset();
        self.relog_credentials = Some(Credentials {
            account_name,
            password: Zeroizing::new(password),
            server_name,
            character_name,
        });

        if config.camp_before_relog {
            self.send_relog_progress(RelogPhase::Camping);
            self.relog_config = Some(config);
            self.transition(State::RelogCamping);
        } else {
            self.relog_config = Some(config);
            self.send_relog_progress(RelogPhase::LoggedOut);
            self.transition(State::RelogWaitingBackoff);
        }
    }

    /// Cancel an in-progress relog. Stay wherever we are.
    fn cancel_relog(&mut self) {
        if matches!(
            self.state,
            State::RelogCamping | State::RelogWaitingBackoff | State::RelogReconnecting
        ) {
            tracing::info!("Relog cancelled by operator");
            self.relog_config = None;
            self.relog_credentials = None;
            self.relog_retry.reset();
            self.transition(State::Idle);
        }
    }

    fn tick_relog_camping(&mut self) {
        // Issue `/camp desktop` once
        if !self.action_taken {
            tracing::info!("Issuing /camp desktop for relog");
            issue_slash_command("/camp desktop");
            self.action_taken = true;
        }

        // Check if we've left the world (local player pointer goes null)
        if self.is_local_player_null() {
            tracing::info!("Camp complete — player gone from world");
            self.send_relog_progress(RelogPhase::LoggedOut);
            self.transition(State::RelogWaitingBackoff);
            return;
        }

        // Timeout: if camp takes too long, force transition
        let camp_timeout = self
            .relog_config
            .as_ref()
            .map_or(35, |c| c.camp_timeout_secs);
        if self.state_entered_at.elapsed().as_secs() > camp_timeout {
            tracing::warn!("Camp timeout exceeded — proceeding to reconnect");
            self.send_relog_progress(RelogPhase::LoggedOut);
            self.transition(State::RelogWaitingBackoff);
        }
    }

    fn tick_relog_waiting_backoff(&mut self) {
        let Some(config) = self.relog_config.as_ref() else {
            self.transition(State::Error(LoginError::Timeout {
                phase: "RelogWaitingBackoff (no config)".into(),
            }));
            return;
        };

        let delay = config.retry_policy.next_delay(&self.relog_retry);
        match delay {
            Some(d) => {
                let attempt = self.relog_retry.attempt_count + 1;
                // Only send progress once when entering this state
                if !self.action_taken {
                    self.send_relog_progress(RelogPhase::WaitingToReconnect {
                        attempt,
                        delay_secs: d.as_secs_f64(),
                    });
                    self.action_taken = true;
                    tracing::info!(
                        attempt,
                        delay_secs = d.as_secs_f64(),
                        "Waiting before relog attempt"
                    );
                }

                // Wait for the backoff delay
                if self.state_entered_at.elapsed() >= d {
                    self.relog_retry.attempt_count += 1;
                    self.send_relog_progress(RelogPhase::Reconnecting { attempt });
                    self.transition(State::RelogReconnecting);
                }
            }
            None => {
                // Retries exhausted
                let reason = self
                    .relog_retry
                    .last_error
                    .clone()
                    .unwrap_or(LoginError::Timeout {
                        phase: "Relog retries exhausted".into(),
                    });
                tracing::error!(
                    attempts = self.relog_retry.attempt_count,
                    ?reason,
                    "Relog failed — retries exhausted"
                );
                self.send_relog_progress(RelogPhase::Failed {
                    reason: reason.clone(),
                });
                self.relog_config = None;
                self.relog_credentials = None;
                self.transition(State::Error(reason));
            }
        }
    }

    fn tick_relog_reconnecting(&mut self) {
        // Feed credentials back into the normal login flow.
        // The relog_credentials are cloned (not consumed) so retries can reuse them.
        if !self.action_taken {
            if let Some(creds) = self.relog_credentials.as_ref() {
                tracing::info!(
                    account = %creds.account_name,
                    server = %creds.server_name,
                    character = %creds.character_name,
                    attempt = self.relog_retry.attempt_count,
                    "Relog: starting login sequence"
                );
                self.server_name = creds.server_name.clone();
                self.character_name = creds.character_name.clone();
                self.credentials = Some(Credentials {
                    account_name: creds.account_name.clone(),
                    password: Zeroizing::new(creds.password.as_str().to_owned()),
                    server_name: creds.server_name.clone(),
                    character_name: creds.character_name.clone(),
                });
                self.action_taken = true;
                // Transition into the normal login flow
                self.transition(State::WaitForLoginScreen);
            } else {
                self.transition(State::Error(LoginError::Timeout {
                    phase: "RelogReconnecting (no credentials)".into(),
                }));
            }
        }
    }

    // ── Switch methods ──────────────────────────────────────────────────

    /// Switch to a different server — camp desktop → re-authenticate on new server.
    fn start_switch_server(
        &mut self,
        server_name: String,
        character_name: String,
        account_name: String,
        password: String,
    ) {
        tracing::info!(
            server = %server_name,
            character = %character_name,
            "Starting server switch"
        );
        self.server_name = server_name.clone();
        self.character_name = character_name.clone();
        self.credentials = Some(Credentials {
            account_name,
            password: Zeroizing::new(password),
            server_name,
            character_name,
        });
        self.transition(State::SwitchServerCamping);
    }

    /// Switch to a different character on the same server — camp → char select.
    fn start_switch_character(&mut self, character_name: String) {
        tracing::info!(character = %character_name, "Starting character switch");
        self.character_name = character_name;
        self.transition(State::SwitchCharacterCamping);
    }

    fn tick_switch_server_camping(&mut self) {
        if !self.action_taken {
            tracing::info!("Issuing /camp desktop for server switch");
            issue_slash_command("/camp desktop");
            self.action_taken = true;
        }

        if self.is_local_player_null() {
            tracing::info!("Camp complete — transitioning to login for server switch");
            self.transition(State::WaitForLoginScreen);
            return;
        }

        if self.state_entered_at.elapsed().as_secs() > 45 {
            tracing::error!("Server switch camp timeout");
            crate::ipc::send_response(textquest_common::ipc::Response::SwitchResult {
                success: false,
                message: "Camp timeout during server switch".into(),
            });
            self.transition(State::Error(LoginError::Timeout {
                phase: "SwitchServerCamping".into(),
            }));
        }
    }

    fn tick_switch_character_camping(&mut self) {
        if !self.action_taken {
            tracing::info!("Issuing /camp for character switch");
            issue_slash_command("/camp");
            self.action_taken = true;
        }

        // Detect char select: local player null while eqmain stays unloaded
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base != 0 && self.is_local_player_null() && self.eqmain_base == 0 {
            tracing::info!("At character select — selecting new character");
            self.transition(State::SelectingCharacter);
            return;
        }

        if self.state_entered_at.elapsed().as_secs() > 45 {
            tracing::error!("Character switch camp timeout");
            crate::ipc::send_response(textquest_common::ipc::Response::SwitchResult {
                success: false,
                message: "Camp timeout during character switch".into(),
            });
            self.transition(State::Error(LoginError::Timeout {
                phase: "SwitchCharacterCamping".into(),
            }));
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Check if local player pointer is null (not in world).
    fn is_local_player_null(&self) -> bool {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return true;
        }
        #[cfg(windows)]
        {
            if let Some(addr) = textquest_common::offsets::rebase(
                textquest_common::offsets::PINST_LOCAL_PLAYER,
                eq_base,
            ) {
                return unsafe { *(addr as *const usize) } == 0;
            }
            true
        }
        #[cfg(not(windows))]
        true
    }

    /// Send a relog progress IPC response.
    fn send_relog_progress(&self, phase: RelogPhase) {
        crate::ipc::send_response(textquest_common::ipc::Response::RelogProgress { phase });
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
        if matches!(new_state, State::InWorld) {
            // Successful login/relog: clear retry metadata and cached relog credentials
            // so account secrets are not retained in memory for the rest of the session.
            self.relog_config = None;
            self.relog_credentials = None;
            self.relog_retry.reset();
        }
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
            State::Error(e) => LoginPhase::Failed { reason: e.clone() },
            // Relog states map to AtLoginScreen since we're cycling back through login
            State::RelogCamping | State::RelogWaitingBackoff | State::RelogReconnecting => {
                LoginPhase::AtLoginScreen
            }
            // Switch states also map back to login phases
            State::SwitchServerCamping => LoginPhase::ServerSelecting,
            State::SwitchCharacterCamping => LoginPhase::CharacterSelecting,
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
        assert!(matches!(
            fsm.state,
            State::Error(LoginError::Timeout { .. })
        ));
    }

    #[test]
    fn test_credentials_password_zeroized_on_drop() {
        let creds = Credentials {
            account_name: "test".into(),
            password: Zeroizing::new("secret123".to_string()),
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

    #[test]
    fn classify_conflict_dialog_detects_variants() {
        assert_eq!(
            LoginFsm::classify_conflict_dialog("Kick Active Character?"),
            Some(ConflictDialog::KickActiveCharacter)
        );
        assert_eq!(
            LoginFsm::classify_conflict_dialog("Character is in offline trader mode"),
            Some(ConflictDialog::OfflineTrader)
        );
        assert!(LoginFsm::classify_conflict_dialog("random dialog").is_none());
    }

    #[test]
    fn offline_trader_dialog_transitions_to_error() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::WaitForLoginScreen);
        let transitioned = fsm.handle_yesno_dialog(0, "Character is in offline trader mode.");
        assert!(transitioned);
        assert!(matches!(fsm.state, State::Error(LoginError::OfflineTrader)));
    }

    #[test]
    fn kick_active_character_dialog_retries_without_error() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::WaitForLoginScreen);
        let transitioned =
            fsm.handle_yesno_dialog(0, "This account already has an active character.");
        assert!(!transitioned);
        assert_eq!(fsm.state, State::WaitForLoginScreen);
    }

    // ── Relog tests ──────────────────────────────────────────────────────

    fn make_relog_config(max_retries: u32) -> RelogConfig {
        RelogConfig {
            retry_policy: textquest_common::login::RetryPolicy {
                max_retries,
                initial_delay: std::time::Duration::from_millis(10),
                max_delay: std::time::Duration::from_secs(1),
                backoff_multiplier: 2.0,
                jitter: false,
            },
            camp_before_relog: true,
            camp_timeout_secs: 35,
        }
    }

    #[test]
    fn relog_with_camp_starts_in_camping_state() {
        let mut fsm = LoginFsm::new();
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            make_relog_config(3),
        );
        assert_eq!(fsm.state, State::RelogCamping);
        assert!(fsm.relog_config.is_some());
        assert!(fsm.relog_credentials.is_some());
    }

    #[test]
    fn relog_without_camp_skips_to_backoff() {
        let mut fsm = LoginFsm::new();
        let mut config = make_relog_config(3);
        config.camp_before_relog = false;
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            config,
        );
        assert_eq!(fsm.state, State::RelogWaitingBackoff);
    }

    #[test]
    fn cancel_relog_returns_to_idle() {
        let mut fsm = LoginFsm::new();
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            make_relog_config(3),
        );
        assert_eq!(fsm.state, State::RelogCamping);

        fsm.cancel_relog();
        assert_eq!(fsm.state, State::Idle);
        assert!(fsm.relog_config.is_none());
        assert!(fsm.relog_credentials.is_none());
    }

    #[test]
    fn cancel_relog_noop_from_idle() {
        let mut fsm = LoginFsm::new();
        fsm.cancel_relog();
        assert_eq!(fsm.state, State::Idle);
    }

    #[test]
    fn relog_reconnecting_transitions_to_login_flow() {
        let mut fsm = LoginFsm::new();
        let mut config = make_relog_config(3);
        config.camp_before_relog = false;
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            config,
        );
        assert_eq!(fsm.state, State::RelogWaitingBackoff);

        // Simulate backoff elapsed by transitioning directly
        fsm.relog_retry.attempt_count = 1;
        fsm.transition(State::RelogReconnecting);

        // Tick should feed credentials into login flow
        fsm.tick_relog_reconnecting();
        assert_eq!(fsm.state, State::WaitForLoginScreen);
        assert!(fsm.credentials.is_some());
    }

    #[test]
    fn relog_retries_exhausted_transitions_to_error() {
        let mut fsm = LoginFsm::new();
        let mut config = make_relog_config(0); // zero retries
        config.camp_before_relog = false;
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            config,
        );
        assert_eq!(fsm.state, State::RelogWaitingBackoff);

        // Tick should detect exhausted retries and error out
        fsm.tick_relog_waiting_backoff();
        assert!(matches!(fsm.state, State::Error(_)));
    }

    #[test]
    fn relog_camping_phase_maps_to_at_login_screen() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::RelogCamping);
        assert_eq!(fsm.phase, LoginPhase::AtLoginScreen);
    }

    #[test]
    fn relog_waiting_backoff_phase_maps_to_at_login_screen() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::RelogWaitingBackoff);
        assert_eq!(fsm.phase, LoginPhase::AtLoginScreen);
    }

    // ── Switch tests ──────────────────────────────────────────────────────

    #[test]
    fn switch_server_starts_camping() {
        let mut fsm = LoginFsm::new();
        fsm.start_switch_server(
            "Firiona Vie".into(),
            "NewChar".into(),
            "acc".into(),
            "pass".into(),
        );
        assert_eq!(fsm.state, State::SwitchServerCamping);
        assert_eq!(fsm.server_name, "Firiona Vie");
        assert_eq!(fsm.character_name, "NewChar");
        assert!(fsm.credentials.is_some());
    }

    #[test]
    fn switch_character_starts_camping() {
        let mut fsm = LoginFsm::new();
        fsm.start_switch_character("AltChar".into());
        assert_eq!(fsm.state, State::SwitchCharacterCamping);
        assert_eq!(fsm.character_name, "AltChar");
    }

    #[test]
    fn switch_server_camping_phase_maps_to_server_selecting() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::SwitchServerCamping);
        assert_eq!(fsm.phase, LoginPhase::ServerSelecting);
    }

    #[test]
    fn switch_character_camping_phase_maps_to_character_selecting() {
        let mut fsm = LoginFsm::new();
        fsm.transition(State::SwitchCharacterCamping);
        assert_eq!(fsm.phase, LoginPhase::CharacterSelecting);
    }

    #[test]
    fn transition_resets_relog_unrelated_fields() {
        let mut fsm = LoginFsm::new();
        fsm.retries = 5;
        fsm.ticks_in_state = 100;
        fsm.action_taken = true;
        fsm.transition(State::RelogCamping);
        assert_eq!(fsm.retries, 0);
        assert_eq!(fsm.ticks_in_state, 0);
        assert!(!fsm.action_taken);
    }

    #[test]
    fn relog_credentials_preserved_across_retries() {
        let mut fsm = LoginFsm::new();
        let mut config = make_relog_config(3);
        config.camp_before_relog = false;
        fsm.start_relog(
            "myacc".into(),
            "mypass".into(),
            "Teek".into(),
            "MyChar".into(),
            config,
        );

        // Simulate first reconnect
        fsm.relog_retry.attempt_count = 1;
        fsm.transition(State::RelogReconnecting);
        fsm.tick_relog_reconnecting();
        assert_eq!(fsm.state, State::WaitForLoginScreen);

        // relog_credentials should still be available for retry
        assert!(fsm.relog_credentials.is_some());
        let creds = fsm.relog_credentials.as_ref().unwrap();
        assert_eq!(creds.account_name, "myacc");
        assert_eq!(creds.server_name, "Teek");
    }

    #[test]
    fn transition_to_inworld_clears_relog_sensitive_state() {
        let mut fsm = LoginFsm::new();
        fsm.start_relog(
            "acc".into(),
            "pass".into(),
            "Teek".into(),
            "Char".into(),
            make_relog_config(3),
        );
        assert!(fsm.relog_config.is_some());
        assert!(fsm.relog_credentials.is_some());

        fsm.transition(State::InWorld);

        assert!(fsm.relog_config.is_none());
        assert!(fsm.relog_credentials.is_none());
        assert_eq!(fsm.relog_retry.attempt_count, 0);
        assert!(fsm.relog_retry.last_error.is_none());
    }
}
