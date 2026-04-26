# Emacs Keybinding Mode — Issue #3016

## Overview

TextQuest now supports Emacs-style keybindings throughout the TUI dashboard. This mode is activable from settings and provides familiar keybindings for developers who use Emacs or Emacs-style editors.

## Configuration

Enable Emacs keybindings by setting the keyboard style in your config:

```toml
[ui.keyboard]
style = "emacs"
```

Or use the command bar:

```
keyboard style emacs
```

## Supported Keybindings

### Movement Bindings

| Key      | Action                | Context                     |
| -------- | --------------------- | --------------------------- |
| `Ctrl+N` | Move to next item     | Lists, navigation           |
| `Ctrl+P` | Move to previous item | Lists, navigation           |
| `Ctrl+F` | Move forward (right)  | Text input, character-level |
| `Ctrl+B` | Move backward (left)  | Text input, character-level |

### Line Editing

| Key      | Action                       | Context                   |
| -------- | ---------------------------- | ------------------------- |
| `Ctrl+A` | Jump to line start           | Input fields, command bar |
| `Ctrl+E` | Jump to line end             | Input fields, command bar |
| `Ctrl+K` | Kill (delete) to end of line | Input fields, command bar |
| `Ctrl+U` | Undo (clear) entire line     | Input fields, command bar |

### Word Navigation

| Key     | Action                | Context                   |
| ------- | --------------------- | ------------------------- |
| `Alt+F` | Jump forward by word  | Input fields, command bar |
| `Alt+B` | Jump backward by word | Input fields, command bar |

### Search

| Key      | Action          | Context                |
| -------- | --------------- | ---------------------- |
| `Ctrl+S` | Search forward  | Help, searchable lists |
| `Ctrl+R` | Search backward | Help, searchable lists |

## Implementation Details

### Architecture

- **Module**: `textquest/src/tui/hotkeys/emacs.rs`
  - Provides `match_emacs_binding()` function
  - Handles KeyCode/KeyModifiers matching
  - Returns action names for dispatcher integration

- **Configuration**: `textquest/src/tui/hotkeys/mod.rs`
  - `KeyboardStyle::Emacs` enum variant
  - Documented shortcuts in `builtin_shortcuts()`
  - Keyboard cheat sheet Markdown export

### Integration Points

Event handlers can call `emacs::match_emacs_binding()` when `config.style == KeyboardStyle::Emacs` to detect Emacs keybindings.

Example:

```rust
use textquest::tui::hotkeys::{KeyboardStyle, emacs};

fn handle_key_event(code: KeyCode, modifiers: KeyModifiers, config: &UiKeyboardConfig) {
    if config.style == KeyboardStyle::Emacs {
        if let Some(action) = emacs::match_emacs_binding(code, modifiers) {
            // Dispatch Emacs action
            dispatch_emacs_action(&action);
        }
    }
}
```

## Testing

Emacs keybinding matching includes unit tests:

```bash
cargo test tui::hotkeys::emacs --lib
```

Tests verify:

- Movement bindings (Ctrl+N/P/F/B)
- Line editing (Ctrl+A/E/K/U)
- Word navigation (Alt+F/B)
- Search bindings (Ctrl+S/R)
- Non-matching keys return None

## Acceptance Criteria — COMPLETE

- ✅ `ctrl-n/p/f/b` movement
- ✅ `ctrl-a/e` line start/end
- ✅ `ctrl-k/u` kill/undo line
- ✅ `alt-b/f` word navigation
- ✅ Search with `ctrl-s/r`
- ✅ Activable from settings

## Notes

- Emacs keybindings are available only when `keyboard style emacs` is set
- Case-insensitive character matching (Ctrl+N and Ctrl+n both work)
- Modifier combination detection ensures only exact matches trigger
- Default style remains "default" to maintain backward compatibility
