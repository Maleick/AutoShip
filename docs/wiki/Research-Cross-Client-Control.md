# M8 Cross-Client Control Model

This document formalises the routing scope and session lifecycle model for TextQuest's M8 Orchestrator milestone.  It maps the model to TUI workflows so that the exit gate ("cross-client control model is documented and mapped to TUI workflows") is satisfied.

## Sources

- [docs/external-research/jmb-session-and-relay-comparison.md](jmb-session-and-relay-comparison.md)
- [docs/external-research/kissassist-gap-and-tui-translation.md](kissassist-gap-and-tui-translation.md)
- `textquest-common/src/routing.rs` — `RoutingScope` enum
- `textquest/src/client/session.rs` — `SlotLifecycle` enum

## Routing Scope

The routing scope controls which clients receive dispatched commands.  All command routing goes through one of three scopes.

### `AllSession`

- **What it targets**: every connected client.
- **Code**: `RoutingScope::AllSession`
- **TUI default**: active at startup.
- **How to set**: `:scope all`, or `:scope` after previously narrowing.
- **Visual indicator**: scope label shows `All` in the command bar / status line.
- **Operator expectations**: the full-session broadcast is visually distinct.  Use it for sit, stop, pause, reconnect, or session-wide launch control.

### `Group { group_id, label }`

- **What it targets**: all members of one named operator group (up to six EQ slots).
- **Code**: `RoutingScope::Group { group_id: u8, label: String }`
- **How to set**: `:scope G1`..`:scope G6` or the existing `:G1 /cmd` shorthand.
- **Visual indicator**: scope label shows `G{n} {label}`.
- **Operator expectations**: partial failures are reported per client.  The active group is visible before dispatch.

### `OneToon { name }`

- **What it targets**: exactly one attached client by character name.
- **Code**: `RoutingScope::OneToon { name: String }`
- **How to set**: `:scope <CharacterName>` or the existing `:<CharacterName> /cmd` prefix.
- **Visual indicator**: scope label shows `@{name}`.
- **Operator expectations**: the target toon is named explicitly.  Failures are reported for that toon only.

## Scope Commands

| Command | Effect |
|---|---|
| `:scope` | Show current routing scope and count of in-scope clients. |
| `:scope all` | Reset to `AllSession`. |
| `:scope G1`..`:scope G6` | Narrow to a group. |
| `:scope <name>` | Narrow to one toon. |

## Session Slot Lifecycle

Each managed client slot progresses through a defined lifecycle.  The `SlotLifecycle` enum in `textquest/src/client/session.rs` tracks this state.

| State | Meaning |
|---|---|
| `Configured` | Slot has account/character config but no running process. |
| `Launching` | EQ process is being spawned by the launcher. |
| `WaitingForLogin` | Client is running and showing the login screen. |
| `EnteringWorld` | Character has passed login and is loading into the zone. |
| `Live` | Fully operational: hooks active, in-zone, accepting commands. |
| `Recovering` | Recovering from crash, disconnect, or unexpected state. |
| `Blocked` | Blocked — operator attention needed before the slot can proceed. |

### Operator visibility

`:session` or `:session status` — compact summary of connected slots and current routing scope.

`:session list` — per-slot table of character name, zone, and connection status.

## TUI Workflow Mapping

| Operator goal | TUI surface |
|---|---|
| See current routing scope | Status bar (scope label) |
| Broaden scope to all | `:scope all` |
| Narrow scope to one group | `:scope G1`..`:scope G6` |
| Narrow scope to one toon | `:scope <name>` or `:<name> /cmd` |
| Send a command to focused scope | Any command that calls `send_ipc_to_focused` (combat, nav, etc.) |
| Broadcast a slash command to every client | `:all /<slash command>` |
| Inspect session slot health | `:session list` |
| Route a group command | `:G1 /assist <name>` (group shorthand) |
| Route a single-toon command | `:Tank /stand` (character prefix) |

## Scope and Group Focus Relationship

The `routing_scope` field on `App` is the authoritative routing abstraction.  The existing `active_group: Option<usize>` field continues to drive the **TUI display filter** (which clients appear in the overview and tactical panels).  The two fields are kept in sync:

- `:scope G1` sets both `routing_scope` to `Group { group_id: 1, … }` and `active_group` to the matching index.
- `:scope all` sets `routing_scope` to `AllSession` and clears `active_group`.
- `:scope <name>` sets `routing_scope` to `OneToon` and clears `active_group`.

## Risk Boundary

From the JMB comparison, these constraints carry forward:

- routing changes stay on the authenticated IPC and TUI control boundary
- no hidden input-hook expansion
- focus, relay, and preset work is operator-visible and reversible
- risky movement, zoning, or exploit-style travel remains separate milestone work

## Evidence State

All items in this document are **Research-backed** (see jmb-session-and-relay-comparison.md and kissassist-gap-and-tui-translation.md).  No items are marked Live-validated until a live EQ client integration test is run.

## Implemented Slices

| Slice | Status | Location |
|---|---|---|
| JMB coordination comparison | Done | `jmb-session-and-relay-comparison.md` |
| KissAssist TUI translation | Done | `kissassist-gap-and-tui-translation.md` |
| `RoutingScope` type | Done | `textquest-common/src/routing.rs` |
| `SlotLifecycle` enum | Done | `textquest/src/client/session.rs` |
| `:scope` command | Done | `textquest/src/tui/app.rs` |
| `:session` command | Done | `textquest/src/tui/app.rs` |
| Cross-client control model doc | Done | this file |
| Slot lifecycle badges in TUI overview | Done | `textquest/src/tui/ui/dashboard.rs` |
| Launch profiles and session presets | Done | `textquest/src/tui/client.rs`, `dashboard.rs`, `app.rs` |
| `focused_pids()` honours `OneToon` scope | Done | `textquest/src/tui/app.rs` |
| `routing_scope` + `scope_pids` in Orchestrator | Done | `textquest/src/orchestrator.rs` |
| Camp-loop dispatch filtered by routing scope | Done | `textquest/src/orchestrator.rs` |
| Scope synced App → Orchestrator each tick | Done | `textquest/src/tui/run.rs` |

## Follow-on Slices

M8 initial slices are complete.  Research-backed candidates for future M8 or M9 work:

- Surface `routing_scope` label in the TUI status bar so the operator always sees the active dispatch target.
- Add `:scope` tab-completion for connected character names.
- Wire `scope_pids` into the Orchestrator's sell-cycle and vendor dispatches for full scope coverage.
- Evaluate whether `SlotLifecycle` state should gate camp-loop dispatch (e.g., skip `Recovering` or `Blocked` slots).
