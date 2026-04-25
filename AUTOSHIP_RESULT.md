# Result: #994 — Feature: TUI Theme System & Customization

Status: DONE

Changes Made:
- Added Light, High Contrast, and Minimal built-in TUI themes alongside the existing Dark, Neriak, Dracula, and Classic themes.
- Added base foreground/background colors and WCAG-style body contrast helpers to the TUI theme model.
- Added Vim-style named runtime switching through `:theme <name>` while preserving `:theme` cycling and persistent selection.
- Added persistent theme preference scaffolding with per-character override storage and lookup.
- Kept existing TOML custom theme loading compatible with optional foreground/background fields.
- Updated map cache theme identifiers for the expanded theme set.
- Updated `feature-list.json` to track the issue as partial.

Tests:
- `cargo check` passed after production wiring.
- `cargo check --tests` passed after adding focused theme tests.
- Focused theme tests were added for built-in cycling, name parsing, accessibility contrast, and per-character preferences.

Notes:
- Full custom theme import/export UI, save-custom-theme workflows, live preview panels, renderer-level per-character theme application, and auto dark/light scheduling remain follow-up work.

COMPLETE
