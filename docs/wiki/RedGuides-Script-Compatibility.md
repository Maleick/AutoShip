# RedGuides Script Compatibility Matrix

TextQuest compatibility inventory for the RedGuides Lua-script ecosystem.
Tracks which community scripts are supported, at what tier, and what their
current status is under TextQuest's embedded Lua 5.4 VM (mlua 0.11).

Generated: 2026-04-25 | Issue: #2617

---

## Tier Definitions

| Tier                     | Commitment                                                                       |
| ------------------------ | -------------------------------------------------------------------------------- |
| **Tier 1 — must run**    | TextQuest commits to running unmodified. Any incompatibility is a TextQuest bug. |
| **Tier 2 — should run**  | TextQuest aims for compatibility. Failures filed as follow-up gaps.              |
| **Tier 3 — best-effort** | Tested manually; documented as compatible or incompatible.                       |

---

## Compatibility Matrix

| Script               | Author      | Purpose                                | Tier          | Dependencies                    | TextQuest Status | Notes                                                                                             |
| -------------------- | ----------- | -------------------------------------- | ------------- | ------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------- |
| **aqobot**           | aquietone   | rgmercs-alternative class automation   | Tier 1        | Lua VM #791, plugin loader #792 | Pending VM       | Competes with rgmercs; must be usable as drop-in alternative                                      |
| **lem**              | aquietone   | WeakAuras-style event/condition engine | Tier 1        | Lua VM #791                     | Pending VM       | Event system must expose `OnEvent` hook from TextQuest bindings                                   |
| **lootnscoot**       | aquietone   | Loot utility                           | Tier 1        | Lua VM #791, inventory TLO      | Pending VM       | Already integrated by rgmercs; first-class support required                                       |
| **boxhud**           | aquietone   | DanNet-based box HUD (ImGui)           | Tier 1        | DanNet #2613                    | Blocked          | Blocked on DanNet interop; unblocked once #2613 ships                                             |
| **luaconsole**       | aquietone   | In-game Lua REPL                       | Tier 1        | Lua VM #791                     | Pending VM       | Alternative to rgmercs Zep editor (#2611 gap 6); `execute_string()` path must work                |
| **AlertMaster**      | grimmier378 | Alert manager (named-mob integration)  | Tier 1        | Lua VM #791                     | Pending VM       | rgmercs Named module integrates with this; required for raid workflow                             |
| **shareddata**       | aquietone   | Actor-based SharedData TLO             | Tier 1        | Actors mailbox #2607            | Blocked          | Blocked on Actors interop; unblocked once #2607 ships                                             |
| **luachase**         | aquietone   | MQ2Nav-based chase utility             | Tier 2        | Lua VM #791, Nav stack          | Pending VM       | Nav API (`tq.nav.*`) must expose `chase_target()` equivalent                                      |
| **buttonmaster**     | DerpleDude  | Maintained Buttonmaster fork           | Tier 2        | Lua VM #791                     | Pending VM       | Requires ImGui bindings for button rendering                                                      |
| **MyUI / MQGrimGUI** | grimmier378 | UI bundle (ImGui)                      | Tier 2        | Lua VM #791, ImGui              | Pending VM       | Heavy ImGui usage; tracked separately from class widgets                                          |
| **CombatControl**    | DerpleDude  | AI combat controller                   | Tier 2        | Lua VM #791, combat API         | Pending VM       | Requires spell-cast and target APIs                                                               |
| **MyDPS**            | grimmier378 | DPS meter widget                       | Tier 3        | Lua VM #791, ImGui              | Untested         | Parse `/log` or combat events                                                                     |
| **MyPaths**          | grimmier378 | Path recording widget                  | Tier 3        | Lua VM #791, Nav                | Untested         | Requires nav coordinate API                                                                       |
| **MyChat**           | grimmier378 | Chat UI widget                         | Tier 3        | Lua VM #791, ImGui              | Untested         |                                                                                                   |
| **MyPet**            | grimmier378 | Pet control widget                     | Tier 3        | Lua VM #791                     | Untested         |                                                                                                   |
| **MyGroup**          | grimmier378 | Group management widget                | Tier 3        | Lua VM #791                     | Untested         |                                                                                                   |
| **MyBuffs**          | grimmier378 | Buff tracker widget                    | Tier 3        | Lua VM #791                     | Untested         |                                                                                                   |
| **EMUMeshes**        | DerpleDude  | Custom EMU navmesh data                | Tier 1 (data) | Nav loader #M3                  | Supported        | No script; mesh files drop into `data/navmesh/`. See [EMUMeshes Ingestion](#emumeshes-ingestion). |

---

## Tier 1 Script Details

### aqobot

