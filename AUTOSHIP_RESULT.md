# Result: #997 — Feature: TUI Keyboard Shortcuts & Accessibility

Status: DONE

Changes Made:
- Added serializable TUI keyboard and accessibility preference structs for the requested `[ui.keyboard]` shape.
- Added built-in shortcut documentation and Markdown cheat sheet export support.
- Wired `:help keyboard`, `:help keyboard export [path]`, `:keyboard ...`, and `:accessibility ...` command handling.
- Added Shift-Tab reverse panel focus, text-first focus announcements, and Emacs `Ctrl+N` / `Ctrl+P` aliases.
- Added a Shortcuts tab to the searchable help overlay.
- Updated `feature-list.json` with issue #997 partial status and remaining scope.

Tests:
- `cargo check` passed.

Notes:
- PARTIAL because full config-file loading, renderer-wide text sizing, mouse click focus/select routing, copy-paste handling, and complete customizable shortcut routing are larger than one focused worktree pass.

COMPLETE
