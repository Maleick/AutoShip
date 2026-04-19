# MacroQuest / OpenVanilla Coverage Gap Audit And Implementation Plan

Date: 2026-04-14
Branch: autoship/issue-1510

## Objective

Establish a defensible one-to-one minimum parity plan against the current MacroQuest core plugin surface, the RedGuides OpenVanilla fork, and the RedGuides extension repository set.

The goal is not to claim broad parity by analogy. The goal is explicit coverage: every relevant plugin or extension must be traceable to one of these states:

- natively complete in TextQuest
- partially complete in TextQuest
- owned by an existing implementation issue
- owned by a new gap issue
- intentionally deferred with a documented rationale

Operator-facing behavior that matters in practice must also be configurable through the local web UI.

## Sources Reviewed

External inventories reviewed during this audit:

- MacroQuest core plugin surface: 15 core plugin directories under `src/plugins`
- OpenVanilla fork inventory: 54 plugin submodules from `.gitmodules`
- RedGuides extension inventory: 75 `MQ2*` or related extension repositories visible from the RedGuides org repo listing

Repo-local planning and evidence reviewed during this audit:

- `docs/implementation-roadmap.md`
- `docs/RGMERCS_FEATURE_PARITY.md`
- `docs/MACROQUEST_PLUGIN_SUPPORT.md`
- `docs/wiki/Research-MQ2-Parity-Matrix.md`
- `docs/wiki/Research-MQ2-Comparison.md`
- `docs/wiki/Research-KissAssist-Gap-Analysis.md`
- `feature-list.json`
- `AUDIT_WORK_GAPS_REPORT.md`
- open issues and parity indexes including `#790`, `#819`-`#860`, `#1102`, `#1177`, `#1561`, `#1578`

## Current Coverage Position

### Already complete or materially implemented

The canonical roadmap still marks `M1` through `M6` complete. That means TextQuest already has a large native base that does not need to be reinvented:

- external memory reading and TUI control plane
- DLL injection and typed IPC
- login automation and launch coordination
- navigation, stick, map, and route handling
- combat rotation, class strategies, puller logic, and camp loop structure
- web dashboard foundation, even if parts of it remain placeholder or demo-only

The repo also already contains completed or previously tracked parity work around several MacroQuest capability families, including:

- MQ2Nav and MQ2Map command surface work
- MQ2Cast command and telemetry work
- bard medley or twist support
- spawn tracking and map overlays
- EQBC-style communication as an open parent issue
- OpenVanilla parity epics for plugins, Lua, group coordination, loot, economy, alerts, and stability

### Already planned in existing issue trees

The existing parity trees are broader than the current docs alone suggest. The repo already has meaningful planned coverage for:

- plugin architecture and Lua support: `#819`, `#820`
- EQBC-style communication: `#822`
- group and raid coordination: `#836`, `#838`
- tell relaying and alerting: `#851`, `#859`
- loot, banking, vendor, and economy work: `#824`, `#825`, `#826`, `#830`, `#848`, `#849`, `#858`
- UI and overlay expansion: `#845`, `#857`
- performance and metrics: `#802`, `#840`

### Why coverage was still insufficient

The remaining problem is traceability and operator usability.

The current plan mostly tracks capability families. The external references, especially OpenVanilla and RedGuides, are plugin catalogs. A one-to-one claim is weak unless every relevant plugin is explicitly classified.

This audit found that many plugin names from OpenVanilla and RedGuides were not explicitly represented anywhere in the issue corpus, even when adjacent generic capability issues already existed. That means the repo had partial planning coverage but not full plugin-level coverage accounting.

The second major gap is operator configuration. The current dashboard has real foundations, but parity-critical config surfaces for imported or adapted RedGuides behavior are still missing or placeholder-driven. That breaks the requirement that these features be configurable in the local web UI.

## Gaps Identified

### 1. No canonical plugin-by-plugin traceability layer

The repo has parity docs and issue groups, but no single source that maps every MacroQuest core plugin, OpenVanilla submodule plugin, and RedGuides extension repo to a current TextQuest state.

Impact:

- impossible to prove one-to-one minimum parity
- easy to miss long-tail utility plugins
- hard to decide whether a missing plugin needs a native feature, an adapter, or an explicit defer note

New issue opened: `#1688`

### 2. No migration path from legacy MQ2, KissAssist, CWTN, or OpenVanilla configs into TextQuest-native profiles

The repo already plans plugin and Lua support, but it does not yet own the config translation problem. Existing operators will need their established settings imported, normalized, and preserved.

Impact:

- high migration cost for real users
- dashboard parity impossible without a normalized settings model
- repeated manual re-entry work for behavior and thresholds

New issue opened: `#1689`

### 3. No first-class extension catalog and settings surface in the local web UI

The dashboard has useful foundations, but plugin or extension settings are not yet modeled as persisted, schema-driven, runtime-aware configuration. Some backend APIs remain placeholder or demo-only, which is below the required parity bar.

Impact:

- no operator-facing configuration parity for RedGuides and OpenVanilla extensions
- imported legacy settings cannot become durable dashboard-managed configuration
- no way to show compatibility tier, source provenance, or adapter health in one place

New issue opened: `#1690`

### 4. Awareness and coordination utility surface still lacks explicit ownership

Several RedGuides utility plugins are not fully captured by the existing group, raid, and alert issues when measured plugin-by-plugin. This includes status, target and spawn visibility, assist helpers, proximity utilities, and alerting helpers.

Representative plugin set:

