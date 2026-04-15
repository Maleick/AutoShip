//! Banking cycle controller — deposit plat, consolidate currency to a mule.
//!
//! The banking cycle is a two-level FSM:
//! - Outer: `BankingState` — `Idle` → `NavigatingToBank` → `Depositing` →
//!   `Consolidating` → `ReturningHome`
//! - Inner: `DepositStep` — sub-states within `Depositing` that drive the
//!   banker UI interaction (target, open window, deposit plat, close).
//!
//! # Plat tracking
//!
//! The controller tracks per-character plat totals and a deposit history ring
//! buffer. Consolidation moves plat from regular characters to a designated
//! mule/bank character by generating `/platinum` slash commands (same pattern
//! as MQ2 `/platinum give`).
//!
//! # Offline-testable design
//!
//! Navigation and plat transfers are represented as slash commands returned
//! from `tick()`. No live EQ process or IPC is required — callers mock
//! navigation by advancing ticks.

use std::collections::HashMap;

// ── Configuration ────────────────────────────────────────────────────────────

/// Configuration for the banking cycle.
#[derive(Debug, Clone)]
pub struct BankingConfig {
    /// Name of the banker NPC to target.
    pub banker_name: String,
    /// Name of the character that holds consolidated plat (the mule).
    pub mule_character: String,
    /// Minimum plat a character must have before it contributes to
    /// consolidation.
    pub consolidate_threshold: u64,
    /// Ticks to simulate traveling to/from bank.
    pub travel_ticks: u64,
    /// Ticks to wait between deposit sub-steps (prevents UI race).
    pub step_delay: u64,
    /// Ticks between automatic banking runs (0 = manual only).
    pub bank_interval_ticks: u64,
}

// ── Deposit sub-steps ────────────────────────────────────────────────────────

/// Sub-steps within the `Depositing` state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositStep {
    /// Target the banker NPC.
    Targeting,
    /// Open the bank window.
    OpeningWindow,
    /// Deposit plat into the bank.
    DepositingPlat,
    /// Close the bank window.
    ClosingWindow,
}

// ── Main state ───────────────────────────────────────────────────────────────

/// Outer states of the banking cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BankingState {
    /// No banking run is in progress.
    Idle,
    /// En route to the bank.
    NavigatingToBank,
    /// At the bank, interacting with the banker UI.
    Depositing { step: DepositStep },
    /// Moving plat from group characters to the mule character.
    Consolidating {
        /// Index into the consolidation queue (characters contributing plat).
        index: usize,
    },
    /// Returning to the camp/home point after the banking run.
    ReturningHome,
}

// ── Plat ledger ──────────────────────────────────────────────────────────────

/// A single deposit history entry.
#[derive(Debug, Clone)]
pub struct DepositRecord {
    /// Tick when the deposit occurred.
    pub tick: u64,
    /// Character name that made the deposit.
    pub character: String,
    /// Plat amount deposited.
    pub amount: u64,
}

/// Tracks plat state for the fleet.
#[derive(Debug, Default)]
pub struct PlatLedger {
    /// Current plat total per character name.
    pub balances: HashMap<String, u64>,
    /// Deposit history (most recent last, capped at `MAX_HISTORY`).
    pub history: Vec<DepositRecord>,
}

impl PlatLedger {
    const MAX_HISTORY: usize = 256;

    /// Record a deposit. Reduces the character's balance and appends to
    /// history.
    pub fn record_deposit(&mut self, character: &str, amount: u64, tick: u64) {
        let balance = self.balances.entry(character.to_string()).or_insert(0);
        *balance = balance.saturating_sub(amount);
        if self.history.len() >= Self::MAX_HISTORY {
            self.history.remove(0);
        }
        self.history.push(DepositRecord {
            tick,
            character: character.to_string(),
            amount,
        });
    }

    /// Set a character's plat balance (call when reading from game state).
    pub fn set_balance(&mut self, character: &str, plat: u64) {
        self.balances.insert(character.to_string(), plat);
    }

    /// Return the balance for a character (0 if unknown).
    #[must_use]
    pub fn balance(&self, character: &str) -> u64 {
        self.balances.get(character).copied().unwrap_or(0)
    }

    /// Total plat held across all characters.
    #[must_use]
    pub fn total_plat(&self) -> u64 {
        self.balances.values().sum()
    }
}

