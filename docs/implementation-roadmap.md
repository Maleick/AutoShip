# TextQuest Implementation Roadmap

Canonical roadmap for the packet-first reset adopted in April 2026.

This document is the source of truth for roadmap order, milestone gates, evidence handling, external research promotion, and GitHub Project mirroring. Older design and review docs remain useful historical evidence, but they are not canonical roadmap sources once they conflict with this file.

## Milestone Status Summary

| Milestone | Name                    | Status       | Notes                                                              |
| --------- | ----------------------- | ------------ | ------------------------------------------------------------------ |
| M1        | External Memory Reading | **COMPLETE** | TUI dashboard, spawn reading, process interaction                  |
| M2        | DLL Injection + IPC     | **COMPLETE** | Injection, hooks, IPC, self-healing monitor                        |
| M2.5      | Login Automation        | **COMPLETE** | Credential store, process spawner, login FSM, launch coordinator   |
| M3        | Navigation              | **COMPLETE** | Waypoint pathfinding, Navigator FSM, humanization, stuck detection |
| M4        | Combat                  | **COMPLETE** | ClassStrategy trait, 17 classes, HolyShit system, puller FSM       |
| M5        | Anti-Cheat              | **COMPLETE** | #355 closed — launchpad bypassed via /patchme                      |
| M6        | Web Dashboard           | **COMPLETE** | TUI enhancements, axum + React/Vite/Tailwind SPA, fleet metrics   |
| M7–M11    | Future                  | Planned      | Zoning, Orchestrator, RL, Economy, Soul Engine                     |

_Last updated: 2026-04-07. ~1284 commits, ~2944 tests across 4 crates._

## Historical Base

The repository already contains substantial implemented surface area before this roadmap reset:

- `M1`: external memory reading and TUI dashboard — **COMPLETE**
- `M2`: DLL injection, hooks, IPC, and self-healing monitor — **COMPLETE**
- `M2.5`: login automation and launch coordination — **COMPLETE**
- `M3`: navigation and route handling — **COMPLETE**
- `M4`: combat automation — **COMPLETE**

These milestones stay part of the project history.

Packet Engine research (send/receive pipeline inventory, ability packet coverage, opcode scrambler requirements) was conducted as a research track and its findings are archived in `docs/external-research/`. Packet-related constraints feed into anti-cheat, zoning, and orchestrator milestones as needed rather than standing as a separate implementation milestone.

The canonical active roadmap now resumes at `M5`.

## Current Validated State vs Provisional Findings

### Current validated state

- external TUI with four primary screens and command bar
- live Windows injection and command dispatch path
- orchestrator plus DLL split with typed IPC
- class-driven combat and camp-loop structure
- login automation structure and post-login sequencing
- existing Soul Engine and provider abstraction in code

### Provisional or patch-sensitive areas

- packet-level control paths inferred from imported research
- zoning state-machine details that still require live client validation
- Daybreak detection behavior beyond explicit public policy
- high-risk community claims around warping, PiggyZone-style travel, and exploit-grade travel hacks

## Canonical Milestone Order

### `M5` Anti-Cheat

Objective:

- harden the injected DLL against detection by replacing every high-signal artifact (injection method, hook style, memory layout, syscall pattern, thread model) with evasion-grade alternatives sourced from modern C2 research and SME-guided Ghidra decompilation

Full research: [`docs/anti-detection.md`](anti-detection.md)

#### Implementation slices (GitHub issues #344–#355)

**P1 — Critical path (implement in order)**

| Issue | Slice                            | Summary                                                                                                              |
| ----- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| #344  | Reflective DLL injection         | Replace `CreateRemoteThread` + `LoadLibraryW` with reflective loader; DLL never touches disk or appears in `PEB.Ldr` |
| #345  | Hardware breakpoint hooking      | Replace detour (inline patch) hooks with DR0–DR3 hardware breakpoint hooks; zero modified bytes in `.text`           |
| #346  | Per-frame sleep obfuscation      | Gargoyle-style timer-based sleep with XOR encryption of DLL pages while idle; wake via APC                           |
| #347  | Indirect syscalls (RecycledGate) | Replace all `ntdll.dll` imports with indirect syscalls resolved at runtime; no direct `ntdll` calls in IAT           |
| #355  | Launchpad token handshake        | Launch via legitimate patcher path; acquire session token through official launchpad flow                            |

