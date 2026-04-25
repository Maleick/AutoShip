# Session Replay Player

The replay player now has three synchronized views over the same recorded session:

- `Tables`: facet summaries for DPS, healing, casts, buffs, debuffs, deaths, pulls, and resources.
- `Timeline`: PC swimlanes, engaged-NPC activity, operator key input, and orchestrator decisions.
- `Events`: a virtualized raw-log table with search and column filters.

## Shared Cursor

Hovering a timeline marker or event row previews the same timestamp across all views. Clicking pins the cursor so the selection stays synchronized while switching tabs or playback speed.

## Controls

- `J` and `L` jump backward or forward by 2 seconds, with a repeat tap stepping further.
- `K` toggles play and pause.
- `[` and `]` frame-step the cursor.
- `,` and `.` jump by 2 seconds.
- `<` and `>` change playback speed across `0.25x`, `0.5x`, `1x`, `2x`, and `4x`.
- `B` toggles a bookmark at the current cursor position.
- `/` focuses event search.

## Auto-Highlights

The side rail surfaces detected replay moments for deaths, wipes, mez breaks, named kills, full-clears, close calls, long mez chains, and operator bookmarks. The current implementation evaluates the labeled fixture in `replay-data.ts` so the UI can surface representative examples immediately.

## Storage Seek Model

The frontend mirrors the intended seek flow:

1. Resolve `meta.json` and the companion dictionary.
2. Jump to the nearest keyframe at or before the requested timestamp.
3. Replay deltas forward to the cursor.
4. Render the resulting snapshot.

The implementation is currently fixture-backed in the web app, but the seek path and cursor model are structured to match the eventual recorded-session storage pipeline.
