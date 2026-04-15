//! Zone failure code mapping and recovery action registry.
//!
//! Maps EverQuest zone failure codes to deterministic recovery actions.
//! Each code represents a specific failure mode during zone transitions,
//! with an associated recovery strategy to attempt before retrying.

use std::time::{Duration, Instant};

/// Zone failure codes returned by EQ zone operations.
///
/// These codes indicate specific failure modes during zone transitions.
/// Each code maps to a recovery action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ZoneFailureCode {
    /// Success — zone transition completed (code 0).
    Success = 0,

    /// General zone failure — unable to process zone request (code -1).
    GeneralFailure = -1,

    /// Zone too far — destination zone or waypoint is out of reach (code -2).
    TooFar = -2,

    /// Wrong zone type — attempted zone transition to incompatible zone type
    /// (code -3).
    WrongType = -3,

    /// Character level too low — character does not meet zone entry
    /// requirements (code -4).
    LevelTooLow = -4,

    /// Character level too high — character exceeds zone level restrictions
    /// (code -5).
    LevelTooHigh = -5,

    /// Invalid coordinates — landing position is invalid or out of bounds (code
    /// -6).
    InvalidCoordinates = -6,

    /// No path available — no valid path to zone or waypoint exists (code -7).
    NoPathAvailable = -7,

    /// Zone temporarily locked — zone is instanced and not available (code -8).
    ZoneLockedInstance = -8,

    /// Insufficient mana for port spell — caster does not have enough mana
    /// (code -9).
    InsufficientMana = -9,

    /// Spell resisted — port spell or zone ability was resisted (code -10).
    SpellResisted = -10,

    /// Zone queue full — zone has reached max capacity (code -11).
    ZoneQueueFull = -11,

    /// Raid lockout active — character has active raid lockout for zone (code
    /// -12).
    RaidLockoutActive = -12,

    /// Character already zoning — concurrent zone transition detected (code
    /// -13).
    AlreadyZoning = -13,

    /// Zone load timeout — zone data failed to load within time limit (code
    /// -14).
    ZoneLoadTimeout = -14,

    /// Movement blocked — collision or environmental obstruction (code -15).
    MovementBlocked = -15,

    /// Out of zone bounds — character position is outside valid zone area (code
    /// -16).
    OutOfBounds = -16,

    /// Guild hall unavailable — guild hall zone not accessible (code -17).
    GuildHallUnavailable = -17,

    /// Player in combat — cannot zone while in active combat (code -18).
    PlayerInCombat = -18,

    /// Port spell duration expired — zone port spell timeout (code -19).
    PortSpellExpired = -19,

    /// Zone restricted by server — zone entry forbidden by server rules (code
    /// -20).
    ZoneRestricted = -20,

    /// Expansion not unlocked — character or account lacks expansion access
    /// (code -21).
    ExpansionNotUnlocked = -21,

    /// Corpse recovery zone — character has corpse in zone blocking entry (code
    /// -22).
    CorpseInZone = -22,

    /// Network timeout — communication with zone server lost (code -23).
    NetworkTimeout = -23,

    /// Authentication failed — session validation failed for zone (code -24).
    AuthenticationFailed = -24,

    /// Zone data corrupted — zone data received is invalid (code -25).
    ZoneDataCorrupted = -25,

    /// Unknown failure — unmapped or unexpected failure code (code -999).
    Unknown = -999,
}

