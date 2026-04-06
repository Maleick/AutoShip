use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configurable retry policy with exponential backoff for login failures.
///
/// Delays escalate as: `initial_delay * backoff_multiplier^attempt`, capped at `max_delay`.
/// Optional jitter adds up to ±25% randomization to prevent thundering-herd retries
/// across multiple clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Delay before the first retry.
    #[serde(with = "serde_duration_secs")]
    pub initial_delay: Duration,
    /// Upper bound on any single retry delay.
    #[serde(with = "serde_duration_secs")]
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each attempt.
    pub backoff_multiplier: f64,
    /// Whether to add ±25% random jitter to each delay.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Returns the next retry delay if retries remain, or `None` if exhausted.
    ///
    /// The returned duration includes jitter (if enabled) and is clamped to `max_delay`.
    pub fn next_delay(&self, state: &RetryState) -> Option<Duration> {
        if state.attempt_count >= self.max_retries {
            return None;
        }

        let base_secs = self.initial_delay.as_secs_f64()
            * self.backoff_multiplier.powi(state.attempt_count as i32);
        let clamped_secs = base_secs.min(self.max_delay.as_secs_f64());

        let final_secs = if self.jitter {
            apply_jitter(clamped_secs)
        } else {
            clamped_secs
        };

        Some(Duration::from_secs_f64(final_secs.max(0.0)))
    }
}

/// Tracks retry progress for a single login attempt sequence.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetryState {
    /// Number of retry attempts made so far.
    pub attempt_count: u32,
    /// The last error that triggered a retry.
    pub last_error: Option<LoginError>,
}

impl RetryState {
    /// Record a failed attempt.
    pub fn record_failure(&mut self, error: LoginError) {
        self.attempt_count += 1;
        self.last_error = Some(error);
    }

    /// Reset state for a fresh retry sequence.
    pub fn reset(&mut self) {
        self.attempt_count = 0;
        self.last_error = None;
    }
}

/// Apply ±25% jitter to a delay value.
fn apply_jitter(secs: f64) -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .subsec_nanos();
    let jitter_factor = (nanos as f64 / u32::MAX as f64) * 0.5 - 0.25;
    secs * (1.0 + jitter_factor)
}

/// Serde helper to serialize `Duration` as fractional seconds (f64) for TOML/JSON.
mod serde_duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dur: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(dur.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        if secs < 0.0 {
            return Err(serde::de::Error::custom("duration cannot be negative"));
        }
        Ok(Duration::from_secs_f64(secs))
    }
}

/// Current phase of the automated login state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginPhase {
    /// Login has not been initiated.
    NotStarted,
    /// EQ process is being launched.
    ProcessLaunching,
    /// At the EQ login screen (eqmain.dll loaded).
    AtLoginScreen,
    /// Typing account name and password into login fields.
    EnteringCredentials,
    /// Navigating the server selection screen.
    ServerSelecting,
    /// At the character select screen, picking a character.
    CharacterSelecting,
    /// Character is zoning into the game world.
    Zoning,
    /// Character is fully in-world.
    InWorld,
    /// Running post-login setup (buffs, group join, camp positioning).
    PostLoginSetup,
    /// Login complete, client is ready for orchestration.
    Ready,
    /// Login failed with an error.
    Failed {
        /// The error that caused login to fail.
        reason: LoginError,
    },
}

/// Errors that can occur during the automated login process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginError {
    /// Authentication failed (bad password).
    WrongPassword,
    /// Account is locked or suspended.
    AccountLocked,
    /// Character is already logged in and needs an explicit kick.
    CharacterAlreadyLoggedIn,
    /// Character is in offline trader mode and cannot log in automatically.
    OfflineTrader,
    /// Target server is down.
    ServerDown,
    /// Target server is at capacity.
    ServerFull,
    /// Expected character was not found at character select.
    CharacterNotFound {
        /// Character name we were looking for.
        expected: String,
        /// What was actually found (may be empty or a different name).
        found: String,
    },
    /// A phase exceeded its timeout.
    Timeout {
        /// Name of the phase that timed out.
        phase: String,
    },
    /// Multiple clients failed simultaneously (circuit breaker triggered).
    MassFailure,
}

