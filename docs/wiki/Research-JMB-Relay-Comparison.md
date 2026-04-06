# JMB Session and Relay Comparison

This document is the second orchestration-focused external research pass for `M8` after the KissAssist workflow audit.

## Goal

Formalize TextQuest operator routing scopes and session concepts from the Joe Multiboxer ecosystem without treating JMB's runtime model as a feature-parity target.

## Source anchors

Primary and bounded comparison inputs:

- [Joe Multiboxer developer getting started](https://joemultiboxer.com/docs/developer-launchpad/)
- [JMB Basic Core](https://github.com/LavishSoftware/JMB-Basic-Core)
- [JMB WinEQ 2022](https://github.com/LavishSoftware/JMB-WinEQ-2022)
- [JMB Input Hook Example](https://github.com/LavishSoftware/JMB-Input-Hook-Example)
- [docs/wineq-research.md](../wineq-research.md)
- [docs/external-research/daybreak-detection-digest.md](daybreak-detection-digest.md)

## Research-backed observations

### Session identity is a first-class operator concept in JMB

The Joe Multiboxer developer guide uses separate `Session` and `Uplink` entry points in its example agent layout and describes launching multiple game instances from one operator surface. That implies a stable distinction between:

- per-slot or per-session execution
- an uplink or coordinator surface that can focus and route work
- operator-visible launch and agent lifecycle controls

Repo fit for TextQuest:

- TextQuest already has per-client sessions in [textquest/src/client/session.rs](../../textquest/src/client/session.rs)
- the TUI already exposes group focus and scope state in [textquest/src/tui/app.rs](../../textquest/src/tui/app.rs) and [textquest/src/tui/ui/dashboard.rs](../../textquest/src/tui/ui/dashboard.rs)
- launch and login coordination already exist, but they are not yet modeled as an operator-facing session shell

Evidence state: `Research-backed`

### Relay and focus are explicit, not hidden side effects

[docs/wineq-research.md](../wineq-research.md) shows JMB using an explicit `uplink focus` plus `relay` pattern for slot switching and per-session event delivery. The useful takeaway is not the exact hook stack. It is the operator model:

- focus one slot intentionally
- relay commands to one target or one named subset
- keep routing visible to the operator

Repo fit for TextQuest:

- TextQuest already supports one-character commands, focused-group routing, and `all` broadcast routing in the command bar
- combat summaries already expose current scope labels instead of burying routing in logs
- TextQuest does not yet define a stable abstraction for `one-toon`, `group`, and `all-session` routing beyond the current command grammar

Evidence state: `Research-backed`

### Window presets and launch presets should stay operator-facing

The Joe Multiboxer guide and WinEQ research both treat launch setup as a combination of reusable profiles plus per-slot window or focus behavior. TextQuest does not need the same runtime or virtual file model, but it does need the same operator clarity:

- which clients belong to a launch profile
- which groups or slots belong to a session preset
- which slots are launched, attached, logged in, recovering, or blocked

Repo fit for TextQuest:

- this maps cleanly onto the existing launcher and login coordinator
- it does not require new hook behavior or gameplay automation changes
- it should surface in the TUI as state and controls, not as hidden config-only behavior

Evidence state: `Research-backed`

### Input-hook examples are comparison input only

The JMB Input Hook Example is useful for understanding the shape of JMB's operator runtime, but TextQuest should not translate it into broader hidden input capture or control escalation. The anti-cheat digest keeps hook and module exposure as explicit `M7` categories.

Repo fit for TextQuest:

- keep command routing on TextQuest's authenticated IPC path
- prefer explicit operator actions and visible TUI scope over new background input hooks
- treat any future focus or hotkey support as an operator convenience slice that still stays inside the current trust boundary

Evidence state: `Provisional` for direct implementation guidance, `Research-backed` as a risk boundary

## TextQuest routing model

These routing scopes are the repo-fit translation from the JMB comparison.

### One-toon scope

Definition:

- target exactly one attached client or logical actor
- use when issuing a direct slash command, recovery action, or per-toon override

Operator expectations:

- the target toon is named explicitly
- the UI shows the selected toon and current routing scope
- failures are reported per toon rather than summarized as a group result

### Group scope

Definition:

- target the current six-slot group or another named operator grouping
- use for combat mode changes, regroup commands, or behavior overrides that should stay bounded to one party

Operator expectations:

- the active group is visible before dispatch
- partial failure stays visible by client, not hidden behind a single success banner
- the operator can promote or narrow the scope without rewriting commands

### All-session scope

Definition:

- target every client in the active session set
- use sparingly for deliberate operator-wide actions such as sit, stop, pause, reconnect, or session-wide launch control

Operator expectations:

- all-session routing is visually distinct from one-toon and group scope
- broadcast actions are explicit and reversible where possible
- operator confirmation or stronger visibility is warranted for disruptive actions

## Session-shell concepts worth translating

The JMB comparison suggests three operator-facing concepts that fit TextQuest well.

### Launch profile

A reusable description of which characters or accounts should be launched together and with which startup options.

Repo-fit note:

- this belongs in TextQuest's launcher and credential flow, not in the combat engine

### Session preset

A reusable control grouping that defines how launched clients are presented and addressed once attached.

Repo-fit note:

- this should control grouping and scope labels in the TUI, not invent a separate hidden runtime

### Slot lifecycle

A visible state machine for each slot:

- configured
- launching
- waiting for login
- entering world
- live
- recovering
- blocked

Repo-fit note:

- TextQuest already has the underlying launcher and login sequencing; the gap is operator visibility and stable naming

## Risk boundary

The JMB comparison does not justify broader control reach on its own.

Rules carried forward from the anti-cheat digest:

- keep routing changes on the current authenticated IPC and TUI control boundary
- do not infer permission for hidden input-hook expansion from JMB examples
- keep focus, relay, and preset work operator-visible and reversible
- treat risky movement, zoning, or exploit-style travel as separate milestone work with their own validation gates

## Follow-on slices

Concrete implementation slices that already fit this comparison:

- #109 Expose launch profiles, session presets, and slot health in the TUI
- #152 Add addressable actor routing abstraction

Recommended sequencing:

1. land the actor-routing abstraction first so one-toon, group, and all-session semantics are explicit in code
2. then expose launch profiles, session presets, and slot lifecycle in the TUI on top of that routing model

## Outcome

The JMB comparison supports a bounded `M8` conclusion:

- TextQuest should formalize routing scope and session-shell concepts
- TextQuest should not chase JMB runtime parity or hidden hook parity
- the next implementation work should improve operator-visible routing and lifecycle control, not expand gameplay automation risk