- `MQ2Status`
- `MQ2Targets`
- `MQ2Spawns`
- `MQ2SpawnSort`
- `MQ2Tracking`
- `MQ2ToolTip`
- `MQ2OTD`
- `MQ2Posse`
- `MQ2WorstHurt`
- `MQ2GroupInfo`
- `MQ2XAssist`
- `MQ2Paranoid`
- `MQ2Say`

New issue opened: `#1691`

### 5. Item knowledge and inventory utility surface still lacks explicit ownership

The repo has economy and loot issues, but the long-tail utility set around item knowledge, rewards, collections, tribute, cursor policies, and relocation is not explicitly owned in one place.

Representative plugin set:

- `MQ2LinkDB`
- `MQ2ItemScore`
- `MQ2Cursor`
- `MQ2Collections`
- `MQ2Collectible`
- `MQ2TributeManager`
- `MQ2TSTrophy`
- `MQ2Rewards`
- `MQ2FeedMe`
- `MQ2PortalSetter`
- `MQ2Relocate`
- `MQ2Vendors`
- `MQ2AutoClaim`

New issue opened: `#1692`

### 6. Operator productivity and system utility surface still lacks explicit ownership

The repo has hotkeys, overlay, metrics, and dashboard work, but the remaining QoL and system utilities are still not represented explicitly enough for one-to-one coverage.

Representative plugin set:

- `MQ2Clipboard`
- `MQ2Notepad`
- `MQ2PluginManager`
- `MQ2MyButtons`
- `MQ2HUDMove`
- `MQ2WinTitle`
- `MQ2CPULoad`
- `MQ2AutoSize`
- `MQItemColor`
- `MQ2Log`
- `MQ2Profiler`
- `MQ2DamageMeter`
- `MQ2Camera`

New issue opened: `#1693`

## Newly Opened Issues

This audit created the following gap issues:

- `#1688` Audit: OpenVanilla and RedGuides plugin coverage matrix with compatibility tiers
- `#1689` Feature: Import MQ2, KissAssist, CWTN, and OpenVanilla configs into normalized TextQuest profiles
- `#1690` Feature: Web Dashboard extension catalog, settings editor, and runtime control for RedGuides parity
- `#1691` Feature: Awareness and coordination utility parity pack for RedGuides extensions
- `#1692` Feature: Item knowledge and inventory utility parity pack for RedGuides extensions
- `#1693` Feature: Operator productivity and system utility parity pack for RedGuides extensions

## Recommended Execution Order

### Phase 0: Traceability first

Start with `#1688`.

Deliverables:

- a plugin-by-plugin matrix
- a machine-readable manifest
- explicit classification of every plugin in scope
- links from each plugin to either code evidence, an existing issue, or one of the new gap issues

Why this comes first:

- it prevents duplicate implementation work
- it turns vague parity claims into an auditable checklist
- it determines which long-tail plugins really need native features versus simple adapters or UI surfaces

### Phase 1: Normalize imported settings

Run `#1689` immediately after the first usable pass of `#1688`.

Deliverables:

- normalized TextQuest schema for imported legacy settings
- import adapters for the highest-value legacy formats
- unsupported-field reporting that can feed later implementation decisions

Why this comes second:

- the dashboard work should operate on normalized config, not raw legacy files
- imported profiles are the bridge between parity research and actual operator adoption

### Phase 2: Make extension parity operator-controllable

Run `#1690` once the schema from `#1689` is stable enough to support persisted editing.

Deliverables:

- real backend persistence for extension settings
- per-scope editing in the local web UI
- runtime status, compatibility tier, and adapter health visibility

Why this comes before long-tail plugin packs:

- the user requirement explicitly says the extension set should be configurable in the local web UI
- the long-tail plugin packs should plug into a common configuration surface, not each build its own

### Phase 3: Close the uncovered utility packs

Once traceability and config surfaces exist, implement the three new plugin packs in this order:

1. `#1691` awareness and coordination utilities
2. `#1692` item knowledge and inventory utilities
3. `#1693` operator productivity and system utilities

Rationale:

- awareness and coordination utilities affect live operator control and safety first
- inventory and item knowledge utilities are high-value but can follow once shared config surfaces exist
- operator productivity utilities are important, but many can piggyback on dashboard, metrics, hotkey, and logging work already in flight

### Phase 4: Fold results back into the existing parity tree

After the first pass of `#1688` through `#1693`, update the existing parity parents and indexes:

- `#1177` should reference the new gap issues explicitly
- `#1578` should narrow from a broad survey into concrete plugin classification tasks
- `#1561` should absorb the extension settings surfaces now called out in `#1690`
- domain parents like `#836`, `#838`, `#824`, `#825`, `#830`, `#845`, and `#857` should absorb sub-issues created from the matrix

## Minimum Exit Criteria For Parity Claims

TextQuest should not claim one-to-one minimum parity until all of the following are true:

- every relevant MacroQuest core plugin, OpenVanilla submodule plugin, and RedGuides extension repo is classified in the canonical matrix
- every plugin has code evidence, an owning issue, or an explicit defer rationale
- every operator-facing setting that materially changes behavior is configurable in the local web UI
- legacy configs can be imported into normalized TextQuest profiles with unsupported fields clearly reported
- the existing parity indexes are updated to include the newly opened gap issues and their follow-on decomposition

## Practical Next Step

The best immediate move is to execute `#1688` first and use it as the gate for all later parity claims. Without that explicit matrix, implementation can continue, but coverage cannot be proven.