// ── Controller ───────────────────────────────────────────────────────────────

/// Banking cycle controller for one camp group.
pub struct BankingCycleController {
    /// Banking configuration.
    pub config: BankingConfig,
    /// Current outer FSM state.
    pub state: BankingState,
    /// Plat ledger (balances + history).
    pub ledger: PlatLedger,
    /// Tick when the last full banking run completed.
    pub last_bank_tick: u64,
    /// Tick when the current state was entered.
    pub state_entered_tick: u64,
    /// Characters queued for consolidation this cycle (those above threshold).
    consolidation_queue: Vec<(String, u32)>, // (character_name, pid)
    /// Amount of plat each character in the queue will send to the mule.
    consolidation_amounts: Vec<u64>,
    /// Amount of plat deposited by the banker character this run.
    deposit_amount: u64,
    /// PID of the character doing the bank visit.
    banker_pid: u32,
}

impl BankingCycleController {
    /// Create a new controller.
    #[must_use]
    pub fn new(config: BankingConfig) -> Self {
        Self {
            config,
            state: BankingState::Idle,
            ledger: PlatLedger::default(),
            last_bank_tick: 0,
            state_entered_tick: 0,
            consolidation_queue: Vec::new(),
            consolidation_amounts: Vec::new(),
            deposit_amount: 0,
            banker_pid: 0,
        }
    }

    /// Returns true if it is time for an automatic banking run.
    #[must_use]
    pub fn needs_bank_run(&self, current_tick: u64) -> bool {
        self.config.bank_interval_ticks > 0
            && self.state == BankingState::Idle
            && current_tick.saturating_sub(self.last_bank_tick) >= self.config.bank_interval_ticks
    }

    /// Start a banking run manually or automatically.
    ///
    /// * `banker_pid` — PID of the character that will walk to the bank.
    /// * `character_pids` — slice of `(character_name, pid)` for all group
    ///   members. Characters whose balance exceeds `consolidate_threshold` will
    ///   be added to the consolidation queue.
    /// * `current_tick` — current simulation tick.
    pub fn start_banking_run(
        &mut self,
        banker_pid: u32,
        character_pids: &[(String, u32)],
        current_tick: u64,
    ) {
        if self.state != BankingState::Idle {
            return;
        }
        self.banker_pid = banker_pid;

        // Build consolidation queue — all chars above threshold excluding mule itself.
        self.consolidation_queue.clear();
        self.consolidation_amounts.clear();
        for (name, pid) in character_pids {
            if name == &self.config.mule_character {
                continue;
            }
            let balance = self.ledger.balance(name);
            if balance >= self.config.consolidate_threshold {
                self.consolidation_queue.push((name.clone(), *pid));
                // Transfer everything above the threshold
                self.consolidation_amounts
                    .push(balance.saturating_sub(self.config.consolidate_threshold / 2));
            }
        }

        // Deposit amount = mule's current balance
        self.deposit_amount = self.ledger.balance(&self.config.mule_character);

        self.state = BankingState::NavigatingToBank;
        self.state_entered_tick = current_tick;
    }

    /// Advance the FSM by one tick. Returns `(pid, command)` pairs to execute.
    pub fn tick(&mut self, current_tick: u64) -> Vec<(u32, String)> {
        match self.state.clone() {
            BankingState::Idle => Vec::new(),

            BankingState::NavigatingToBank => {
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                // Arrived at bank — start depositing
                self.state = BankingState::Depositing {
                    step: DepositStep::Targeting,
                };
                self.state_entered_tick = current_tick;
                Vec::new()
            }

            BankingState::Depositing { step } => self.tick_deposit_step(step, current_tick),

            BankingState::Consolidating { index } => self.tick_consolidation(index, current_tick),

            BankingState::ReturningHome => {
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                let cmds = vec![(self.banker_pid, "/stand".into())];
                self.state = BankingState::Idle;
                self.last_bank_tick = current_tick;
                cmds
            }
        }
    }

    // ── Deposit sub-FSM ──────────────────────────────────────────────────────

