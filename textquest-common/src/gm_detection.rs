//! GM interaction detection and handling.
//!
//! This module detects tells and messages from Game Masters (GMs) or related to
//! account safety issues. When detected, alerts are raised for manual operator review.
//!
//! GM detection relies on pattern matching common to EverQuest GMs:
//! - Names containing "[GM]" or "GM_" prefix
//! - Messages mentioning account safety, third-party tools, or CSR interactions
//! - Known GM behavior patterns

/// Detects if a tell is from a Game Master or contains account safety warnings.
///
/// Returns `true` if the sender is likely a GM or the message contains GM/CSR-related
/// content that requires operator review.
///
/// Detection criteria:
/// - Sender name contains "[GM]", "GM_", or matches known GM naming patterns
/// - Message contains keywords like "account", "security", "third-party", "violation"
/// - Message references CSR (Customer Service Representative) interactions
pub fn detect_gm_tell(sender: &str, text: &str) -> bool {
    is_likely_gm_name(sender) || contains_account_safety_warning(sender, text)
}

/// Checks if a sender name matches known GM naming patterns.
fn is_likely_gm_name(sender: &str) -> bool {
    let normalized = sender.to_lowercase();

    // Check for common GM name patterns
    if normalized.contains("[gm]") || normalized.contains("gm_") {
        return true;
    }

    // Check for known GM prefixes/suffixes
    if normalized.starts_with("gm_") || normalized.starts_with("gm-") {
        return true;
    }

    // Some servers use specific GM naming conventions
    // Daybreak GMs often use names like "GM_Name" or "[GM] Name"
    if normalized.contains("_gm") || normalized.contains("-gm") {
        return true;
    }

    false
}

/// Checks if a message contains keywords indicating account safety or CSR interaction.
fn contains_account_safety_warning(sender: &str, text: &str) -> bool {
    // Only check messages that are reasonably suspicious
    // Avoid false positives from normal player chatter
    let normalized_text = text.to_lowercase();
    let normalized_sender = sender.to_lowercase();

    // CSR/Account safety keywords that warrant immediate review
    let warning_keywords = [
        "account",
        "security",
        "violation",
        "third party",
        "third-party",
        "unauthorized",
        "exploit",
        "ban",
        "suspended",
        "suspended account",
        "investigate",
        "csr",
        "customer service",
        "daybreak",
        "eula",
        "terms of service",
    ];

    // Count how many warning keywords appear in the message
    // Require at least one keyword for account-safety context
    let mut keyword_count = 0;
    for keyword in &warning_keywords {
        if normalized_text.contains(keyword) {
            keyword_count += 1;
        }
    }

    if keyword_count == 0 {
        return false;
    }

    // Additional context: some GMs may not have obvious names
    // If message contains safety keywords + sender is not known hostile/trusted,
    // and it's a tell (not say), flag it
    // This helps catch impersonators or unknown senders
    if keyword_count > 0 {
        // Check if sender looks like a legitimate player name
        // (short names, all lowercase, common patterns are less likely to be GMs)
        let is_suspicious_name = normalized_sender.len() > 4
            && !normalized_sender.chars().all(|c| c.is_lowercase())
            || normalized_sender.contains('_')
            || normalized_sender.contains('-');

        // If we have safety keywords + suspicious name pattern, flag it
        return is_suspicious_name || keyword_count > 1;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── GM Name Detection ──────────────────────────────────────────────────

    #[test]
    fn detect_gm_with_bracket_notation() {
        assert!(is_likely_gm_name("[GM] Nexus"));
        assert!(is_likely_gm_name("[GM] Support"));
    }

    #[test]
    fn detect_gm_with_underscore() {
        assert!(is_likely_gm_name("GM_Nexus"));
        assert!(is_likely_gm_name("GM_Support"));
    }

    #[test]
    fn detect_gm_with_suffix() {
        assert!(is_likely_gm_name("Support_GM"));
        assert!(is_likely_gm_name("Nexus_GM"));
    }

    #[test]
    fn detect_gm_case_insensitive() {
        assert!(is_likely_gm_name("[gm] nexus"));
        assert!(is_likely_gm_name("gm_support"));
        assert!(is_likely_gm_name("support_gm"));
    }

    #[test]
    fn normal_player_names_not_detected_as_gm() {
        assert!(!is_likely_gm_name("Soandso"));
        assert!(!is_likely_gm_name("Alice"));
    }

    // ── Account Safety Warning Detection ────────────────────────────────────

    #[test]
    fn detect_account_warning_keywords() {
        assert!(contains_account_safety_warning(
            "Support",
            "Your account has been flagged for third-party tool usage"
        ));
        assert!(contains_account_safety_warning(
            "CSR_Agent",
            "Your account is under investigation"
        ));
    }

    #[test]
    fn detect_exploit_mention() {
        assert!(contains_account_safety_warning(
            "Support_Team",
            "We detected exploit usage on your account"
        ));
    }

    #[test]
    fn detect_suspension_warning() {
        assert!(contains_account_safety_warning(
            "Daybreak_Support",
            "Your account has been suspended"
        ));
    }

    #[test]
    fn normal_messages_do_not_trigger_warning() {
        assert!(!contains_account_safety_warning(
            "Soandso",
            "You don't have enough mana"
        ));
        assert!(!contains_account_safety_warning("Friend", "WTS some items"));
        assert!(!contains_account_safety_warning(
            "Guildie",
            "Let's go farming"
        ));
    }

    #[test]
    fn word_account_in_normal_sentence_not_flagged() {
        // "Account" in a normal context (e.g., "according to my account")
        // should not trigger a false positive unless combined with other keywords
        assert!(!contains_account_safety_warning(
            "NormalPlayer",
            "According to my account, that's wrong"
        ));
    }

    // ── Full detect_gm_tell Integration ────────────────────────────────────

    #[test]
    fn detect_gm_tell_from_gm_name() {
        assert!(detect_gm_tell("[GM] Nexus", "Hello"));
        assert!(detect_gm_tell("GM_Support", "We need to talk"));
    }

    #[test]
    fn detect_gm_tell_from_warning_keywords() {
        assert!(detect_gm_tell(
            "Support_Team",
            "Your account has been flagged for suspicious activity"
        ));
    }

    #[test]
    fn normal_tell_not_detected() {
        assert!(!detect_gm_tell("Soandso", "Hey, want to group?"));
        assert!(!detect_gm_tell("Friend", "WTS some items"));
    }

    #[test]
    fn raid_warning_about_third_party_tools() {
        // Example of a CSR posting in raid/say
        assert!(detect_gm_tell(
            "CSR_Agent",
            "Please note: third-party tools are prohibited"
        ));
    }

    #[test]
    fn multiple_safety_keywords_with_suspicious_name() {
        assert!(detect_gm_tell(
            "Support_Admin",
            "We detected a violation of the EULA and your account security has been compromised"
        ));
    }

    #[test]
    fn edge_case_legitimate_player_discussing_account_issues() {
        // A legitimate player name shouldn't trigger even if discussing account topics
        // unless they have a suspicious name pattern
        assert!(!detect_gm_tell(
            "alice",
            "my account got hacked, what do I do?"
        ));
    }

    #[test]
    fn edge_case_unknown_sender_with_single_keyword() {
        // Unknown sender with single keyword but not a GM-like name
        // should generally not trigger
        assert!(!detect_gm_tell("RandomPlayer", "account"));
    }

    #[test]
    fn edge_case_suspiciously_named_player_with_keywords() {
        // Suspicious name + warning keywords = should trigger
        assert!(detect_gm_tell("Admin_Bot", "your account needs review"));
    }
}
