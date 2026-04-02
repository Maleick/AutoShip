# Roadmap and Known Gaps

The canonical roadmap source is `docs/implementation-roadmap.md`.

This page summarizes the active milestone order, the evidence model, and the main validation gaps that still matter operationally.

## Historical Base

These milestones remain part of project history:

- `M1`: external memory reading and TUI dashboard
- `M2`: DLL injection, hooks, IPC, and self-healing monitor
- `M2.5`: login automation and launch coordination
- `M3`: navigation and route handling
- `M4`: combat automation

## Canonical Active Milestone Order

The active roadmap now resumes at `M5`:

- `M5`: Packet Engine
- `M6`: Zoning/Movement
- `M7`: Anti-Cheat
- `M8`: Orchestrator
- `M9`: Learning/RL
- `M10`: Soul Engine + LLM
- `M11`: Economy

External research may add slices and validation tasks, but it may not reorder milestones on its own.

## Current Validated State

- TUI with four primary screens and command bar
- demo mode for non-Windows and no-client workflows
- live Windows injection and authenticated IPC path
- login automation structure and in-client login logic
- navmesh-backed routing and map overlays
- combat FSM plus class strategies and CH chain
- Soul Engine with deterministic fallback and persistent memory

## Main Gaps Still Requiring Live Validation

- packet-level control paths inferred from research rather than live validation
- zoning state-machine details and recovery behavior after client changes
- offset stability after upstream EQ updates
- cross-zone travel behavior in more zones than the current dev/test set
- class-by-class combat tuning under real combat load
- large-scale multibox validation closer to the 36-client target
- post-login group sequencing and camp setup under real launch conditions
- anti-cheat conclusions beyond explicit public Daybreak policy

## Evidence Model

Every roadmap slice, validation task, or GitHub Project mirror item should carry one evidence state:

- `Provisional`
- `Research-backed`
- `Needs Live Proof`
- `Live-validated`
- `Invalidated`

Rules:

- `Research-backed` items may enter execution.
- `Live-validated` is required to retire critical unknowns or satisfy milestone exit gates.
- `Provisional` items may stay in research ledgers, but they are not proof of milestone completion.

## Current Research Lane

External research is docs-first and checkpoint-based.

Current deep-dive order:

1. KissAssist capability audit and TUI translation
2. Joe Multiboxer and JMB orchestration pass
3. MacroQuest Lua and broader runtime pass
4. ongoing Daybreak detection digest updates

Use:

- `docs/external-research/automation-source-ledger.md`
- `docs/external-research/kissassist-gap-and-tui-translation.md`
- `docs/external-research/daybreak-detection-digest.md`

## Developer Guidance

When writing docs, PRs, or GitHub Project mirror items:

- keep roadmap claims anchored to `docs/implementation-roadmap.md`
- separate current behavior from provisional findings
- mark live-validation gaps explicitly
- prefer evidence-state language over vague confidence claims