    fn tick_deposit_step(&mut self, step: DepositStep, current_tick: u64) -> Vec<(u32, String)> {
        // Enforce step delay
        if current_tick.saturating_sub(self.state_entered_tick) < self.config.step_delay {
            return Vec::new();
        }

        let pid = self.banker_pid;

        match step {
            DepositStep::Targeting => {
                let cmds = vec![
                    (pid, format!("/target {}", self.config.banker_name)),
                    (pid, "/face fast".into()),
                ];
                self.state = BankingState::Depositing {
                    step: DepositStep::OpeningWindow,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            DepositStep::OpeningWindow => {
                let cmds = vec![(pid, "/click right target".into())];
                self.state = BankingState::Depositing {
                    step: DepositStep::DepositingPlat,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            DepositStep::DepositingPlat => {
                let cmds = if self.deposit_amount > 0 {
                    // EQ-style: /platinum deposit <amount>
                    let mule = self.config.mule_character.clone();
                    let amount = self.deposit_amount;
                    self.ledger.record_deposit(&mule, amount, current_tick);
                    vec![(pid, format!("/platinum deposit {amount}"))]
                } else {
                    Vec::new()
                };
                self.state = BankingState::Depositing {
                    step: DepositStep::ClosingWindow,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            DepositStep::ClosingWindow => {
                let cmds = vec![(pid, "/notify BigBankWnd BBW_Done_Button leftmouseup".into())];
                // After closing, start consolidation (or skip directly to returning)
                if self.consolidation_queue.is_empty() {
                    self.state = BankingState::ReturningHome;
                } else {
                    self.state = BankingState::Consolidating { index: 0 };
                }
                self.state_entered_tick = current_tick;
                cmds
            }
        }
    }

    // ── Consolidation step ───────────────────────────────────────────────────

    fn tick_consolidation(&mut self, index: usize, current_tick: u64) -> Vec<(u32, String)> {
        // Enforce step delay between consolidation commands
        if current_tick.saturating_sub(self.state_entered_tick) < self.config.step_delay {
            return Vec::new();
        }

        if index >= self.consolidation_queue.len() {
            // All characters consolidated — head home
            self.state = BankingState::ReturningHome;
            self.state_entered_tick = current_tick;
            return Vec::new();
        }

        let (char_name, char_pid) = self.consolidation_queue[index].clone();
        let amount = self.consolidation_amounts[index];

        // Record plat movement in ledger
        self.ledger.record_deposit(&char_name, amount, current_tick);
        // Credit mule
        let mule = self.config.mule_character.clone();
        let mule_balance = self.ledger.balance(&mule);
        self.ledger
            .ledger_credit_internal(&mule, mule_balance + amount);

        // Generate slash command: /plat give <amount> <mule_character>
        let cmds = vec![(char_pid, format!("/plat give {amount} {mule}"))];

        self.state = BankingState::Consolidating { index: index + 1 };
        self.state_entered_tick = current_tick;
        cmds
    }
}

// PlatLedger helper for internal credit (not a deposit — no history entry)
impl PlatLedger {
    fn ledger_credit_internal(&mut self, character: &str, new_balance: u64) {
        self.balances.insert(character.to_string(), new_balance);
    }
}

// ── Error type ───────────────────────────────────────────────────────────────

/// Errors that can occur during a banking cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BankingError {
    /// Navigation to the bank failed (e.g. stuck or zone unavailable).
    NavigationFailed,
    /// Could not open the bank window.
    BankWindowFailed,
}

// ── Fail-safe wrapper ────────────────────────────────────────────────────────

/// Result type for banking operations.
pub type BankingResult<T> = Result<T, BankingError>;

/// Abort the current banking run and reset to Idle, recording the error.
pub fn abort_banking_run(
    controller: &mut BankingCycleController,
    error: BankingError,
) -> BankingError {
    controller.state = BankingState::Idle;
    controller.consolidation_queue.clear();
    controller.consolidation_amounts.clear();
    error
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BankingConfig {
        BankingConfig {
            banker_name: "Teller_Grimbold".into(),
            mule_character: "BankMule".into(),
            consolidate_threshold: 100,
            travel_ticks: 5,
            step_delay: 2,
            bank_interval_ticks: 200,
        }
    }

    fn make_controller() -> BankingCycleController {
        BankingCycleController::new(test_config())
    }

    // ── Happy-path deposit ────────────────────────────────────────────────────

    #[test]
    fn test_happy_path_deposit() {
        let mut ctrl = make_controller();
        let pid = 200_u32;

        // Give BankMule 500pp
        ctrl.ledger.set_balance("BankMule", 500);

        // Start run — only mule, no consolidation chars
        ctrl.start_banking_run(pid, &[("BankMule".into(), pid)], 0);
        assert_eq!(ctrl.state, BankingState::NavigatingToBank);

        // Travel — no commands while in transit
        for t in 0..5 {
            let cmds = ctrl.tick(t);
            assert!(
                cmds.is_empty(),
                "tick {t} should emit nothing during travel"
            );
        }

        // Arrive at bank — transition to Depositing(Targeting), no cmds yet
        let cmds = ctrl.tick(5);
        assert!(
            matches!(
                ctrl.state,
                BankingState::Depositing {
                    step: DepositStep::Targeting
                }
            ),
            "expected Depositing(Targeting), got {:?}",
            ctrl.state
        );
        assert!(cmds.is_empty());

        // step_delay=2: tick 6 → still blocked (5+2>6 is false — 6-5=1 < 2)
        let cmds = ctrl.tick(6);
        assert!(cmds.is_empty());

        // tick 7 → Targeting executed → OpeningWindow
        let cmds = ctrl.tick(7);
        assert!(
            matches!(
                ctrl.state,
                BankingState::Depositing {
                    step: DepositStep::OpeningWindow
                }
            ),
            "expected OpeningWindow"
        );
        assert!(cmds.iter().any(|(_, c)| c.contains("Teller_Grimbold")));

        // tick 9 → OpeningWindow → DepositingPlat
        let cmds = ctrl.tick(9);
        assert!(matches!(
            ctrl.state,
            BankingState::Depositing {
                step: DepositStep::DepositingPlat
            }
        ),);
        assert!(cmds.iter().any(|(_, c)| c.contains("right target")));

        // tick 11 → DepositingPlat → ClosingWindow; deposit command emitted
        let cmds = ctrl.tick(11);
        assert!(matches!(
            ctrl.state,
            BankingState::Depositing {
                step: DepositStep::ClosingWindow
            }
        ),);
        assert!(
            cmds.iter()
                .any(|(_, c)| c.contains("/platinum deposit 500"))
        );
        // Ledger should reflect deducted balance
        assert_eq!(ctrl.ledger.balance("BankMule"), 0);
        assert_eq!(ctrl.ledger.history.len(), 1);
        assert_eq!(ctrl.ledger.history[0].amount, 500);

        // tick 13 → ClosingWindow → ReturningHome (no consolidation)
        let cmds = ctrl.tick(13);
        assert_eq!(ctrl.state, BankingState::ReturningHome);
        assert!(cmds.iter().any(|(_, c)| c.contains("BBW_Done_Button")));

        // Return travel
        for t in 13..18 {
            let cmds = ctrl.tick(t);
            assert!(
                cmds.is_empty(),
                "tick {t} should be empty during return travel"
            );
        }

        // tick 18 → Idle
        let cmds = ctrl.tick(18);
        assert_eq!(ctrl.state, BankingState::Idle);
        assert_eq!(ctrl.last_bank_tick, 18);
        assert!(!cmds.is_empty());
    }

    // ── Consolidation across 3 characters ────────────────────────────────────

    #[test]
    fn test_consolidation_across_3_characters() {
        let mut ctrl = make_controller();
        let mule_pid = 300_u32;

        // Setup: mule + 3 rich characters
        ctrl.ledger.set_balance("BankMule", 0);
        ctrl.ledger.set_balance("Warrior1", 500);
        ctrl.ledger.set_balance("Cleric2", 300);
        ctrl.ledger.set_balance("Wizard3", 200);

        let chars: Vec<(String, u32)> = vec![
            ("BankMule".into(), mule_pid),
            ("Warrior1".into(), 301),
            ("Cleric2".into(), 302),
            ("Wizard3".into(), 303),
        ];

        ctrl.start_banking_run(mule_pid, &chars, 0);
        assert_eq!(ctrl.consolidation_queue.len(), 3);

        // Navigate to bank (travel_ticks=5)
        ctrl.tick(5); // arrive → Depositing(Targeting)

        // Skip through deposit steps (step_delay=2)
        ctrl.tick(7); // Targeting → OpeningWindow
        ctrl.tick(9); // OpeningWindow → DepositingPlat
        ctrl.tick(11); // DepositingPlat → ClosingWindow  (deposit_amount=0, no cmd)
        ctrl.tick(13); // ClosingWindow → Consolidating{0}

        assert!(
            matches!(ctrl.state, BankingState::Consolidating { index: 0 }),
            "expected Consolidating{{0}}, got {:?}",
            ctrl.state
        );

        // Consolidating index 0 — Warrior1
        let cmds = ctrl.tick(15);
        assert!(matches!(
            ctrl.state,
            BankingState::Consolidating { index: 1 }
        ),);
        let cmd_text = cmds
            .iter()
            .map(|(_, c)| c.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            cmd_text.contains("BankMule"),
            "cmd should target mule: {cmd_text}"
        );
        assert!(
            cmd_text.contains("/plat give"),
            "should use /plat give: {cmd_text}"
        );

        // Consolidating index 1 — Cleric2
        let cmds = ctrl.tick(17);
        assert!(matches!(
            ctrl.state,
            BankingState::Consolidating { index: 2 }
        ));
        let cmd_text = cmds
            .iter()
            .map(|(_, c)| c.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(cmd_text.contains("BankMule"));

        // Consolidating index 2 — Wizard3
        let cmds = ctrl.tick(19);
        assert!(matches!(
            ctrl.state,
            BankingState::Consolidating { index: 3 }
        ));
        let cmd_text = cmds
            .iter()
            .map(|(_, c)| c.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(cmd_text.contains("BankMule"));

        // index 3 >= queue.len(3) → ReturningHome
        ctrl.tick(21);
        assert_eq!(ctrl.state, BankingState::ReturningHome);

        // Return home
        let _ = ctrl.tick(26);
        assert_eq!(ctrl.state, BankingState::Idle);

        // All 3 consolidation deposits should be in history
        let consolidation_entries: Vec<_> = ctrl
            .ledger
            .history
            .iter()
            .filter(|r| r.character != "BankMule")
            .collect();
        assert_eq!(consolidation_entries.len(), 3);
    }

    // ── Fail-safe on navigation error ─────────────────────────────────────────

    #[test]
    fn test_fail_safe_on_navigation_error() {
        let mut ctrl = make_controller();
        ctrl.ledger.set_balance("BankMule", 100);

        ctrl.start_banking_run(200, &[("BankMule".into(), 200)], 0);
        assert_eq!(ctrl.state, BankingState::NavigatingToBank);

        // Simulate navigation error mid-travel
        let err = abort_banking_run(&mut ctrl, BankingError::NavigationFailed);
        assert_eq!(err, BankingError::NavigationFailed);
        assert_eq!(ctrl.state, BankingState::Idle);
        assert!(ctrl.consolidation_queue.is_empty());

        // Controller is usable again — can start a new run
        ctrl.start_banking_run(200, &[("BankMule".into(), 200)], 50);
        assert_eq!(ctrl.state, BankingState::NavigatingToBank);
    }

    // ── Additional unit tests ─────────────────────────────────────────────────

    #[test]
    fn test_idle_tick_is_noop() {
        let mut ctrl = make_controller();
        let cmds = ctrl.tick(999);
        assert!(cmds.is_empty());
        assert_eq!(ctrl.state, BankingState::Idle);
    }

    #[test]
    fn test_needs_bank_run_timing() {
        let ctrl = make_controller();
        assert!(!ctrl.needs_bank_run(100));
        assert!(!ctrl.needs_bank_run(199));
        assert!(ctrl.needs_bank_run(200));
        assert!(ctrl.needs_bank_run(500));
    }

    #[test]
    fn test_needs_bank_run_false_when_not_idle() {
        let mut ctrl = make_controller();
        ctrl.state = BankingState::NavigatingToBank;
        assert!(!ctrl.needs_bank_run(999));
    }

    #[test]
    fn test_start_banking_run_idempotent() {
        let mut ctrl = make_controller();
        ctrl.start_banking_run(100, &[], 0);
        assert_eq!(ctrl.state, BankingState::NavigatingToBank);
        // Second call while not Idle is a no-op
        ctrl.start_banking_run(100, &[], 0);
        assert_eq!(ctrl.state, BankingState::NavigatingToBank);
    }

    #[test]
    fn test_below_threshold_chars_not_queued() {
        let mut ctrl = make_controller(); // threshold = 100
        ctrl.ledger.set_balance("PoorChar", 50); // below threshold
        ctrl.ledger.set_balance("RichChar", 200); // above threshold

        let chars = vec![("PoorChar".into(), 301_u32), ("RichChar".into(), 302_u32)];
        ctrl.start_banking_run(300, &chars, 0);
        assert_eq!(ctrl.consolidation_queue.len(), 1);
        assert_eq!(ctrl.consolidation_queue[0].0, "RichChar");
    }

    #[test]
    fn test_mule_excluded_from_consolidation() {
        let mut ctrl = make_controller();
        ctrl.ledger.set_balance("BankMule", 9999); // mule should never consolidate itself
        ctrl.ledger.set_balance("Fighter", 500);

        let chars = vec![("BankMule".into(), 300_u32), ("Fighter".into(), 301_u32)];
        ctrl.start_banking_run(300, &chars, 0);
        // Only Fighter should be in the queue
        assert_eq!(ctrl.consolidation_queue.len(), 1);
        assert_eq!(ctrl.consolidation_queue[0].0, "Fighter");
    }

    #[test]
    fn test_plat_ledger_set_and_balance() {
        let mut ledger = PlatLedger::default();
        ledger.set_balance("Char1", 1000);
        assert_eq!(ledger.balance("Char1"), 1000);
        assert_eq!(ledger.balance("Unknown"), 0);
    }

    #[test]
    fn test_plat_ledger_record_deposit() {
        let mut ledger = PlatLedger::default();
        ledger.set_balance("Char1", 500);
        ledger.record_deposit("Char1", 200, 10);
        assert_eq!(ledger.balance("Char1"), 300);
        assert_eq!(ledger.history.len(), 1);
        assert_eq!(ledger.history[0].amount, 200);
        assert_eq!(ledger.history[0].tick, 10);
    }

    #[test]
    fn test_plat_ledger_total_plat() {
        let mut ledger = PlatLedger::default();
        ledger.set_balance("A", 100);
        ledger.set_balance("B", 200);
        ledger.set_balance("C", 300);
        assert_eq!(ledger.total_plat(), 600);
    }

    #[test]
    fn test_plat_ledger_saturating_subtract() {
        let mut ledger = PlatLedger::default();
        ledger.set_balance("Poor", 10);
        ledger.record_deposit("Poor", 999, 1); // would underflow without saturation
        assert_eq!(ledger.balance("Poor"), 0);
    }

    #[test]
    fn test_history_capped_at_256() {
        let mut ledger = PlatLedger::default();
        ledger.set_balance("Char", u64::MAX);
        for i in 0..300_u64 {
            ledger.record_deposit("Char", 0, i);
        }
        assert_eq!(ledger.history.len(), PlatLedger::MAX_HISTORY);
    }

    #[test]
    fn test_abort_clears_queue() {
        let mut ctrl = make_controller();
        ctrl.ledger.set_balance("Rich", 500);
        ctrl.start_banking_run(100, &[("Rich".into(), 101)], 0);
        assert!(!ctrl.consolidation_queue.is_empty());

        abort_banking_run(&mut ctrl, BankingError::BankWindowFailed);
        assert!(ctrl.consolidation_queue.is_empty());
        assert_eq!(ctrl.state, BankingState::Idle);
    }

    #[test]
    fn test_banking_error_equality() {
        assert_eq!(
            BankingError::NavigationFailed,
            BankingError::NavigationFailed
        );
        assert_ne!(
            BankingError::NavigationFailed,
            BankingError::BankWindowFailed
        );
    }

    #[test]
    fn test_state_debug_format() {
        let s = format!("{:?}", BankingState::Consolidating { index: 2 });
        assert!(s.contains("Consolidating"));
        assert!(s.contains('2'));
    }

    #[test]
    fn test_no_deposit_when_mule_has_zero_plat() {
        let mut ctrl = make_controller();
        // BankMule has 0pp — deposit command should not be emitted
        ctrl.ledger.set_balance("BankMule", 0);
        ctrl.start_banking_run(200, &[("BankMule".into(), 200)], 0);

        ctrl.tick(5); // arrive
        ctrl.tick(7); // Targeting
        ctrl.tick(9); // OpeningWindow
        let cmds = ctrl.tick(11); // DepositingPlat — no deposit
        assert!(
            cmds.is_empty(),
            "no deposit command expected when balance is 0"
        );
    }
}
