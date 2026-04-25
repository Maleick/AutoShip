# Result: #2269 — Offload navmesh download from TUI render thread to background worker
Implemented asynchronous navmesh overlay loading using a per-app background channel, polling completion each loop, and added a map header loading badge while in-flight.

- `textquest/src/tui/app.rs`: added background navmesh loader state, non-blocking spawn path, and completion polling/apply logic.
- `textquest/src/tui/run.rs`: poll background navmesh load each render tick before drawing.
- `textquest/src/tui/ui/map.rs`: show `Loading navmesh...` status in the map panel header while load is pending.
- `cargo check` succeeded.
- No other source files modified.
