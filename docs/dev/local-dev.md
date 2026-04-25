# Local Development (Decoupled Stack)

Use this flow when running the web backend and frontend separately from the
TUI-oriented orchestrator.

## Required service commands

- Backend: `cargo run -p textquest-web` (serves API + WebSocket on port `3001`)
- Frontend: `npm run dev --prefix web` (Vite SPA on port `5173`)
- Orchestrator/TUI: `cargo run -p textquest` (includes DLL control and web socket
  client wiring)

## Precondition: DLL websocket target

The decoupled stack expects the injected DLL/client connection target to remain
`ws://localhost:3001`.

Verify this in `config/textquest.toml`:

```toml
backend_url = "ws://localhost:3001"
```

## Quick start

1. From the repo root, run:

   ```bash
   ./scripts/dev.sh
   ```

2. The script creates a tmux session named `textquest-dev` with three panes:

   - pane 1: `cargo run -p textquest-web`
   - pane 2: `cargo run -p textquest`
   - pane 3: `npm run dev --prefix web`

3. Open your browser to `http://127.0.0.1:5173`.

4. Optionally validate services:

   ```bash
   curl -sSf http://127.0.0.1:3001/health
   ```

## TUI-in-Claude limitation

Claude TUI does not provide an inline HTTP preview for the SPA.
Use tmux window management + log tailing:

- open an extra tmux split in a local terminal for the browser/UI checks
- or tail logs in a split:

```bash
tail -n 120 -F logs/textquest.log*
```

## If you prefer a one-command check

- The `./scripts/dev.sh` script is the local-equivalent of the expected
  `.claude/launch.json` workflow for this issue (`tmux` with all three services).
- Re-run it anytime, and close the session with `tmux kill-session -t textquest-dev`.

