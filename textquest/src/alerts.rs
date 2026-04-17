//! Operational alerting — persistence, routing, and delivery integrations.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use lettre::{
    Message, SmtpTransport, Transport, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::discord::webhook::{AlertLevel, DiscordAlert, EventCategory, WebhookSender};

/// Default SQLite path for persisted operational alerts (relative to the
/// repo root). Consumers should call [`resolve_alert_db_path`] to obtain
/// the absolute path actually opened so TUI and web always agree.
pub const ALERT_DB_PATH: &str = "data/alerts.db";

/// Resolve the alerts-DB path to an absolute location shared by both the
/// TUI and the web dashboard regardless of each binary's working directory.
///
/// Resolution order:
///   1. `TEXTQUEST_ALERT_DB_PATH` environment variable (absolute path).
///   2. The first ancestor of `CARGO_MANIFEST_DIR` that contains `Cargo.lock`
///      (the repo root), joined with [`ALERT_DB_PATH`].
///   3. `ALERT_DB_PATH` itself, which falls back to CWD-relative.
///
/// Both the TUI (`App::open_alert_store`) and the web binary
/// (`textquest-web::open_alert_store`) call this helper so a mis-aligned
/// working directory between the two processes cannot split alert history.
pub fn resolve_alert_db_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("TEXTQUEST_ALERT_DB_PATH") {
        if !override_path.is_empty() {
            return PathBuf::from(override_path);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        if ancestor.join("Cargo.lock").exists() {
            return ancestor.join(ALERT_DB_PATH);
        }
    }

    PathBuf::from(ALERT_DB_PATH)
}

const ALERT_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS alerts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at       TEXT NOT NULL,
    severity         TEXT NOT NULL,
    kind             TEXT NOT NULL,
    message          TEXT NOT NULL,
    source           TEXT,
    actor            TEXT,
    zone             TEXT,
    metadata_json    TEXT,
    acknowledged_at  TEXT,
    acknowledged_by  TEXT
);
CREATE INDEX IF NOT EXISTS idx_alerts_created_at ON alerts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_alerts_severity ON alerts(severity);
CREATE INDEX IF NOT EXISTS idx_alerts_kind ON alerts(kind);
CREATE INDEX IF NOT EXISTS idx_alerts_unack ON alerts(acknowledged_at);
";

/// Alert severity tier controlling delivery urgency and UI emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

impl AlertSeverity {
    #[must_use]
    pub const fn delivery_policy(self) -> DeliveryPolicy {
        match self {
            Self::Critical => DeliveryPolicy::Immediate,
            Self::Warning => DeliveryPolicy::Batch,
            Self::Info => DeliveryPolicy::LogOnly,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "critical" => Ok(Self::Critical),
            "warning" => Ok(Self::Warning),
            "info" => Ok(Self::Info),
            other => bail!("Unknown alert severity: {other}"),
        }
    }

    #[must_use]
    pub const fn discord_level(self) -> AlertLevel {
        match self {
            Self::Critical => AlertLevel::Critical,
            Self::Warning => AlertLevel::Warning,
            Self::Info => AlertLevel::Info,
        }
    }
}

/// Canonical alert categories for operational failures and summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    Death,
    InjectionFailure,
    Stuck,
    ZoneTransitionTimeout,
    VendorTimeout,
    BankTimeout,
    EconomyFailure,
    MemoryUsageHigh,
    IpcLatencyHigh,
    ErrorRateHigh,
    DpsDrop,
    DailySummary,
    ConfigChanged,
}

