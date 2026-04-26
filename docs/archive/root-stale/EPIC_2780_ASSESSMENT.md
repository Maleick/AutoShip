# Epic #2780 — Lua + Script Ecosystem Pack: Scope & Status

**Date**: 2026-04-25  
**Assessment**: This is a parent epic spanning 12+ scripts with multiple dependencies. Single-worker implementation infeasible. Recommend team-based approach.

## Current Ecosystem Status

### F2 Scripts in Scope (from openvanilla audit)

1. **aqobot** — AQ automation bot (follow, assist, react)
2. **lem** — Lua event manager (event-driven dispatcher)
3. **lootnscoot** — Automated looting with filter lists
4. **boxhud** — Real-time HUD overlay (all box character state)
5. **luachase** — Follow/chase logic for melee assist
6. **luaconsole** — In-game Lua REPL console
7. **buttonmaster** — Configurable in-game button panels
8. **MyUI/MQGrimGUI** — Custom ImGui-based HUD overlay
9. **AlertMaster** — Alert system (spell readiness, debuffs, events)
10. **MyDPS** — DPS meter with parse/export
11. **MyPaths** — Pathfinding and waypoint navigation
12. **shareddata** — Cross-script shared data store (DanNet-backed)

### Prior Work Completed (#2617)

- Compatibility matrix doc: `docs/wiki/RedGuides-Script-Compatibility.md`
- Tier 1 conformance tests (5 scripts)
- Python test harness: `tests/test_lua_script_conformance.py`
- Fixture stubs: `tests/lua_fixtures/` (aqobot, lem, lootnscoot, luaconsole, alertmaster)
- MQ compatibility shim: `tests/lua_fixtures/mq_compat.lua`

### Prerequisites Status

| Prerequisite  | Issue | Status        | Notes                                                                        |
| ------------- | ----- | ------------- | ---------------------------------------------------------------------------- |
| Lua VM        | #791  | ✓ Implemented | mlua 0.11 in Cargo.toml; core API bindings exist                             |
| Plugin Loader | #792  | ✓ Implemented | `textquest-dll/src/mq2/plugin_loader.rs` supports .dll/.mq2 discovery & load |
| MQ2DanNet     | #2775 | ⚠️ Partial    | Transport layer exists; shareddata integration pending                       |

### Tier 1 Implementation Targets (Ready Now)

These have the lowest dependency footprint and can be started immediately:

1. **lem** (Lua event manager)
   - Requires: Lua VM ✓
   - Provides: Event dispatcher foundation for other scripts
   - Scope: ~200-300 LoC

2. **aqobot** (AQ assist bot)
   - Requires: Lua VM ✓, lem ✓
   - Provides: Basic automation proof-of-concept
   - Scope: ~150-200 LoC core

3. **luaconsole** (In-game Lua REPL)
   - Requires: Lua VM ✓, overlay system ✓
   - Provides: Live code testing & debugging
   - Scope: ~100-150 LoC

### Tier 2 Scripts (Blocked by DanNet or Overlay Maturity)

4. **boxhud** — Requires mature overlay system + multi-client state sync
5. **shareddata** — Requires MQ2DanNet #2775 fully integrated
6. **lootnscoot** — Requires loot-detection integration + filter parser
7. **MyUI/AlertMaster/MyDPS** — Require ImGui overlay framework maturity

### Architectural Gaps

1. **Script lifecycle management**
   - How are Lua scripts loaded/unloaded at runtime?
   - Which plugins own script discovery?
   - Reload semantics (hot-reload, full teardown, state preservation)?

2. **DanNet integration**
   - shareddata requires reliable cross-client messaging
   - boxhud requires multi-client state aggregation
   - Existing: DanNet plugin hook exists; Lua bindings TBD

3. **ImGui overlay framework**
   - MyUI, boxhud, AlertMaster, MyDPS all need custom ImGui surfaces
   - Current: overlay system exists (`textquest-dll/src/overlay/`)
   - Gap: Lua FFI bindings to ImGui not yet exposed

4. **MQ2 TLO & types**
   - lootnscoot, buttonmaster, AlertMaster need spawn/type/ability TLOs
   - Existing: Plugin bridge exists; script-facing type system TBD

## Realistic Scope for Single Worker

A single AutoShip worker can accomplish:

### Phase 1 (This Session)

- [ ] Assess Lua VM capability
- [ ] Create script harness infrastructure
  - Plugin discovery for Lua files
  - Script load/unload lifecycle
  - Error handling & isolation
- [ ] Implement lem (event dispatcher)
- [ ] Implement luaconsole (REPL)
- [ ] Write tests for both

**Estimated**: 4-6 hours | **Outcome**: Foundation + 2 Tier 1 scripts working

### Phase 2 (Subsequent Worker or Team)

- [ ] aqobot (requires lem foundation)
- [ ] lootnscoot (requires loot detection)
- [ ] boxhud (requires overlay maturity)

### Phase 3 (Post Foundation)

- [ ] DanNet Lua bindings
- [ ] ImGui Lua FFI
- [ ] Multi-client coordination

## Action Items for Next Worker

1. **Start with lem** — smallest & most foundational
2. **Test with Python harness** from #2617
3. **Document script-host contract** before implementing multiple scripts
4. **Create issue #2781-2792** as child issues per-script once pattern established

## Recommendation

**Convert to TeamCreate setup** (instead of single worker serial sessions):

- Lead agent: architecture & lem implementation
- Parallel worker 1: luaconsole REPL
- Parallel worker 2: script harness infrastructure
- Result: All Phase 1 items done in one cycle

This epic is too large for linear single-worker progression. Parallel agents provide visibility and faster iteration.
