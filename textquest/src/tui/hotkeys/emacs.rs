//! Emacs-style keybinding handlers for TUI list navigation and text editing.
//!
//! Supports:
//! - Movement: Ctrl+N/P (next/previous), Ctrl+F/B (forward/backward)
//! - Line editing: Ctrl+A/E (start/end), Ctrl+K (kill), Ctrl+U (undo)
//! - Word navigation: Alt+F/B (forward/backward by word)
//! - Search: Ctrl+S/R (search forward/backward)

use crossterm::event::{KeyCode, KeyModifiers};

/// Check if a key press matches an Emacs keybinding.
///
/// Returns the action name if matched, or None if not an Emacs binding.
pub fn match_emacs_binding(code: KeyCode, modifiers: KeyModifiers) -> Option<String> {
    match code {
        KeyCode::Char(c) => match c {
            // Movement bindings
            'n' | 'N' if modifiers.contains(KeyModifiers::CONTROL) => Some("next".to_string()),
            'p' | 'P' if modifiers.contains(KeyModifiers::CONTROL) => Some("previous".to_string()),
            'f' | 'F' if modifiers.contains(KeyModifiers::CONTROL) => Some("forward".to_string()),
            'b' | 'B' if modifiers.contains(KeyModifiers::CONTROL) => Some("backward".to_string()),

            // Line editing bindings
            'a' | 'A' if modifiers.contains(KeyModifiers::CONTROL) => Some("line_start".to_string()),
            'e' | 'E' if modifiers.contains(KeyModifiers::CONTROL) => Some("line_end".to_string()),
            'k' | 'K' if modifiers.contains(KeyModifiers::CONTROL) => Some("kill_line".to_string()),
            'u' | 'U' if modifiers.contains(KeyModifiers::CONTROL) => Some("undo_line".to_string()),

            // Word navigation (Alt modifier)
            'f' | 'F' if modifiers.contains(KeyModifiers::ALT) => Some("word_forward".to_string()),
            'b' | 'B' if modifiers.contains(KeyModifiers::ALT) => Some("word_backward".to_string()),

            // Search bindings
            's' | 'S' if modifiers.contains(KeyModifiers::CONTROL) => Some("search_forward".to_string()),
            'r' | 'R' if modifiers.contains(KeyModifiers::CONTROL) => Some("search_backward".to_string()),

            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_bindings() {
        assert_eq!(
            match_emacs_binding(KeyCode::Char('n'), KeyModifiers::CONTROL),
            Some("next".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('p'), KeyModifiers::CONTROL),
            Some("previous".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('f'), KeyModifiers::CONTROL),
            Some("forward".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('b'), KeyModifiers::CONTROL),
            Some("backward".to_string())
        );
    }

    #[test]
    fn test_line_editing_bindings() {
        assert_eq!(
            match_emacs_binding(KeyCode::Char('a'), KeyModifiers::CONTROL),
            Some("line_start".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('e'), KeyModifiers::CONTROL),
            Some("line_end".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('k'), KeyModifiers::CONTROL),
            Some("kill_line".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Some("undo_line".to_string())
        );
    }

    #[test]
    fn test_word_navigation_bindings() {
        assert_eq!(
            match_emacs_binding(KeyCode::Char('f'), KeyModifiers::ALT),
            Some("word_forward".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('b'), KeyModifiers::ALT),
            Some("word_backward".to_string())
        );
    }

    #[test]
    fn test_search_bindings() {
        assert_eq!(
            match_emacs_binding(KeyCode::Char('s'), KeyModifiers::CONTROL),
            Some("search_forward".to_string())
        );
        assert_eq!(
            match_emacs_binding(KeyCode::Char('r'), KeyModifiers::CONTROL),
            Some("search_backward".to_string())
        );
    }

    #[test]
    fn test_non_emacs_keys() {
        assert_eq!(match_emacs_binding(KeyCode::Char('x'), KeyModifiers::CONTROL), None);
        assert_eq!(match_emacs_binding(KeyCode::Enter, KeyModifiers::NONE), None);
    }
}
