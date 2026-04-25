# TextQuest ↔ OpenVanilla / RedGuides / rgmercs / MQ2Boxr — 2026-04-25 Audit

**Date**: 2026-04-25
**Branch**: `claude/audit-textquest-openvanilla-HPifL`
**Predecessors**: `docs/openvanilla-macroquest-coverage-gap-plan.md`, `docs/MQ2_COVERAGE_GAP_ANALYSIS.md`, `docs/RGMERCS_FEATURE_PARITY.md`, `docs/MACROQUEST_PLUGIN_SUPPORT.md`

## Why this audit

The 2026-04-14 plan opened parent gap issues `#1688`-`#1693` and produced a coverage matrix. State on 2026-04-25:

- `#1688`-`#1692` are **closed as completed without ever spawning the named per-plugin sub-issues** the plan said they should.
- `#1693` did spawn 5 children (`#1881`-`#1885`) and all 5 closed.
- Active rgmercs/Lua/MQ-plugin parity work has migrated under epic **#790** with ~17 open children (`#2430`-`#2461`).
- ~50 plugins from the original coverage matrix have **no named owning issue** today.

This audit re-confirms the gap, narrows the unowned set, and adds three new categories the prior plan did not cover:

1. **MQ2Boxr interoperability contract** — TextQuest must be a "Boxr-compatible target" so the broader RedGuides ecosystem can pause/chase/camp it identically to CWTN/RGMercs/KissAssist.
2. **rgmercs runtime contract** — rgmercs requires `MQ2Rez/MQ2AdvPath/MQ2MoveUtils/MQ2Nav/MQ2DanNet` loaded and force-unloads `MQ2Melee/MQ2Twist`. Coexistence requires this contract.
3. **Post-session self-improvement loop** — record session data, derive aggregates, surface tuning suggestions, operator-in-the-loop accept/undo. New epic.

## Sources reviewed

- OpenVanilla `.gitmodules` (`https://github.com/RedGuides/openvanilla`) — 54+ submodule plugins
- RedGuides org repos (`https://github.com/orgs/RedGuides/repositories`) — 85 repos, of which ~75 are plugins/macros/Lua
- MacroQuest core plugins (`https://github.com/macroquest/macroquest/tree/master/src/plugins`) — 15 in-tree plugins
- rgmercs main (`https://github.com/DerpleDude/rgmercs`) — 16 modules, 16 classes × 3 server profiles
- MQ2Boxr (`https://github.com/RedGuides/MQ2Boxr`) — `MQ2Boxr.cpp`, `boxr.cpp`, `boxr_type.cpp`
- TextQuest open issue corpus (search + `mcp__github__issue_read get_sub_issues`)

## Findings

### F1 — Sub-issue spawn never happened for `#1688`-`#1692`

The 2026-04-14 plan said these parents would each spawn named children, one per plugin. They closed without doing so. `#1693` is the only parent that produced children, and only for 5 of its 13 named plugins. Every other plugin named inside `#1691`/`#1692`/`#1693` body text remains unowned.

### F2 — ~50 plugins from the audit matrix have no named owner

Concrete unowned set (cross-checked against open + recently-closed issue titles and bodies):

**Cross-client communication transports**
`MQ2EQBC`, `MQ2NetBots`, `MQ2DanNet`, `MQ2NetMQ`

**Auto-acceptance and safety**
`MQ2AutoAccept`, `MQ2Rez`, `MQ2AutoCamp`, `MQ2GMCheck`, `MQ2Paranoid`, `MQ2Spawns`

**Session tracking**
`MQ2XPTracker`, `MQ2KillTracker`, `MQ2PlatTracker`

**Combat depth (non-rgmercs)**
`MQ2Melee`, `MQ2BardSwap`, `MQ2Headshot`, `MQ2XAssist`, `MQ2WorstHurt`

**Alerts and events**
`MQ2Sound`, `MQ2Log`, `MQ2Timestamp`, `MQ2React`, `MQ2ChatEvents`, `MQ2Say`, `MQ2Discord`, `MQTextToSpeech`

**Movement extensions**
`MQ2MoveUtils` (only stick/makecamp owned), `MQ2Relocate`, `MQ2Ice`, `MQ2AdvPath`

**Inventory utilities**
`MQ2Cursor`, `MQ2FakeLink`, `MQ2ItemScore`, `MQ2FeedMe`, `MQ2Debuffs`

**Spawn / target / awareness UI**
`MQ2OTD`, `MQ2SpawnSort`, `MQ2Posse`, `MQ2Status`, `MQ2Targets`, `MQ2ToolTip`, `MQ2GroupInfo`