/// Per-character account and server metadata for login orchestration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Login account name.
    pub account_name: String,
    /// Character name to select at character select.
    pub character_name: String,
    /// EQ class name (e.g. "Warrior", "Cleric").
    pub class_name: String,
    /// Character level.
    pub level: u8,
    /// Logical group ID for post-login grouping.
    pub group_id: u32,
    /// Target server name (e.g. "Teek", "FV").
    pub server_name: String,
}

/// A character discovered during login, with class/level/server metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeenCharacter {
    /// Character name.
    pub name: String,
    /// EQ class name (e.g. "Warrior", "Cleric").
    pub class_name: String,
    /// Character level at time of observation.
    pub level: u8,
    /// Server the character was seen on.
    pub server: String,
    /// ISO-8601 timestamp of when the character was last seen.
    pub last_seen: String,
}

/// Persistent cache of characters discovered during login.
///
/// Characters are keyed by `(server, name)` so re-observing a character on the
/// same server updates the existing entry rather than creating a duplicate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterCache {
    /// Map from `"server:name"` to the seen character data.
    characters: HashMap<String, SeenCharacter>,
}

impl CharacterCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Composite key for the internal map.
    fn key(server: &str, name: &str) -> String {
        format!("{server}:{name}")
    }

    /// Insert or update a character in the cache.
    pub fn insert(&mut self, character: SeenCharacter) {
        let k = Self::key(&character.server, &character.name);
        self.characters.insert(k, character);
    }

    /// Look up a character by server and name.
    pub fn lookup(&self, server: &str, name: &str) -> Option<&SeenCharacter> {
        self.characters.get(&Self::key(server, name))
    }

    /// Return the number of cached characters.
    pub fn len(&self) -> usize {
        self.characters.len()
    }

    /// Return true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }

    /// All cached characters as a slice-friendly iterator.
    pub fn iter(&self) -> impl Iterator<Item = &SeenCharacter> {
        self.characters.values()
    }

    /// Load a cache from a JSON file. Returns an empty cache if the file does
    /// not exist.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        serde_json::from_str(&data).map_err(|e| format!("failed to parse {}: {e}", path.display()))
    }

    /// Save the cache to a JSON file, creating parent directories if needed.
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create dir {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize cache: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))
    }
}

/// Maps human-readable EQ server display names to internal JoinServer IDs
/// and vice versa.
///
/// The resolver ships with a built-in table of known live and TLP servers.
/// Config-based overrides can be added at runtime via [`Self::add`].
/// All lookups are case-insensitive.
#[derive(Debug, Clone)]
pub struct ServerNameResolver {
    /// display name (lowercase) -> internal ID
    display_to_internal: HashMap<String, String>,
    /// internal ID (lowercase) -> display name (canonical casing)
    internal_to_display: HashMap<String, String>,
}

impl ServerNameResolver {
    /// Creates a resolver pre-populated with known EQ servers.
    #[must_use]
    pub fn new() -> Self {
        let mut resolver = Self {
            display_to_internal: HashMap::new(),
            internal_to_display: HashMap::new(),
        };

        let servers: &[(&str, &str)] = &[
            // --- Active TLP servers ---
            ("Teek", "teek"),
            ("Oakwynd", "oakwynd"),
            ("Mischief", "mischief"),
            ("Thornblade", "thornblade"),
            ("Aradune", "aradune"),
            ("Mangler", "mangler"),
            ("Selo", "selo"),
            ("Coirnav", "coirnav"),
            ("Agnarr", "agnarr"),
            ("Phinigel", "phinigel"),
            ("Ragefire", "ragefire"),
            ("Lockjaw", "lockjaw"),
            ("Yelinak", "yelinak"),
            ("Vaniki", "vaniki"),
            ("Tormax", "tormax"),
            // --- Live servers ---
            ("Firiona Vie", "firionavie"),
            ("FV", "firionavie"),
            ("Antonius Bayle", "antoniusbayle"),
            ("Bertoxxulous", "bertoxxulous"),
            ("Bristlebane", "bristlebane"),
            ("Cazic-Thule", "cazicthule"),
            ("Cazic Thule", "cazicthule"),
            ("Drinal", "drinal"),
            ("Erollisi Marr", "erollisimarr"),
            ("Luclin", "luclin"),
            ("Povar", "povar"),
            ("The Rathe", "rathe"),
            ("Rathe", "rathe"),
            ("Tunare", "tunare"),
            ("Xegony", "xegony"),
            ("Zek", "zek"),
            ("Vox", "vox"),
            // --- Test/Beta ---
            ("Test", "test"),
            ("Beta", "beta"),
        ];

        for &(display, internal) in servers {
            resolver.add(display, internal);
        }

        resolver
    }

