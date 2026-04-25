# TextQuest Re-Audit — OpenVanilla / RedGuides / rgmercs / MQ2Boxr

**Date:** 2026-04-25
**Scope:** MQ2 plugin parity gaps, rgmercs runtime contract, MQ2Boxr interop, self-improvement loop
**Predecessor:** `docs/openvanilla-macroquest-coverage-gap-plan.md` (2026-04-14 audit, issues #1688–#1693)
**Umbrella issue:** #2592

---

## Executive Summary

- **~50 plugins** from the original plugin matrix have no owning GitHub issue.
- **#1688–#1692** closed as completed without spawning the named sub-issues the plan specified.
- **#1693** produced only 5 of its 13 named plugin children (#1881–#1885).
- Active rgmercs/Lua/MQ-plugin parity work migrated under epic **#790** (~17 open children #2430–#2461).
- **6 new gap categories** identified: runtime contract, depth gaps, MQ2Boxr interop, self-improvement loop, newly-inventoried plugins, cross-client comms.

---

## F1 — Sub-issue spawn never happened for #1688–#1692

The four "feature pack" parents closed completed without children. Re-filing as grouped capability packs rather than re-opening. Prior names were too broad to drive implementation.

**Action:** File grouped pack issues (see §Issue Tree below).

---

## F2 — ~50 Unowned Plugins

| Domain                         | Plugins                                                                                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cross-client comm transports   | MQ2EQBC, MQ2NetBots, MQ2DanNet, MQ2NetMQ                                                                                                                                  |
| Auto-acceptance / safety       | MQ2AutoAccept, MQ2Rez, MQ2AutoCamp, MQ2GMCheck, MQ2Paranoid, MQ2Spawns                                                                                                    |
| Session tracking               | MQ2XPTracker, MQ2KillTracker, MQ2PlatTracker                                                                                                                              |
| Combat depth (non-rgmercs)     | MQ2Melee, MQ2BardSwap, MQ2Headshot, MQ2XAssist, MQ2WorstHurt                                                                                                              |
| Alerts and events              | MQ2Sound, MQ2Log, MQ2Timestamp, MQ2React, MQ2ChatEvents, MQ2Say, MQ2Discord, MQTextToSpeech                                                                               |
| Movement extensions            | MQ2MoveUtils (stick/makecamp only owned), MQ2Relocate, MQ2Ice, MQ2AdvPath                                                                                                 |
| Inventory utilities            | MQ2Cursor, MQ2FakeLink, MQ2ItemScore, MQ2FeedMe, MQ2Debuffs                                                                                                               |
| Spawn/target/awareness UI      | MQ2OTD, MQ2SpawnSort, MQ2Posse, MQ2Status, MQ2Targets, MQ2ToolTip, MQ2GroupInfo, MQ2HUDMove                                                                               |
| Group / raid mgmt              | MQ2AutoGroup, MQ2Rand, MQ2Rewards, raidhud                                                                                                                                |
| Economy / subscription         | MQ2TributeManager, MQ2TSTrophy, MQ2AutoClaim, MQ2PortalSetter                                                                                                             |
| Operator productivity / system | MQ2WinTitle, MQ2Boxr, MQ2PluginManager, MQ2MyButtons, MQ2Profiler, MQ2DamageMeter, MQ2HotButton, MQ2KeyBinds                                                              |
| Lua / scripting ecosystem      | MQ2Lua, aqobot, lem, lootnscoot, boxhud, luachase, luaconsole, buttonmaster, MyUI/MQGrimGUI, AlertMaster, MyDPS, MyPaths, shareddata, DerpleDude/CombatControl, EMUMeshes |
| Class-bot ecosystem            | KissAssist, CWTN, MuleAssist, MQ2Mercs                                                                                                                                    |

---

## F3 — Newly Inventoried Plugins (Not in Prior Audit)

- MQ2AdvPath, MQ2DPSAdv, MQ2DamageMeter, MQ2Debuffs, MQTextToSpeech
- MQ2ShellCmd, MQ2SQLite, MQRemote, MQ2KissTemplate, MQ2FarmTest
- MQ2LootManager, MQ2MeshManager

**Action:** Add to FEATURE_PARITY_MATRIX.md and assign to appropriate pack issues.

---

## F4 — rgmercs Runtime Contract Gaps

rgmercs declares hard runtime dependencies and managed plugin state:

| Contract       | Detail                                              |
| -------------- | --------------------------------------------------- |
| Required-load  | MQ2Rez, MQ2AdvPath, MQ2MoveUtils, MQ2Nav, MQ2DanNet |
| Force-unloaded | MQ2Melee, MQ2Twist                                  |
| Pause signal   | `/<class> pause on` (CWTN auto-pause)               |
| Peer messaging | MQ Actors mailbox                                   |

TextQuest's plugin loader (#792) must honor or replace this contract before rgmercs can be treated as supported.

**Action:** File "rgmercs runtime contract" issue against #792.

---

## F5 — rgmercs Depth Gaps

Features present in rgmercs but absent or incomplete in TextQuest:

| Feature                     | rgmercs mechanism                              |
| --------------------------- | ---------------------------------------------- |
| Pull state machine          | Normal / Chain / Hunt / Farm modes             |
| Per-server named lists      | Server-specific NPC/mob targeting lists        |
| `tempset` runtime overrides | In-session setting mutation without persisting |
| `set_peer` / `set_all`      | Cross-client setting push                      |
| Base64 config share         | Compact config export/import strings           |
| In-game Lua editor + REPL   | Live code editing from EQ chat                 |
| Travel-spell auto-detection | Peer broadcast of available travel spells      |
| Drag via 3 channels         | EQBC, DanNet, peer-to-peer drag coordination   |
| Force Target window         | Dedicated UI for forced target management      |
| Runtime mode-switching      | `OnModeChange` callback hook                   |
| `burnnow` trigger           | Manual burn rotation activation + burn states  |
| Versioned ability sets      | Per-expansion ability definitions              |
| Coordinated `qsay`          | Random-delay group speech coordination         |
| lsqlite3 DB config          | SQLite-backed persistent configuration         |

**Action:** File "rgmercs depth gaps" issue (epic) with sub-issues per feature group.

---

## F6 — MQ2Boxr Interop Missing

TextQuest cannot be driven by `/bcaa //boxr Pause` from the RedGuides ecosystem.

| Gap                          | Description                                   |
| ---------------------------- | --------------------------------------------- |
| No `${TextQuest.Paused}` TLO | Can't query pause state from MQ2 macros       |
| No `/boxr` verb shim         | `/bcaa //boxr Pause/Resume/Status` do nothing |
| No `BoxrPause` event         | No pub/sub for state change listeners         |

**Action:** File dedicated issue for MQ2Boxr interop.

---

## F7 — Post-Session Self-Improvement Loop (New Category)

Record raw session events → derive aggregates → surface tuning suggestions in web UI with operator-in-the-loop accept/undo/promote-to-config. **Never auto-apply.**

Three-tier algorithm:

1. **Heuristics** — rule-based pattern detection
2. **Bayesian baseline** — probabilistic behavior modeling
3. **Multi-armed bandit** (optional, operator opt-in) — exploration/exploitation of config variants

Sub-issues: event schema, aggregation pipeline, heuristic engine, Bayesian baseline, web UI surface, accept/undo/promote-to-config, operator notifications, A/B variant management.

**Action:** File self-improvement loop epic + 8 sub-issues.

---

## Issue Tree

```
#2592 (umbrella)
├── MQ2Boxr interop — ${TextQuest.Paused} TLO + /boxr verb shim
├── Self-improvement loop [epic]
│   ├── Event schema + raw session recording
│   ├── Aggregation pipeline
│   ├── Heuristic engine (tier 1)
│   ├── Bayesian baseline (tier 2)
│   ├── Multi-armed bandit (tier 3, opt-in)
│   ├── Web UI: suggestion surface
│   ├── Accept/undo/promote-to-config
│   └── Operator notification system
├── rgmercs runtime contract (against #792)
├── rgmercs depth gaps [epic]
│   ├── Pull state machine modes
│   ├── Named lists + tempset + set_peer
│   ├── Config share (base64) + DB config
│   ├── Force Target window + runtime modes
│   ├── burnnow + burn rotation states
│   └── qsay + travel-spell detection
├── Cross-client comms transport pack (EQBC, NetBots, DanNet, NetMQ)
├── Auto-acceptance and safety pack (AutoAccept, Rez, AutoCamp, GMCheck, Paranoid)
├── Session tracking pack (XPTracker, KillTracker, PlatTracker)
├── Combat depth pack (Melee, BardSwap, XAssist, WorstHurt)
├── Alerts + events pack (Sound, Log, Timestamp, React, Say, Discord, TTS)
└── Lua + script ecosystem pack (aqobot, lem, lootnscoot, boxhud, luachase, luaconsole, buttonmaster, MyUI, AlertMaster, MyDPS, MyPaths, shareddata)
```

---

## Exit Criteria

Parity claim is defensible when:

- [ ] Every F2 plugin has an owning issue or explicit defer rationale
- [ ] MQ2Boxr verb contract works (`${TextQuest.Paused}`, `/boxr Pause/Resume/Status`)
- [ ] rgmercs runtime contract honored by plugin loader (#792)
- [ ] Self-improvement loop ships heuristic tier (tier 1)
- [ ] Web UI surfaces every operator-facing setting introduced

---

## Related Issues

| Issue       | Description                                                      |
| ----------- | ---------------------------------------------------------------- |
| #1688–#1692 | Prior feature pack parents (closed, no children)                 |
| #1693       | Combat/class parity parent (5 of 13 children filed: #1881–#1885) |
| #790        | Active rgmercs/Lua/MQ parity epic                                |
| #2430–#2461 | Active parity children under #790                                |
| #792        | Plugin loader                                                    |
| #791        | Lua VM                                                           |
| #793        | Hotkeys                                                          |
