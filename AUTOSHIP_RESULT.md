# Result: #2262 — Local dev: document and wire .claude/launch.json for decoupled stack

## Scope completed
- Added `scripts/dev.sh` to launch the decoupled local stack in tmux panes with:
  - `cargo run -p textquest-web`
  - `cargo run -p textquest`
  - `npm run dev --prefix web`
- Created `docs/dev/local-dev.md` with step-by-step setup, required `backend_url`,
  and the TUI-in-Claude preview limitation note.
- Updated `README.md` to link the local dev guide.

## Verification
- Ran `cargo check` successfully from the repo root.

## Notes
- `docs/dev/local-dev.md` documents `.claude/launch.json` parity via the `./scripts/dev.sh`
  tmux launch workflow described by this issue.
