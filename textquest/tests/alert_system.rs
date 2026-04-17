use std::time::Duration;

use textquest::{
    alerts::{
        AlertKind, AlertManager, AlertSeverity, AlertStore, AlertThresholdEvaluator,
        DeliveryPolicy, NewAlert,
    },
    config::AppConfig,
};

#[test]
fn alert_config_parses_thresholds_and_channels() {
    let cfg: AppConfig = toml::from_str(
        r#"
        [alerts]
        enable_discord = true
        discord_webhook_url = "https://discord.invalid/webhook"
        enable_email = true
        email_recipients = ["farmer@example.com"]
        warning_batch_window_secs = 300

        [alerts.thresholds]
        memory_warning_mb = 256
        ipc_latency_warning_ms = 15
        error_rate_warning_per_min = 7
        dps_drop_warning_pct = 25
        zone_timeout_secs = 90
        "#,
    )
    .expect("alert config should deserialize");

    assert!(cfg.alerts.enable_discord);
    assert!(cfg.alerts.enable_email);
    assert_eq!(
        cfg.alerts.discord_webhook_url,
        "https://discord.invalid/webhook"
    );
    assert_eq!(cfg.alerts.email_recipients, vec!["farmer@example.com"]);
    assert_eq!(cfg.alerts.warning_batch_window_secs, 300);
    assert_eq!(cfg.alerts.thresholds.memory_warning_mb, 256);
    assert_eq!(cfg.alerts.thresholds.ipc_latency_warning_ms, 15);
    assert_eq!(cfg.alerts.thresholds.error_rate_warning_per_min, 7);
    assert_eq!(cfg.alerts.thresholds.dps_drop_warning_pct, 25);
    assert_eq!(cfg.alerts.thresholds.zone_timeout_secs, 90);
}

#[test]
fn alert_enums_serialize_with_snake_case_wire_values() {
    assert_eq!(
        serde_json::to_string(&AlertSeverity::Critical).expect("serialize severity"),
        "\"critical\""
    );
    assert_eq!(
        serde_json::to_string(&AlertKind::ZoneTransitionTimeout).expect("serialize kind"),
        "\"zone_transition_timeout\""
    );
}

#[test]
fn alert_store_tracks_recent_unread_and_acknowledged_alerts() {
    let store = AlertStore::open_memory().expect("in-memory alert db");

    let critical_id = store
        .insert(&NewAlert::new(
            AlertSeverity::Critical,
            AlertKind::Death,
            "Aelrindel died in Plane of Hate",
        ))
        .expect("critical alert inserted");
    let warning_id = store
        .insert(&NewAlert::new(
            AlertSeverity::Warning,
            AlertKind::IpcLatencyHigh,
            "IPC latency p95 reached 15.2ms",
        ))
        .expect("warning alert inserted");

    let recent = store.recent(10).expect("recent alerts");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, warning_id);
    assert_eq!(recent[1].id, critical_id);
    assert_eq!(store.unread_count().expect("unread count"), 2);

    store
        .acknowledge(warning_id, "operator")
        .expect("warning alert acknowledged");

    let unread = store.unread(10).expect("unread alerts");
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].id, critical_id);

    let acknowledged = store.recent(10).expect("recent alerts after ack");
    let warning = acknowledged
        .into_iter()
        .find(|alert| alert.id == warning_id)
        .expect("warning alert present");
    assert_eq!(warning.acknowledged_by.as_deref(), Some("operator"));
    assert!(warning.acknowledged_at.is_some());
}

#[test]
fn alert_manager_routes_critical_immediately_and_batches_warnings() {
    let store = AlertStore::open_memory().expect("in-memory alert db");
    let mut manager = AlertManager::new(store);

    manager.set_warning_batch_window(Duration::from_secs(300));

    let critical = manager
        .publish(NewAlert::new(
            AlertSeverity::Critical,
            AlertKind::Death,
            "Glacialsurge18 died in Great Divide",
        ))
        .expect("critical alert published");
    let warning = manager
        .publish(NewAlert::new(
            AlertSeverity::Warning,
            AlertKind::MemoryUsageHigh,
            "Client memory hit 244MB",
        ))
        .expect("warning alert published");

    assert_eq!(critical.delivery_policy, DeliveryPolicy::Immediate);
    assert_eq!(warning.delivery_policy, DeliveryPolicy::Batch);
    assert_eq!(manager.pending_warning_count(), 1);

    manager.set_warning_batch_window(Duration::ZERO);
    let flushed = manager
        .flush_warning_batch_if_due()
        .expect("warning batch flush should succeed");
    assert_eq!(flushed.len(), 1);
    assert_eq!(flushed[0].kind, AlertKind::MemoryUsageHigh);
    assert_eq!(manager.pending_warning_count(), 0);
}

#[test]
fn alert_threshold_evaluator_generates_runtime_alerts() {
    let evaluator = AlertThresholdEvaluator::new(AppConfig::default_config().alerts.thresholds);

    let death = evaluator
        .death_alert("Frostreaver", Some("CLR"), Some("greatdivide"), true)
        .expect("death alert enabled");
    assert_eq!(death.kind, AlertKind::Death);
    assert_eq!(death.severity, AlertSeverity::Critical);
    assert_eq!(death.zone.as_deref(), Some("greatdivide"));

    let stuck = evaluator
        .stuck_alert("Noxus", Some("kael"), Duration::from_secs(31))
        .expect("stuck alert enabled");
    assert_eq!(stuck.kind, AlertKind::Stuck);
    assert_eq!(stuck.actor.as_deref(), Some("Noxus"));

    let zoning = evaluator
        .zone_transition_timeout_alert("Aelrindel", "skyshrine", Duration::from_secs(90))
        .expect("zone timeout exceeded");
    assert_eq!(zoning.kind, AlertKind::ZoneTransitionTimeout);

    let vendor = evaluator.vendor_timeout_alert("Valerius", Some("bazaar"), "vendor timed out");
    assert_eq!(vendor.kind, AlertKind::VendorTimeout);
    assert_eq!(vendor.source.as_deref(), Some("economy"));

    let memory = evaluator
        .memory_usage_alert("Grok", 256)
        .expect("memory threshold exceeded");
    assert_eq!(memory.kind, AlertKind::MemoryUsageHigh);
    assert_eq!(memory.severity, AlertSeverity::Warning);

    let latency = evaluator
        .ipc_latency_alert(15)
        .expect("latency threshold exceeded");
    assert_eq!(latency.kind, AlertKind::IpcLatencyHigh);

    let error_rate = evaluator
        .error_rate_alert(6)
        .expect("error rate threshold exceeded");
    assert_eq!(error_rate.kind, AlertKind::ErrorRateHigh);

    let dps_drop = evaluator
        .dps_drop_alert("Glacialsurge18", 1000.0, 750.0)
        .expect("dps drop threshold exceeded");
    assert_eq!(dps_drop.kind, AlertKind::DpsDrop);

    assert!(evaluator.memory_usage_alert("Grok", 120).is_none());
    assert!(evaluator.ipc_latency_alert(3).is_none());
    assert!(evaluator.error_rate_alert(2).is_none());
    assert!(
        evaluator
            .zone_transition_timeout_alert("Aelrindel", "skyshrine", Duration::from_secs(20))
            .is_none()
    );
}