impl AlertKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::InjectionFailure => "injection_failure",
            Self::Stuck => "stuck",
            Self::ZoneTransitionTimeout => "zone_transition_timeout",
            Self::VendorTimeout => "vendor_timeout",
            Self::BankTimeout => "bank_timeout",
            Self::EconomyFailure => "economy_failure",
            Self::MemoryUsageHigh => "memory_usage_high",
            Self::IpcLatencyHigh => "ipc_latency_high",
            Self::ErrorRateHigh => "error_rate_high",
            Self::DpsDrop => "dps_drop",
            Self::DailySummary => "daily_summary",
            Self::ConfigChanged => "config_changed",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "death" => Ok(Self::Death),
            "injection_failure" => Ok(Self::InjectionFailure),
            "stuck" => Ok(Self::Stuck),
            "zone_transition_timeout" => Ok(Self::ZoneTransitionTimeout),
            "vendor_timeout" => Ok(Self::VendorTimeout),
            "bank_timeout" => Ok(Self::BankTimeout),
            "economy_failure" => Ok(Self::EconomyFailure),
            "memory_usage_high" => Ok(Self::MemoryUsageHigh),
            "ipc_latency_high" => Ok(Self::IpcLatencyHigh),
            "error_rate_high" => Ok(Self::ErrorRateHigh),
            "dps_drop" => Ok(Self::DpsDrop),
            "daily_summary" => Ok(Self::DailySummary),
            "config_changed" => Ok(Self::ConfigChanged),
            other => bail!("Unknown alert kind: {other}"),
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Death => "Character Death",
            Self::InjectionFailure => "DLL Injection Failure",
            Self::Stuck => "Stuck Detection",
            Self::ZoneTransitionTimeout => "Zone Transition Timeout",
            Self::VendorTimeout => "Vendor Timeout",
            Self::BankTimeout => "Bank Timeout",
            Self::EconomyFailure => "Economy Failure",
            Self::MemoryUsageHigh => "Memory Warning",
            Self::IpcLatencyHigh => "IPC Latency Warning",
            Self::ErrorRateHigh => "Error Rate Warning",
            Self::DpsDrop => "DPS Drop",
            Self::DailySummary => "Daily Summary",
            Self::ConfigChanged => "Config Changed",
        }
    }
}

/// How an alert should be delivered after persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryPolicy {
    Immediate,
    Batch,
    LogOnly,
}

/// An alert ready to be inserted into persistent storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAlert {
    pub severity: AlertSeverity,
    pub kind: AlertKind,
    pub message: String,
    pub source: Option<String>,
    pub actor: Option<String>,
    pub zone: Option<String>,
    pub metadata_json: Option<String>,
}

impl NewAlert {
    #[must_use]
    pub fn new(severity: AlertSeverity, kind: AlertKind, message: impl Into<String>) -> Self {
        Self {
            severity,
            kind,
            message: message.into(),
            source: None,
            actor: None,
            zone: None,
            metadata_json: None,
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    #[must_use]
    pub fn with_zone(mut self, zone: impl Into<String>) -> Self {
        self.zone = Some(zone.into());
        self
    }

    #[must_use]
    pub fn with_metadata_json(mut self, metadata_json: impl Into<String>) -> Self {
        self.metadata_json = Some(metadata_json.into());
        self
    }
}

/// A persisted alert record returned to UIs and integrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRecord {
    pub id: i64,
    pub created_at: String,
    pub severity: AlertSeverity,
    pub kind: AlertKind,
    pub message: String,
    pub source: Option<String>,
    pub actor: Option<String>,
    pub zone: Option<String>,
    pub metadata_json: Option<String>,
    pub acknowledged_at: Option<String>,
    pub acknowledged_by: Option<String>,
}

impl AlertRecord {
    #[must_use]
    pub fn unread(&self) -> bool {
        self.acknowledged_at.is_none()
    }
}

/// Result of publishing an alert through the alert manager.
#[derive(Debug, Clone)]
pub struct PublishedAlert {
    pub record: AlertRecord,
    pub delivery_policy: DeliveryPolicy,
}

/// Daily summary payload for info alerts and email digests.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailySummary {
    pub generated_at: String,
    pub kills: u64,
    pub loot_items: u64,
    pub profit_plat: i64,
    pub session_count: u32,
    pub notes: Vec<String>,
}

