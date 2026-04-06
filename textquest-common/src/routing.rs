//! Routing scope types for cross-client command dispatch.
//!
//! `RoutingScope` formalizes the three operator-addressable dispatch targets:
//! one specific toon, a named group, or every attached session.  This is the
//! M8 actor-routing abstraction derived from the JMB session-and-relay
//! comparison and the KissAssist TUI-translation research.

use std::fmt;

/// The operator-selected target for a dispatched command.
///
/// All command routing in the TUI and orchestrator is routed through one of
/// these three scopes.  The TUI exposes the current scope in the status bar
/// and allows it to be changed via `:scope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingScope {
    /// Target exactly one attached client by character name.
    ///
    /// Use for per-toon overrides, recovery actions, or single-character
    /// commands.  Failures are reported for that toon individually.
    OneToon {
        /// In-game character name of the target toon.
        name: String,
    },

    /// Target all members of a named operator group (up to six slots).
    ///
    /// Use for combat-mode changes, regroup commands, or behavior overrides
    /// that should stay bounded to one party.  Partial failures are reported
    /// per client rather than hidden behind a single summary.
    Group {
        /// Numeric group identifier (1-based, matching G1..G6 labels).
        group_id: u8,
        /// Human-readable group label (e.g. "Alpha").
        label: String,
    },

    /// Target every client in the active session set.
    ///
    /// Use sparingly for deliberate operator-wide actions: sit, stop, pause,
    /// reconnect, or session-wide launch control.  Broadcast routing is
    /// visually distinct in the TUI status bar.
    AllSession,
}

impl RoutingScope {
    /// Returns a compact human-readable label for the current scope.
    ///
    /// Shown in the TUI status bar and feedback messages.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::OneToon { name } => format!("@{name}"),
            Self::Group { group_id, label } => format!("G{group_id} {label}"),
            Self::AllSession => String::from("All"),
        }
    }

    /// Returns the short scope abbreviation used in command bar hints.
    #[must_use]
    pub fn short_label(&self) -> &str {
        match self {
            Self::OneToon { .. } => "toon",
            Self::Group { .. } => "group",
            Self::AllSession => "all",
        }
    }

    /// Returns `true` when the scope targets every session.
    #[must_use]
    pub fn is_all_session(&self) -> bool {
        matches!(self, Self::AllSession)
    }

    /// Returns `true` when the scope is narrowed to one toon or one group.
    #[must_use]
    pub fn is_narrowed(&self) -> bool {
        !self.is_all_session()
    }
}

impl Default for RoutingScope {
    /// The default scope is `AllSession` — commands reach every connected
    /// client unless the operator narrows the focus.
    fn default() -> Self {
        Self::AllSession
    }
}

impl fmt::Display for RoutingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_session() {
        assert_eq!(RoutingScope::default(), RoutingScope::AllSession);
    }

    #[test]
    fn label_all_session() {
        assert_eq!(RoutingScope::AllSession.label(), "All");
    }

    #[test]
    fn label_one_toon() {
        let s = RoutingScope::OneToon {
            name: "Warrior".to_string(),
        };
        assert_eq!(s.label(), "@Warrior");
    }

    #[test]
    fn label_group() {
        let s = RoutingScope::Group {
            group_id: 2,
            label: "Beta".to_string(),
        };
        assert_eq!(s.label(), "G2 Beta");
    }

    #[test]
    fn short_label_variants() {
        assert_eq!(RoutingScope::AllSession.short_label(), "all");
        assert_eq!(
            RoutingScope::OneToon {
                name: "x".to_string()
            }
            .short_label(),
            "toon"
        );
        assert_eq!(
            RoutingScope::Group {
                group_id: 1,
                label: "x".to_string()
            }
            .short_label(),
            "group"
        );
    }

    #[test]
    fn is_all_session_and_is_narrowed() {
        assert!(RoutingScope::AllSession.is_all_session());
        assert!(!RoutingScope::AllSession.is_narrowed());

        let toon = RoutingScope::OneToon {
            name: "Bob".to_string(),
        };
        assert!(!toon.is_all_session());
        assert!(toon.is_narrowed());
    }

    #[test]
    fn display_matches_label() {
        let s = RoutingScope::OneToon {
            name: "Cleric".to_string(),
        };
        assert_eq!(format!("{s}"), s.label());
    }

    #[test]
    fn clone_and_eq() {
        let a = RoutingScope::Group {
            group_id: 3,
            label: "Gamma".to_string(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
