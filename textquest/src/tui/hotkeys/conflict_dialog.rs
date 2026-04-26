//! Interactive hotkey conflict dialog overlay
//!
//! Provides a modal dialog to handle user hotkey binding conflicts.
//! When a user attempts to bind a key that conflicts with an existing binding,
//! this module displays the conflict and allows the user to override or cancel.

use super::{HotkeyRegistry, KeyBinding};
use anyhow::Result;

/// A conflict detection result from a user's hotkey binding attempt
#[derive(Debug, Clone)]
pub struct ConflictNotice {
    /// The key binding the user is trying to set
    pub new_binding: KeyBinding,
    /// Description of the action the user wants to assign
    pub new_action_desc: String,
    /// The existing binding that conflicts
    pub existing_binding: KeyBinding,
    /// Description of the existing action
    pub existing_action_desc: String,
    /// Whether the conflicting hotkey is built-in (cannot be overridden)
    pub is_builtin: bool,
}

impl ConflictNotice {
    /// Format a user-friendly conflict message
    pub fn format_message(&self) -> String {
        let binding_str = self.new_binding.to_string_pretty();
        let existing_str = self.existing_binding.to_string_pretty();

        if self.is_builtin {
            format!(
                "Cannot override built-in hotkey:\n\n\
                {} → {}\n\
                is reserved for: {}\n\n\
                Choose a different key binding.",
                binding_str, self.new_action_desc, self.existing_action_desc
            )
        } else {
            format!(
                "Hotkey conflict detected:\n\n\
                {} is already bound to: {}\n\
                You want to bind it to: {}\n\n\
                Override existing binding?",
                binding_str, self.existing_action_desc, self.new_action_desc
            )
        }
    }
}

/// Dialog state for hotkey binding conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictDialogState {
    /// Dialog is not shown
    Hidden,
    /// Dialog is visible, waiting for user choice
    Waiting,
    /// User chose to override
    Confirmed,
    /// User chose to cancel
    Cancelled,
}

/// Interactive conflict resolution handler
pub struct ConflictDialogHandler {
    /// Current state of the dialog
    pub state: ConflictDialogState,
    /// The conflict being displayed (if any)
    pub conflict: Option<ConflictNotice>,
}

impl ConflictDialogHandler {
    /// Create a new conflict dialog handler
    pub fn new() -> Self {
        Self {
            state: ConflictDialogState::Hidden,
            conflict: None,
        }
    }

    /// Show a conflict dialog for the user
    pub fn show_conflict(&mut self, conflict: ConflictNotice) {
        self.conflict = Some(conflict);
        self.state = ConflictDialogState::Waiting;
    }

    /// Hide the dialog
    pub fn hide(&mut self) {
        self.state = ConflictDialogState::Hidden;
        self.conflict = None;
    }

    /// User confirmed the override
    pub fn confirm_override(&mut self) -> Option<ConflictNotice> {
        if self.state == ConflictDialogState::Waiting {
            self.state = ConflictDialogState::Confirmed;
            self.conflict.take()
        } else {
            None
        }
    }

    /// User cancelled the binding
    pub fn cancel_binding(&mut self) {
        if self.state == ConflictDialogState::Waiting {
            self.state = ConflictDialogState::Cancelled;
            self.conflict = None;
        }
    }

    /// Reset the dialog for the next interaction
    pub fn reset(&mut self) {
        self.state = ConflictDialogState::Hidden;
        self.conflict = None;
    }

    /// Check if a conflict exists and return the notice if so
    pub fn check_binding_conflict(
        registry: &HotkeyRegistry,
        binding: &KeyBinding,
        action_desc: impl Into<String>,
        is_builtin: bool,
    ) -> Option<ConflictNotice> {
        if let Some(conflict_desc) = registry.find_conflict(binding) {
            // Parse the conflict description to extract existing binding details
            // Format: "Global hotkey conflict: 'Ctrl+A'" or "Character hotkey conflict for 'char': 'Ctrl+B'"
            let parts: Vec<&str> = conflict_desc.split('\'').collect();
            let existing_binding_str = if parts.len() >= 2 {
                parts[1].to_string()
            } else {
                "Unknown".to_string()
            };

            Some(ConflictNotice {
                new_binding: binding.clone(),
                new_action_desc: action_desc.into(),
                existing_binding: KeyBinding::simple(&existing_binding_str),
                existing_action_desc: "Existing binding".to_string(),
                is_builtin,
            })
        } else {
            None
        }
    }
}

impl Default for ConflictDialogHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_notice_formats_builtin_message() {
        let notice = ConflictNotice {
            new_binding: KeyBinding::simple("F1"),
            new_action_desc: "My Custom Action".to_string(),
            existing_binding: KeyBinding::simple("F1"),
            existing_action_desc: "Open Help".to_string(),
            is_builtin: true,
        };

        let msg = notice.format_message();
        assert!(msg.contains("Cannot override built-in"));
        assert!(msg.contains("F1"));
        assert!(msg.contains("Open Help"));
    }

    #[test]
    fn conflict_notice_formats_override_message() {
        let notice = ConflictNotice {
            new_binding: KeyBinding::new("a", true, false, false),
            new_action_desc: "New Action".to_string(),
            existing_binding: KeyBinding::new("a", true, false, false),
            existing_action_desc: "Old Action".to_string(),
            is_builtin: false,
        };

        let msg = notice.format_message();
        assert!(msg.contains("Hotkey conflict detected"));
        assert!(msg.contains("already bound to"));
        assert!(msg.contains("Override existing binding"));
    }

    #[test]
    fn dialog_handler_state_transitions() {
        let mut handler = ConflictDialogHandler::new();
        assert_eq!(handler.state, ConflictDialogState::Hidden);

        let conflict = ConflictNotice {
            new_binding: KeyBinding::simple("x"),
            new_action_desc: "Test".to_string(),
            existing_binding: KeyBinding::simple("x"),
            existing_action_desc: "Old".to_string(),
            is_builtin: false,
        };

        handler.show_conflict(conflict.clone());
        assert_eq!(handler.state, ConflictDialogState::Waiting);
        assert!(handler.conflict.is_some());

        let confirmed = handler.confirm_override();
        assert!(confirmed.is_some());
        assert_eq!(handler.state, ConflictDialogState::Confirmed);
    }

    #[test]
    fn dialog_handler_cancel() {
        let mut handler = ConflictDialogHandler::new();

        let conflict = ConflictNotice {
            new_binding: KeyBinding::simple("y"),
            new_action_desc: "Test".to_string(),
            existing_binding: KeyBinding::simple("y"),
            existing_action_desc: "Old".to_string(),
            is_builtin: false,
        };

        handler.show_conflict(conflict);
        assert_eq!(handler.state, ConflictDialogState::Waiting);

        handler.cancel_binding();
        assert_eq!(handler.state, ConflictDialogState::Cancelled);
        assert!(handler.conflict.is_none());
    }
}