impl ZoneFailureCode {
    /// Convert a numeric code to a ZoneFailureCode enum variant.
    ///
    /// Returns `ZoneFailureCode::Unknown` for unrecognized codes.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Success,
            -1 => Self::GeneralFailure,
            -2 => Self::TooFar,
            -3 => Self::WrongType,
            -4 => Self::LevelTooLow,
            -5 => Self::LevelTooHigh,
            -6 => Self::InvalidCoordinates,
            -7 => Self::NoPathAvailable,
            -8 => Self::ZoneLockedInstance,
            -9 => Self::InsufficientMana,
            -10 => Self::SpellResisted,
            -11 => Self::ZoneQueueFull,
            -12 => Self::RaidLockoutActive,
            -13 => Self::AlreadyZoning,
            -14 => Self::ZoneLoadTimeout,
            -15 => Self::MovementBlocked,
            -16 => Self::OutOfBounds,
            -17 => Self::GuildHallUnavailable,
            -18 => Self::PlayerInCombat,
            -19 => Self::PortSpellExpired,
            -20 => Self::ZoneRestricted,
            -21 => Self::ExpansionNotUnlocked,
            -22 => Self::CorpseInZone,
            -23 => Self::NetworkTimeout,
            -24 => Self::AuthenticationFailed,
            -25 => Self::ZoneDataCorrupted,
            _ => Self::Unknown,
        }
    }

    /// Return the raw numeric code.
    pub const fn as_code(&self) -> i32 {
        *self as i32
    }

    /// Human-readable description of the failure code.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Success => "Zone transition succeeded",
            Self::GeneralFailure => "General zone failure — unable to process request",
            Self::TooFar => "Zone too far — destination out of reach",
            Self::WrongType => "Wrong zone type — incompatible destination",
            Self::LevelTooLow => "Character level too low — does not meet requirements",
            Self::LevelTooHigh => "Character level too high — exceeds restrictions",
            Self::InvalidCoordinates => "Invalid coordinates — landing position invalid",
            Self::NoPathAvailable => "No path available — cannot reach destination",
            Self::ZoneLockedInstance => "Zone locked — instance not available",
            Self::InsufficientMana => "Insufficient mana — cannot cast port spell",
            Self::SpellResisted => "Spell resisted — zone ability was resisted",
            Self::ZoneQueueFull => "Zone queue full — at capacity",
            Self::RaidLockoutActive => "Raid lockout active — character cannot enter",
            Self::AlreadyZoning => "Already zoning — concurrent transition detected",
            Self::ZoneLoadTimeout => "Zone load timeout — data failed to load",
            Self::MovementBlocked => "Movement blocked — collision or obstruction",
            Self::OutOfBounds => "Out of zone bounds — invalid position",
            Self::GuildHallUnavailable => "Guild hall unavailable — not accessible",
            Self::PlayerInCombat => "Player in combat — cannot zone during combat",
            Self::PortSpellExpired => "Port spell duration expired — spell timed out",
            Self::ZoneRestricted => "Zone restricted — entry forbidden by server",
            Self::ExpansionNotUnlocked => "Expansion not unlocked — no access",
            Self::CorpseInZone => "Corpse in zone — blocks entry",
            Self::NetworkTimeout => "Network timeout — lost connection",
            Self::AuthenticationFailed => "Authentication failed — session invalid",
            Self::ZoneDataCorrupted => "Zone data corrupted — invalid data received",
            Self::Unknown => "Unknown failure — unmapped error code",
        }
    }
}

/// Recovery actions to attempt when a zone failure occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry the zone transition immediately.
    RetryZone,

    /// Retry with new coordinates (fallback to safe location).
    UseNewCoords,

    /// Clear the zone command queue and retry.
    ClearQueue,

    /// Wait for player to leave combat, then retry.
    WaitOutOfCombat,

    /// Wait for spell cooldown, then retry port spell.
    WaitSpellCooldown,

    /// Wait for mana to regenerate, then retry port spell.
    WaitManaRegen,

    /// Wait for zone queue to deplete, then retry.
    WaitZoneQueue,

    /// Abandon zone attempt — failure is unrecoverable.
    Abandon,

    /// Investigate player state (corpse, lockout, level) before retry.
    InvestigateState,

    /// Revalidate zone path and try alternate route.
    RevalidatePath,
}

/// Zone failure recovery state — tracks failure, recovery action, and retry
/// progress.
#[derive(Debug, Clone)]
pub struct ZoneFailureState {
    /// The failure code that occurred.
    pub code: ZoneFailureCode,

    /// Human-readable reason for the failure.
    pub reason: String,

    /// Recommended recovery action.
    pub recovery_action: RecoveryAction,

    /// Number of recovery attempts made so far.
    pub retry_count: u32,

    /// Maximum number of retries before giving up.
    pub max_retries: u32,

    /// Time when this failure was first recorded.
    pub failed_at: Instant,

    /// Current backoff deadline (when next retry is permitted).
    pub backoff_until: Instant,
}

impl ZoneFailureState {
    /// Create a new failure state for a zone code.
    pub fn new(code: ZoneFailureCode, max_retries: u32) -> Self {
        let recovery_action = Self::map_action(code);
        let reason = code.description().to_string();
        let now = Instant::now();

        Self {
            code,
            reason,
            recovery_action,
            retry_count: 0,
            max_retries,
            failed_at: now,
            backoff_until: now,
        }
    }