**P2 — Hardening**

| Issue | Slice                             | Summary                                                                                                |
| ----- | --------------------------------- | ------------------------------------------------------------------------------------------------------ |
| #348  | PEB unlinking + PE header erasure | Unlink DLL from `PEB.Ldr` doubly-linked lists and zero PE headers in-memory after init                 |
| #349  | DLL staging with legitimate names | Stage the DLL using legitimate Microsoft process/module names to blend with expected loaded modules    |
| #350  | Replace VirtualAlloc              | Use `HeapAlloc` / `NtCreateSection` instead of `VirtualAlloc` for memory allocation; avoid `RWX` pages |
| #351  | Patchless ETW blinding            | Blind ETW via hardware breakpoints on `NtTraceEvent` rather than patching `EtwEventWrite`              |
| #352  | Thread pool execution (PoolParty) | Replace `CreateThread` with Windows thread pool work items (`TpAllocWork` / `TpPostWork`)              |

**P3 — Advanced**

| Issue | Slice                           | Summary                                                                                             |
| ----- | ------------------------------- | --------------------------------------------------------------------------------------------------- |
| #353  | Per-API call stack spoofing     | Spoof return addresses on sensitive API calls to appear as legitimate caller chains                 |
| #354  | Nighthawk-style page encryption | Per-page encryption with ~2% plaintext exposure; only the executing page is decrypted at any moment |

#### Rust crate dependencies

- **`dinvoke_rs`** — dynamic invocation and indirect syscalls
- **`rust_syscalls`** — raw syscall wrappers
- **`goblin`** — PE parsing for reflective loader and header erasure
- **`hypnus`** — Gargoyle-style sleep obfuscation primitives
- **`shelter`** — PEB manipulation and module unlinking

#### Critical constraints

- **Main game loop is a NO-TOUCH ZONE**: the game's inline byte count + memshift with circular protection makes patching the main loop a guaranteed detection vector. All hooks target secondary functions only.
- **Per-frame overhead budget**: 1–2 ms per 33 ms frame (3–6% ceiling). Every P1/P2 slice must benchmark against this budget.
- **SME-sourced intel**: Key findings from Matt (Blownt) via Ghidra decompilation of the EQ client informed hook target selection and memory layout constraints.

#### Implementation order

