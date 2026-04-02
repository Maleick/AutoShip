use crate::eq::structs::{CastDurationSource, CastState};
use dmft_common::offsets::launch_spell_data;
use std::sync::LazyLock;

/// Environment variable that enables live cast capture logs in `logs/dmft.log`.
pub const LIVE_CAST_CAPTURE_ENV: &str = "DMFT_CAST_CAPTURE";

/// Quantize remaining cast time to reduce log spam while still showing progress updates.
const LIVE_CAST_CAPTURE_BUCKET_MS: u32 = 250;

static LIVE_CAST_CAPTURE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var(LIVE_CAST_CAPTURE_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
});

/// Returns `true` when live cast capture logging is enabled for this process.
#[must_use]
pub fn live_cast_capture_enabled() -> bool {
    *LIVE_CAST_CAPTURE_ENABLED
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveCastCaptureFingerprint {
    spell_id: i32,
    spell_name: Option<String>,
    target_id: u32,
    spell_slot: u8,
    item_id: i32,
    remaining_bucket_ms: Option<u32>,
    total_cast_ms: Option<u32>,
    duration_source: CastDurationSource,
}

/// Snapshot of the active cast fields we want to validate against a real EQ client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCastCaptureSnapshot {
    pub spell_id: i32,
    pub spell_name: Option<String>,
    pub target_id: u32,
    pub spell_slot: u8,
    pub item_id: i32,
    pub remaining_ms: Option<u32>,
    pub total_cast_ms: Option<u32>,
    pub duration_source: CastDurationSource,
}

impl LiveCastCaptureSnapshot {
    /// Build a loggable capture snapshot from an active cast.
    #[must_use]
    pub fn from_cast(cast: Option<&CastState>) -> Option<Self> {
        let cast = cast.filter(|cast| cast.is_casting())?;
        Some(Self {
            spell_id: cast.spell_id,
            spell_name: cast.spell_name.clone(),
            target_id: cast.target_id,
            spell_slot: cast.spell_slot,
            item_id: cast.item_id,
            remaining_ms: cast.remaining_ms,
            total_cast_ms: cast.total_cast_ms,
            duration_source: cast.duration_source,
        })
    }

    fn fingerprint(&self) -> LiveCastCaptureFingerprint {
        LiveCastCaptureFingerprint {
            spell_id: self.spell_id,
            spell_name: self.spell_name.clone(),
            target_id: self.target_id,
            spell_slot: self.spell_slot,
            item_id: self.item_id,
            remaining_bucket_ms: quantize_remaining_ms(self.remaining_ms),
            total_cast_ms: self.total_cast_ms,
            duration_source: self.duration_source,
        }
    }

    fn spell_gem(&self) -> Option<u8> {
        (self.spell_slot != launch_spell_data::NOT_CASTING_SPELL_SLOT)
            .then_some(self.spell_slot + 1)
    }

    fn has_exact_total_cast_time(&self) -> bool {
        self.duration_source.is_exact() && self.total_cast_ms.is_some()
    }

    fn timing_precision_label(&self) -> &'static str {
        if self.has_exact_total_cast_time() {
            "exact"
        } else {
            "est"
        }
    }
}

/// Meaningful active-cast transition worth logging during a live client validation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveCastCaptureEvent {
    Started {
        current: LiveCastCaptureSnapshot,
    },
    Updated {
        previous: LiveCastCaptureSnapshot,
        current: LiveCastCaptureSnapshot,
        changed_fields: Vec<&'static str>,
    },
    Cleared {
        previous: LiveCastCaptureSnapshot,
    },
}

/// Compare two live cast snapshots and return the next event to log, if any.
#[must_use]
pub fn diff_live_cast_capture(
    previous: Option<&LiveCastCaptureSnapshot>,
    current: Option<&LiveCastCaptureSnapshot>,
) -> Option<LiveCastCaptureEvent> {
    match (previous, current) {
        (None, None) => None,
        (None, Some(current)) => Some(LiveCastCaptureEvent::Started {
            current: current.clone(),
        }),
        (Some(previous), None) => Some(LiveCastCaptureEvent::Cleared {
            previous: previous.clone(),
        }),
        (Some(previous), Some(current)) => {
            let changed_fields = changed_fields(previous, current);
            (!changed_fields.is_empty()).then(|| LiveCastCaptureEvent::Updated {
                previous: previous.clone(),
                current: current.clone(),
                changed_fields,
            })
        }
    }
}

