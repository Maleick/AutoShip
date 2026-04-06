# KissAssist Gap and TUI Translation

This document is the first deep external-research pass for TextQuest's operator workflow roadmap.

Primary source anchors:

- [KissAssist](https://www.redguides.com/docs/projects/kissassist/)
- [RedGuides Docs](https://www.redguides.com/docs/)
- [RedGuides Plugins](https://www.redguides.com/docs/plugins/)

Supporting comparison inputs:

- [Joe Multiboxer](https://joemultiboxer.com/)
- [JMB Basic Core](https://github.com/LavishSoftware/JMB-Basic-Core)
- [JMB WinEQ 2022](https://github.com/LavishSoftware/JMB-WinEQ-2022)
- [JMB Input Hook Example](https://github.com/LavishSoftware/JMB-Input-Hook-Example)

## Goal

Use KissAssist as a workflow benchmark, not as a UI clone target.

The objective is to identify operator capabilities that TextQuest still lacks or does not expose clearly enough, then translate them into native TextQuest TUI surfaces and milestone slices.

## Capability Matrix

| Capability | External reference | Current TextQuest position | Translation target | Evidence state |
| --- | --- | --- | --- | --- |
| assist and pull control | [KissAssist](https://www.redguides.com/docs/projects/kissassist/) | TextQuest already has combat, puller, and camp-loop structure, but operator surfaces are still command-heavy | add a clearer assist or pull operator surface in the TUI, with status visibility and fast overrides | Research-backed |
| heal, buff, and debuff priority visibility | [KissAssist](https://www.redguides.com/docs/projects/kissassist/) | class automation exists, but live priority visibility is still diffuse | expose priority queues, current intent, and blocked reasons in a compact status panel | Research-backed |
| per-toon and per-group behavior configuration | [KissAssist](https://www.redguides.com/docs/projects/kissassist/) | configuration exists in TOML and command flows, but not yet as a first-class TUI editing workflow | add operator panels for role, behavior mode, thresholds, and scope overrides | Research-backed |
| camp loop, medding, buffing, looting, and emergency state inspection | [KissAssist](https://www.redguides.com/docs/projects/kissassist/) | camp-loop logic is present, but live state explanation is still thin | surface state machine phase, blockers, timers, and next-action reasoning in the TUI | Research-backed |
| multibox broadcast and relay patterns | [Joe Multiboxer](https://joemultiboxer.com/), [JMB Basic Core](https://github.com/LavishSoftware/JMB-Basic-Core) | TextQuest already supports group focus and broadcast-style commands, but coordination models are not yet formalized | build clearer session, relay, and target-scope control surfaces | Research-backed |
| window and session orchestration | [JMB WinEQ 2022](https://github.com/LavishSoftware/JMB-WinEQ-2022) | TextQuest has launch and login coordination, but not a mature session orchestration surface | add session grouping, launch state, and operator-visible control routing | Research-backed |
| hook and input model comparison | [JMB Input Hook Example](https://github.com/LavishSoftware/JMB-Input-Hook-Example) | useful as a bounded comparison input only | feed anti-cheat and operator-routing review, not direct feature parity | Provisional |

## Missing Operator Workflows

The main gaps are not raw capability count. The larger gap is operator visibility and control density.

TextQuest still needs clearer native workflows for:

- switching between assist, pull, camp, and emergency overrides without dropping into free-form command entry
- seeing why a character is medding, buffing, waiting, or blocked
- editing per-character and per-group behavior without hunting through config files
- routing commands cleanly across one toon, one group, or the full session set
- understanding team or session state at a glance during launch, regroup, or recovery

## TUI Translation Targets

### Assist and pull surface

Add a dedicated control surface for:

- current assist target
- pull target and pull state
- current combat mode
- one-key overrides for engage, disengage, assist, and emergency stop

### Priority visibility

Expose live intent and blockers for:

- heal
- buff
- debuff
- crowd control
- medding
- loot

This should be visible as status rows or an inspector panel, not buried in logs.

### Behavior configuration

Translate TOML-first behavior into TUI-first operator controls for:

- per-toon role
- per-group behavior mode
- mana or health thresholds
- assist and pull rules
- safe overrides and resets

### State inspection

The operator should be able to inspect:

- current camp-loop phase
- next planned action
- cooldown or resource blockers
- emergency state
- recovery state after death, zone, or regroup

### Command bar shortcuts and panel flow

Keep the `:` command bar, but reduce the need to memorize everything by adding:

- presets for common assist, camp, and regroup actions
- panel-driven overrides for selected scope
- clearer scope display for character, group, and all-session routing

## Milestone Slice Mapping

### `M8` Orchestrator

Primary slices from this research:

- team and session grouping model
- relay and broadcast routing model
- TUI control scope and routing visibility
- operator workflow for regroup, assist, and emergency override

### `M6` Zoning/Movement

Supporting slices:

- route-state visibility during travel and regroup
- clearer explanation of zoning blockers and recovery state

### `M7` Anti-Cheat

Supporting slices:

- avoid translating capability benchmarks into unsafe implementation choices
- prefer bounded operator visibility work over hidden automation escalation

## Current Recommendation

The first code-facing slice should be a bounded operator workflow improvement under `M8`, not a broad automation rewrite.

The most practical starting point is a TUI or command-routing slice that makes assist, pull, scope, and emergency state visible without changing the deeper combat engine first.
