# AutoShip Result: #1153 — #866.2: GM alert detection

## Summary
Successfully implemented GM interaction detection and alerting for TextQuest. The system detects incoming tells from Game Masters and account safety warnings, logs them prominently for operator review, and continues testing (non-blocking).

## Implementation

### 1. GM Detection Module (`textquest-common/src/gm_detection.rs`)
- **Function**: `detect_gm_tell(sender: &str, text: &str) -> bool`
- **Detection Criteria**:
  - **GM Names**: Patterns like `[GM]`, `GM_`, `_GM`, `-GM` (case-insensitive)
  - **Safety Keywords**: account, security, violation, third-party, exploit, ban, suspended, unauthorized, investigate, csr, customer service, daybreak, eula, terms of service
  - **Heuristics**: Suspicious name patterns (e.g., Admin_Bot, CSR_Agent, Support_Team) combined with safety keywords
- **Tests**: 18 comprehensive unit tests, all passing
  - GM name detection (bracket, underscore, suffix patterns)
  - Account safety keyword detection
  - False positive avoidance (legitimate player names)
  - Edge cases (multiple keywords, suspicious names)

### 2. Alert Integration (`textquest/src/alerts.rs`)
- **New AlertKind**: `GmInteraction`
  - Severity: Warning (batched delivery policy)
  - Display name: "GM Interaction"
- **Method**: `AlertThresholdEvaluator::gm_interaction_alert(actor, sender, message) -> NewAlert`
- **Delivery**: Warning-tier alerts are batched and can be sent to Discord/email

### 3. Orchestrator Integration (`textquest/src/orchestrator/mod.rs`)
- **Method**: `emit_gm_alert(actor, sender, message)` - logs prominent warning
- **Hook**: In `poll_and_log_chat()`, after parsing tell messages:
  - Check if `channel == ChatChannel::Tell`
  - Call `detect_gm_tell()` on sender and message
  - Emit alert if detected
  - **Non-blocking**: Continues with normal chat logging and processing
- **Logging**: Uses `tracing::warn!()` with "**SECURITY ALERT**" prefix for operator visibility

## Key Features

✅ **Detects GM Names**: Daybreak GM patterns [GM], GM_Name  
✅ **Account Safety Alerts**: Third-party tools, violations, suspensions, exploits  
✅ **Suspicious Name Heuristics**: Reduces false positives from legitimate players  
✅ **Operator Notification**: Prominent warning logs for manual review  
✅ **Non-Blocking**: Testing continues after alert (informational only)  
✅ **Comprehensive Tests**: 18 unit tests validate all detection scenarios  
✅ **Well-Documented**: Inline docs, test comments, and issue references  

## Test Results
```
running 18 tests
test gm_detection::tests::detect_gm_case_insensitive ... ok
test gm_detection::tests::detect_gm_tell_from_gm_name ... ok
test gm_detection::tests::detect_gm_with_bracket_notation ... ok
test gm_detection::tests::detect_gm_with_underscore ... ok
test gm_detection::tests::detect_gm_with_suffix ... ok
test gm_detection::tests::detect_account_warning_keywords ... ok
test gm_detection::tests::detect_exploit_mention ... ok
test gm_detection::tests::detect_suspension_warning ... ok
test gm_detection::tests::normal_messages_do_not_trigger_warning ... ok
test gm_detection::tests::normal_player_names_not_detected_as_gm ... ok
test gm_detection::tests::normal_tell_not_detected ... ok
test gm_detection::tests::detect_gm_tell_from_warning_keywords ... ok
test gm_detection::tests::raid_warning_about_third_party_tools ... ok
test gm_detection::tests::multiple_safety_keywords_with_suspicious_name ... ok
test gm_detection::tests::edge_case_legitimate_player_discussing_account_issues ... ok
test gm_detection::tests::edge_case_unknown_sender_with_single_keyword ... ok
test gm_detection::tests::edge_case_suspiciously_named_player_with_keywords ... ok
test gm_detection::tests::word_account_in_normal_sentence_not_flagged ... ok

test result: ok. 18 passed; 0 failed
```

## Files Changed
1. **textquest-common/src/gm_detection.rs** (NEW)
   - 235 lines: Core detection logic + 18 tests

2. **textquest-common/src/lib.rs**
   - Added module declaration: `pub mod gm_detection;`

3. **textquest/src/alerts.rs**
   - Added `AlertKind::GmInteraction` enum variant
   - Updated `as_str()`, `from_db()`, `display_name()` match arms
   - Added `gm_interaction_alert()` method to AlertThresholdEvaluator

4. **textquest/src/orchestrator/mod.rs**
   - Added `emit_gm_alert()` method
   - Integrated detection in `poll_and_log_chat()` for Tell channel messages

## Commit
- **Branch**: `autoship/issue-1153`
- **Hash**: `3dbe6fd0f`
- **Message**: "Feature: #1153 GM alert detection"

## Architecture Notes
- **Module Location**: `textquest-common::gm_detection` (shared across DLL/orchestrator)
- **Logging**: Via `tracing::warn!()` with "**SECURITY ALERT**" prefix
- **Alert Flow**: Detected → emit_gm_alert → tracing → visible in logs + Discord (when configured)
- **Extensibility**: Alert manager integration ready (via NewAlert type) for future metrics/archival

## Edge Cases Handled
- Legitimate players with names containing underscores (not flagged alone)
- Players discussing account topics (require suspicious name + keywords)
- Single keyword mentions (require additional heuristics)
- Case-insensitive GM name matching
- Impersonators with suspicious patterns (Admin_Bot, CSR_Agent, Support_Team)
