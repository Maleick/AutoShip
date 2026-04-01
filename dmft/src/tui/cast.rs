use crate::eq::structs::EqClass;

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
        Self {
            label: label.into(),
            short_label: short_label.into(),
            progress: progress.clamp(0.0, 1.0),
            remaining_secs: None,
            elapsed_secs: None,
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
        format!("{role} Gem {gem}"),
        format!("{role} G{gem}"),
        phase,
        Some(format!("gem {gem}")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