impl DailySummary {
    #[must_use]
    pub fn new() -> Self {
        Self {
            generated_at: now_iso(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let mut lines = vec![
            format!("Generated: {}", self.generated_at),
            format!("Kills: {}", self.kills),
            format!("Loot Items: {}", self.loot_items),
            format!("Profit: {}pp", self.profit_plat),
            format!("Sessions: {}", self.session_count),
        ];
        if !self.notes.is_empty() {
            lines.push(String::new());
            lines.push(String::from("Notes:"));
            lines.extend(self.notes.iter().map(|note| format!("- {note}")));
        }
        lines.join("\n")
    }
}

/// Threshold-driven alert generator for runtime health observations.
#[derive(Debug, Clone)]
pub struct AlertThresholdEvaluator {
    thresholds: crate::config::AlertThresholdConfig,
}

impl AlertThresholdEvaluator {
    #[must_use]
    pub fn new(thresholds: crate::config::AlertThresholdConfig) -> Self {
        Self { thresholds }
    }

    #[must_use]
    pub fn death_alert(
        &self,
        actor: &str,
        class_name: Option<&str>,
        zone: Option<&str>,
        resurrect_available: bool,
    ) -> Option<NewAlert> {
        if !self.thresholds.death_alert {
            return None;
        }

        let mut message = format!("{actor} died");
        if let Some(class_name) = class_name.filter(|value| !value.trim().is_empty()) {
            message.push_str(&format!(" ({class_name})"));
        }
        if resurrect_available {
            message.push_str(" — resurrection available");
        }

        let mut alert = NewAlert::new(AlertSeverity::Critical, AlertKind::Death, message)
            .with_source("combat")
            .with_actor(actor);
        if let Some(zone) = zone.filter(|value| !value.trim().is_empty()) {
            alert = alert.with_zone(zone);
        }
        Some(alert)
    }

    #[must_use]
    pub fn injection_failure_alert(&self, actor: &str, detail: impl AsRef<str>) -> NewAlert {
        NewAlert::new(
            AlertSeverity::Critical,
            AlertKind::InjectionFailure,
            format!("{actor}: DLL injection failure — {}", detail.as_ref()),
        )
        .with_source("inject")
        .with_actor(actor)
    }

    #[must_use]
    pub fn stuck_alert(
        &self,
        actor: &str,
        zone: Option<&str>,
        stuck_for: Duration,
    ) -> Option<NewAlert> {
        if !self.thresholds.stuck_alert {
            return None;
        }

        let mut alert = NewAlert::new(
            AlertSeverity::Critical,
            AlertKind::Stuck,
            format!("{actor} stopped moving for {}s", stuck_for.as_secs().max(1)),
        )
        .with_source("navigation")
        .with_actor(actor);
        if let Some(zone) = zone.filter(|value| !value.trim().is_empty()) {
            alert = alert.with_zone(zone);
        }
        Some(alert)
    }

    #[must_use]
    pub fn zone_transition_timeout_alert(
        &self,
        actor: &str,
        target_zone: &str,
        elapsed: Duration,
    ) -> Option<NewAlert> {
        if elapsed.as_secs() < self.thresholds.zone_timeout_secs {
            return None;
        }

        Some(
            NewAlert::new(
                AlertSeverity::Critical,
                AlertKind::ZoneTransitionTimeout,
                format!(
                    "{actor} exceeded zone transition timeout into {target_zone} ({}s)",
                    elapsed.as_secs()
                ),
            )
            .with_source("navigation")
            .with_actor(actor)
            .with_zone(target_zone),
        )
    }

    #[must_use]
    pub fn vendor_timeout_alert(
        &self,
        actor: &str,
        zone: Option<&str>,
        detail: impl AsRef<str>,
    ) -> NewAlert {
        self.economy_failure_alert(AlertKind::VendorTimeout, actor, zone, detail)
    }

    #[must_use]
    pub fn bank_timeout_alert(
        &self,
        actor: &str,
        zone: Option<&str>,
        detail: impl AsRef<str>,
    ) -> NewAlert {
        self.economy_failure_alert(AlertKind::BankTimeout, actor, zone, detail)
    }

    #[must_use]
    pub fn economy_failure_alert(
        &self,
        kind: AlertKind,
        actor: &str,
        zone: Option<&str>,
        detail: impl AsRef<str>,
    ) -> NewAlert {
        let mut alert = NewAlert::new(
            AlertSeverity::Critical,
            kind,
            format!("{actor}: {}", detail.as_ref()),
        )
        .with_source("economy")
        .with_actor(actor);
        if let Some(zone) = zone.filter(|value| !value.trim().is_empty()) {
            alert = alert.with_zone(zone);
        }
        alert
    }

    #[must_use]
    pub fn memory_usage_alert(&self, actor: &str, usage_mb: u32) -> Option<NewAlert> {
        if usage_mb < self.thresholds.memory_warning_mb {
            return None;
        }

        Some(
            NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::MemoryUsageHigh,
                format!(
                    "{actor} memory usage reached {usage_mb}MB (threshold {}MB)",
                    self.thresholds.memory_warning_mb
                ),
            )
            .with_source("observability")
            .with_actor(actor),
        )
    }

    #[must_use]
    pub fn ipc_latency_alert(&self, p95_latency_ms: u32) -> Option<NewAlert> {
        if p95_latency_ms < self.thresholds.ipc_latency_warning_ms {
            return None;
        }

        Some(
            NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::IpcLatencyHigh,
                format!(
                    "IPC latency p95 reached {p95_latency_ms}ms (threshold {}ms)",
                    self.thresholds.ipc_latency_warning_ms
                ),
            )
            .with_source("observability"),
        )
    }