    /// Map a failure code to its recommended recovery action.
    fn map_action(code: ZoneFailureCode) -> RecoveryAction {
        match code {
            ZoneFailureCode::Success => RecoveryAction::RetryZone,
            ZoneFailureCode::GeneralFailure => RecoveryAction::RetryZone,
            ZoneFailureCode::TooFar => RecoveryAction::UseNewCoords,
            ZoneFailureCode::WrongType => RecoveryAction::Abandon,
            ZoneFailureCode::LevelTooLow => RecoveryAction::Abandon,
            ZoneFailureCode::LevelTooHigh => RecoveryAction::Abandon,
            ZoneFailureCode::InvalidCoordinates => RecoveryAction::UseNewCoords,
            ZoneFailureCode::NoPathAvailable => RecoveryAction::RevalidatePath,
            ZoneFailureCode::ZoneLockedInstance => RecoveryAction::WaitZoneQueue,
            ZoneFailureCode::InsufficientMana => RecoveryAction::WaitManaRegen,
            ZoneFailureCode::SpellResisted => RecoveryAction::RetryZone,
            ZoneFailureCode::ZoneQueueFull => RecoveryAction::WaitZoneQueue,
            ZoneFailureCode::RaidLockoutActive => RecoveryAction::Abandon,
            ZoneFailureCode::AlreadyZoning => RecoveryAction::RetryZone,
            ZoneFailureCode::ZoneLoadTimeout => RecoveryAction::RetryZone,
            ZoneFailureCode::MovementBlocked => RecoveryAction::UseNewCoords,
            ZoneFailureCode::OutOfBounds => RecoveryAction::UseNewCoords,
            ZoneFailureCode::GuildHallUnavailable => RecoveryAction::Abandon,
            ZoneFailureCode::PlayerInCombat => RecoveryAction::WaitOutOfCombat,
            ZoneFailureCode::PortSpellExpired => RecoveryAction::RetryZone,
            ZoneFailureCode::ZoneRestricted => RecoveryAction::Abandon,
            ZoneFailureCode::ExpansionNotUnlocked => RecoveryAction::Abandon,
            ZoneFailureCode::CorpseInZone => RecoveryAction::InvestigateState,
            ZoneFailureCode::NetworkTimeout => RecoveryAction::RetryZone,
            ZoneFailureCode::AuthenticationFailed => RecoveryAction::RetryZone,
            ZoneFailureCode::ZoneDataCorrupted => RecoveryAction::RetryZone,
            ZoneFailureCode::Unknown => RecoveryAction::RetryZone,
        }
    }

    /// Check if retries are still available.
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Check if the backoff period has elapsed.
    pub fn backoff_expired(&self) -> bool {
        Instant::now() >= self.backoff_until
    }

    /// Increment retry count and update backoff deadline using exponential
    /// backoff.
    ///
    /// Backoff time = 2^retry_count seconds (capped at `max_backoff`).
    pub fn advance_backoff(&mut self, max_backoff: Duration) {
        self.retry_count += 1;
        let backoff_secs = 2_u64.pow(self.retry_count.saturating_sub(1).min(10));
        let backoff = Duration::from_secs(backoff_secs).min(max_backoff);
        self.backoff_until = Instant::now() + backoff;

        tracing::debug!(
            code = ?self.code,
            retry_count = self.retry_count,
            backoff_secs = backoff.as_secs(),
            "Zone failure backoff scheduled"
        );
    }

    /// Check if this failure is retryable (has a retry action and retries
    /// available).
    pub fn is_retryable(&self) -> bool {
        !matches!(
            self.recovery_action,
            RecoveryAction::Abandon | RecoveryAction::InvestigateState
        ) && self.can_retry()
    }

    /// Total elapsed time since the failure first occurred.
    pub fn elapsed(&self) -> Duration {
        self.failed_at.elapsed()
    }

