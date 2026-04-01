use crate::eq::structs::{CastState, EqClass};
use dmft_common::offsets::launch_spell_data;

/// Shared cast-strip presentation model for TUI surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct CastDisplay {
    /// Long-form label for wider layouts.
    pub label: String,
    /// Short label for compact layouts.
    pub short_label: String,
    /// Normalized progress from 0.0 to 1.0.
    pub progress: f64,
    /// Remaining cast time in seconds, when known.
    pub remaining_secs: Option<f32>,
    /// Elapsed cast time in seconds, when known.
    pub elapsed_secs: Option<f32>,
    /// Supplemental status text, usually a slot hint or cast state.
    pub status_text: Option<String>,
    /// Whether the label/timing are exact rather than heuristic.
    pub exact: bool,
}

impl CastDisplay {
    /// Create an exact progress display from a known total cast duration.
    #[must_use]
    pub fn exact_progress(
        label: impl Into<String>,
        short_label: impl Into<String>,
        progress: f64,
        total_secs: f32,
    ) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        Self {
            label: label.into(),
            short_label: short_label.into(),
            progress,
            remaining_secs: Some((total_secs * (1.0 - progress as f32)).max(0.0)),
            elapsed_secs: Some((total_secs * progress as f32).max(0.0)),
            status_text: Some(String::from("casting")),
            exact: true,
        }
    }

    /// Create a provisional cast strip when only coarse local cast state is known.
    #[must_use]
    pub fn provisional(
        label: impl Into<String>,
        short_label: impl Into<String>,
        progress: f64,
        status_text: Option<String>,
    ) -> Self {
        Self::provisional_timed(label, short_label, progress, None, None, status_text)
    }

    /// Create a provisional cast strip with whatever timing data is available.
    #[must_use]
    pub fn provisional_timed(
        label: impl Into<String>,
        short_label: impl Into<String>,
        progress: f64,
        remaining_secs: Option<f32>,
        elapsed_secs: Option<f32>,
        status_text: Option<String>,
    ) -> Self {
        Self {
            label: label.into(),
            short_label: short_label.into(),
            progress: progress.clamp(0.0, 1.0),
            remaining_secs,
            elapsed_secs,
            status_text,
            exact: false,
        }
    }

    /// Choose the appropriate label for the available width.
    #[must_use]
    pub fn preferred_label(&self, compact: bool) -> &str {
        if compact {
            &self.short_label
        } else {
            &self.label
        }
    }
}

/// Build a compact label for width-constrained cast strips.
#[must_use]
pub fn short_cast_label(label: &str) -> String {
    if label.eq_ignore_ascii_case("Complete Heal") {
        return String::from("CH");
    }

    let initials: String = label
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(4)
        .collect();
    if initials.len() >= 2 {
        return initials.to_uppercase();
    }

    label.chars().take(4).collect()
}

/// Build a heuristic live-cast label from class role and gem slot.
#[must_use]
pub fn heuristic_live_cast(
    slot: u8,
    class: Option<EqClass>,
    tick_count: u64,
    pid: u32,
) -> CastDisplay {
    let gem = slot.saturating_add(1);
    let has_known_gem = slot != launch_spell_data::NOT_CASTING_SPELL_SLOT;
    let role = if matches!(
        class,
        Some(EqClass::Cleric | EqClass::Druid | EqClass::Shaman | EqClass::Paladin)
    ) {
        "Heal"
    } else if matches!(
        class,
        Some(EqClass::Enchanter | EqClass::Shaman | EqClass::Necromancer | EqClass::Bard)
    ) {
        "Debuff"
    } else {
        "Cast"
    };
    let phase = ((tick_count / 2).wrapping_add(u64::from(pid)) % 12) as f64 / 11.0;
    CastDisplay::provisional(
        if has_known_gem {
            format!("{role} Gem {gem}")
        } else {
            role.to_string()
        },
        if has_known_gem {
            format!("{role} G{gem}")
        } else {
            role.chars().take(4).collect()
        },
        phase,
        Some(if has_known_gem {
            format!("gem {gem}")
        } else {
            String::from("casting")
        }),
    )
}

fn live_cast_status_text(cast: &CastState) -> Option<String> {
    if let Some(gem) = cast.spell_gem() {
        Some(format!("gem {gem}"))
    } else if cast.item_id > 0 {
        Some(format!("item {}", cast.item_id))
    } else {
        Some(String::from("casting"))
    }
}

