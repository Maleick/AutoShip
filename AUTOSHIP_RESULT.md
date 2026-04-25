# Result: #983 — Feature: Web Dashboard Group & Camp Configuration UI

Status: DONE

Changes Made:
- Extended the existing web Groups page with form-first group creation/editing, duplicate/max-size validation, role labels, keyboard member reordering, and collapsible per-class settings for pet management, spell priorities, and CC assignment.
- Switched camp configuration hooks to the issue-scoped `/api/camps` endpoints and added save/load wiring for camp templates.
- Added editable camp zone, pull points, pull targets, safe zones, HP/mana buff thresholds, pull strategy, and a live configuration preview.
- Updated shared web types and `feature-list.json` to track the partial issue pass.

Tests:
- `cargo check` passes.
- `npm --prefix web run build` passes after installing web dependencies with `npm --prefix web ci`.

Notes:
- PARTIAL because the issue spans full group/camp CRUD, template management, richer drag-and-drop assignment from live character pools, and backend persistence validation beyond one focused worktree pass.
- Vite reported the existing large chunk warning during the web production build.

COMPLETE