    #[must_use]
    pub fn error_rate_alert(&self, errors_per_min: u32) -> Option<NewAlert> {
        if errors_per_min < self.thresholds.error_rate_warning_per_min {
            return None;
        }

        Some(
            NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::ErrorRateHigh,
                format!(
                    "Error rate reached {errors_per_min}/min (threshold {}/min)",
                    self.thresholds.error_rate_warning_per_min
                ),
            )
            .with_source("observability"),
        )
    }

    #[must_use]
    pub fn dps_drop_alert(
        &self,
        actor: &str,
        baseline_dps: f64,
        current_dps: f64,
    ) -> Option<NewAlert> {
        if baseline_dps <= 0.0 || current_dps >= baseline_dps {
            return None;
        }

        let drop_pct = ((baseline_dps - current_dps) / baseline_dps) * 100.0;
        if drop_pct < f64::from(self.thresholds.dps_drop_warning_pct) {
            return None;
        }

        Some(
            NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::DpsDrop,
                format!(
                    "{actor} DPS fell {:.0}% below baseline ({current_dps:.0} vs \
                     {baseline_dps:.0})",
                    drop_pct
                ),
            )
            .with_source("observability")
            .with_actor(actor),
        )
    }
}

/// SQLite-backed operational alert store.
#[derive(Clone)]
pub struct AlertStore {
    conn: Arc<Mutex<Connection>>,
}

impl AlertStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open alert DB: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set alert store PRAGMA")?;
        conn.execute_batch(ALERT_SCHEMA)
            .context("Failed to initialize alert schema")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory alert DB")?;
        conn.execute_batch(ALERT_SCHEMA)
            .context("Failed to initialize in-memory alert schema")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert(&self, alert: &NewAlert) -> Result<i64> {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        conn.execute(
            "INSERT INTO alerts (created_at, severity, kind, message, source, actor, zone, \
             metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                now_iso(),
                alert.severity.as_str(),
                alert.kind.as_str(),
                alert.message.as_str(),
                alert.source.as_deref(),
                alert.actor.as_deref(),
                alert.zone.as_deref(),
                alert.metadata_json.as_deref(),
            ],
        )
        .context("Failed to insert alert")?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get(&self, id: i64) -> Result<Option<AlertRecord>> {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        conn.query_row(
            "SELECT id, created_at, severity, kind, message, source, actor, zone, metadata_json, \
             acknowledged_at, acknowledged_by FROM alerts WHERE id = ?1",
            params![id],
            map_alert_row,
        )
        .optional()
        .context("Failed to fetch alert")
    }

    pub fn recent(&self, limit: u32) -> Result<Vec<AlertRecord>> {
        self.query_many(
            "SELECT id, created_at, severity, kind, message, source, actor, zone, metadata_json, \
             acknowledged_at, acknowledged_by FROM alerts ORDER BY id DESC LIMIT ?1",
            params![limit],
        )
    }

    pub fn unread(&self, limit: u32) -> Result<Vec<AlertRecord>> {
        self.query_many(
            "SELECT id, created_at, severity, kind, message, source, actor, zone, metadata_json, \
             acknowledged_at, acknowledged_by FROM alerts WHERE acknowledged_at IS NULL ORDER BY \
             id DESC LIMIT ?1",
            params![limit],
        )
    }

    pub fn unread_count(&self) -> Result<u64> {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM alerts WHERE acknowledged_at IS NULL",
                [],
                |row| row.get(0),
            )
            .context("Failed to count unread alerts")?;
        Ok(count as u64)
    }

    pub fn acknowledge(&self, id: i64, operator: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        let updated = conn
            .execute(
                "UPDATE alerts SET acknowledged_at = ?1, acknowledged_by = ?2 WHERE id = ?3 AND \
                 acknowledged_at IS NULL",
                params![now_iso(), operator.trim(), id],
            )
            .with_context(|| format!("Failed to acknowledge alert {id}"))?;
        Ok(updated)
    }

    pub fn acknowledge_all(&self, operator: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        let updated = conn
            .execute(
                "UPDATE alerts SET acknowledged_at = ?1, acknowledged_by = ?2 WHERE \
                 acknowledged_at IS NULL",
                params![now_iso(), operator.trim()],
            )
            .context("Failed to acknowledge all alerts")?;
        Ok(updated)
    }

    fn query_many<P>(&self, sql: &str, params: P) -> Result<Vec<AlertRecord>>
    where
        P: rusqlite::Params,
    {
        let conn = self.conn.lock().expect("alert store lock poisoned");
        let mut stmt = conn.prepare(sql).context("Failed to prepare alert query")?;
        let rows = stmt
            .query_map(params, map_alert_row)
            .context("Failed to execute alert query")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect alert rows")
    }
}

