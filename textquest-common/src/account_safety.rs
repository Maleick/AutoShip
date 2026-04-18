use crate::types::ClientId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Detects account bans, suspensions, or account lockouts in EQ messages.
///
/// This function analyzes text from various sources (login screen, chat, disconnect
/// messages) to identify ban/suspension patterns. The detection is case-insensitive
/// and looks for common EQ ban-related keywords and phrases.
///
/// # Detected Messages
///
/// - "Your account has been suspended"
/// - "Your account has been banned"
/// - "Account locked"
/// - "You have been removed from the server"
/// - "This account has been locked"
/// - "Your account is locked"
/// - Similar variations
///
/// # Args
///
/// * `text` - The message text to check (typically from login screen, chat, or disconnect messages)
///
/// # Returns
///
/// `true` if the text contains indicators of an account ban, suspension, or lockout; `false` otherwise.
///
/// # Examples
///
/// ```ignore
/// assert!(detect_ban_message("Your account has been suspended due to ToS violation"));
/// assert!(detect_ban_message("Account locked - contact support"));
/// assert!(!detect_ban_message("Welcome to EverQuest"));
/// ```
pub fn detect_ban_message(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let lower = text.to_lowercase();

    // Common ban/suspension patterns
    let ban_patterns = [
        "account has been suspended",
        "account has been banned",
        "account locked",
        "you have been removed from the server",
        "this account has been locked",
        "your account is locked",
        "account suspension",
        "account ban",
        "permanently banned",
        "permanently suspended",
        "banned from everquest",
        "suspended from everquest",
        "account terminated",
        "account closure",
        "access denied - account",
        "violation of terms",
        "terms of service violation",
        "cheating detected",
        "exploit detected",
        "account locked for security",
    ];

    for pattern in &ban_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Account ban status tracker.
///
/// Tracks whether an account (identified by ClientId) has been detected as banned
/// or suspended, along with metadata about when the ban was detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BanDetection {
    /// The client that encountered the ban.
    pub client_id: ClientId,
    /// The message that triggered the ban detection.
    pub message: String,
    /// ISO-8601 timestamp of when the ban was detected.
    pub detected_at: String,
    /// Optional context (e.g., "login_screen", "chat", "disconnect").
    pub context: String,
}

impl BanDetection {
    /// Create a new ban detection record.
    ///
    /// # Args
    ///
    /// * `client_id` - The client ID experiencing the ban.
    /// * `message` - The full message text that triggered detection.
    /// * `context` - Where the message was seen (e.g., "login_screen", "chat", "disconnect").
    pub fn new(client_id: ClientId, message: String, context: String) -> Self {
        // Generate a simplified ISO-8601 timestamp using system time.
        // Format: YYYY-MM-DDTHH:MM:SSZ (UTC only, no timezone info needed for UTC)
        let detected_at = Self::current_iso8601_timestamp();
        Self {
            client_id,
            message,
            detected_at,
            context,
        }
    }

    /// Generate a simple timestamp string using system time.
    /// Format: Unix timestamp in seconds (suitable for logging and comparison).
    fn current_iso8601_timestamp() -> String {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}", duration.as_secs())
    }
}

/// Result of handling a ban detection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BanHandlingResult {
    /// Ban detected and account marked as banned; no further attempts will be made.
    Halted {
        /// The ban detection record.
        detection: BanDetection,
        /// Human-readable reason for halting.
        reason: String,
    },
    /// An error occurred while handling the ban detection.
    Error(String),
}

/// Global registry of banned client IDs.
///
/// Once an account is detected as banned, its client ID is added to this set
/// to prevent further login attempts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BannedAccountRegistry {
    /// Set of client IDs that are known to be banned.
    banned_clients: HashSet<ClientId>,
    /// History of ban detections for audit purposes.
    detection_history: Vec<BanDetection>,
}

impl BannedAccountRegistry {
    /// Create a new, empty banned account registry.
    pub fn new() -> Self {
        Self {
            banned_clients: HashSet::new(),
            detection_history: Vec::new(),
        }
    }

    /// Check if a client is known to be banned.
    pub fn is_banned(&self, client_id: ClientId) -> bool {
        self.banned_clients.contains(&client_id)
    }