/// Build the best available live cast-strip model from a real EQ `CastState`.
///
/// Exact runtime progress is used only when the backend can prove the total cast
/// duration. Otherwise we still surface exact remaining time and resolved spell
/// names while keeping `exact = false`.
#[must_use]
pub fn live_cast_display(
    cast: &CastState,
    class: Option<EqClass>,
    tick_count: u64,
    pid: u32,
) -> CastDisplay {
    let fallback = heuristic_live_cast(cast.spell_slot, class, tick_count, pid);
    let label = cast
        .spell_name
        .clone()
        .unwrap_or_else(|| fallback.label.clone());
    let short_label = short_cast_label(&label);
    let remaining_secs = cast.cast_time_remaining_ms().map(|ms| ms as f32 / 1000.0);
    let elapsed_secs = cast.cast_time_elapsed_ms().map(|ms| ms as f32 / 1000.0);
    let status_text = live_cast_status_text(cast);

    if let (Some(progress), Some(total_ms)) = (cast.cast_progress(), cast.cast_time_total_ms()) {
        if cast.has_exact_total_cast_time() {
            return CastDisplay::exact_progress(
                label,
                short_label,
                progress,
                total_ms as f32 / 1000.0,
            );
        }

        return CastDisplay::provisional_timed(
            label,
            short_label,
            progress,
            remaining_secs,
            elapsed_secs,
            status_text,
        );
    }

    CastDisplay::provisional_timed(
        label,
        short_label,
        fallback.progress,
        remaining_secs,
        elapsed_secs,
        status_text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::CastDurationSource;
    use dmft_common::offsets::launch_spell_data;

    #[test]
    fn exact_progress_computes_remaining_and_elapsed() {
        let display = CastDisplay::exact_progress("Complete Heal", "CH", 0.25, 10.0);
        assert!((display.progress - 0.25).abs() < f64::EPSILON);
        assert_eq!(display.remaining_secs, Some(7.5));
        assert_eq!(display.elapsed_secs, Some(2.5));
        assert!(display.exact);
    }

    #[test]
    fn provisional_progress_clamps() {
        let display = CastDisplay::provisional("Cast", "C", 2.0, None);
        assert!((display.progress - 1.0).abs() < f64::EPSILON);
        assert!(!display.exact);
    }

    #[test]
    fn provisional_timed_keeps_supplied_timing() {
        let display = CastDisplay::provisional_timed("Cast", "C", 0.5, Some(1.5), Some(1.0), None);
        assert_eq!(display.remaining_secs, Some(1.5));
        assert_eq!(display.elapsed_secs, Some(1.0));
        assert!(!display.exact);
    }

    #[test]
    fn heuristic_live_cast_uses_heal_role_for_cleric() {
        let display = heuristic_live_cast(2, Some(EqClass::Cleric), 10, 100);
        assert!(display.label.starts_with("Heal"));
        assert_eq!(display.status_text.as_deref(), Some("gem 3"));
    }

    #[test]
    fn short_cast_label_uses_ch_for_complete_heal() {
        assert_eq!(short_cast_label("Complete Heal"), "CH");
    }

    #[test]
    fn short_cast_label_prefers_initials() {
        assert_eq!(short_cast_label("Ice Comet"), "IC");
    }

    #[test]
    fn live_cast_display_prefers_resolved_spell_name_and_live_remaining() {
        let cast = CastState {
            spell_id: 123,
            spell_name: Some("Celestial Remedy".to_string()),
            target_id: 77,
            spell_eta: 4_000,
            item_id: 0,
            spell_slot: 2,
            remaining_ms: Some(1_500),
            total_cast_ms: Some(3_000),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: None,
        };

        let display = live_cast_display(&cast, Some(EqClass::Cleric), 10, 100);
        assert_eq!(display.label, "Celestial Remedy");
        assert_eq!(display.short_label, "CR");
        assert_eq!(display.remaining_secs, Some(1.5));
        assert_eq!(display.elapsed_secs, Some(1.5));
        assert!(!display.exact);
    }

    #[test]
    fn live_cast_display_reports_item_casts_without_fake_exact_duration() {
        let cast = CastState {
            spell_id: 555,
            spell_name: Some("Gate".to_string()),
            target_id: 0,
            spell_eta: 2_000,
            item_id: 9_999,
            spell_slot: launch_spell_data::NOT_CASTING_SPELL_SLOT,
            remaining_ms: Some(750),
            total_cast_ms: None,
            duration_source: CastDurationSource::Unknown,
            gem_etas: None,
        };

        let display = live_cast_display(&cast, Some(EqClass::Wizard), 12, 42);
        assert_eq!(display.label, "Gate");
        assert_eq!(display.remaining_secs, Some(0.75));
        assert_eq!(display.status_text.as_deref(), Some("item 9999"));
        assert!(!display.exact);
    }
}