/// Discord webhook integration for operational alerts.
pub struct DiscordAlertClient {
    sender: WebhookSender,
}

impl DiscordAlertClient {
    #[must_use]
    pub fn new(webhook_url: String) -> Self {
        Self {
            sender: WebhookSender::new(webhook_url),
        }
    }

    pub fn send_alert(&self, alert: &AlertRecord) {
        let mut message = alert.message.clone();
        if alert.kind == AlertKind::Death {
            message = format!("@everyone {message}");
        }

        let mut discord_alert = DiscordAlert::simple(
            alert.kind.display_name(),
            message,
            alert.severity.discord_level(),
            EventCategory::Status,
        );

        if let Some(actor) = &alert.actor {
            discord_alert = discord_alert.with_field("Actor", actor, true);
        }
        if let Some(zone) = &alert.zone {
            discord_alert = discord_alert.with_field("Zone", zone, true);
        }
        if let Some(source) = &alert.source {
            discord_alert = discord_alert.with_field("Source", source, true);
        }

        self.sender.send(discord_alert);
    }

    pub fn send_warning_batch(&self, alerts: &[AlertRecord]) {
        if alerts.is_empty() {
            return;
        }

        let mut discord_alert = DiscordAlert::simple(
            format!("Warning Batch ({})", alerts.len()),
            "Batched warning alerts from the last alert window.",
            AlertLevel::Warning,
            EventCategory::Status,
        );
        for alert in alerts.iter().take(10) {
            discord_alert =
                discord_alert.with_field(alert.kind.display_name(), alert.message.clone(), false);
        }
        self.sender.send(discord_alert);
    }

    pub fn send_daily_summary(&self, summary: &DailySummary) {
        self.sender.send(
            DiscordAlert::simple(
                "Daily Summary",
                summary.render_text(),
                AlertLevel::Info,
                EventCategory::Status,
            )
            .with_field("Kills", summary.kills.to_string(), true)
            .with_field("Loot", summary.loot_items.to_string(), true)
            .with_field("Profit", format!("{}pp", summary.profit_plat), true),
        );
    }
}

/// SMTP email delivery for critical alerts and daily summaries.
pub struct EmailAlertClient {
    transport: SmtpTransport,
    from: Mailbox,
    recipients: Vec<Mailbox>,
    subject_prefix: String,
}