/// Emit a structured trace line for a live cast transition.
pub fn log_live_cast_capture_event(
    tick: u64,
    pid: u32,
    character_name: &str,
    event: &LiveCastCaptureEvent,
) {
    let character_name = if character_name.is_empty() {
        "<unknown>"
    } else {
        character_name
    };

    match event {
        LiveCastCaptureEvent::Started { current } => {
            tracing::info!(
                target: "dmft::cast_capture",
                tick,
                pid,
                character = character_name,
                spell_id = current.spell_id,
                spell_name = current.spell_name.as_deref().unwrap_or(""),
                target_id = current.target_id,
                spell_gem = ?current.spell_gem(),
                item_id = ?(current.item_id > 0).then_some(current.item_id),
                remaining_ms = ?current.remaining_ms,
                total_cast_ms = ?current.total_cast_ms,
                duration_source = ?current.duration_source,
                precision = current.timing_precision_label(),
                "cast_capture start"
            );
        }
        LiveCastCaptureEvent::Updated {
            previous,
            current,
            changed_fields,
        } => {
            tracing::info!(
                target: "dmft::cast_capture",
                tick,
                pid,
                character = character_name,
                changed = %changed_fields.join(","),
                spell_id = current.spell_id,
                spell_name = current.spell_name.as_deref().unwrap_or(""),
                target_id = current.target_id,
                previous_target_id = previous.target_id,
                spell_gem = ?current.spell_gem(),
                previous_spell_gem = ?previous.spell_gem(),
                item_id = ?(current.item_id > 0).then_some(current.item_id),
                previous_item_id = ?(previous.item_id > 0).then_some(previous.item_id),
                remaining_ms = ?current.remaining_ms,
                previous_remaining_ms = ?previous.remaining_ms,
                total_cast_ms = ?current.total_cast_ms,
                previous_total_cast_ms = ?previous.total_cast_ms,
                duration_source = ?current.duration_source,
                previous_duration_source = ?previous.duration_source,
                precision = current.timing_precision_label(),
                "cast_capture update"
            );
        }
        LiveCastCaptureEvent::Cleared { previous } => {
            tracing::info!(
                target: "dmft::cast_capture",
                tick,
                pid,
                character = character_name,
                spell_id = previous.spell_id,
                spell_name = previous.spell_name.as_deref().unwrap_or(""),
                target_id = previous.target_id,
                spell_gem = ?previous.spell_gem(),
                item_id = ?(previous.item_id > 0).then_some(previous.item_id),
                remaining_ms = ?previous.remaining_ms,
                total_cast_ms = ?previous.total_cast_ms,
                duration_source = ?previous.duration_source,
                precision = previous.timing_precision_label(),
                "cast_capture clear"
            );
        }
    }
}

fn changed_fields(
    previous: &LiveCastCaptureSnapshot,
    current: &LiveCastCaptureSnapshot,
) -> Vec<&'static str> {
    let previous_fingerprint = previous.fingerprint();
    let current_fingerprint = current.fingerprint();
    let mut changed = Vec::new();

    if previous_fingerprint.spell_id != current_fingerprint.spell_id {
        changed.push("spell_id");
    }
    if previous_fingerprint.spell_name != current_fingerprint.spell_name {
        changed.push("spell_name");
    }
    if previous_fingerprint.target_id != current_fingerprint.target_id {
        changed.push("target_id");
    }
    if previous_fingerprint.spell_slot != current_fingerprint.spell_slot {
        changed.push("spell_slot");
    }
    if previous_fingerprint.item_id != current_fingerprint.item_id {
        changed.push("item_id");
    }
    if previous_fingerprint.remaining_bucket_ms != current_fingerprint.remaining_bucket_ms {
        changed.push("remaining_ms");
    }
    if previous_fingerprint.total_cast_ms != current_fingerprint.total_cast_ms {
        changed.push("total_cast_ms");
    }
    if previous_fingerprint.duration_source != current_fingerprint.duration_source {
        changed.push("duration_source");
    }

    changed
}

