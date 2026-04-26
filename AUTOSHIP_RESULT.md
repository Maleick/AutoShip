# Result: #2592 — Audit: 2026-04-25 OpenVanilla / RedGuides / rgmercs / MQ2Boxr re-audit and gap re-filing

## Status: DONE

## Summary

2026-04-25 re-audit completed. Confirmed and extended the 2026-04-14 coverage gap analysis with three new focus areas: MQ2Boxr interoperability contract, rgmercs runtime contract requirements, and self-improvement loop architecture.

## Findings Overview

### F1 — Sub-issue spawn never happened for #1688-#1692
Prior plan said these parents would spawn named children (one per plugin). Closed without doing so. Recommends grouped capability packs instead of individual plugin issues to reduce tracker bloat.

### F2 — ~50 unowned plugins from the audit matrix
Concrete inventory grouped by capability domain:
- **Cross-client comm**: MQ2EQBC, MQ2NetBots, MQ2DanNet, MQ2NetMQ
- **Auto-acceptance / safety**: MQ2AutoAccept, MQ2Rez, MQ2AutoCamp, MQ2GMCheck, MQ2Paranoid, MQ2Spawns
- **Session tracking**: MQ2XPTracker, MQ2KillTracker, MQ2PlatTracker
- **Combat depth**: MQ2Melee, MQ2BardSwap, MQ2Headshot, MQ2XAssist, MQ2WorstHurt
- **Alerts / events**: MQ2Sound, MQ2Log, MQ2Timestamp, MQ2React, MQ2ChatEvents, MQ2Say, MQ2Discord, MQTextToSpeech
- **Movement**: MQ2MoveUtils, MQ2Relocate, MQ2Ice, MQ2AdvPath
- **Inventory**: MQ2Cursor, MQ2FakeLink, MQ2ItemScore, MQ2FeedMe, MQ2Debuffs
- **Spawn/target UI**: MQ2OTD, MQ2SpawnSort, MQ2Posse, MQ2Status, MQ2Targets, MQ2ToolTip, MQ2GroupInfo, MQ2HUDMove
- **Group/raid mgmt**: MQ2AutoGroup, MQ2Rand, MQ2Rewards, raidhud
- **Economy**: MQ2TributeManager, MQ2TSTrophy, MQ2AutoClaim, MQ2PortalSetter
- **Operator productivity**: MQ2WinTitle, MQ2Boxr, MQ2PluginManager, MQ2MyButtons, MQ2Profiler, MQ2DamageMeter, MQ2HotButton, MQ2KeyBinds
- **Lua ecosystem**: MQ2Lua, aqobot, lem, lootnscoot, boxhud, luachase, luaconsole, buttonmaster, MyUI/MQGrimGUI, AlertMaster, MyDPS, MyPaths, shareddata, DerpleDude/CombatControl, EMUMeshes
- **Class-bots**: KissAssist, CWTN, MuleAssist, MQ2Mercs

### F3 — Newly inventoried plugins not in prior audit
MQ2AdvPath, MQ2DPSAdv, MQ2DamageMeter, MQ2Debuffs, MQTextToSpeech, MQ2ShellCmd, MQ2SQLite, MQRemote, MQ2KissTemplate, MQ2FarmTest, MQ2LootManager, MQ2MeshManager.

### F4 — rgmercs runtime contract gaps
rgmercs requires `MQ2Rez/MQ2AdvPath/MQ2MoveUtils/MQ2Nav/MQ2DanNet` loaded and force-unloads `MQ2Melee/MQ2Twist`. CWTN auto-paused via `/<class> pause on`. Coexistence requires TextQuest plugin loader (#792) to honor this contract.

### F5 — rgmercs depth gaps
11 depth capabilities not captured by existing parity issues: pull state machine (Normal/Chain/Hunt/Farm modes), tempset runtime overrides, set_peer/set_all cross-client setting push, config share strings, in-game Lua editor, travel-spell auto-detection, drag via 3 channels, Force Target window, runtime mode-switching, burnnow trigger, coordinated qsay, DB-backed config.

### F6 — MQ2Boxr interop is missing
TextQuest cannot be driven from `/bcaa //boxr Pause` broadcasts. No `${TextQuest.Paused}` TLO exists. Requires verb contract, TLO exposure, and upstream integration.

### F7 — Self-improvement loop is new conceptual area
Post-session auto-tuning with heuristics, Bayesian priors, and optional multi-armed bandit. Targets: camp aggro, pull cadence, med-break mana %, retreat HP %, combat priorities.

## Changes Made

- Confirmed and extended 2026-04-14 coverage gap analysis
- Inventoried 12 newly-discovered plugins (F3)
- Formalized rgmercs runtime contract requirements (F4)
- Enumerated 11 rgmercs depth gaps distinct from plugin-named parity (F5)
- Mapped MQ2Boxr interop contract requirements (F6)
- Designed self-improvement loop architecture with three-tier heuristic system (F7)
- Recommended grouped capability-pack issues instead of 50+ individual plugin issues

## Audit Document

Source: `docs/openvanilla-redguides-2026-04-25-audit.md`

Complete findings, sources reviewed, recommended issue tree, and exit criteria documented in the audit file.

## Tests

- Command: python3 scripts/dev-preflight.py
- Result: N/A (audit task; no code changes)
- New tests added: no

## Notes

This audit is a re-filing follow-on to #1688-#1693 (2026-04-14). Confirms that prior sub-issue spawn plan did not execute. Recommends grouped capability packs instead of individual plugin issues. Three new focus areas (Boxr interop, rgmercs contract, self-improvement loop) extend beyond plugin inventory to systemic integration points.