    /// Mark a client as banned.
    pub fn mark_banned(&mut self, detection: BanDetection) {
        self.banned_clients.insert(detection.client_id);
        self.detection_history.push(detection);
    }

    /// Get all ban detections for a specific client.
    pub fn get_detections(&self, client_id: ClientId) -> Vec<&BanDetection> {
        self.detection_history
            .iter()
            .filter(|d| d.client_id == client_id)
            .collect()
    }

    /// Get the full detection history (for audit/logging).
    pub fn get_history(&self) -> &[BanDetection] {
        &self.detection_history
    }

    /// Get the count of banned accounts.
    pub fn banned_count(&self) -> usize {
        self.banned_clients.len()
    }

    /// Clear all ban records (for testing or account reinstatement).
    pub fn clear(&mut self) {
        self.banned_clients.clear();
        self.detection_history.clear();
    }
}

/// Handle a detected ban by marking the account and stopping further attempts.
///
/// This function should be called whenever a ban message is detected. It:
///
/// 1. Verifies the client is not already marked as banned
/// 2. Creates a ban detection record
/// 3. Marks the client as banned in the global registry
/// 4. Returns a result indicating what action was taken
///
/// # Args
///
/// * `client_id` - The ID of the client experiencing the ban
/// * `message` - The message that triggered ban detection
/// * `context` - Where the message came from (e.g., "login_screen", "chat", "disconnect")
/// * `registry` - Mutable reference to the banned account registry
///
/// # Returns
///
/// A `BanHandlingResult` indicating success or error.
///
/// # Examples
///
/// ```ignore
/// let mut registry = BannedAccountRegistry::new();
/// let result = handle_ban_detection(
///     1,
///     "Your account has been banned".to_string(),
///     "login_screen".to_string(),
///     &mut registry,
/// );
/// assert_eq!(registry.is_banned(1), true);
/// ```
pub fn handle_ban_detection(
    client_id: ClientId,
    message: String,
    context: String,
    registry: &mut BannedAccountRegistry,
) -> BanHandlingResult {
    // Check if already banned to avoid duplicate processing
    if registry.is_banned(client_id) {
        return BanHandlingResult::Halted {
            detection: BanDetection::new(client_id, message, context),
            reason: "Client already marked as banned".to_string(),
        };
    }

    // Create detection record and mark as banned
    let detection = BanDetection::new(client_id, message, context);
    registry.mark_banned(detection.clone());

    BanHandlingResult::Halted {
        detection,
        reason: "Account ban/suspension detected; halting all login attempts for this client"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_ban_message tests ──────────────────────────────────────

    #[test]
    fn detect_empty_string_returns_false() {
        assert!(!detect_ban_message(""));
    }

    #[test]
    fn detect_normal_messages_return_false() {
        assert!(!detect_ban_message("Welcome to EverQuest!"));
        assert!(!detect_ban_message("You have earned a new ability"));
        assert!(!detect_ban_message("You receive 100 experience"));
        assert!(!detect_ban_message("Returning to home point"));
    }

    #[test]
    fn detect_account_suspended() {
        assert!(detect_ban_message("Your account has been suspended"));
        assert!(detect_ban_message(
            "Your account has been suspended due to ToS violation"
        ));
    }

    #[test]
    fn detect_account_banned() {
        assert!(detect_ban_message("Your account has been banned"));
        assert!(detect_ban_message("Your account has been banned permanently"));
    }

    #[test]
    fn detect_account_locked() {
        assert!(detect_ban_message("Account locked"));
        assert!(detect_ban_message("Account locked - contact support"));
        assert!(detect_ban_message("This account has been locked"));
        assert!(detect_ban_message("Your account is locked"));
    }

    #[test]
    fn detect_removed_from_server() {
        assert!(detect_ban_message("You have been removed from the server"));
        assert!(detect_ban_message(
            "You have been removed from the server due to inactivity"
        ));
    }

    #[test]
    fn detect_case_insensitive() {
        assert!(detect_ban_message("YOUR ACCOUNT HAS BEEN SUSPENDED"));
        assert!(detect_ban_message("account locked"));
        assert!(detect_ban_message("Account Locked"));
        assert!(detect_ban_message("ACCOUNT LOCKED"));
    }

    #[test]
    fn detect_with_extra_text() {
        assert!(detect_ban_message(
            "[System] Your account has been suspended. Please contact support."
        ));
        assert!(detect_ban_message("[Error] Account locked due to security violation"));
    }

    #[test]
    fn detect_account_suspension() {
        assert!(detect_ban_message("account suspension"));
        assert!(detect_ban_message("Account Suspension Notice"));
    }

    #[test]
    fn detect_account_ban_patterns() {
        assert!(detect_ban_message("account ban"));
        assert!(detect_ban_message("Account Ban: Exploiting"));
    }

    #[test]
    fn detect_permanently_banned() {
        assert!(detect_ban_message("You are permanently banned"));
        assert!(detect_ban_message("Permanently banned from this server"));
    }

    #[test]
    fn detect_permanently_suspended() {
        assert!(detect_ban_message("You are permanently suspended"));
        assert!(detect_ban_message("Permanently suspended from play"));
    }

    #[test]
    fn detect_banned_from_everquest() {
        assert!(detect_ban_message("Banned from EverQuest"));
        assert!(detect_ban_message("You are banned from EverQuest"));
    }

    #[test]
    fn detect_suspended_from_everquest() {
        assert!(detect_ban_message("Suspended from EverQuest"));
        assert!(detect_ban_message("You are suspended from EverQuest"));
    }

    #[test]
    fn detect_account_terminated() {
        assert!(detect_ban_message("Account terminated"));
        assert!(detect_ban_message("Your account has been terminated"));
    }

    #[test]
    fn detect_account_closure() {
        assert!(detect_ban_message("Account closure"));
        assert!(detect_ban_message("Your account is closed"));
    }

    #[test]
    fn detect_access_denied_account() {
        assert!(detect_ban_message("Access denied - account"));
        assert!(detect_ban_message("Access denied - account locked"));
    }

    #[test]
    fn detect_violation_patterns() {
        assert!(detect_ban_message("Violation of terms"));
        assert!(detect_ban_message("Terms of service violation"));
    }

    #[test]
    fn detect_cheating_detected() {
        assert!(detect_ban_message("Cheating detected"));
        assert!(detect_ban_message("Illegal activity detected - account banned"));
    }

    #[test]
    fn detect_exploit_detected() {
        assert!(detect_ban_message("Exploit detected"));
        assert!(detect_ban_message("Exploit detected - account suspended"));
    }

    #[test]
    fn detect_account_locked_for_security() {
        assert!(detect_ban_message("Account locked for security"));
        assert!(detect_ban_message("Your account is locked for security reasons"));
    }

    #[test]
    fn detect_false_positives_containing_account() {
        // These should NOT trigger - they contain "account" but not ban patterns
        assert!(!detect_ban_message("Account balance: 5000 platinum"));
        assert!(!detect_ban_message("Log into your account here"));
        assert!(!detect_ban_message("Account created successfully"));
        assert!(!detect_ban_message("Account settings updated"));
    }

    #[test]
    fn detect_false_positives_containing_lock() {
        assert!(!detect_ban_message("You unlock the chest"));
        assert!(!detect_ban_message("This door is locked"));
        assert!(!detect_ban_message("Picked lock with key"));
    }

    #[test]
    fn detect_false_positives_containing_ban() {
        // "ban" in different context
        assert!(!detect_ban_message("You cast ban pet"));
        assert!(!detect_ban_message("Fire ban is active in this zone"));
    }

    // ── BanDetection tests ────────────────────────────────────────────

    #[test]
    fn ban_detection_new() {
        let detection = BanDetection::new(42, "Your account has been suspended".into(), "login_screen".into());
        assert_eq!(detection.client_id, 42);
        assert_eq!(detection.message, "Your account has been suspended");
        assert_eq!(detection.context, "login_screen");
        assert!(!detection.detected_at.is_empty());
    }

    #[test]
    fn ban_detection_clone() {
        let d1 = BanDetection::new(1, "msg".into(), "ctx".into());
        let d2 = d1.clone();
        assert_eq!(d1, d2);
    }

    #[test]
    fn ban_detection_equality() {
        let d1 = BanDetection::new(1, "msg".into(), "ctx".into());
        let d2 = BanDetection::new(1, "msg".into(), "ctx".into());
        // Note: detected_at will be different since it uses current time
        // So we can't assert exact equality without mocking
        assert_eq!(d1.client_id, d2.client_id);
        assert_eq!(d1.message, d2.message);
        assert_eq!(d1.context, d2.context);
    }

    #[test]
    fn ban_detection_serialization() {
        let d = BanDetection::new(5, "banned".into(), "chat".into());
        let json = serde_json::to_string(&d).expect("serialize");
        let restored: BanDetection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.client_id, 5);
        assert_eq!(restored.message, "banned");
        assert_eq!(restored.context, "chat");
    }

    // ── BannedAccountRegistry tests ────────────────────────────────────

    #[test]
    fn registry_new_is_empty() {
        let registry = BannedAccountRegistry::new();
        assert_eq!(registry.banned_count(), 0);
        assert!(!registry.is_banned(1));
    }

    #[test]
    fn registry_is_banned_false_by_default() {
        let registry = BannedAccountRegistry::new();
        assert!(!registry.is_banned(1));
        assert!(!registry.is_banned(999));
    }

    #[test]
    fn registry_mark_banned() {
        let mut registry = BannedAccountRegistry::new();
        let detection = BanDetection::new(42, "banned".into(), "login".into());
        registry.mark_banned(detection.clone());
        assert!(registry.is_banned(42));
        assert_eq!(registry.banned_count(), 1);
    }

    #[test]
    fn registry_multiple_bans() {
        let mut registry = BannedAccountRegistry::new();
        for i in 1..=5 {
            let detection = BanDetection::new(i, format!("client {i}"), "login".into());
            registry.mark_banned(detection);
        }
        assert_eq!(registry.banned_count(), 5);
        for i in 1..=5 {
            assert!(registry.is_banned(i));
        }
    }

    #[test]
    fn registry_get_detections() {
        let mut registry = BannedAccountRegistry::new();
        let d1 = BanDetection::new(1, "first".into(), "login".into());
        let d2 = BanDetection::new(1, "second".into(), "chat".into());
        registry.mark_banned(d1.clone());
        registry.mark_banned(d2.clone());

        let detections = registry.get_detections(1);
        assert_eq!(detections.len(), 2);
    }

    #[test]
    fn registry_get_detections_other_client() {
        let mut registry = BannedAccountRegistry::new();
        let d1 = BanDetection::new(1, "msg".into(), "login".into());
        registry.mark_banned(d1);

        let detections = registry.get_detections(2);
        assert_eq!(detections.len(), 0);
    }

    #[test]
    fn registry_get_history() {
        let mut registry = BannedAccountRegistry::new();
        let d1 = BanDetection::new(1, "msg1".into(), "login".into());
        let d2 = BanDetection::new(2, "msg2".into(), "chat".into());
        registry.mark_banned(d1);
        registry.mark_banned(d2);

        let history = registry.get_history();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn registry_clear() {
        let mut registry = BannedAccountRegistry::new();
        let d1 = BanDetection::new(1, "msg".into(), "login".into());
        registry.mark_banned(d1);
        assert_eq!(registry.banned_count(), 1);

        registry.clear();
        assert_eq!(registry.banned_count(), 0);
        assert!(!registry.is_banned(1));
        assert_eq!(registry.get_history().len(), 0);
    }

    #[test]
    fn registry_clone() {
        let mut registry = BannedAccountRegistry::new();
        let d1 = BanDetection::new(1, "msg".into(), "login".into());
        registry.mark_banned(d1);

        let cloned = registry.clone();
        assert_eq!(cloned.banned_count(), 1);
        assert!(cloned.is_banned(1));
    }

    // ── handle_ban_detection tests ────────────────────────────────────

    #[test]
    fn handle_ban_detection_marks_client_banned() {
        let mut registry = BannedAccountRegistry::new();
        let result = handle_ban_detection(
            1,
            "Your account has been suspended".into(),
            "login_screen".into(),
            &mut registry,
        );

        assert!(registry.is_banned(1));
        match result {
            BanHandlingResult::Halted { detection, .. } => {
                assert_eq!(detection.client_id, 1);
            }
            _ => panic!("Expected Halted result"),
        }
    }

    #[test]
    fn handle_ban_detection_returns_halted() {
        let mut registry = BannedAccountRegistry::new();
        let result = handle_ban_detection(
            42,
            "Account locked".into(),
            "login_screen".into(),
            &mut registry,
        );

        match result {
            BanHandlingResult::Halted { reason, .. } => {
                assert!(reason.contains("halting"));
            }
            _ => panic!("Expected Halted result"),
        }
    }

    #[test]
    fn handle_ban_detection_already_banned() {
        let mut registry = BannedAccountRegistry::new();

        // First ban
        let result1 = handle_ban_detection(
            5,
            "banned".into(),
            "login".into(),
            &mut registry,
        );
        assert!(matches!(result1, BanHandlingResult::Halted { .. }));

        // Second detection (already banned)
        let result2 = handle_ban_detection(
            5,
            "banned again".into(),
            "chat".into(),
            &mut registry,
        );

        match result2 {
            BanHandlingResult::Halted { reason, .. } => {
                assert!(reason.contains("already marked"));
            }
            _ => panic!("Expected Halted result"),
        }

        // Should still only have one entry in detection history for the real ban
        // (The second call also gets recorded in the registry, but the logic
        // recognizes it as already-banned and says so in the result)
        assert!(registry.is_banned(5));
    }

    #[test]
    fn handle_ban_detection_multiple_clients() {
        let mut registry = BannedAccountRegistry::new();

        for i in 1..=3 {
            handle_ban_detection(
                i,
                format!("client {} banned", i),
                "login".into(),
                &mut registry,
            );
        }

        assert_eq!(registry.banned_count(), 3);
        for i in 1..=3 {
            assert!(registry.is_banned(i));
        }
    }

    #[test]
    fn handle_ban_detection_preserves_message() {
        let mut registry = BannedAccountRegistry::new();
        let msg = "Your account has been permanently suspended due to repeated violations".to_string();

        let result = handle_ban_detection(
            10,
            msg.clone(),
            "disconnect".into(),
            &mut registry,
        );

        match result {
            BanHandlingResult::Halted { detection, .. } => {
                assert_eq!(detection.message, msg);
                assert_eq!(detection.context, "disconnect");
            }
            _ => panic!("Expected Halted result"),
        }
    }

    #[test]
    fn handle_ban_detection_serialization() {
        let result = BanHandlingResult::Halted {
            detection: BanDetection::new(1, "msg".into(), "login".into()),
            reason: "test reason".into(),
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let restored: BanHandlingResult = serde_json::from_str(&json).expect("deserialize");

        match restored {
            BanHandlingResult::Halted { reason, .. } => {
                assert_eq!(reason, "test reason");
            }
            _ => panic!("Expected Halted result"),
        }
    }

    #[test]
    fn ban_handling_result_error() {
        let result = BanHandlingResult::Error("Something went wrong".into());
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: BanHandlingResult = serde_json::from_str(&json).expect("deserialize");

        match restored {
            BanHandlingResult::Error(msg) => {
                assert_eq!(msg, "Something went wrong");
            }
            _ => panic!("Expected Error result"),
        }
    }

    #[test]
    fn detect_ban_comprehensive_contexts() {
        // Messages that might appear in different contexts
        let ban_messages = vec![
            // Login screen messages
            "Your account has been suspended",
            "Account locked - contact support",
            // Chat messages (system messages)
            "You have been removed from the server",
            // Disconnect messages
            "Your account is locked due to multiple failed login attempts",
            // Email-style notifications
            "Account Ban: Your account has been terminated",
        ];

        for msg in ban_messages {
            assert!(
                detect_ban_message(msg),
                "Failed to detect ban in message: {}",
                msg
            );
        }
    }

    #[test]
    fn ban_registry_stress_test() {
        let mut registry = BannedAccountRegistry::new();
        const NUM_CLIENTS: usize = 1000;

        for i in 0..NUM_CLIENTS {
            let client_id = i as ClientId;
            let detection = BanDetection::new(
                client_id,
                format!("Client {} banned", i),
                "test".into(),
            );
            registry.mark_banned(detection);
        }

        assert_eq!(registry.banned_count(), NUM_CLIENTS);

        // Verify all are marked as banned
        for i in 0..NUM_CLIENTS {
            assert!(registry.is_banned(i as ClientId));
        }
    }
}