impl EmailAlertClient {
    pub fn from_config(config: &crate::config::AlertingConfig) -> Result<Option<Self>> {
        if !config.enable_email || config.email_recipients.is_empty() {
            return Ok(None);
        }
        if config.smtp_server.trim().is_empty() || config.email_from.trim().is_empty() {
            return Ok(None);
        }

        let from: Mailbox = config
            .email_from
            .parse()
            .with_context(|| format!("Invalid alert email_from address: {}", config.email_from))?;
        let recipients = config
            .email_recipients
            .iter()
            .map(|recipient| {
                recipient
                    .parse()
                    .with_context(|| format!("Invalid alert recipient address: {recipient}"))
            })
            .collect::<Result<Vec<Mailbox>>>()?;

        let mut builder = SmtpTransport::relay(&config.smtp_server)
            .with_context(|| format!("Invalid SMTP relay host: {}", config.smtp_server))?;
        if config.smtp_port != 0 {
            builder = builder.port(config.smtp_port);
        }
        if !config.smtp_username.trim().is_empty() {
            builder = builder.credentials(Credentials::new(
                config.smtp_username.clone(),
                config.smtp_password.clone(),
            ));
        }

        Ok(Some(Self {
            transport: builder.build(),
            from,
            recipients,
            subject_prefix: config.email_subject_prefix.clone(),
        }))
    }

    pub fn send_critical_alert(&self, alert: &AlertRecord) -> Result<()> {
        let subject = format!("{}{}", self.subject_prefix, alert.kind.display_name());
        let body = render_email_body(alert);
        self.send_mail(&subject, &body)
    }

    pub fn send_daily_summary(&self, summary: &DailySummary) -> Result<()> {
        let subject = format!("{}Daily Summary", self.subject_prefix);
        self.send_mail(&subject, &summary.render_text())
    }

    fn send_mail(&self, subject: &str, body: &str) -> Result<()> {
        let mut builder = Message::builder().from(self.from.clone()).subject(subject);
        for recipient in &self.recipients {
            builder = builder.to(recipient.clone());
        }
        let email = builder
            .body(body.to_string())
            .context("Failed to construct alert email")?;
        self.transport
            .send(&email)
            .context("Failed to send alert email")?;
        Ok(())
    }
}

/// Alert manager that persists alerts and dispatches them by severity tier.
pub struct AlertManager {
    store: AlertStore,
    warning_batch_window: Duration,
    pending_warning_ids: Vec<i64>,
    last_warning_flush: Instant,
    discord: Option<DiscordAlertClient>,
    email: Option<EmailAlertClient>,
}

impl AlertManager {
    #[must_use]
    pub fn new(store: AlertStore) -> Self {
        Self {
            store,
            warning_batch_window: Duration::from_secs(300),
            pending_warning_ids: Vec::new(),
            last_warning_flush: Instant::now(),
            discord: None,
            email: None,
        }
    }

    pub fn store(&self) -> &AlertStore {
        &self.store
    }

    pub fn set_warning_batch_window(&mut self, window: Duration) {
        self.warning_batch_window = window;
    }

    pub fn set_discord_client(&mut self, client: DiscordAlertClient) {
        self.discord = Some(client);
    }

    pub fn set_email_client(&mut self, client: EmailAlertClient) {
        self.email = Some(client);
    }

    pub fn publish(&mut self, alert: NewAlert) -> Result<PublishedAlert> {
        let id = self.store.insert(&alert)?;
        let record = self
            .store
            .get(id)?
            .with_context(|| format!("Inserted alert {id} was not found"))?;
        let delivery_policy = record.severity.delivery_policy();

        match delivery_policy {
            DeliveryPolicy::Immediate => self.deliver_immediate(&record)?,
            DeliveryPolicy::Batch => {
                self.pending_warning_ids.push(record.id);
                // flush_warning_batch_if_due is time-gated by warning_batch_window,
                // so rapid warning bursts still collapse into a single delivery.
                self.flush_warning_batch_if_due()?;
            }
            DeliveryPolicy::LogOnly => {}
        }

        Ok(PublishedAlert {
            record,
            delivery_policy,
        })
    }

    pub fn pending_warning_count(&self) -> usize {
        self.pending_warning_ids.len()
    }

