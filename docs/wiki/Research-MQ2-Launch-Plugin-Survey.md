# MQ2 Launch Plugin Survey

> Research date: 2026-04-15
> Tracking issue: [#1578](https://github.com/Maleick/TextQuest/issues/1578)

Primary source anchors:

- [MQ2Bzsrch wiki](https://www.redguides.com/wiki/MQ2Bzsrch)
- [MQ2Bzsrch source (`macroquest/macroquest`)](https://github.com/macroquest/macroquest/blob/4256855534bf7e58beaed3d20541c870cf1ad2b3/src/plugins/bzsrch/MQ2Bzsrch.cpp)
- [Bazaar.mac resource](https://www.redguides.com/community/resources/bazaar-mac.23/)
- [RedGuides plugin catalog](https://www.redguides.com/docs/plugins/)
- [RGMercs Lua Edition resource](https://www.redguides.com/community/resources/rgmercs-lua-edition.3040/)
- [KissAssist docs](https://www.redguides.com/docs/projects/kissassist/)
- [Epic Laziness resource](https://www.redguides.com/community/resources/epic-laziness.2372/)
- [EpicReq.mac thread](https://www.redguides.com/community/threads/epicreq-mac-an-epic-ornament-achievement-macro-mq2-legacy-only.72925/)

Repo-fit anchors:

- [Architecture Overview](Architecture-Overview)
- [DLL Injection and IPC Pipeline](DLL-Injection-and-IPC-Pipeline)
- [MQ2 Comparison](Research-MQ2-Comparison)
- [RGMercs Analysis](Research-RGMercs-Analysis)

## Goal

Catalog the MQ2 / RedGuides plugin areas that matter for pre-launch readiness and decide whether TextQuest should:

1. adapt the external behavior into native DLL or orchestrator code,
2. treat the external tool as a workflow reference only, or
3. defer the area because the launch payoff is too low.

The key constraint is architectural: TextQuest does not host the MacroQuest plugin API or macro interpreter. The supported execution path is authenticated IPC from `textquest` into the injected `textquest-dll`, followed by in-process EQ reads, UI interaction, and slash-command dispatch. That means "adaptation" here means reusing behavior, data shapes, and operator workflows, not loading MQ2 plugins directly.

## Adaptation Rubric

| Classification | Meaning in TextQuest |
| --- | --- |
| Native adaptation | Reuse the behavior or data model, but implement it in Rust inside the current DLL or orchestrator architecture. |
| Workflow reference | Use the external macro or plugin as an operator benchmark, config example, or scenario reference. Do not attempt a direct port. |
| Defer | Known useful capability, but not worth launch-window implementation compared with higher-value work already tracked. |

## Priority Survey Matrix

| Target | External behavior | Current TextQuest fit | Best path | Launch priority | Tracking |
| --- | --- | --- | --- | --- | --- |
| `MQ2Bzsrch` | Bazaar search plugin with a result hook, type data, and command-driven query flow | TextQuest has DLL injection, in-process reads, and operator-side economy controls, but no bazaar result capture yet | Native adaptation of the data capture only; do not port the search-trigger behavior | High | [#1584](https://github.com/Maleick/TextQuest/issues/1584), [#1581](https://github.com/Maleick/TextQuest/issues/1581) |
| `Bazaar.mac` and related price-log macros | Macro-driven repricing, trader or buyer updates, and CSV or INI history export | TextQuest has vendor-cycle logic and economy UI, but no macro runtime and no trader repricing workflow | Workflow reference only; reuse the logging and analytics ideas after bazaar capture exists | Medium | [#1584](https://github.com/Maleick/TextQuest/issues/1584), [#1581](https://github.com/Maleick/TextQuest/issues/1581), [#899](https://github.com/Maleick/TextQuest/issues/899) |
| Epic quest automation plugins and macros | Quest-specific scripted movement, dialog, hand-ins, and item checks | TextQuest can drive navigation, slash commands, and dialog interaction, but it does not yet have a reusable quest-scenario layer | New native quest-scenario work, but only after selecting a narrow first quest target | Medium | [#1752](https://github.com/Maleick/TextQuest/issues/1752) |
| Leveling and combat macro stack (`KissAssist`, `RGMercs`, `MuleAssist`) | Pull routines, assist flow, camp management, heal or buff priorities, and multibox control surfaces | TextQuest already has combat, pull, camp, nav, and IPC structure; major gaps are config density and operator visibility, not core direction | Native adaptation of control semantics and state visibility; do not port the macro language | Highest | [#790](https://github.com/Maleick/TextQuest/issues/790), [#800](https://github.com/Maleick/TextQuest/issues/800), [#1060](https://github.com/Maleick/TextQuest/issues/1060)-[#1066](https://github.com/Maleick/TextQuest/issues/1066), [#1633](https://github.com/Maleick/TextQuest/issues/1633) |
| Charm management | Charm-break detection, automatic re-charm, mez or stun fallback, and failure budgets | TextQuest already has charm-break detection and crowd-control scaffolding in the codebase, but no automatic re-charm loop yet | Native adaptation on top of the existing CC tracker, with external tools used only as safety-pattern references | High | [#1582](https://github.com/Maleick/TextQuest/issues/1582), [#799](https://github.com/Maleick/TextQuest/issues/799), [#1049](https://github.com/Maleick/TextQuest/issues/1049)-[#1056](https://github.com/Maleick/TextQuest/issues/1056) |

## Findings by Plugin Area

### 1. `MQ2Bzsrch`

What matters:

- The upstream plugin is source-available and still ships in the `macroquest/macroquest` tree.
- Its useful part for TextQuest is not the `/bzsrch` command surface. It is the data path.
- The plugin detours `CBazaarSearchWnd::HandleSearchResults`, reads the serialized result buffer into `BazaarSearchItem`, and then enriches each item with trader names from `pBazaarSearchWnd->Traders`.

What does not fit directly:

- The plugin depends on the MQ2 plugin runtime, MQ2 type system, and MQ2 window helpers.
- It actively manipulates bazaar UI widgets such as `BZR_QueryButton`, `BZR_ItemNameInput`, and class or race combo boxes to construct queries.
- That active query pattern is the opposite of the low-noise approach already captured in [#1584](https://github.com/Maleick/TextQuest/issues/1584): let a human perform one normal bazaar search, then read the results passively from memory.

Recommendation:

- Treat `MQ2Bzsrch` as a structural reference for result parsing and window layout assumptions.
- Implement the TextQuest version as a passive DLL reader and expose the data over the existing shared-memory plus IPC path.
- Keep any future operator surface in the TUI or web UI, not as an MQ2-style command parser.

### 2. `Bazaar.mac`

What matters:

- The RedGuides resource describes the macro as an automated bazaar updater that adjusts trader or buyer prices and can export CSV price logs.
- That proves the operational value of two separate capabilities:
  - rapid price intelligence and history capture,
  - trader repricing automation.

What does not fit directly:

- This is macro-layer automation, not a reusable low-level bazaar data engine.
- It assumes MQ2 macro execution, direct bazaar-window driving, and trader-specific flows that TextQuest does not expose today.
- Jeff's note in [#1584](https://github.com/Maleick/TextQuest/issues/1584) is directionally right: bazaar value is highest during the early inflation window, while passive chat and market monitoring likely outlive direct trader repricing as a launch concern.

Recommendation:

- Reuse the operator workflow ideas, especially price-history export and comparison reporting.
- Do not port the macro.
- Sequence the work behind passive bazaar capture and durable chat-based price monitoring.

### 3. Epic quest automation

What matters:

- There is a real ecosystem here, not just one abandoned macro:
  - `Epic Laziness` is a public quest-automation resource with a Lua repo layout (`init.lua`, `lib/`, `utils/`, and quest data files).
  - `EpicReq.mac` shows a parallel macro lineage for epic ornament and progression chores.
- These tools are valuable because they encode quest order, item checks, and movement or dialog expectations that are easy to forget during pre-launch prep.

What does not fit directly:

- The automation is quest-specific and brittle by nature. Quest steps hard-code NPC names, hand-ins, dialog prompts, and location assumptions.
- TextQuest does not yet have a generalized quest-scenario execution layer for "move here, hand this item to that NPC, wait for this dialog, then continue."
- Existing closed research items such as [#1585](https://github.com/Maleick/TextQuest/issues/1585) cover quest value and random-loot implications, not a reusable automation adapter.

Recommendation:

- Do not build a generic "epic plugin host."
- Open a focused follow-up issue to define a quest-scenario adapter and select the first quest worth porting.
- Use the external scripts as runbooks and scenario fixtures, not as code to embed.

### 4. Leveling and combat macros

What matters:

- `KissAssist`, `RGMercs`, and `MuleAssist` already solve the operator problem TextQuest cares about most before launch: stable assist, pulling, camp control, and recoverable group automation.
- The highest-value reusable content is not the literal macro logic. It is:
  - operator command semantics,
  - pull and camp state visibility,
  - configurable role and threshold surfaces,
  - "why am I waiting?" explanations for combat and camp loops.

Current TextQuest position:

- TextQuest already has the right architectural bones for this family:
  - combat and camp-loop orchestration,
  - puller FSMs and route planning,
  - authenticated multi-client IPC,
  - a native TUI and a planned web UI.
- The remaining gap is translation quality, not total absence. The repo already tracks the important slices in [#790](https://github.com/Maleick/TextQuest/issues/790), [#800](https://github.com/Maleick/TextQuest/issues/800), [#1060](https://github.com/Maleick/TextQuest/issues/1060)-[#1066](https://github.com/Maleick/TextQuest/issues/1066), and [#1633](https://github.com/Maleick/TextQuest/issues/1633).

Recommendation:

- Continue adapting the workflow model into native TextQuest controls and state reporting.
- Do not spend time building compatibility with the MQ2 macro language itself.
- This is the most launch-relevant area because it directly governs leveling throughput, wipe avoidance, and operator attention load.

### 5. Charm management

What matters:

- External tools clearly treat charm as a first-class automation concern. Public discussion and documentation cover charm-capable configurations, charm-safe control flow, and break-handling expectations.
- TextQuest is already partway there. The repo contains charm-break detection and crowd-control response scaffolding, but the actual automatic re-charm loop is still missing.

What fits well:

- This is a clean native adaptation candidate because the existing TextQuest control boundary is already in-process and stateful.
- The external material mainly contributes safety patterns:
  - detect the break quickly,
  - interrupt or stun if needed,
  - retry charm with guardrails,
  - fall back to kill mode if the loop fails.

Recommendation:

- Keep the implementation in the DLL and camp or CC state machine.
- Use external charm workflows only to validate retry limits, fallback rules, and operator override expectations.
- [#1582](https://github.com/Maleick/TextQuest/issues/1582) is the right immediate implementation issue.

## Launch Ordering

1. Leveling and combat workflow parity
   - Highest immediate payoff because it affects the main pre-launch leveling loop every session.
   - Most of the work is already tracked; the survey mainly confirms those issues are the right launch focus.

2. Charm automation
   - High payoff for charm-based leveling groups and low marginal architecture risk because the CC foundation already exists.

3. Passive bazaar intelligence
   - Useful for the early economy window, but the safest value is in passive capture and later reporting rather than active query spam or repricing automation.

4. Epic quest automation
   - Valuable, but too quest-specific to outrank combat, pull, charm, or bazaar intelligence for general launch readiness.

5. Direct `Bazaar.mac`-style repricing automation
   - Defer until passive bazaar capture, logging, and trader workflows prove worth the operator complexity.

## Final Recommendation

The launch-ready path is selective borrowing:

- borrow `MQ2Bzsrch` data structures and result-shape assumptions,
- borrow `Bazaar.mac` logging ideas,
- borrow `KissAssist` / `RGMercs` operator semantics,
- borrow charm safety patterns,
- but keep every implementation native to TextQuest's DLL plus orchestrator architecture.

The only clear follow-up gap that was not already tracked before this survey was epic-quest automation as a reusable TextQuest scenario adapter. That gap is now tracked in [#1752](https://github.com/Maleick/TextQuest/issues/1752). Everything else in the issue's requested research set already maps cleanly onto active TextQuest issues.