    /// Time remaining until the next retry is permitted (or zero if backoff
    /// expired).
    pub fn time_until_retry(&self) -> Duration {
        if self.backoff_until > Instant::now() {
            self.backoff_until - Instant::now()
        } else {
            Duration::from_secs(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // ─── ZoneFailureCode enum tests ───────────────────────────────────────

    #[test]
    fn code_from_code_success() {
        assert_eq!(ZoneFailureCode::from_code(0), ZoneFailureCode::Success);
    }

    #[test]
    fn code_from_code_negative_values() {
        assert_eq!(ZoneFailureCode::from_code(-2), ZoneFailureCode::TooFar);
        assert_eq!(
            ZoneFailureCode::from_code(-18),
            ZoneFailureCode::PlayerInCombat
        );
        assert_eq!(
            ZoneFailureCode::from_code(-25),
            ZoneFailureCode::ZoneDataCorrupted
        );
    }

    #[test]
    fn code_from_code_unknown() {
        assert_eq!(ZoneFailureCode::from_code(-999), ZoneFailureCode::Unknown);
        assert_eq!(ZoneFailureCode::from_code(9999), ZoneFailureCode::Unknown);
    }

    #[test]
    fn code_as_code_round_trip() {
        let codes = [
            ZoneFailureCode::Success,
            ZoneFailureCode::TooFar,
            ZoneFailureCode::InvalidCoordinates,
            ZoneFailureCode::PlayerInCombat,
        ];
        for code in &codes {
            let numeric = code.as_code();
            let back = ZoneFailureCode::from_code(numeric);
            assert_eq!(*code, back);
        }
    }

    #[test]
    fn code_description_is_not_empty() {
        let codes = [
            ZoneFailureCode::Success,
            ZoneFailureCode::GeneralFailure,
            ZoneFailureCode::TooFar,
            ZoneFailureCode::WrongType,
            ZoneFailureCode::PlayerInCombat,
            ZoneFailureCode::Unknown,
        ];
        for code in &codes {
            assert!(!code.description().is_empty());
            assert!(code.description().len() > 3);
        }
    }

    // ─── Recovery action mapping tests ────────────────────────────────────

    #[test]
    fn map_action_too_far() {
        assert_eq!(
            ZoneFailureState::map_action(ZoneFailureCode::TooFar),
            RecoveryAction::UseNewCoords
        );
    }

    #[test]
    fn map_action_invalid_coordinates() {
        assert_eq!(
            ZoneFailureState::map_action(ZoneFailureCode::InvalidCoordinates),
            RecoveryAction::UseNewCoords
        );
    }

    #[test]
    fn map_action_player_in_combat() {
        assert_eq!(
            ZoneFailureState::map_action(ZoneFailureCode::PlayerInCombat),
            RecoveryAction::WaitOutOfCombat
        );
    }

    #[test]
    fn map_action_insufficient_mana() {
        assert_eq!(
            ZoneFailureState::map_action(ZoneFailureCode::InsufficientMana),
            RecoveryAction::WaitManaRegen
        );
    }

    #[test]
    fn map_action_zone_queue_full() {
        assert_eq!(
            ZoneFailureState::map_action(ZoneFailureCode::ZoneQueueFull),
            RecoveryAction::WaitZoneQueue
        );
    }

    #[test]
    fn map_action_abandonment_cases() {
        let abandon_codes = [
            ZoneFailureCode::WrongType,
            ZoneFailureCode::LevelTooLow,
            ZoneFailureCode::LevelTooHigh,
            ZoneFailureCode::RaidLockoutActive,
            ZoneFailureCode::GuildHallUnavailable,
            ZoneFailureCode::ZoneRestricted,
            ZoneFailureCode::ExpansionNotUnlocked,
        ];
        for code in &abandon_codes {
            assert_eq!(ZoneFailureState::map_action(*code), RecoveryAction::Abandon);
        }
    }

    // ─── ZoneFailureState lifecycle tests ─────────────────────────────────

    #[test]
    fn state_new_initializes_correctly() {
        let state = ZoneFailureState::new(ZoneFailureCode::TooFar, 3);
        assert_eq!(state.code, ZoneFailureCode::TooFar);
        assert_eq!(state.retry_count, 0);
        assert_eq!(state.max_retries, 3);
        assert_eq!(state.recovery_action, RecoveryAction::UseNewCoords);
        assert!(!state.reason.is_empty());
    }

    #[test]
    fn state_can_retry_before_max() {
        let state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        assert!(state.can_retry());
        assert_eq!(state.retry_count, 0);
    }

    #[test]
    fn state_can_retry_after_increment() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        state.retry_count = 1;
        assert!(state.can_retry());
        state.retry_count = 2;
        assert!(state.can_retry());
        state.retry_count = 3;
        assert!(!state.can_retry());
    }

    #[test]
    fn state_backoff_expired_initially() {
        let state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        assert!(state.backoff_expired());
    }

    #[test]
    fn state_advance_backoff_single_retry() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        state.advance_backoff(Duration::from_secs(60));
        assert_eq!(state.retry_count, 1);
        assert!(!state.backoff_expired());
    }

    #[test]
    fn state_advance_backoff_exponential() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 5);
        let max_backoff = Duration::from_secs(120);