    pub fn flush_warning_batch(&mut self) -> Result<Vec<AlertRecord>> {
        if self.pending_warning_ids.is_empty() {
            self.last_warning_flush = Instant::now();
            return Ok(Vec::new());
        }

        // Fetch without draining so a transient SQLite read failure doesn't
        // lose queued IDs. Only clear the queue once the batch has been
        // assembled successfully.
        let mut alerts = Vec::with_capacity(self.pending_warning_ids.len());
        for id in &self.pending_warning_ids {
            if let Some(alert) = self.store.get(*id)? {
                alerts.push(alert);
            }
        }
        self.pending_warning_ids.clear();

        if let Some(discord) = &self.discord {
            discord.send_warning_batch(&alerts);
        }

        self.last_warning_flush = Instant::now();
        Ok(alerts)
    }

    pub fn flush_warning_batch_if_due(&mut self) -> Result<Vec<AlertRecord>> {
        if self.last_warning_flush.elapsed() < self.warning_batch_window {
            return Ok(Vec::new());
        }
        self.flush_warning_batch()
    }

    pub fn publish_daily_summary(&mut self, summary: DailySummary) -> Result<PublishedAlert> {
        let alert = NewAlert::new(
            AlertSeverity::Info,
            AlertKind::DailySummary,
            format!(
                "{} kills, {} loot items, {}pp profit",
                summary.kills, summary.loot_items, summary.profit_plat
            ),
        )
        .with_metadata_json(serde_json::to_string(&summary).context("Daily summary JSON")?);

        let published = self.publish(alert)?;

        if let Some(discord) = &self.discord {
            discord.send_daily_summary(&summary);
        }
        if let Some(email) = &self.email {
            email.send_daily_summary(&summary)?;
        }

        Ok(published)
    }

    fn deliver_immediate(&self, alert: &AlertRecord) -> Result<()> {
        if let Some(discord) = &self.discord {
            discord.send_alert(alert);
        }
        if alert.severity == AlertSeverity::Critical
            && let Some(email) = &self.email
        {
            email.send_critical_alert(alert)?;
        }
        Ok(())
    }
}

fn map_alert_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlertRecord> {
    let severity: String = row.get(2)?;
    let kind: String = row.get(3)?;
    Ok(AlertRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        severity: AlertSeverity::from_db(&severity)
            .map_err(|error| invalid_alert_enum(2, error.to_string()))?,
        kind: AlertKind::from_db(&kind)
            .map_err(|error| invalid_alert_enum(3, error.to_string()))?,
        message: row.get(4)?,
        source: row.get(5)?,
        actor: row.get(6)?,
        zone: row.get(7)?,
        metadata_json: row.get(8)?,
        acknowledged_at: row.get(9)?,
        acknowledged_by: row.get(10)?,
    })
}

fn invalid_alert_enum(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn render_email_body(alert: &AlertRecord) -> String {
    let mut lines = vec![
        format!("Severity: {}", alert.severity.as_str()),
        format!("Kind: {}", alert.kind.display_name()),
        format!("Created: {}", alert.created_at),
        String::new(),
        alert.message.clone(),
    ];
    if let Some(actor) = &alert.actor {
        lines.push(format!("Actor: {actor}"));
    }
    if let Some(zone) = &alert.zone {
        lines.push(format!("Zone: {zone}"));
    }
    if let Some(source) = &alert.source {
        lines.push(format!("Source: {source}"));
    }
    lines.join("\n")
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_maps_to_delivery_policy() {
        assert_eq!(
            AlertSeverity::Critical.delivery_policy(),
            DeliveryPolicy::Immediate
        );
        assert_eq!(
            AlertSeverity::Warning.delivery_policy(),
            DeliveryPolicy::Batch
        );
        assert_eq!(
            AlertSeverity::Info.delivery_policy(),
            DeliveryPolicy::LogOnly
        );
    }

    #[test]
    fn store_acknowledge_all_marks_unread_records() {
        let store = AlertStore::open_memory().expect("memory store");
        store
            .insert(&NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::MemoryUsageHigh,
                "warning",
            ))
            .expect("insert warning");
        store
            .insert(&NewAlert::new(
                AlertSeverity::Critical,
                AlertKind::Death,
                "critical",
            ))
            .expect("insert critical");

        let updated = store.acknowledge_all("ops").expect("ack all");
        assert_eq!(updated, 2);
        assert_eq!(store.unread_count().expect("count"), 0);
    }
}