1. **P1 in sequence**: injection (#344) → hooking (#345) → sleep obfuscation (#346) → syscalls (#347) → launchpad token (#355). Each layer depends on the previous — reflective injection must land before hooks can be installed without detection.
2. **P2 in parallel**: once P1 is stable, P2 slices (#348–#352) are largely independent and can be worked concurrently.
3. **P3 after P2**: call stack spoofing (#353) and page encryption (#354) are polish layers that build on the full P1+P2 stack.

Entry gate:

- Daybreak detection digest exists and separates official policy from community inference
- full anti-detection research complete ([`docs/anti-detection.md`](anti-detection.md))
- SME decompilation findings reviewed and integrated

Exit gate:

- anti-cheat gates exist for packet, zoning, and orchestration work
- unsupported high-risk inputs are labeled as low-confidence or blocked
- all P1 slices pass per-frame overhead benchmark (≤2 ms)
- DLL has zero static detection signatures (no `PEB.Ldr` entry, no IAT imports to `ntdll`, no `RWX` pages, no detour patches)

### `M6` Web Dashboard

Objective:

- replace TUI-based configuration with a browser UI for managing clients, credentials, groups, camp config, and session monitoring — the TUI stays as the gameplay/monitoring surface, the dashboard owns all configuration and setup

Tech stack:

- **Backend**: `axum` + `tokio-tungstenite` HTTP + WebSocket server embedded in the orchestrator binary
- **Frontend**: Vite + React + Tailwind SPA, built to `dist/` and embedded via `rust-embed` or `include_dir`
- **State**: SQLite (credentials already use it) + in-memory state broadcast via WebSocket
- **Reactivity**: Watch internal TextQuest state changes, broadcast over WebSocket to connected dashboards

#### Implementation slices

**P1 — Critical path (implement in order)**

| Slice                        | Summary                                                                                                   |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- |
| Axum server scaffold         | Embedded HTTP + WS server in orchestrator, serves SPA from `rust-embed`, WebSocket upgrade for live state |
| Client/credential management | CRUD for Daybreak accounts, character mapping, login sequencing — replaces TOML/CLI credential workflows  |
| Group and camp configuration | Class roles, camp assignments, combat settings, HolyShit thresholds — visual editor replaces TOML editing |
| Session monitoring dashboard | Live view of all clients: logged-in state, health, zone, stuck detection, camp loop phase                 |

**P2 — Extended configuration**

| Slice                      | Summary                                                                       |
| -------------------------- | ----------------------------------------------------------------------------- |
| Navigation/combat tuning   | Waypoint route editor, spell priority drag-and-drop, combat parameter sliders |
| Launch sequencing UI       | Visual launch order, stagger timing, post-login sequence configuration        |
| Log viewer and diagnostics | Streamed logs per client, error highlighting, IPC message inspector           |

**P3 — Polish**

| Slice                    | Summary                                                                    |
| ------------------------ | -------------------------------------------------------------------------- |
| Multi-group overview     | Aggregate view across all groups: DPS meters, heal coverage, camp progress |
| Mobile-responsive layout | Responsive design for monitoring from phone/tablet while AFK               |
| Theme and accessibility  | Dark/light mode, keyboard navigation, WCAG compliance                      |

#### Implementation order

1. **P1 in sequence**: server scaffold → credentials UI → group/camp config → session monitor. Each layer builds on the server and state infrastructure.
2. **P2 in parallel**: once P1 is stable, nav/combat tuning, launch UI, and log viewer are independent panels.
3. **P3 after P2**: overview, mobile, and theme are polish layers.

Entry gate:

- M5 anti-cheat work is stable enough that the orchestrator binary architecture is settled
- SQLite credential store exists and is tested (M2.5)
- axum and tokio are already in the dependency tree

Exit gate:

- all TOML-based configuration can be managed through the dashboard
- credential CRUD works end-to-end with encrypted storage
- live session state updates via WebSocket with <1s latency
- TUI continues to function for gameplay monitoring (dashboard does not replace TUI runtime views)

### `M7` Zoning/Movement

Objective:

- turn zoning and movement internals into a reliable, operator-visible travel and recovery surface

Initial slices:

- zone transition state and failure-code mapping
- movement validation rules and queue flushing
- safe-coord, zone-line, and teleport category handling
- TUI visibility for route state, stuck state, and zoning blockers

Current curated intake:

- `docs/external-research/packet-zoning-send-path-and-state-ledger.md`

Entry gate:

- packet-engine validation tasks define which travel actions stay packet-driven vs in-process

Exit gate:

- zoning workflows have live validation tasks and operator-facing failure states

### `M8` Orchestrator

Objective:

- deepen multibox coordination, group/session control, relay surfaces, and operator workflows

Initial slices:

- Joe Multiboxer and JMB coordination comparison
- team and session grouping model
- relay, broadcast, and control-routing model
- TUI session and group command surfaces

Entry gate:

- packet, zoning, and anti-cheat gates are defined well enough to constrain orchestration behavior

Exit gate:

- cross-client control model is documented and mapped to TUI workflows

### `M9` Learning/RL

Objective:

- add tuning and optimization loops after core control and orchestration surfaces exist

Initial slices:

- behavior optimization targets
- measurable tuning loops
- guardrails that prevent regressions from training-driven changes

### `M10` Economy

Objective:

- convert stable control and orchestration into loot, vendor, banking, and economy execution loops — the self-sustaining Krono farm that funds hardware and accounts

#### Implementation slices

**P1 — Core execution loops (implement in order)**

| Slice | Summary |
| ----- | ------- |
| Loot intake and distribution workflow | Turn corpse, item, and recipient selection into an explicit queue with operator-visible ownership, reserve, and handoff state |
| Vendor cycle controller | Promote the existing sell-cycle primitives into a bounded vendor run with route state, keep/sell policy visibility, and abort/resume controls |
| Banking cycle controller | Add deposit, withdrawal, and mule-transfer queues that reuse the same routing, inventory, and confirmation model as vendor runs |
| Economy-facing TUI summaries and overrides | Surface the active queue, acting character, destination, cash/inventory deltas, and pause/skip/abort controls in the operator UI |

**P2 — Sustaining-loop support**

| Slice | Summary |
| ----- | ------- |
| Wishlist and reserve rules | Track keep/sell/bank/distribute intent per item class so loot and vendor loops stay deterministic instead of ad hoc |
| Economy ledger and trend summaries | Record plat, item, and handoff outcomes so the operator can verify that the loop is productive and bounded |
| Failure routing and recovery | Make overweight inventory, full bank, missing vendor, and unsafe-path failures visible with explicit fallback states instead of silent retries |

#### Implementation order

1. Start with loot intake/distribution because it defines the item-routing contract the later vendor and banking loops must obey.
2. Layer vendor and banking cycles on top of that contract, keeping both flows on the existing authenticated control path instead of inventing a parallel economy channel.
3. Keep every phase operator-visible with explicit summaries and overrides before claiming any self-sustaining farm loop.

Initial slices:

- loot and distribution workflow
- vendor and banking cycles
- economy-facing TUI summaries and operator overrides

Entry gate:

- `M8` orchestration routing and session/group visibility are stable enough to target a single toon, team, or session set intentionally
- `M9` guardrails exist so economy automation can be measured without regressing the underlying control stack

Exit gate:

- loot, vendor, and banking execution loops are all bounded by explicit operator-visible queue/state surfaces
- every economy loop exposes at least pause, skip, abort, and resume controls
- TUI summaries make current actor, target destination, and recent cash/item outcomes visible without digging through logs
- the self-sustaining farming loop is tracked as a validated automation surface, not inferred from disconnected subsystems

### `M11` Soul Engine + LLM

Objective:

- connect personality, operator-safe chat behavior, and locally-hosted AI (Gemma 4 or equivalent) only after the full control and economy stack is stable — no external API dependencies

Initial slices:

- Soul Engine workflow review against current code
- local model hosting and inference pipeline (Gemma 4, ollama, or similar)
- TUI and operator controls for safe AI usage
- personality and idle behavior driven by local inference

## Domain Tracks

- `Packet Engine`
- `Zoning/Movement`
- `Anti-Cheat`
- `Orchestrator`
- `Docs/Workflow`

Use these domains in roadmap docs, GitHub Project fields, and external research promotion.

## Evidence Model

Every slice, validation task, and draft GitHub Project item should carry one evidence state:

- `Provisional`
- `Research-backed`
- `Needs Live Proof`
- `Live-validated`
- `Invalidated`

Rules:

- `Research-backed` items may enter active execution work.
- `Live-validated` is required to retire critical unknowns or satisfy milestone exit gates.
- `Provisional` items may exist in research ledgers and checkpoint intake, but they should not be used as proof of milestone completion.

## External Research Lane

External research is docs-first and checkpoint-based.

### Source tiers

Primary sources:

- RedGuides docs and plugin docs
- KissAssist project docs
- MacroQuest docs, including Lua docs
- public code repositories such as JMB Basic Core, JMB WinEQ 2022, and JMB Input Hook Example
- official Daybreak support and policy pages

Secondary sources:

- Joe Multiboxer site
- MMOBugs forums
- EverQuestBot
- Bonzz guides
- EQ Might AA code reference

Low-confidence or risk-oriented inputs:

- exploit threads
- PiggyZone-style hack discussions
- community claims that lack either official backing or repo-fit corroboration

### Promotion rule

Before an external finding becomes a roadmap slice candidate, it must include:

1. at least one citation
2. explicit repo-fit rationale
3. a concrete slice or validation task
4. an assigned evidence state

### Deep-dive order

1. KissAssist capability audit and TUI translation
2. Joe Multiboxer and JMB orchestration pass
3. MacroQuest Lua and broader scripting/runtime extension pass
4. ongoing Daybreak detection digest updates

## GitHub Project Mirror

GitHub Projects are a mirror, not a source of truth.

Current mirror:

- `TextQuest Roadmap`: `https://github.com/users/Maleick/projects/1`

Use one roadmap project with these fields:

- `Status`
- `Milestone`
- `Domain`
- `Evidence State`
- `Priority`
- `Item Type`
- `Checkpoint Batch`
- `Source Doc`
- `Target Window`
- `Effort`

Rules:

- milestone epics begin as draft items
- mature tasks, validations, and research items should be promoted from draft items into GitHub issues once they have a concrete scope and checkpoint batch
- sync is checkpoint-based, after repo docs are updated
- remove overlapping draft items after issue promotion so the active board has one execution item per slice
- external research may add draft items and validation tasks, but it may not reorder milestones on its own
- CLI bootstrap requires `gh auth refresh -s read:project -s project`

## Autoresearch Workflow

TextQuest uses `codex-autoresearch` for the nightly external-research digest and milestone-slice discovery.

Nightly digest defaults:

- focus on one deep source family plus one anti-cheat digest refresh
- cap at `6-8` iterations
- keep changes proposal-oriented rather than implementation-heavy
- promote only evidence-backed slice candidates

Nightly checkpoint cadence:

1. update repo docs and research ledgers first
2. run the roadmap verifier and wiki guard
3. reconcile the active checkpoint batch in the `TextQuest Roadmap` GitHub Project
4. promote mature current-window items into GitHub issues and remove overlapping drafts
5. record project-sync results in the autoresearch artifacts

Mechanical verifier:

- `python scripts/validate_roadmap_unknowns.py --plan docs/implementation-roadmap.md --domains packet,zoning,anticheat`

Expected outputs:

- updates to curated research ledgers
- proposed checkpoint batch items
- GitHub Project mirror updates for the active checkpoint batch
- GitHub issue promotion for mature checkpoint items
- evidence-state changes
- no milestone reordering

## Nightly Automation

### GitHub Actions

- `.github/workflows/wiki-nightly.yml`
  - scheduled wiki auto-publish using `scripts/sync_wiki.py --push`
  - requires runner-local `gh auth`
- `.github/workflows/nightly-release.yml`
  - scheduled rolling nightly prerelease build on the self-hosted Windows runner

### Codex automation

The nightly external-research digest runs as a Codex automation after the GitHub workflows. It is intentionally separate from GitHub Actions because it needs evidence modeling, source weighting, and slice promotion rules that are easier to enforce in a Codex-guided research loop.

Automation definition: `.codex/automations/textquest-night-research/automation.toml`

Nightly project-sync rules:

- repo docs remain the source of truth; project sync only begins after the roadmap verifier and wiki guard both pass
- the active checkpoint batch may update GitHub Project fields after the docs pass guard and verifier checks
- mature, cited, evidence-scored items (Research-backed or higher) may be promoted into GitHub issues using `python scripts/sync_project.py --promote`
- provisional or low-confidence findings must remain draft project items until they are strengthened by additional research or live validation
- after promoting a draft item to an issue, remove the overlapping draft from the project board so the active board has exactly one execution item per slice
- every project-sync run records its results (items promoted, skipped, errored) in `autoresearch-project-sync.json` before the loop exits
- `autoresearch-project-sync.json` and all other transient loop state files are excluded from git via `.gitignore` and must never be committed

## Near-Term Backlog

### Current checkpoint focus

- archive packet and zoning import set
- establish the canonical roadmap and research ledgers
- align README and wiki surfaces with the new roadmap
- enable nightly wiki publish and rolling nightly prerelease workflows
- build the first KissAssist gap matrix and TUI translation draft
- build the first Daybreak detection digest

## Historical References

These remain useful, but they are no longer canonical roadmap sources:

- `docs/roadmap-review.md`
- `docs/orchestration-design.md`
- `docs/dll-injection-plan.md`
- `docs/mq2-deep-dive.md`
- `docs/superpowers/plans/`
