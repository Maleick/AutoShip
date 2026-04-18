# ADR-001: Logout Sequencer Design

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest orchestrates up to 36 EverQuest client accounts across 6 groups. When an account logs out, multiple subsystems must be notified in the correct order:

1. Combat rotations must stop immediately
2. Zone transitions must be cancelled
3. IPC listeners must be torn down
4. Character state must be persisted
5. Account resources (socket connections, DLL state) must be cleaned up

Without careful coordination, logout events can race with:
- Pending ability casts
- Zone transition state machines
- Vendor transactions mid-flight
- Loot window interactions

The challenge is ensuring logout happens atomically from the orchestrator's perspective while allowing subsystems to gracefully finalize their work.

## Decision

Implement a **logout state machine** in `textquest-common/src/types.rs` and handled by `textquest/src/login/sequencer.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutState {
    /// Character is online; automation running normally.
    Online,
    /// Logout has been requested; subsystems are being notified.
    LogoutRequested,
    /// Combat and activities stopped; waiting for zone transitions to finalize.
    WaitingForZoneTransitions,
    /// Zone transitions complete; tearing down IPC.
    TearingDownIpc,
    /// All subsystems shut down; safe to remove character from state.
    Offline,
}
```

The sequencer owns these transitions and orchestrates notifications via typed events:

```rust
pub struct LogoutSequencer {
    state: LogoutState,
    // ... character/group context
}

impl LogoutSequencer {
    /// Initiate logout for a character.
    pub async fn request_logout(&mut self, char_name: &str) -> Result<()> {
        self.state = LogoutState::LogoutRequested;
        // notify combat engine to stop
        // broadcast via IPC
        // wait for acks or timeout
    }

    /// Finalize logout after all subsystems acknowledge.
    pub async fn finalize(&mut self) -> Result<()> {
        self.state = LogoutState::TearingDownIpc;
        // close sockets, cleanup DLL state, persist character data
        self.state = LogoutState::Offline;
    }
}
```

## Rationale

1. **Explicit State** — Prevents ambiguous transitional states. Each logout stage is visible to the orchestrator and testable.

2. **Ordered Notifications** — Subsystems receive logout events in dependency order:
   - Combat stops first (doesn't care about zones)
   - Zone transitions finalize (may need combat to already be stopped)
   - IPC tears down last (depends on zone transitions being safe)

3. **Timeout Safety** — If a subsystem hangs, the sequencer can force-close after a deadline rather than deadlocking the entire fleet.

4. **Testability** — State transitions are deterministic and can be unit-tested without spawning actual EQ clients.

5. **Observable** — Logs and debug output can trace the sequencer's path through states, aiding post-mortem analysis.

## Implementation Notes

- State transitions are **only forward** (Online → LogoutRequested → ... → Offline). No transitions backward.
- Timeout values are configurable per group (e.g., 5 seconds before force-close).
- Per-account sequencers share a group-level broadcast channel so other group members are notified of logout (e.g., for re-formation).

## Alternatives Considered

1. **Ad-hoc callbacks** — Each subsystem registers a shutdown callback. Risk: forgotten dependencies, fragile ordering.
2. **Global flag + polling** — Set a `logged_out` flag; each subsystem polls and self-shuts down. Risk: race conditions, slow detection.
3. **Transaction-style rollback** — Treat logout as a two-phase commit (prepare + commit). Overkill for logout use case; better for login sequencing.

## Related ADRs

- ADR-005: Graceful Shutdown on Ctrl+C (uses logout sequencer for orderly fleet shutdown)

## References

- `textquest-common/src/types.rs` — LogoutState enum definition
- `textquest/src/login/sequencer.rs` — LogoutSequencer implementation (if exists; check actual paths)
- `orchestration-design.md` — Group lifecycle and group state machine