        // Retry 1: 2^0 = 1 second
        state.advance_backoff(max_backoff);
        assert_eq!(state.retry_count, 1);

        // Simulate backoff expiry and retry 2: 2^1 = 2 seconds
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        state.advance_backoff(max_backoff);
        assert_eq!(state.retry_count, 2);

        // Retry 3: 2^2 = 4 seconds
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        state.advance_backoff(max_backoff);
        assert_eq!(state.retry_count, 3);
    }

    #[test]
    fn state_advance_backoff_respects_max() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 10);
        let max_backoff = Duration::from_secs(10);

        // Advance multiple times to trigger large exponent
        for _ in 0..5 {
            state.backoff_until = Instant::now() - Duration::from_millis(1);
            state.advance_backoff(max_backoff);
        }

        // After many retries, backoff should be capped at max_backoff
        let now = Instant::now();
        let remaining = state.backoff_until.saturating_duration_since(now);
        assert!(remaining <= max_backoff + Duration::from_millis(100));
    }

    #[test]
    fn state_is_retryable_for_retry_codes() {
        let state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        assert!(state.is_retryable());
    }

    #[test]
    fn state_not_retryable_for_abandon() {
        let state = ZoneFailureState::new(ZoneFailureCode::WrongType, 3);
        assert!(!state.is_retryable());
    }

    #[test]
    fn state_not_retryable_when_exhausted() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 2);
        state.retry_count = 2;
        assert!(!state.is_retryable());
    }

    #[test]
    fn state_not_retryable_for_investigate_state() {
        let state = ZoneFailureState::new(ZoneFailureCode::CorpseInZone, 3);
        assert!(!state.is_retryable());
    }

    #[test]
    fn state_elapsed_time_accumulates() {
        let state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        let elapsed1 = state.elapsed();
        thread::sleep(Duration::from_millis(10));
        let elapsed2 = state.elapsed();
        assert!(elapsed2 >= elapsed1);
        assert!(elapsed2 > Duration::from_millis(5));
    }

    #[test]
    fn state_time_until_retry_after_advance() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        state.advance_backoff(Duration::from_secs(60));
        let time_until = state.time_until_retry();
        assert!(time_until > Duration::from_millis(900));
        assert!(time_until <= Duration::from_secs(2));
    }

    #[test]
    fn state_time_until_retry_expired() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        state.advance_backoff(Duration::from_secs(60));
        state.backoff_until = Instant::now() - Duration::from_secs(1);
        assert_eq!(state.time_until_retry(), Duration::from_secs(0));
    }

    #[test]
    fn state_reason_matches_code_description() {
        let state = ZoneFailureState::new(ZoneFailureCode::PlayerInCombat, 3);
        assert_eq!(state.reason, ZoneFailureCode::PlayerInCombat.description());
    }

    #[test]
    fn state_handles_all_code_types() {
        let codes = [
            ZoneFailureCode::Success,
            ZoneFailureCode::GeneralFailure,
            ZoneFailureCode::TooFar,
            ZoneFailureCode::WrongType,
            ZoneFailureCode::PlayerInCombat,
            ZoneFailureCode::InsufficientMana,
            ZoneFailureCode::NetworkTimeout,
            ZoneFailureCode::Unknown,
        ];

        for code in &codes {
            let state = ZoneFailureState::new(*code, 3);
            assert_eq!(state.code, *code);
            assert_eq!(state.retry_count, 0);
            assert!(state.can_retry() || !state.is_retryable());
        }
    }

    #[test]
    fn state_exhaustion_scenario() {
        let mut state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
        let max_backoff = Duration::from_secs(60);

        assert!(state.is_retryable());
        state.advance_backoff(max_backoff);
        assert!(state.is_retryable());

        state.advance_backoff(max_backoff);
        assert!(state.is_retryable());

        state.advance_backoff(max_backoff);
        assert!(!state.is_retryable());
    }

    #[test]
    fn state_corpse_in_zone_requires_investigation() {
        let state = ZoneFailureState::new(ZoneFailureCode::CorpseInZone, 3);
        assert_eq!(state.recovery_action, RecoveryAction::InvestigateState);
        assert!(!state.is_retryable());
    }
}