    /// Adds or overwrites a display-name to internal-ID mapping.
    pub fn add(&mut self, display_name: &str, internal_id: &str) {
        self.display_to_internal
            .insert(display_name.to_lowercase(), internal_id.to_string());
        // Only set the canonical display name if this internal ID hasn't been
        // mapped yet (preserves the first/primary name for aliases like "FV").
        self.internal_to_display
            .entry(internal_id.to_lowercase())
            .or_insert_with(|| display_name.to_string());
    }

    /// Resolves a human-readable display name to its internal server ID.
    #[must_use]
    pub fn resolve(&self, display_name: &str) -> Option<&str> {
        self.display_to_internal
            .get(&display_name.to_lowercase())
            .map(String::as_str)
    }

    /// Reverse-resolves an internal server ID to its canonical display name.
    #[must_use]
    pub fn display_name(&self, internal_id: &str) -> Option<&str> {
        self.internal_to_display
            .get(&internal_id.to_lowercase())
            .map(String::as_str)
    }

    /// Returns the number of unique internal server IDs in the resolver.
    #[must_use]
    pub fn server_count(&self) -> usize {
        self.internal_to_display.len()
    }
}

impl Default for ServerNameResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_phase_all_variants_constructible() {
        let phases = [
            LoginPhase::NotStarted,
            LoginPhase::ProcessLaunching,
            LoginPhase::AtLoginScreen,
            LoginPhase::EnteringCredentials,
            LoginPhase::ServerSelecting,
            LoginPhase::CharacterSelecting,
            LoginPhase::Zoning,
            LoginPhase::InWorld,
            LoginPhase::PostLoginSetup,
            LoginPhase::Ready,
            LoginPhase::Failed {
                reason: LoginError::WrongPassword,
            },
        ];
        assert_eq!(phases.len(), 11);
    }

    #[test]
    fn login_phase_equality() {
        assert_eq!(LoginPhase::NotStarted, LoginPhase::NotStarted);
        assert_ne!(LoginPhase::NotStarted, LoginPhase::Ready);
        assert_ne!(LoginPhase::InWorld, LoginPhase::Zoning);
    }

    #[test]
    fn login_error_all_variants_constructible() {
        let errors = [
            LoginError::WrongPassword,
            LoginError::AccountLocked,
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::OfflineTrader,
            LoginError::ServerDown,
            LoginError::ServerFull,
            LoginError::CharacterNotFound {
                expected: "Legolas".into(),
                found: "Gimli".into(),
            },
            LoginError::Timeout {
                phase: "ServerSelecting".into(),
            },
            LoginError::MassFailure,
        ];
        assert_eq!(errors.len(), 9);
    }

    #[test]
    fn login_error_character_not_found_stores_names() {
        let err = LoginError::CharacterNotFound {
            expected: "Legolas".into(),
            found: "Gimli".into(),
        };
        if let LoginError::CharacterNotFound { expected, found } = err {
            assert_eq!(expected, "Legolas");
            assert_eq!(found, "Gimli");
        } else {
            panic!("expected CharacterNotFound");
        }
    }

    #[test]
    fn login_error_timeout_stores_phase() {
        let err = LoginError::Timeout {
            phase: "CharacterSelecting".into(),
        };
        if let LoginError::Timeout { phase } = err {
            assert_eq!(phase, "CharacterSelecting");
        } else {
            panic!("expected Timeout");
        }
    }

    #[test]
    fn login_phase_failed_carries_error() {
        let phase = LoginPhase::Failed {
            reason: LoginError::ServerFull,
        };
        if let LoginPhase::Failed { reason } = phase {
            assert_eq!(reason, LoginError::ServerFull);
        } else {
            panic!("expected Failed");
        }
    }

    #[test]
    fn login_phase_serialization_roundtrip() {
        let phases = vec![
            LoginPhase::NotStarted,
            LoginPhase::Ready,
            LoginPhase::Failed {
                reason: LoginError::WrongPassword,
            },
            LoginPhase::Failed {
                reason: LoginError::CharacterNotFound {
                    expected: "A".into(),
                    found: "B".into(),
                },
            },
        ];
        for phase in &phases {
            let json = serde_json::to_string(phase).expect("serialize");
            let restored: LoginPhase = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*phase, restored);
        }
    }

    #[test]
    fn account_info_construction() {
        let info = AccountInfo {
            account_name: "testuser".into(),
            character_name: "Legolas".into(),
            class_name: "Ranger".into(),
            level: 65,
            group_id: 1,
            server_name: "Teek".into(),
        };
        assert_eq!(info.account_name, "testuser");
        assert_eq!(info.character_name, "Legolas");
        assert_eq!(info.class_name, "Ranger");
        assert_eq!(info.level, 65);
        assert_eq!(info.group_id, 1);
        assert_eq!(info.server_name, "Teek");
    }

    #[test]
    fn account_info_serialization_roundtrip() {
        let info = AccountInfo {
            account_name: "user".into(),
            character_name: "Char".into(),
            class_name: "Warrior".into(),
            level: 50,
            group_id: 0,
            server_name: "FV".into(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        let restored: AccountInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.account_name, info.account_name);
        assert_eq!(restored.level, info.level);
    }

    #[test]
    fn login_error_equality() {
        assert_eq!(LoginError::WrongPassword, LoginError::WrongPassword);
        assert_eq!(LoginError::ServerDown, LoginError::ServerDown);
        assert_eq!(
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::CharacterAlreadyLoggedIn
        );
        assert_eq!(LoginError::OfflineTrader, LoginError::OfflineTrader);
        assert_ne!(LoginError::WrongPassword, LoginError::AccountLocked);
        assert_ne!(LoginError::ServerDown, LoginError::ServerFull);
    }

    #[test]
    fn login_error_serialization_roundtrip() {
        let errors = vec![
            LoginError::WrongPassword,
            LoginError::AccountLocked,
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::OfflineTrader,
            LoginError::ServerDown,
            LoginError::ServerFull,
            LoginError::MassFailure,
            LoginError::CharacterNotFound {
                expected: "A".into(),
                found: "B".into(),
            },
            LoginError::Timeout {
                phase: "test".into(),
            },
        ];
        for error in &errors {
            let json = serde_json::to_string(error).expect("serialize");
            let restored: LoginError = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*error, restored);
        }
    }

    #[test]
    fn login_phase_failed_with_different_errors_not_equal() {
        let p1 = LoginPhase::Failed {
            reason: LoginError::WrongPassword,
        };
        let p2 = LoginPhase::Failed {
            reason: LoginError::AccountLocked,
        };
        assert_ne!(p1, p2);
    }

    #[test]
    fn login_phase_debug_format() {
        let phase = LoginPhase::AtLoginScreen;
        let debug = format!("{:?}", phase);
        assert!(debug.contains("AtLoginScreen"));
    }

    #[test]
    fn login_error_debug_format() {
        let err = LoginError::CharacterNotFound {
            expected: "Foo".into(),
            found: "Bar".into(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Foo"));
        assert!(debug.contains("Bar"));
    }

    #[test]
    fn account_info_clone() {
        let info = AccountInfo {
            account_name: "user".into(),
            character_name: "Char".into(),
            class_name: "Warrior".into(),
            level: 50,
            group_id: 1,
            server_name: "FV".into(),
        };
        let cloned = info.clone();
        assert_eq!(cloned, info);
    }

    // ── RetryPolicy / RetryState tests ──────────────────────────────────

    #[test]
    fn retry_policy_default_values() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay, Duration::from_secs(2));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!(policy.jitter);
    }

    #[test]
    fn retry_state_default_values() {
        let state = RetryState::default();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.last_error, None);
    }

    #[test]
    fn retry_state_record_failure() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerDown);
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.last_error, Some(LoginError::ServerDown));

        state.record_failure(LoginError::Timeout {
            phase: "test".into(),
        });
        assert_eq!(state.attempt_count, 2);
        assert_eq!(
            state.last_error,
            Some(LoginError::Timeout {
                phase: "test".into()
            })
        );
    }

    #[test]
    fn retry_state_reset() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerDown);
        state.record_failure(LoginError::ServerFull);
        state.reset();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.last_error, None);
    }

    #[test]
    fn retry_policy_exponential_backoff_without_jitter() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let expected = [2.0, 4.0, 8.0, 16.0, 32.0];
        for (i, &exp) in expected.iter().enumerate() {
            let state = RetryState {
                attempt_count: i as u32,
                last_error: None,
            };
            let delay = policy.next_delay(&state).unwrap();
            assert!(
                (delay.as_secs_f64() - exp).abs() < 0.001,
                "attempt {i}: expected {exp}, got {}",
                delay.as_secs_f64()
            );
        }
    }

    #[test]
    fn retry_policy_respects_max_delay() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 3.0,
            jitter: false,
        };

        let state = RetryState {
            attempt_count: 3,
            last_error: None,
        };
        let delay = policy.next_delay(&state).unwrap();
        assert!((delay.as_secs_f64() - 10.0).abs() < 0.001);
    }

    #[test]
    fn retry_policy_exhausted_returns_none() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let state = RetryState {
            attempt_count: 3,
            last_error: Some(LoginError::ServerDown),
        };
        assert!(policy.next_delay(&state).is_none());

        let state = RetryState {
            attempt_count: 10,
            last_error: None,
        };
        assert!(policy.next_delay(&state).is_none());
    }

    #[test]
    fn retry_policy_zero_retries_always_exhausted() {
        let policy = RetryPolicy {
            max_retries: 0,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };
        let state = RetryState::default();
        assert!(policy.next_delay(&state).is_none());
    }

    #[test]
    fn retry_policy_jitter_stays_within_bounds() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay: Duration::from_secs(10),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.0,
            jitter: true,
        };
        let state = RetryState::default();

        for _ in 0..20 {
            let delay = policy.next_delay(&state).unwrap();
            let secs = delay.as_secs_f64();
            assert!((7.4..=12.6).contains(&secs), "jitter out of bounds: {secs}");
        }
    }

    #[test]
    fn retry_policy_serialization_roundtrip() {
        let policy = RetryPolicy::default();
        let json = serde_json::to_string(&policy).expect("serialize");
        let restored: RetryPolicy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.max_retries, policy.max_retries);
        assert_eq!(restored.initial_delay, policy.initial_delay);
        assert_eq!(restored.max_delay, policy.max_delay);
        assert!((restored.backoff_multiplier - policy.backoff_multiplier).abs() < f64::EPSILON);
        assert_eq!(restored.jitter, policy.jitter);
    }

    #[test]
    fn retry_state_serialization_roundtrip() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerFull);

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: RetryState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.attempt_count, 1);
        assert_eq!(restored.last_error, Some(LoginError::ServerFull));
    }

    #[test]
    fn retry_policy_custom_multiplier() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(100),
            backoff_multiplier: 3.0,
            jitter: false,
        };

        let expected = [1.0, 3.0, 9.0];
        let delays: Vec<f64> = (0..3)
            .map(|i| {
                let state = RetryState {
                    attempt_count: i,
                    last_error: None,
                };
                policy.next_delay(&state).unwrap().as_secs_f64()
            })
            .collect();
        for (i, (&got, &exp)) in delays.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 0.001,
                "attempt {i}: expected {exp}, got {got}"
            );
        }
    }

    #[test]
    fn serde_duration_rejects_negative() {
        let json = r#"{"max_retries":5,"initial_delay":-1.0,"max_delay":60.0,"backoff_multiplier":2.0,"jitter":false}"#;
        let result: Result<RetryPolicy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ---- SeenCharacter tests ----

    fn make_seen(name: &str, server: &str) -> SeenCharacter {
        SeenCharacter {
            name: name.into(),
            class_name: "Warrior".into(),
            level: 60,
            server: server.into(),
            last_seen: "2026-04-03T12:00:00Z".into(),
        }
    }

    #[test]
    fn seen_character_construction_and_fields() {
        let sc = make_seen("Legolas", "Teek");
        assert_eq!(sc.name, "Legolas");
        assert_eq!(sc.class_name, "Warrior");
        assert_eq!(sc.level, 60);
        assert_eq!(sc.server, "Teek");
        assert_eq!(sc.last_seen, "2026-04-03T12:00:00Z");
    }

    #[test]
    fn seen_character_serialization_roundtrip() {
        let sc = make_seen("Gimli", "FV");
        let json = serde_json::to_string(&sc).expect("serialize");
        let restored: SeenCharacter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sc, restored);
    }

    #[test]
    fn seen_character_clone() {
        let sc = make_seen("Aragorn", "Teek");
        let cloned = sc.clone();
        assert_eq!(sc, cloned);
    }

    // ---- CharacterCache tests ----

    #[test]
    fn cache_new_is_empty() {
        let cache = CharacterCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_insert_and_lookup() {
        let mut cache = CharacterCache::new();
        cache.insert(make_seen("Legolas", "Teek"));
        assert_eq!(cache.len(), 1);
        let found = cache.lookup("Teek", "Legolas").expect("should find");
        assert_eq!(found.name, "Legolas");
        assert_eq!(found.server, "Teek");
    }

    #[test]
    fn cache_lookup_miss() {
        let cache = CharacterCache::new();
        assert!(cache.lookup("Teek", "Nobody").is_none());
    }

    #[test]
    fn cache_dedup_by_server_name() {
        let mut cache = CharacterCache::new();
        cache.insert(SeenCharacter {
            name: "Legolas".into(),
            class_name: "Ranger".into(),
            level: 50,
            server: "Teek".into(),
            last_seen: "2026-04-01T00:00:00Z".into(),
        });
        cache.insert(SeenCharacter {
            name: "Legolas".into(),
            class_name: "Ranger".into(),
            level: 60,
            server: "Teek".into(),
            last_seen: "2026-04-03T00:00:00Z".into(),
        });
        assert_eq!(cache.len(), 1);
        let found = cache.lookup("Teek", "Legolas").unwrap();
        assert_eq!(found.level, 60);
        assert_eq!(found.last_seen, "2026-04-03T00:00:00Z");
    }

    #[test]
    fn cache_same_name_different_servers() {
        let mut cache = CharacterCache::new();
        cache.insert(make_seen("Legolas", "Teek"));
        cache.insert(make_seen("Legolas", "FV"));
        assert_eq!(cache.len(), 2);
        assert!(cache.lookup("Teek", "Legolas").is_some());
        assert!(cache.lookup("FV", "Legolas").is_some());
    }

    #[test]
    fn cache_iter() {
        let mut cache = CharacterCache::new();
        cache.insert(make_seen("A", "S1"));
        cache.insert(make_seen("B", "S1"));
        cache.insert(make_seen("C", "S2"));
        let mut names: Vec<&str> = cache.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn cache_serialization_roundtrip() {
        let mut cache = CharacterCache::new();
        cache.insert(make_seen("Legolas", "Teek"));
        cache.insert(make_seen("Gimli", "FV"));
        let json = serde_json::to_string(&cache).expect("serialize");
        let restored: CharacterCache = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.lookup("Teek", "Legolas").unwrap().name, "Legolas");
        assert_eq!(restored.lookup("FV", "Gimli").unwrap().name, "Gimli");
    }

    #[test]
    fn cache_file_persistence_roundtrip() {
        let dir = std::env::temp_dir().join("textquest_test_char_cache");
        let path = dir.join("cache.json");
        let _ = std::fs::remove_file(&path);

        let mut cache = CharacterCache::new();
        cache.insert(make_seen("Frodo", "Teek"));
        cache.insert(make_seen("Sam", "Teek"));
        cache.save_to_file(&path).expect("save");

        let loaded = CharacterCache::load_from_file(&path).expect("load");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.lookup("Teek", "Frodo").unwrap().level, 60);
        assert_eq!(loaded.lookup("Teek", "Sam").unwrap().level, 60);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn cache_load_missing_file_returns_empty() {
        let path = std::env::temp_dir().join("textquest_test_nonexistent_cache.json");
        let _ = std::fs::remove_file(&path);
        let cache = CharacterCache::load_from_file(&path).expect("should return empty");
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_load_invalid_json_returns_error() {
        let path = std::env::temp_dir().join("textquest_test_bad_cache.json");
        std::fs::write(&path, "not json").expect("write");
        let result = CharacterCache::load_from_file(&path);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    // --- ServerNameResolver tests ---

    #[test]
    fn resolver_new_has_known_servers() {
        let r = ServerNameResolver::new();
        assert!(
            r.server_count() > 20,
            "expected 20+ servers, got {}",
            r.server_count()
        );
    }

    #[test]
    fn resolver_default_same_as_new() {
        let a = ServerNameResolver::new();
        let b = ServerNameResolver::default();
        assert_eq!(a.server_count(), b.server_count());
    }

    #[test]
    fn resolver_resolve_known_tlp() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("Teek"), Some("teek"));
        assert_eq!(r.resolve("Oakwynd"), Some("oakwynd"));
        assert_eq!(r.resolve("Mischief"), Some("mischief"));
        assert_eq!(r.resolve("Aradune"), Some("aradune"));
    }

    #[test]
    fn resolver_resolve_known_live() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("Firiona Vie"), Some("firionavie"));
        assert_eq!(r.resolve("Bristlebane"), Some("bristlebane"));
        assert_eq!(r.resolve("Xegony"), Some("xegony"));
    }

    #[test]
    fn resolver_case_insensitive() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("teek"), Some("teek"));
        assert_eq!(r.resolve("TEEK"), Some("teek"));
        assert_eq!(r.resolve("TeEk"), Some("teek"));
        assert_eq!(r.resolve("firiona vie"), Some("firionavie"));
        assert_eq!(r.resolve("FIRIONA VIE"), Some("firionavie"));
    }

    #[test]
    fn resolver_unknown_returns_none() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("NonexistentServer"), None);
        assert_eq!(r.resolve(""), None);
    }

    #[test]
    fn resolver_fv_alias() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("FV"), Some("firionavie"));
        assert_eq!(r.resolve("Firiona Vie"), Some("firionavie"));
    }

    #[test]
    fn resolver_cazic_thule_alias() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("Cazic-Thule"), Some("cazicthule"));
        assert_eq!(r.resolve("Cazic Thule"), Some("cazicthule"));
    }

    #[test]
    fn resolver_reverse_lookup() {
        let r = ServerNameResolver::new();
        assert_eq!(r.display_name("teek"), Some("Teek"));
        assert_eq!(r.display_name("firionavie"), Some("Firiona Vie"));
        assert_eq!(r.display_name("bristlebane"), Some("Bristlebane"));
    }

    #[test]
    fn resolver_reverse_case_insensitive() {
        let r = ServerNameResolver::new();
        assert_eq!(r.display_name("TEEK"), Some("Teek"));
        assert_eq!(r.display_name("Teek"), Some("Teek"));
    }

    #[test]
    fn resolver_reverse_unknown_returns_none() {
        let r = ServerNameResolver::new();
        assert_eq!(r.display_name("unknown_id"), None);
        assert_eq!(r.display_name(""), None);
    }

    #[test]
    fn resolver_add_custom_server() {
        let mut r = ServerNameResolver::new();
        let before = r.server_count();
        r.add("My Custom Server", "customid");
        assert_eq!(r.resolve("My Custom Server"), Some("customid"));
        assert_eq!(r.resolve("my custom server"), Some("customid"));
        assert_eq!(r.display_name("customid"), Some("My Custom Server"));
        assert_eq!(r.server_count(), before + 1);
    }

    #[test]
    fn resolver_add_overwrites_display_mapping() {
        let mut r = ServerNameResolver::new();
        r.add("CustomName", "teek");
        assert_eq!(r.resolve("CustomName"), Some("teek"));
        assert_eq!(r.display_name("teek"), Some("Teek"));
    }

    #[test]
    fn resolver_debug_format() {
        let r = ServerNameResolver::new();
        let debug = format!("{:?}", r);
        assert!(debug.contains("ServerNameResolver"));
    }

    #[test]
    fn resolver_clone() {
        let r = ServerNameResolver::new();
        let cloned = r.clone();
        assert_eq!(cloned.server_count(), r.server_count());
        assert_eq!(cloned.resolve("Teek"), r.resolve("Teek"));
    }

    #[test]
    fn resolver_test_and_beta_servers() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("Test"), Some("test"));
        assert_eq!(r.resolve("Beta"), Some("beta"));
    }

    #[test]
    fn resolver_the_rathe_alias() {
        let r = ServerNameResolver::new();
        assert_eq!(r.resolve("The Rathe"), Some("rathe"));
        assert_eq!(r.resolve("Rathe"), Some("rathe"));
        assert_eq!(r.display_name("rathe"), Some("The Rathe"));
    }
}