fn quantize_remaining_ms(remaining_ms: Option<u32>) -> Option<u32> {
    remaining_ms.map(|ms| (ms / LIVE_CAST_CAPTURE_BUCKET_MS) * LIVE_CAST_CAPTURE_BUCKET_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast_state() -> CastState {
        CastState {
            spell_id: 123,
            spell_name: Some("Celestial Remedy".to_string()),
            target_id: 77,
            spell_eta: 3_000,
            item_id: 0,
            spell_slot: 2,
            remaining_ms: Some(1_480),
            total_cast_ms: Some(3_000),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: None,
        }
    }

    #[test]
    fn snapshot_from_cast_ignores_idle_state() {
        let mut cast = cast_state();
        cast.spell_id = launch_spell_data::NOT_CASTING_SPELL_ID;
        assert!(LiveCastCaptureSnapshot::from_cast(Some(&cast)).is_none());
    }

    #[test]
    fn diff_live_cast_capture_logs_start_for_new_cast() {
        let current = LiveCastCaptureSnapshot::from_cast(Some(&cast_state())).expect("snapshot");
        assert_eq!(
            diff_live_cast_capture(None, Some(&current)),
            Some(LiveCastCaptureEvent::Started { current })
        );
    }

    #[test]
    fn diff_live_cast_capture_ignores_small_remaining_ms_changes_within_bucket() {
        let previous = LiveCastCaptureSnapshot::from_cast(Some(&cast_state())).expect("snapshot");
        let mut current_cast = cast_state();
        current_cast.remaining_ms = Some(1_301);
        let current = LiveCastCaptureSnapshot::from_cast(Some(&current_cast)).expect("snapshot");

        assert_eq!(
            diff_live_cast_capture(Some(&previous), Some(&current)),
            None
        );
    }

    #[test]
    fn diff_live_cast_capture_reports_meaningful_updates() {
        let previous = LiveCastCaptureSnapshot::from_cast(Some(&cast_state())).expect("snapshot");
        let mut current_cast = cast_state();
        current_cast.target_id = 88;
        current_cast.remaining_ms = Some(999);
        let current = LiveCastCaptureSnapshot::from_cast(Some(&current_cast)).expect("snapshot");

        assert_eq!(
            diff_live_cast_capture(Some(&previous), Some(&current)),
            Some(LiveCastCaptureEvent::Updated {
                previous,
                current,
                changed_fields: vec!["target_id", "remaining_ms"],
            })
        );
    }

    #[test]
    fn diff_live_cast_capture_logs_clear_when_cast_ends() {
        let previous = LiveCastCaptureSnapshot::from_cast(Some(&cast_state())).expect("snapshot");
        assert_eq!(
            diff_live_cast_capture(Some(&previous), None),
            Some(LiveCastCaptureEvent::Cleared { previous })
        );
    }

    #[test]
    fn snapshot_timing_precision_label_requires_exact_source_and_known_total() {
        let mut cast = cast_state();

        cast.duration_source = CastDurationSource::ExactRuntime;
        cast.total_cast_ms = Some(3_000);
        let snap = LiveCastCaptureSnapshot::from_cast(Some(&cast)).expect("snapshot");
        assert_eq!(snap.timing_precision_label(), "exact");

        cast.total_cast_ms = None;
        let snap = LiveCastCaptureSnapshot::from_cast(Some(&cast)).expect("snapshot");
        assert_eq!(snap.timing_precision_label(), "est");

        cast.duration_source = CastDurationSource::SpellDataBase;
        cast.total_cast_ms = Some(3_000);
        let snap = LiveCastCaptureSnapshot::from_cast(Some(&cast)).expect("snapshot");
        assert_eq!(snap.timing_precision_label(), "est");
    }
}