- **Repo**: `aquietone/aqobot` (RedGuides)
- **Entry point**: `init.lua` → registers class modules, enters event loop via `mq.delay`
- **Required TextQuest APIs**: `tq.me.*`, `tq.spell.*`, `tq.target.*`, `tq.group.*`, `tq.event.*`
- **Conformance gate**: script must `require('aqobot')` without error and reach the `mq.event_loop()` call

### lem (Lua Event Manager)

- **Repo**: `aquietone/lem` (RedGuides)
- **Entry point**: `lem.lua` → exports `lem.register`, `lem.run`
- **Required TextQuest APIs**: `mq.event`, `mq.delay`, `mq.bind`
- **Conformance gate**: `require('lem')` succeeds; `lem.register` is callable

### lootnscoot

- **Repo**: `aquietone/lootnscoot` (RedGuides)
- **Entry point**: `lootnscoot.lua` → loot table config, `lootnscoot.init()`
- **Required TextQuest APIs**: `tq.loot.*`, `tq.me.inventory`, `tq.event`
- **Conformance gate**: `require('lootnscoot')` succeeds and `LootHelper` table is defined

### boxhud

- **Repo**: `aquietone/boxhud` (RedGuides)
- **Entry point**: `boxhud.lua` → DanNet peer queries, ImGui render loop
- **Required TextQuest APIs**: `tq.dannet.*` (blocked on #2613), ImGui
- **Conformance gate**: blocked until DanNet #2613 ships

### luaconsole

- **Repo**: `aquietone/luaconsole` (RedGuides)
- **Entry point**: `luaconsole.lua` → ImGui window, `loadstring` / `pcall` eval loop
- **Required TextQuest APIs**: `mq.imgui`, Lua `loadstring`, `tq.execute_string()`
- **Conformance gate**: module loads; `luaconsole.open()` registers ImGui callback

### AlertMaster

- **Repo**: `grimmier378/AlertMaster` (RedGuides)
- **Entry point**: `alertmaster.lua` → named-spawn alert config, `mq.event` registration
- **Required TextQuest APIs**: `tq.event.*`, `tq.spawn.*`, `mq.event`
- **Conformance gate**: `require('alertmaster')` succeeds; alert table is non-empty

### shareddata

- **Repo**: `aquietone/shareddata` (RedGuides)
- **Entry point**: `shareddata.lua` → Actor-based inter-client TLO
- **Required TextQuest APIs**: `tq.actors.*` (blocked on #2607)
- **Conformance gate**: blocked until Actors mailbox #2607 ships

---

## EMUMeshes Ingestion

**EMUMeshes** provides custom navmesh data files for EMU (private-server) zones
that are not covered by the default MQ2Nav mesh set.

### Format

EMUMeshes distributes `.nav` binary files (Recast/Detour format, same as
MQ2Nav). TextQuest's navmesh loader reads this format natively — no conversion
step is needed.

### Drop-in Path

```
data/navmesh/<zone_short_name>.nav
```

TextQuest's nav loader (`textquest-dll/src/nav/mod.rs`) checks this path before
falling back to the bundled mesh cache. Operator workflow:

1. Download the EMUMeshes archive for your server
2. Extract `.nav` files into `data/navmesh/`
3. Restart the client — the loader auto-detects the override meshes on zone load

### Verification

```lua
-- In-game: confirm a custom mesh loaded
/lua run inline
local nav = tq.nav
print("mesh loaded:", nav.mesh_loaded())
print("zone:", nav.current_zone())
```

---

## Dependency Map

```
TextQuest Lua VM (#791)
├── aqobot          → player, spell, target, group, event APIs
├── lem             → event, delay, bind APIs
├── lootnscoot      → loot, inventory APIs
├── luaconsole      → imgui, execute_string
├── AlertMaster     → event, spawn APIs
├── boxhud          → DanNet #2613 (blocked)
└── shareddata      → Actors #2607 (blocked)

TextQuest Nav Loader (#M3)
└── EMUMeshes       → .nav file drop-in (no deps)
```

---

## Related Issues

| Issue | Topic                                  |
| ----- | -------------------------------------- |
| #791  | Lua VM implementation                  |
| #792  | Plugin loader                          |
| #790  | Parent: Lua/script ecosystem           |
| #2592 | Audit                                  |
| #2607 | Actors mailbox (shareddata dependency) |
| #2611 | rgmercs gaps (luaconsole alternative)  |
| #2613 | DanNet interop (boxhud dependency)     |

---

## Updating This Document

When a conformance test passes or a dependency ships:

1. Update the **TextQuest Status** column: `Pending VM` → `Passing` (or `Failing — see #NNNN`)
2. File a sub-issue for any Tier 1 failure, linking back to #2617
3. Re-run `tests/test_lua_script_conformance.py` and update the Notes column
