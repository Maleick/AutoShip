# Result: #2482 — web(config-ui): token drift — replace inline hex literals with Neriak CSS vars across pages+components

Status: PARTIAL

Changes Made:
- Replaced raw Neriak hex arbitrary Tailwind color utilities across the config UI shell, pages, and components with semantic theme classes such as `bg-panel`, `bg-void`, `border-neriak-dim`, `text-neriak-magenta`, `text-neriak-muted`, and state color classes.
- Converted dynamic SVG/style color values to CSS variable references and used `color-mix(...)` where the old code appended alpha hex suffixes.
- Kept `index.css` as the canonical theme-token definition surface.

Tests:
- `rg -n "#[0-9A-Fa-f]{3,8}" textquest-web/frontend/src --glob '!index.css'` produced no matches.
- `cargo check` passed.
- `npm --prefix textquest-web/frontend run build` could not start because this workspace has no installed frontend dependencies (`tsc: command not found`).

Notes:
- No full cargo test run, per issue instruction to use `cargo check` only.

COMPLETE