**Group / raid management**
`MQ2AutoGroup`, `MQ2Rand`, `MQ2Rewards`, `raidhud`

**Economy and subscription**
`MQ2TributeManager`, `MQ2TSTrophy`, `MQ2AutoClaim`, `MQ2PortalSetter`

**Operator productivity / system**
`MQ2WinTitle`, `MQ2Boxr`, `MQ2PluginManager`, `MQ2MyButtons`, `MQ2Profiler`, `MQ2DamageMeter`, `MQ2HotButton`, `MQ2KeyBinds`, `MQ2HUDMove` (only #845/#857 reference, no owner)

**Lua / scripting ecosystem (RedGuides + community)**
`MQ2Lua` core, `aqobot`, `lem`, `lootnscoot`, `boxhud`, `luachase`, `luaconsole`, `buttonmaster`, `MyUI/MQGrimGUI`, `AlertMaster`, `MyDPS`, `MyPaths`, `shareddata`, `DerpleDude/CombatControl`, `EMUMeshes`

**Class-bot ecosystem**
`KissAssist`, `CWTN` (per-class set), `MuleAssist`, `MQ2Mercs`

### F3 — Plugins from OpenVanilla `.gitmodules` not in the prior audit doc

Newly inventoried this round:

- `MQ2AdvPath` — recorded path playback (rgmercs requires it)
- `MQ2DPSAdv` — rich DPS window
- `MQ2DamageMeter` — damage parsing
- `MQ2EasyFind` — Find Window enhancements (already #2454)
- `MQ2Tracking` — native-style tracking window (already #2459)
- `MQ2Debuffs` — detailed debuff TLO
- `MQTextToSpeech` — TTS
- `MQ2ShellCmd` — shell command execution
- `MQ2SQLite` — SQLite TLO
- `MQRemote` — remote command interface
- `MQ2KissTemplate` — KissAssist INI template generator
- `MQ2FarmTest` — farming helper
- `MQ2LootManager` — loot management
- `MQ2MeshManager` (wired420) — nav-mesh management

### F4 — rgmercs runtime contract gaps

rgmercs `init.lua` declares hard requirements at startup:

- **Required loaded**: MQ2Rez, MQ2AdvPath, MQ2MoveUtils, MQ2Nav, MQ2DanNet
- **Force-unloaded**: MQ2Melee, MQ2Twist (rgmercs handles melee + bard twist itself)
- **CWTN auto-paused**: rgmercs runs `/<class> pause on` at startup so it owns combat
- **MQ Actors mailbox**: used heavily for peer messaging (corpse drag, comms heartbeats)

TextQuest's plugin loader (`#792`) needs to honor a similar contract surface, or provide native equivalents for the required plugins. Without this, "run rgmercs alongside our DLL" cannot work.

### F5 — rgmercs depth gaps not captured by existing parity issues

Each of these is a real feature gap distinct from the named-plugin set:

| Capability | Where rgmercs has it | TextQuest gap |
|---|---|---|
| Per-server named lists | `namedlist/named_{common,default,eqmight,lazarus}.lua` | `#796` covers tracking, not server-aware lists |
| Pull state machine with Normal/Chain/Hunt/Farm modes | `modules/pull.lua` 11-state FSM | `#800` does not enumerate modes |
| Runtime non-persistent overrides | `tempset` / `cleartempset` / `cleartempall` | not modeled |
| Cross-client setting push | `set_peer` / `set_all` | not modeled |
| Config export/share base64 strings | `utils/rg_config_share.lua` | not modeled |
| In-game Lua editor + REPL | `modules/debug.lua` Zep editor | TextQuest has Lua, no editor |
| Travel-spell auto-detection + peer broadcast | `modules/travel.lua` | `#795` plans travel, not detection |
| Drag via 3 channels (DanNet / Actors / Spawn) | `modules/drag.lua` | `#797` does not enumerate channels |
| Force Target window replacing XTarget | `ui/target.lua` | not modeled |
| Mode switching at runtime with OnModeChange hooks | `class_configs/.../*Modes`, `OnModeChange` | TextQuest has class strategies, no runtime mode |
| `burnnow` manual burn trigger + dedicated burn rotation states | `Casting.BurnCheck` + `burnnow` | not modeled |
| Versioned ability sets (pick highest known) | `AbilitySets` per class config | TextQuest classes are static |
| Coordinated `/say` with random delay | `qsay` / `say` / `rsay` | not modeled (anti-detection use case) |
| DB-backed config (lsqlite3) with migrations | `utils/config_db.lua` | TextQuest uses TOML |

### F6 — MQ2Boxr interop is genuinely missing

Current state: `textquest-common/src/box_controller.rs` exists but does not implement the Boxr verb contract. There is no `${TextQuest.Paused}` TLO. Operators cannot drive TextQuest from a CWTN/RGMercs/KissAssist-style `/bcaa //boxr Pause` broadcast.

What is required to be a Boxr-compatible target:

1. Register an in-game slash command (`/textquest box` or alias `/boxr`) that maps the verbs `Pause | Unpause | Camp | Chase | Manual | BurnNow | RaidAssistNum N | Debug | Help` to local-only TextQuest IPC actions (Boxr never broadcasts; the operator does).
2. Expose a `${TextQuest.Paused}` (and ideally `${TextQuest.Mode}`, `${TextQuest.RaidAssistNum}`) TLO via the existing MQ2 plugin in `textquest-dll`.
3. Upstream a `TextQuestControl : BoxControl` subclass into MQ2Boxr (or accept the local shim approach for now).
4. Honor 300 ms multi-step pacing if we ever emit MQ commands.
5. Bard quirk: replicate Boxr's `/twist off` on Pause.

### F7 — Self-improvement loop is a new conceptual area

Goal: after every play session, record raw events to local storage, derive aggregates on session close, surface tuning suggestions in the web UI with operator-in-the-loop accept / undo / promote-to-config. **Never auto-apply**.

Three-tier algorithm:

1. **Heuristics** (default) — hard rules over aggregates ("if mana p10 < 25% across last 5 sessions, suggest +2s pull cadence")
2. **Bayesian baseline** (Beta-Binomial / Gaussian conjugate priors) for threshold knobs — posterior mean ± 1σ
3. **Multi-armed bandit** (optional, opt-in) for genuinely A/B-testable knobs across sessions

Explicitly out of scope: neural nets, RL self-play, GPU dependency. Soul Engine LLM advice complements at the narrative layer ("you died 4 times in PoFire") but is never numeric truth.

Concrete tuning targets:

- Camp aggro radius
- Pull cadence (sec)
- Med-break mana %
- Retreat HP %
- Combat ability priority order
- Heal triggers
- Route waypoint culling / addition

Storage: SQLite for aggregates + append-only JSONL for raw events; 14-day raw retention, 1-year aggregate retention; local-only by default with opt-in salted-hash cloud sync.

## Recommended issue tree

A new issue tree is filed under this audit. The tree is intentionally smaller than 50 plugin-named issues — it bundles the closely-related plugins by capability domain so each plugin gets named exactly once without flooding the tracker.

| Issue | Scope |
|---|---|
| Audit-result umbrella | Lists every unowned plugin from F2-F3, links to evidence. |
| MQ2Boxr interop | F6: verb contract, TLO, upstream PR. |
| Self-improvement loop epic | F7 parent. |
| Self-improvement sub-issues × 8 | Schema, recorder, aggregator, heuristic engine, Bayesian tier, web UI panel, opt-in auto-promote, Soul narrative integration. |
| rgmercs runtime contract | F4: required-load + force-unload semantics in `#792`. |
| rgmercs depth gaps | F5: pull modes, named lists, tempset, set_peer, config share, force target, runtime modes, burnnow, ability sets, qsay, DB config. |
| Cross-client comms transport pack | F2 group: EQBC, NetBots, DanNet, NetMQ. |
| Auto-acceptance and safety pack | F2 group: AutoAccept, Rez, AutoCamp, GMCheck, Paranoid. |
| Session tracking pack | F2 group: XPTracker, KillTracker, PlatTracker. |
| Combat depth pack (non-rgmercs) | F2 group: Melee, BardSwap, XAssist, WorstHurt. |
| Alerts + events pack | F2 group: Sound, Log, Timestamp, React, Say, Discord, TextToSpeech. |
| Lua + script ecosystem pack | F2 group: aqobot, lem, lootnscoot, boxhud, luachase, luaconsole, buttonmaster, MyUI, AlertMaster, MyDPS, MyPaths, shareddata. |

## Exit criteria for "parity claim is defensible"

TextQuest can claim parity when:

1. Every unowned plugin from F2 has either an owning issue or an explicit defer rationale.
2. The MQ2Boxr verb contract is implemented and `${TextQuest.Paused}` is queryable from any other MQ-aware tool.
3. The rgmercs runtime contract is honored: TextQuest can run with rgmercs loaded without conflict, or with rgmercs unloaded and TextQuest providing native equivalents.
4. The self-improvement loop ships at least the heuristic tier with operator-in-the-loop UX.
5. The web UI has configuration surfaces for every operator-facing setting introduced by the above.
