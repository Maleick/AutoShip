# DMFT Implementation Roadmap

Canonical roadmap for the packet-first reset adopted in April 2026.

This document is the source of truth for roadmap order, milestone gates, evidence handling, external research promotion, and GitHub Project mirroring. Older design and review docs remain useful historical evidence, but they are not canonical roadmap sources once they conflict with this file.

## Historical Base

The repository already contains substantial implemented surface area before this roadmap reset:

- `M1`: external memory reading and TUI dashboard
- `M2`: DLL injection, hooks, IPC, and self-healing monitor
- `M2.5`: login automation and launch coordination
- `M3`: navigation and route handling
- `M4`: combat automation

These milestones stay part of the project history. The canonical active roadmap now resumes at `M5`.

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

### `M5` Packet Engine

Objective:

- formalize packet-first capability research and implementation boundaries without letting speculative packet work outrun validation

Initial slices:

- send/receive pipeline inventory and command-path selection rules
- ability packet coverage and targetability rules
- packet-driven validation tasks for combat, utility, and chat paths
- evidence-backed packet vs in-process control matrix

Current curated intake:

- `docs/external-research/packet-engine-send-receive-pipeline.md`
- `docs/external-research/packet-zoning-send-path-and-state-ledger.md`
- `docs/external-research/ability-packet-coverage-and-targetability-validation.md`

Entry gate:

- imported packet and network research archived and indexed
- send and receive pipeline inventory completed with evidence states

Exit gate:

- packet control paths have explicit acceptance criteria
- send and receive pipeline layers are inventoried with `In-process`, `Packet candidate`, or `Blocked` labels
- anti-cheat counter and opcode scrambler requirements are documented
- critical packet unknowns are either live-validated, blocked, or explicitly provisional

### `M6` Zoning/Movement

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

### `M7` Anti-Cheat

Objective:

- harden operational boundaries and validation rules using official Daybreak signals plus clearly labeled secondary community reporting

Initial slices:

- official Daybreak detection digest
- hook and module exposure review
- timing, session, and naming hardening tasks
- milestone-level validation gates for risky movement and control paths

Entry gate:

- Daybreak detection digest exists and separates official policy from community inference

Exit gate:

- anti-cheat gates exist for packet, zoning, and orchestration work
- unsupported high-risk inputs are labeled as low-confidence or blocked

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

### `M10` Soul Engine + LLM

Objective:

- connect personality, operator-safe chat behavior, and provider-backed AI only after the control stack is stable

Initial slices:

- Soul Engine workflow review against current code
- provider-backed AI credential and quota workflow
- TUI and operator controls for safe AI usage

### `M11` Economy

Objective:

- convert stable control and orchestration into loot, vendor, banking, and economy execution loops

Initial slices:

- loot and distribution workflow
- vendor and banking cycles
- economy-facing TUI summaries and operator overrides

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

- `DMFT Roadmap`: `https://github.com/users/Maleick/projects/1`

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

DMFT uses `codex-autoresearch` for the nightly external-research digest and milestone-slice discovery.

Nightly digest defaults:

- focus on one deep source family plus one anti-cheat digest refresh
- cap at `6-8` iterations
- keep changes proposal-oriented rather than implementation-heavy
- promote only evidence-backed slice candidates

Nightly checkpoint cadence:

1. update repo docs and research ledgers first
2. run the roadmap verifier and wiki guard
3. reconcile the active checkpoint batch in the `DMFT Roadmap` GitHub Project
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

Nightly project-sync rules:

- repo docs remain the source of truth
- the active checkpoint batch may update GitHub Project fields after the docs pass guard and verifier checks
- mature, cited, evidence-scored items may be promoted into GitHub issues
- provisional or low-confidence findings should remain draft items until they are strong enough to promote

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
