# Session Handoff — 2026-03-29 ~07:50 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Use autoresearch loops for implementation. Run peer review after each major feature.
```

## Session Stats (Cumulative)
- ~39,000+ lines added (~6,000+ this session)
- ~80 commits (40+ this session)
- 610 tests passing across 3 crates (was 552 at session start)
- Auto-login: full chain working (login → server select → in-world)
- 12 class strategies (was 4): Warrior, Cleric, Paladin, Druid, Wizard, Monk, Rogue, Shaman, Necro, Mage, SK, Enchanter
- Combat FFI: CastSpell, UseSkill, DoCombatAbility, ExecuteCmd, ToggleAutoAttack all wired up
- Per-skill cooldown tracking (individual timers per skill)
- CH chain system (configurable cleric Complete Heal rotation)
- Cleric heal-cancel (duck when group HP > 85%)
- Utility fixes: auto-accept rez, cursor watchdog, /destroy junk, rez priority, auto-group

## What's Done (This Session)

### Auto-Login (BREAKTHROUGH)
- CXWndManager window enumeration (150 UI widgets discoverable)
- CXStr direct widget write (MQ2 approach — InputText at +0x278)
- HeapAlloc CStrRep for empty password field (donor freeList from username)
- Vtable WndNotification click for Login button (no foreground needed)
- Full chain: credentials → Login click → PLAY EVERQUEST click → Enter World
- Character entered world confirmed (eqmain.dll unloaded, EQ memory → 1GB)

### Combat System (from stubs to functional)
- All FFI functions wired: cast_spell, do_attack, use_skill, do_combat_ability, execute_cmd
- Auto-attack toggle on engage/disengage
- Per-class melee skills (taunt, kick, bash, backstab, flying kick) with individual cooldowns
- 12 class strategies covering all EQ TLP classes
- Cleric heal-cancel (duck when group HP > 85%)
- CH chain system (configurable rotation with dynamic members)
- Range check before casting
- /face target + /pet attack on engage
- on_engage/on_kill callbacks properly called (shaman slow tracking fixed)
- Enchanter spawn_type fix (was mez-ing players instead of NPCs)
- UseSkill bAuto parameter fix

### Peer Review Fixes
- Password redaction in Debug logs
- Log rotation (max 7 daily files)
- UseSkill missing bAuto parameter
- Enchanter spawn_type inverted
- on_engage/on_kill never called (shaman broken)

### MQ2 Research (3 reports completed)
- MQ2Melee: combat functions are stubs, need FFI wiring ✅ DONE
- MQ2Cast: casting entirely unimplemented ✅ FFI wired, SpellETA offsets added
- MQ2 Utilities: rez accept, cursor, vendor sell ✅ quick wins implemented

## Priority TODO — Next Session

### 1. Test Auto-Login Full Chain (PRIORITY #1)
The vtable clicks for phases 2/3 need one more test cycle. User has `eq_watchdog.ps1` on desktop.
Steps:
1. Double-click `eq_watchdog.ps1` on frostreaver desktop
2. From SSH: `echo "LAUNCH_AND_LOGIN\nfrostreaver01\ndr698iDBBa1IpTS" > /c/Users/xmale/Projects/DMFT/triggers/launch_001.txt`
3. Watchdog launches EQ, injects DLL, sends login command
4. Monitor DLL log for full chain completion

### 2. Camp↔Combat Integration (HIGH)
The camp loop sends `/attack` slash commands but the Combatant FSM (with 12 strategies) stays in Idle. Need to wire `CombatEngage` IPC command from camp loop to DLL combatant. Biggest integration gap.

### 3. TUI as Control Plane (HIGH)
User wants EVERYTHING controllable through TUI:
- :ma/:mt commands for MA/MT selection
- :engage/:disengage for combat control
- :invite/:accept for group formation
- :ch start/stop/interval for CH chain
- Help overlay expansion
- Research awesome-tuis (github.com/rothgar/awesome-tuis) for UI ideas

### 4. More Combat Features
- Bard strategy (class 8) + melody/twist engine
- Ranger strategy (class 4) — currently falls through to GenericDps
- Camp loop recovery wiring (death detection → rez)
- /destroy only for classified items (not unconditional)
- select_target() integration (currently dead code)

### 5. MQ2 Deep Research
Continue comparing against MQ2 plugins:
- MQ2Melee stick/follow mechanics
- MQ2Cast spell interruption/retry
- MQ2NetHeal cross-group healing
- MQ2AutoGroup raid formation
- MQ2Vendors auto-buy

### 6. TLP Encounter Research
- NToV dragon mechanics (AE rampage, fear, gravity flux)
- Encounter-specific combat profiles
- CH chain timing per boss
- Positioning strategies

### 7. Deferred (from previous sessions)
- Navmesh loader (biggest gap for autonomous navigation)
- Zone name "Unknown" offset
- STANDSTATE offset calibration
- Pipe error backoff (caused 9GB log)

## Key Technical Discoveries (This Session)

### Auto-Login
- EQ login uses DirectInput (IDirectInput8A) — all input simulation fails
- eqmain.dll CXWndManager layout: window array at +0x010, count at +0x018
- CXStr = ptr to CStrRep; data at +0x18, length at +0x08, alloc at +0x04
- Allocating CStrRep with Rust allocator crashes EQ; HeapAlloc + donor freeList works
- CXWnd WndNotification vtable offset in eqmain: 0x110
- WindowText (+0x078) and InputText (+0x278) share the same CStrRep on CEditWnd

### Combat
- SpawnType: 0=Player, 1=NPC (enchanter had this inverted)
- UseSkill requires bAuto parameter (4th arg, bool)
- on_engage/on_kill must be called explicitly (not automatic)
- CastSpell takes (gemid, spellid, item_ptr=null, item_guid=0)
- Melee skills fire independently of spell GCD

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Watchdog: `C:\Users\xmale\Desktop\eq_watchdog.ps1`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
