# ADR-005: Graceful Shutdown on Ctrl+C

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest's orchestrator runs in a foreground process, continuously managing 6 groups of EQ clients. When an operator presses Ctrl+C (SIGINT), the orchestrator must:

1. Stop accepting new commands
2. Stop initiating new actions (pulls, spells, vendoring)
3. Wait for in-flight actions to complete (e.g., ability cooldowns, vendor transactions)
4. Log out all accounts gracefully (not abruptly close sockets)
5. Persist session state (character positions, inventory, cooldowns)
6. Exit cleanly within a timeout (don't hang forever)

Naive shutdown (immediate process exit) causes:

- **In-flight spells interrupted** — Uncast buffs left at camp, mana wasted
- **Incomplete transactions** — Items halfway sold, gold lost
- **Group formation broken** — Leader didn't dismiss group members before logout
- **State inconsistency** — Cached character state differs from server state, next session is confused

## Decision

Implement **watch-channel-based graceful shutdown**:

1. **Signal handler** — System SIGINT handler is caught by `tokio::signal::ctrl_c()`.

2. **Shutdown channel** — A `tokio::sync::watch::channel(false)` propagates the shutdown signal to all subsystems:

```rust
let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

// Install signal handler
let shutdown_tx_clone = shutdown_tx.clone();
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    tracing::info!("Ctrl+C received; initiating graceful shutdown");
    let _ = shutdown_tx_clone.send(true);
});
```

3. **Subscriber pattern** — Each subsystem (orchestrator loop, TUI event loop, web API server) receives a copy of `shutdown_rx` and checks it in its select!():

```rust
pub async fn run_orchestrator_loop(
    mut app: App,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            // Normal operation branches
            _ = process_tick_events(&app) => { /* handle events */ },
            _ = app.ipc_client.wait_for_notification() => { /* handle spawn */ },

            // Shutdown branch
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("Shutdown signal received — stopping orchestrator loop");
                    perform_graceful_shutdown(&mut app).await?;
                    break;
                }
            }
        }
    }
    Ok(())
}
```

4. **Graceful shutdown routine** — When shutdown is signaled:

```rust
async fn perform_graceful_shutdown(app: &mut App) -> Result<()> {
    tracing::info!("Phase 1: Stopping automation for all groups");
    // Send "stop" command to all groups; wait for ack or timeout (5 seconds)

    tracing::info!("Phase 2: Logging out all accounts");
    // Use LogoutSequencer (ADR-001) to log out each account in order
    // Wait for logout to complete or timeout (10 seconds)

    tracing::info!("Phase 3: Closing IPC connections");
    // Close all IPC client sockets

    tracing::info!("Phase 4: Persisting session state");
    // Write final character positions, inventory, cooldowns to disk

    tracing::info!("Graceful shutdown complete");
    Ok(())
}
```

## Rationale

1. **Watch Channel** — Tokio's `watch` is efficient for broadcast signals. All subscribers see the change without polling.

2. **select!() Integration** — The orchestrator loop doesn't need special shutdown threads. It naturally incorporates shutdown checks via `select!()`.

3. **Ordered Phases** — Shutdown happens in four phases, each with its own timeout. Operators can see progress ("stopping automation → logging out → closing connections → done").

4. **Resource cleanup** — Each subsystem's shutdown branch handles its own cleanup (close file handles, flush logs, cleanup temp files).

5. **Testable** — Shutdown can be unit-tested by manually sending `true` on the watch channel and checking that the loop exits cleanly.

## Implementation Notes

- **Timeout values** — Each phase has a timeout (e.g., 5 sec for stopping, 10 sec for logout, 5 sec for IPC closure). If a phase exceeds its timeout, log a warning and move to the next phase.
- **Force shutdown** — If the operator presses Ctrl+C twice within 2 seconds, skip graceful shutdown and exit immediately.
- **TUI integration** — The TUI event loop also subscribes to `shutdown_rx` and closes curses cleanly before exiting.
- **Web API** — The Axum web server also subscribes and closes listener cleanly. In-flight HTTP requests finish or are timed out.

Example log output:

```
2026-04-18T14:30:00Z INFO: Ctrl+C received; initiating graceful shutdown
2026-04-18T14:30:00Z INFO: Phase 1: Stopping automation for all groups
2026-04-18T14:30:01Z INFO: Phase 1: All groups acknowledged stop
2026-04-18T14:30:01Z INFO: Phase 2: Logging out all accounts
2026-04-18T14:30:06Z INFO: Phase 2: All 36 accounts logged out
2026-04-18T14:30:06Z INFO: Phase 3: Closing IPC connections
2026-04-18T14:30:07Z INFO: Phase 3: All IPC connections closed
2026-04-18T14:30:07Z INFO: Phase 4: Persisting session state
2026-04-18T14:30:08Z INFO: Phase 4: Session state persisted
2026-04-18T14:30:08Z INFO: Graceful shutdown complete; exiting
```

## Alternatives Considered

1. **Global atomic bool flag** — Set a flag on signal, all threads poll it. Slower to detect than watch channel; requires explicit polling logic.
2. **Thread channels (mpsc)** — One sender, one receiver per thread. Works but doesn't broadcast; you'd need multiple senders or a custom broadcast layer.
3. **Unix signal handling (nix crate)** — Install a SIGINT handler that directly mutates app state. Unsafe; hard to test; less idiomatic in async Rust.

## Related ADRs

- ADR-001: Logout Sequencer Design (used in Phase 2 of graceful shutdown)
- ADR-003: JSON Event Streaming (shutdown events are logged as events)

## References

- `textquest/src/orchestrator_loop.rs` — Main loop with shutdown integration
- `textquest/src/tui/mod.rs` — TUI event loop with shutdown handling
- `textquest/src/main.rs` — Signal handler setup
