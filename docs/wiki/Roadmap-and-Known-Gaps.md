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

- `M5`: Anti-Cheat (**active**)
- `M6`: Web Dashboard (config + monitoring)
- `M7`: Zoning/Movement
- `M8`: Orchestrator
- `M9`: Learning/RL
- `M10`: Economy
- `M11`: Soul Engine + LLM (local AI only)

External research may add slices and validation tasks, but it may not reorder milestones on its own.

## Current Validated State

As of 2026-04-06: ~1,001 commits, ~113K lines of Rust, 2,584 tests (all passing). M5 Anti-Cheat complete (#355 closed — launchpad bypassed via /patchme). M6 Web Dashboard complete (TUI enhancements, axum + React SPA scaffold, fleet metrics).

- TUI with four primary screens and command bar
- demo mode for non-Windows and no-client workflows
- live Windows injection and authenticated IPC path
- login automation structure and in-client login logic
- navmesh-backed routing and map overlays
- combat FSM plus class strategies and CH chain
- Soul Engine with deterministic fallback and persistent memory
- stick-to-target and player-follow navigation modes
- camp loop state machine with buff/CC/loot/positioning
- Discord webhook integration and command bridge
- encrypted credential store (Argon2id + AES-256-GCM)

## Main Gaps Still Requiring Live Validation

- packet-level control paths inferred from research rather than live validation
- the current packet inventory keeps combat, utility, and chat packet seams separate from the existing IPC plus in-process DLL control boundary; see `docs/external-research/packet-engine-send-receive-pipeline.md` for the full send/receive layer inventory and capability boundary summary
- the send pipeline requires opcode scrambling and anti-cheat counter synchronization before any packet-first path can be treated as safe
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
- `docs/external-research/packet-engine-send-receive-pipeline.md`
- `docs/external-research/packet-zoning-send-path-and-state-ledger.md`
- `docs/external-research/ability-packet-coverage-and-targetability-validation.md`
- `docs/wiki/Research-KissAssist-Gap-Analysis.md`
- `docs/external-research/jmb-session-and-relay-comparison.md`
- `docs/external-research/daybreak-detection-digest.md`
- `docs/external-research/zoning-queue-and-safe-coord-validation.md`

## Current `M8` orchestration guidance

The current JMB comparison keeps `M8` bounded to operator-visible orchestration work:

- formalize routing scopes as `one-toon`, `group`, and `all-session`
- translate launch profile, session preset, and slot lifecycle concepts into TUI-visible state
- keep command routing on the existing authenticated IPC path instead of treating JMB hook examples as direct implementation targets

The current follow-on implementation slices remain:

- #152 for the addressable actor routing abstraction
- #109 for launch profiles, session presets, and slot-health visibility

## Current `M10` economy guidance

The current economy pass stays intentionally bounded to operator-visible execution loops:

- treat loot intake, distribution, vendor, and banking work as explicit queues and state machines, not as hidden background automation
- reuse the existing authenticated routing and session/group scope model instead of inventing a separate economy control plane
- ship pause/skip/abort/resume controls and economy-facing TUI summaries alongside each loop before claiming a self-sustaining farm workflow

The current follow-on implementation slices are:

- loot intake plus distribution ownership and reserve rules
- vendor and banking route controllers with failure-state visibility
- plat/item ledger summaries plus operator overrides

## Developer Guidance

When writing docs, PRs, or GitHub Project mirror items:

- keep roadmap claims anchored to `docs/implementation-roadmap.md`
- separate current behavior from provisional findings
- mark live-validation gaps explicitly
- prefer evidence-state language over vague confidence claims
- treat zoning queue flush, timeout handling, and safe-coordinate recovery as `Needs Live Proof` until current-build validation exists
