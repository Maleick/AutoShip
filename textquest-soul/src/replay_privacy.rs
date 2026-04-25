//! Session replay privacy pipeline.
//!
//! The tests in this module define the expected privacy contract for replay
//! exports: default-private write-time redaction, per-export expansion
//! toggles, scrubbed public shares, and export audit logging.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn session_salt() -> [u8; 32] {
        [0x42; 32]
    }

    fn operator_key() -> [u8; 32] {
        [0x99; 32]
    }

    fn replay_with_sample_data() -> ReplaySession {
        let mut session = ReplaySession::with_operator_meta(
            session_salt(),
            Some("Operator One".to_string()),
            Some("operator.account".to_string()),
        );

        session.record_chat(ReplayChatInput {
            channel: ReplayChatChannel::Tell,
            speaker: Some("RaidLead".to_string()),
            target: Some("Operator One".to_string()),
            guild: None,
            text: "meet at zone".to_string(),
            operator_owned: false,
        });
        session.record_chat(ReplayChatInput {
            channel: ReplayChatChannel::Group,
            speaker: Some("OtherPC".to_string()),
            target: None,
            guild: None,
            text: "pulling now".to_string(),
            operator_owned: false,
        });
        session.record_chat(ReplayChatInput {
            channel: ReplayChatChannel::Guild,
            speaker: Some("OtherPC".to_string()),
            target: None,
            guild: Some("Raiders United".to_string()),
            text: "guild plan".to_string(),
            operator_owned: false,
        });
        session.record_chat(ReplayChatInput {
            channel: ReplayChatChannel::Guild,
            speaker: Some("Operator One".to_string()),
            target: None,
            guild: Some("Raiders United".to_string()),
            text: "operator-only chat".to_string(),
            operator_owned: true,
        });
        session.record_log(ReplayLogInput {
            zone_name: "Sebilis".to_string(),
            mob_name: Some("A froglok".to_string()),
            spell_name: Some("Stun".to_string()),
            item_name: Some("Sage's Ring".to_string()),
            our_party_combat_numbers: Some(CombatNumbers {
                damage: 1234,
                healing: 56,
            }),
            message: "connected from 192.168.1.10 at C:\\Users\\Operator\\Secrets".to_string(),
            ip_address: Some("192.168.1.10".to_string()),
            network_endpoint: Some("10.0.0.2:9000".to_string()),
            file_path: Some("C:\\Users\\Operator\\Secrets".to_string()),
        });

        session
    }

    #[test]
    fn default_write_time_redactions_cover_tells_other_pcs_guilds_private_channels_operator_metadata_and_logs(
    ) {
        let session = replay_with_sample_data();
        let options = ReplayExportOptions::private_defaults();

        let export = session
            .export_public(options.clone())
            .expect("public export");
        let public = export.public_json();

        let public_text = public.to_string();
        assert!(!public_text.contains("Operator One"));
        assert!(!public_text.contains("operator.account"));
        assert!(!public_text.contains("192.168.1.10"));
        assert!(!public_text.contains("10.0.0.2:9000"));
        assert!(!public_text.contains("C:\\Users\\Operator\\Secrets"));

        let events = public["events"]
            .as_array()
            .expect("public export should include events");

        let tell = &events[0];
        let tell_text = tell["text"].as_str().expect("tell text");
        assert!(tell_text.starts_with("[tell:"));
        assert!(tell_text.ends_with(']'));

        let group = &events[1];
        let speaker = group["speaker"].as_str().expect("group speaker");
        assert!(speaker.starts_with("Other_"));

        let guild = &events[2];
        let guild_name = guild["guild"].as_str().expect("guild name");
        assert!(guild_name.starts_with("Guild_"));

        let operator_chat = &events[3];
        assert_eq!(
            operator_chat["text"].as_str().expect("operator text"),
            "operator-only chat"
        );
        assert!(
            operator_chat["speaker"].is_null(),
            "operator display name must not be written to replay"
        );

        let log = &events[4];
        assert_eq!(log["zone_name"], "Sebilis");
        assert_eq!(log["mob_name"], "A froglok");
        assert_eq!(log["spell_name"], "Stun");
        assert_eq!(log["item_name"], "Sage's Ring");
        assert_eq!(log["our_party_combat_numbers"]["damage"], 1234);
        assert_eq!(log["our_party_combat_numbers"]["healing"], 56);
        assert!(
            !log.to_string().contains("192.168.1.10"),
            "network data must be stripped from log export"
        );
        assert!(
            !log.to_string().contains("C:\\Users\\Operator\\Secrets"),
            "file paths must be stripped from log export"
        );
    }

    #[test]
    fn opt_in_expansion_restores_names_and_quotes_first_redacted_tell_line() {
        let session = replay_with_sample_data();
        let options = ReplayExportOptions {
            include_other_pc_names: true,
            include_tells: true,
            include_guild_chat: true,
            include_operator_name: false,
        };

        let export = session
            .export_private(options.clone(), &operator_key())
            .expect("private export");
        let expanded = export
            .expand_private(&operator_key(), &options)
            .expect("expansion with operator key");

        let events = expanded
            .public_json()
            .get("events")
            .and_then(Value::as_array)
            .expect("expanded events");

        assert_eq!(events[0]["speaker"], "RaidLead");
        assert_eq!(events[0]["target"], "Operator One");
        assert_eq!(events[0]["text"], "meet at zone");
        assert_eq!(events[1]["speaker"], "OtherPC");
        assert_eq!(events[2]["guild"], "Raiders United");
        assert!(
            expanded
                .public_json()
                .to_string()
                .contains("operator-only chat"),
            "operator text should still be available while the operator identity stays hidden"
        );
        assert!(
            !expanded.public_json().to_string().contains("Operator One"),
            "operator identity must stay hidden unless explicitly opted in"
        );

        let confirmation = session
            .tell_expansion_confirmation()
            .expect("tell expansion confirmation");
        assert!(
            confirmation.contains("[tell:"),
            "confirmation should quote the first redacted tell line"
        );
    }

    #[test]
    fn public_share_reports_scrubbed_diff_and_private_bundle_requires_operator_key() {
        let session = replay_with_sample_data();
        let private = session
            .export_private(ReplayExportOptions::private_defaults(), &operator_key())
            .expect("private export");
        let bytes = private.write_tqreplay().expect("write tqreplay");

        let without_key = ReplayExportBundle::read_public_bytes(&bytes).expect("read public");
        let without_key_json = without_key.public_json().to_string();
        assert!(
            !without_key_json.contains("RaidLead"),
            "raw tell data must not be readable without the operator key"
        );
        assert!(
            without_key.scrubbed_diff().removed_fields().contains(&"tell_text".to_string()),
            "public export should report stripped tell content"
        );

        let expanded = without_key
            .expand_private(&operator_key(), &ReplayExportOptions {
                include_other_pc_names: true,
                include_tells: true,
                include_guild_chat: true,
                include_operator_name: true,
            })
            .expect("expand private");

        let expanded_json = expanded.public_json().to_string();
        assert!(expanded_json.contains("RaidLead"));
        assert!(expanded_json.contains("Operator One"));
        assert!(expanded_json.contains("Raiders United"));

        let public_share = session
            .export_public(ReplayExportOptions {
                include_other_pc_names: false,
                include_tells: false,
                include_guild_chat: false,
                include_operator_name: false,
            })
            .expect("public share");
        assert!(
            public_share
                .scrubbed_diff()
                .removed_fields()
                .iter()
                .any(|field| field == "operator_name"),
            "public share should list operator identity as stripped"
        );
    }

    #[test]
    fn audit_log_records_replay_export_profile_version() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let log_path = tempdir.path().join("soul_audit.log");
        let logger = SoulAuditLogger::open(&log_path).expect("open logger");
        let session = replay_with_sample_data();
        let export = session
            .export_private(ReplayExportOptions::private_defaults(), &operator_key())
            .expect("private export");

        export
            .log_export_action(&logger, 7, "share_publicly")
            .expect("log export action");

        let entries = logger.entries_for(7).expect("audit entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action_type, AuditActionType::ReplayExport);
        assert_eq!(
            entries[0].redaction_profile_version,
            Some(REDACTION_PROFILE_VERSION)
        );
        assert!(
            entries[0].description.contains("share_publicly"),
            "audit description should record what was exported"
        );
    }

    #[test]
    fn public_bundle_round_trip_keeps_pii_unreadable_without_operator_key() {
        let session = replay_with_sample_data();
        let private = session
            .export_private(ReplayExportOptions::private_defaults(), &operator_key())
            .expect("private export");
        let bytes = private.write_tqreplay().expect("write tqreplay");

        let public_view = ReplayExportBundle::read_public_bytes(&bytes).expect("read public");
        assert!(
            !public_view.public_json().to_string().contains("operator.account"),
            "account name must never be visible in the public bundle"
        );

        let wrong_key = [0x11; 32];
        let result = public_view.expand_private(
            &wrong_key,
            &ReplayExportOptions {
                include_other_pc_names: true,
                include_tells: true,
                include_guild_chat: true,
                include_operator_name: true,
            },
        );
        assert!(
            result.is_err(),
            "expanding private fields with the wrong key must fail"
        );
    }
}
