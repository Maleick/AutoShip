# Frostreaver vs MQ2: Feature Comparison

> Generated 2026-03-29. Compares Frostreaver's current implementation (M1–M5 complete) against the MQ2 reference codebase.

## Legend

| Status | Meaning                                     |
| ------ | ------------------------------------------- |
| ✅     | Parity — Frostreaver matches or exceeds MQ2 |
| ⚠️     | Partial — implemented but incomplete vs MQ2 |
| ❌     | Missing — MQ2 has it, Frostreaver doesn't   |
| 🔥     | Frostreaver Better — capability MQ2 lacks   |

---

## 1. Login Automation

Frostreaver: `textquest-dll/src/login/`, `textquest/src/launcher/`
MQ2: `third_party/macroquest/src/plugins/autologin/`

| Feature                         | MQ2                                                      | Frostreaver                                                            | Status | Notes                                                                                                             |
| ------------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------- |
| Login state machine             | 10 states (tinyfsm)                                      | 10 phases, 9 internal DLL states                                       | ✅     | Both have full FSM coverage                                                                                       |
| Credential storage              | SQLite DB with encryption                                | SQLite + AES-256-GCM + Argon2id                                        | 🔥     | Frostreaver uses modern crypto (Argon2id KDF + AES-GCM) vs MQ2's simpler encryption                               |
| Password zeroization            | Cleared in CurrentLogin struct                           | Unsafe Drop impl + Zeroizing wrapper                                   | 🔥     | Explicit memory zeroing on dealloc                                                                                |
| Splash screen dismissal         | 4+ screens (dbgsplash, soesplash, EULA, order)           | 6+ screens (DBG, SOE, EULA, order, seizure, news)                      | ✅     | Both handle pre-login prompts                                                                                     |
| Credential entry                | SetEditWndText on CEditWnd                               | Direct char[] memory write (primary) + SIDL CXStr (fallback)           | ✅     | Frostreaver has two paths; direct write bypasses SIDL entirely                                                    |
| Server selection                | JoinServer() API + list scanning + long-name DB          | "PLAY EVERQUEST!" click + JoinServer stub                              | ⚠️     | MQ2 has server ID lookup and long-name→short-name mapping; Frostreaver relies on last-server                      |
| Character selection             | Full CCharacterListWnd scan by name                      | Stub — CCharacterListWnd scan TODO                                     | ⚠️     | MQ2 scans character list rows; Frostreaver has EnterWorld() but character-by-name selection is pending            |
| Kick handling                   | KickActiveCharacter setting + offline trader detection   | Auto-kick KickActiveCharacter dialog, offline trader surfaced as error | ✅     | Handles already-logged-in/offline-trader dialogs deterministically                                                |
| Error dialog parsing            | Full STML text extraction + specific error routing       | Generic okdialog detection + dismiss                                   | ⚠️     | MQ2 identifies specific errors (wrong password, account locked); Frostreaver detects but doesn't parse error text |
| Camping/relog                   | /relog command, InGameCamping state, fast-camp detection | Not implemented                                                        | ❌     | MQ2 manages camp→relog→relogin cycle                                                                              |
| Profile groups                  | Multi-character profiles with hotkeys                    | Named profile groups with F-key hotkeys (Ctrl+F1–F9 in TUI)            | ✅     | Both have named profile groups; Frostreaver uses `[[profile_groups]]` in accounts.toml                            |
| Auto-detect characters          | Saves class/level on manual login                        | Not implemented                                                        | ❌     | MQ2 auto-creates DB entries for seen characters                                                                   |
| Pause/Resume                    | HOME/END hotkeys + ImGui overlay                         | IPC command only                                                       | ⚠️     | MQ2 has in-game UI; Frostreaver pauses via orchestrator                                                           |
| /switchserver, /switchcharacter | Full server/character switching mid-session              | Not implemented                                                        | ❌     |                                                                                                                   |
| Post-login sequencing           | Not built-in (macro-driven)                              | JoinGroup → ApplyBuffs → NavigateToCamp → Ready                        | 🔥     | Frostreaver has orchestrated post-login pipeline                                                                  |
| Staggered multi-client launch   | Not built-in (manual or external)                        | Launch coordinator with configurable stagger                           | 🔥     | Deterministic per-client delays, anti-detection                                                                   |
| Two-tier architecture           | Single DLL plugin                                        | DLL FSM + orchestrator state machine + IPC                             | 🔥     | Frostreaver coordinates across processes                                                                          |
| Timeout/retry                   | ConnectRetries (5), 2s backoff                           | 60s per-phase, 3 retries, configurable                                 | ✅     | Both have retry logic                                                                                             |

---

## 2. Combat System

Frostreaver: `textquest-dll/src/combat/`, `textquest/src/combat/`
MQ2: Combat is via external plugins (MQ2Melee, MQ2Cast) — **not in reference codebase**

| Feature              | MQ2 (via plugins)                      | Frostreaver                                                               | Status | Notes                                                                           |
| -------------------- | -------------------------------------- | ------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| Combat state machine | MQ2Melee (external plugin, not in ref) | Full FSM: Idle→Engaging→Casting→OnGcd→Recovering→Fleeing                  | 🔥     | MQ2 relies on third-party plugins; Frostreaver has built-in FSM                 |
| Class strategies     | MQ2Melee/MQ2Cast INI-driven            | 16 class-specific strategies (Warrior→Berserker)                          | 🔥     | Each class has unique rotation, target selection, role behavior                 |
| Spell rotation       | MQ2Cast plugin (INI config)            | Priority-based per-class spell selection                                  | 🔥     | Built into class strategy, not external config                                  |
| GCD tracking         | MQ2Cast handles internally             | 30-tick spell GCD + per-skill melee cooldowns (8 skills)                  | 🔥     | Independent skill timers: kick 7s, bash 10s, backstab 10s, etc.                 |
| Mana management      | MQ2Cast mana checks                    | ManaGovernor with floor enforcement + healer exceptions                   | 🔥     | Healers bypass mana floor in combat; DPS transitions to Recovering              |
| HolyShit emergency   | MQ2Melee HolyShit list (INI)           | Conditional rule engine: HP/Mana/Aggro conditions + And/Or nesting        | ✅     | Both have emergency ability triggers; Frostreaver supports nested boolean logic |
| Pulling              | MQ2 macros / MQ2Melee pull             | Puller sub-FSM: Ready→Pulling→Waiting→Returning                           | 🔥     | Full pull cycle with target tracking, cooldown, range checks                    |
| Aggro detection      | Extended target window / TLO           | Heading-based heuristic (45° threshold)                                   | ⚠️     | MQ2 can read aggro% from extended target; Frostreaver uses geometric inference  |
| Assist train         | MQ2 /assist + macros                   | CombatCoordinator broadcasts MA target to all DPS                         | ✅     | Both support assist-based targeting                                             |
| Camp loop            | MQ2 macros (camp.mac, etc.)            | Full FSM: AtCamp→Pulling→Fighting→Looting→Returning + Recovery            | 🔥     | Integrated camp cycle with wipe recovery                                        |
| Bard melody          | MQ2Twist plugin                        | Built-in melody twist engine (~3s song rotation)                          | ✅     |                                                                                 |
| Pet handling         | /pet attack in macros                  | Automatic /pet attack for SK, Shaman, Necro, Mage                         | ✅     |                                                                                 |
| Combat humanization  | Not built-in                           | Per-client deterministic jitter (assist delay, cast delay, med threshold) | 🔥     | Anti-detection: each client has unique timing personality                       |
| Spell data API       | Full spell TLOs (2000+ spells)         | Static spell_db + SpellEntry config                                       | ⚠️     | MQ2 has runtime spell data access; Frostreaver uses configured spell lists      |
| Buff tracking        | Full buff TLOs + MQ2BuffType           | Not implemented (buffs read externally)                                   | ❌     | MQ2 can enumerate all active buffs with durations                               |
| Extended target      | Extended target window TLO             | Not implemented                                                           | ❌     | MQ2 reads aggro% and hate list from extended target                             |
| Disc/AA management   | MQ2Melee disc timers + AA activation   | CombatForceAbility IPC command                                            | ⚠️     | Basic ability forcing; no disc timer tracking                                   |

---

## 3. Navigation

Frostreaver: `textquest-dll/src/nav/`, `textquest/src/nav/`
MQ2: Navigation is via MQ2Nav/MQ2MoveUtils (external plugins — **not in reference codebase**)

| Feature                 | MQ2 (via plugins)                  | Frostreaver                                                                | Status | Notes                                                                |
| ----------------------- | ---------------------------------- | -------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------- |
| Nav FSM                 | MQ2Nav (external)                  | Idle→Moving→Arrived                                                        | 🔥     | Built-in, not plugin-dependent                                       |
| Waypoint following      | MQ2Nav navmesh pathfinding         | Queue-based waypoint cursor with 2D distance checks                        | ⚠️     | MQ2Nav uses full 3D navmesh; Frostreaver uses pre-recorded waypoints |
| Navmesh pathfinding     | MQ2Nav (Recast/Detour integration) | Not implemented                                                            | ❌     | MQ2Nav has full 3D pathfinding with obstacle avoidance               |
| Stuck detection         | MQ2MoveUtils basic stuck check     | 40-tick threshold + 5-attempt escalating recovery                          | 🔥     | Progressive recovery: 90° → -90° → 180° → 45° → give up              |
| Movement humanization   | Not built-in                       | Per-client personality: speed ±7%, heading wobble 1-4°, detour 0-8%        | 🔥     | Deterministic per client_id via Xorshift32 PRNG                      |
| /stick and /follow      | MQ2MoveUtils (external)            | DLL slash-command handlers for `/stick` / `/follow` with warp/summon guards | ⚠️     | Basic stick/follow command surface exists; full MQ2MoveUtils parity still pending |
| Camp positioning        | MQ2 macros                         | CampManager: role-based spot assignment, standard EQ camp layout generator | 🔥     | Tank forward, healer back, DPS semicircle — automatic                |
| Path recording          | Not built-in                       | WaypointRecorder + RDP simplification (Ramer-Douglas-Peucker)              | 🔥     | Record movement, auto-simplify collinear points                      |
| Zone routing            | MQ2Nav zone connections            | TravelPlan FSM: WalkTo, ZoneTo, PortTo, StaggerWait                        | 🔥     | Multi-zone travel planning with porter awareness                     |
| Zone stagger            | Not built-in                       | Deterministic per-client zone entry delays (anti-detection)                | 🔥     | Prevents simultaneous zone entries                                   |
| Heading control         | MQ2MoveUtils /face                 | EQ heading 0-512 with wobble jitter                                        | ✅     |                                                                      |
| Door/object interaction | MQ2 /door, /click                  | Not implemented                                                            | ❌     |                                                                      |

---

## 4. Window / UI Interaction

Frostreaver: `textquest-dll/src/eq/widgets.rs`, `textquest-dll/src/login/widgets.rs`
MQ2: `third_party/macroquest/src/eqlib/include/eqlib/game/CXWnd.h`, UI headers

| Feature                  | MQ2                                           | Frostreaver                                                | Status | Notes                                                          |
| ------------------------ | --------------------------------------------- | ---------------------------------------------------------- | ------ | -------------------------------------------------------------- |
| Window finding           | ArrayClass container + virtual GetChildItem   | Direct CXWndManager array scan + substring matching        | ✅     | Different approaches, same result                              |
| Child window iteration   | TList with arbitrary depth recursion          | Pointer chaining (2-level: children + grandchildren)       | ⚠️     | MQ2 handles deeper nesting                                     |
| CXStr read               | Abstracted via C++ CXStr class                | Manual CStrRep field reads (refCount, alloc, length, data) | ✅     | Frostreaver has full CStrRep layout knowledge                  |
| CXStr write              | Virtual SetWindowText()                       | In-place buffer overwrite + HeapAlloc clone_cstrrep        | ✅     | Frostreaver allocates EQ-compatible CStrRep on process heap    |
| Button clicking          | Virtual WndNotification()                     | Raw vtable call at CXWND_VTABLE_WND_NOTIFICATION offset    | ✅     | Both use XWM_LCLICK notification                               |
| Edit field manipulation  | SetEditWndText()                              | Direct char[] write (login) + CXStr InputText write (SIDL) | ✅     |                                                                |
| Window visibility check  | CXWnd::IsVisible() virtual                    | Flags field read at offset                                 | ✅     |                                                                |
| UI notification messages | 48+ message types in EXWndNotification enum   | XWM_LCLICK implemented; others as needed                   | ⚠️     | MQ2 has full message vocabulary                                |
| Controller system        | ControllerBase for async notification routing | Not implemented                                            | ❌     | MQ2 has structured event dispatch for UI                       |
| Context menus            | CContextMenuManager                           | Not implemented                                            | ❌     |                                                                |
| Chat window access       | CChatWindowManager                            | SlashCommand IPC (/say, /group)                            | ⚠️     | Frostreaver sends chat via commands, doesn't read chat windows |
| ImGui overlay            | Full ImGui overlay for debugging              | Not implemented (TUI is external)                          | ❌     | Different approach: Frostreaver uses external TUI              |
| Inventory management     | CInvSlotMgr, CContainerWnd                    | Not implemented                                            | ❌     |                                                                |
| Spell book access        | CSpellBookWnd                                 | Not implemented                                            | ❌     |                                                                |
| Map display              | MQ2Map plugin (in reference)                  | Not implemented (planned TUI feature)                      | ❌     |                                                                |

---

## 5. IPC / Multi-Client Communication

Frostreaver: `textquest-dll/src/ipc/`, `textquest/src/ipc/`, `textquest-common/src/protocol.rs`
MQ2: `third_party/macroquest/src/routing/` (PostOffice, NamedPipes, Network)

| Feature                  | MQ2                                            | Frostreaver                                                               | Status | Notes                                                                  |
| ------------------------ | ---------------------------------------------- | ------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------- |
| Transport: Named pipes   | NamedPipes.h/cpp (35 KB impl)                  | Per-client pipes: `\\.\pipe\{session}_cmd_{id}`                           | ✅     | Both use named pipes for command/response                              |
| Transport: Shared memory | Not primary (PostOffice-based)                 | Per-client 64KB shared memory for GameState publishing                    | 🔥     | Lock-free atomic sequence reads; zero-copy state polling               |
| Wire protocol            | Protobuf (Network.proto, Routing.proto)        | Length-prefixed bincode frames (u32 LE + payload)                         | ✅     | MQ2 uses protobuf; Frostreaver uses Rust bincode (more compact)        |
| Peer discovery           | UDP multicast (configurable ports)             | Orchestrator manages client registry                                      | ⚠️     | MQ2 has decentralized discovery; Frostreaver is centralized            |
| PostOffice routing       | Full routing with mailboxes, addresses, actors | Direct pipe per client (no routing needed)                                | ✅     | Different architectures; Frostreaver's hub-spoke is simpler for 36-box |
| Session authentication   | Not documented in reference                    | 32-byte random token + constant-time comparison                           | 🔥     | Prevents unauthorized command injection                                |
| Command vocabulary       | Plugin-defined messages                        | 30+ typed commands (movement, combat, login, chat, system)                | 🔥     | Strongly-typed Rust enums with validation                              |
| Response types           | Plugin-defined                                 | 7 response types (Pong, CommandResult, NavUpdate, LoginPhaseUpdate, etc.) | ✅     |                                                                        |
| Multi-client scaling     | DanNet/EQBC (external, not in ref)             | Designed for 36 concurrent clients (1 pipe + 64KB shm each)               | 🔥     | Native multi-client architecture                                       |
| Thread safety            | Framework-managed                              | Explicit: IPC thread + game loop thread + pending queues                  | ✅     |                                                                        |
| Frame validation         | Protobuf schema                                | Bounds checking (spell slots ≤13, waypoints ≤1000, strings ≤128)          | 🔥     | Built-in parameter validation                                          |
| Fire-and-forget          | Not documented                                 | Async variant for non-blocking commands                                   | ✅     |                                                                        |

---

## 6. Offset Management

Frostreaver: `textquest-common/src/offsets.rs`, `textquest-common/src/offset_db.rs`
MQ2: `third_party/macroquest/src/eqlib/include/eqlib/offsets/eqgame.h`

| Feature                    | MQ2                                                 | Frostreaver                                                       | Status | Notes                                                               |
| -------------------------- | --------------------------------------------------- | ----------------------------------------------------------------- | ------ | ------------------------------------------------------------------- |
| Total offsets defined      | ~681 (eqgame.h 656 + eqmain.h 12 + eqgraphics.h 13) | ~102                                                              | ⚠️     | MQ2 covers 83 classes; Frostreaver covers what's needed for M1-M5   |
| Function addresses         | 406 across 83 classes                               | ~30 core functions (CastSpell, DoAttack, ExecuteCmd, etc.)        | ⚠️     | Frostreaver has essential functions; MQ2 has comprehensive coverage |
| Spawn/player field offsets | Spread across class headers                         | 44 organized by struct (player_base, player_zone, character_zone) | ✅     | Frostreaver's struct-centric layout is cleaner for field reads      |
| UI class methods           | 150+ UI function offsets                            | Manager pointers + key vtable offsets only                        | ⚠️     | MQ2 has method-level granularity for all UI classes                 |
| eqmain.dll offsets         | 12 offsets                                          | 22 offsets (SIDL, CXWndMgr, LoginClient, EQLogin fields)          | 🔥     | Frostreaver has more eqmain detail for login automation             |
| Organization               | C++ #define macros by class                         | Rust const modules by struct hierarchy                            | ✅     | Both well-organized; different idioms                               |
| Hot update mechanism       | Recompile required                                  | JSON offset_db with load/save + rebase()                          | 🔥     | Frostreaver can patch offsets without recompiling                   |
| Version tracking           | **ClientDate + **ExpectedVersionDate                | CLIENT_DATE constant                                              | ✅     | Both track target client version                                    |
| Preferred base addressing  | 0x140000000 (eqgame), 0x180000000 (eqmain)          | Same base addresses + rebase() function                           | ✅     | Both use the same rebasing approach                                 |
| Group offsets              | In class headers                                    | 11 dedicated group offsets (CGroup, CGroupMember layout)          | ✅     |                                                                     |
| Zone info                  | In class headers                                    | 3 offsets (zone info instance, short/long name)                   | ✅     |                                                                     |

---

## 7. Additional Systems (Frostreaver-Only)

These features exist in Frostreaver but have no MQ2 equivalent:

| Feature                        | Frostreaver Module                | Notes                                                                          |
| ------------------------------ | --------------------------------- | ------------------------------------------------------------------------------ |
| Soul Engine (LLM character AI) | `textquest/src/soul/`                  | M5: Character personalities, persistent memory, social dynamics, idle behavior |
| Encrypted credential store     | `textquest/src/credentials/`           | AES-256-GCM + Argon2id + SQLite                                                |
| CPU affinity management        | `textquest/src/client/affinity.rs`     | Per-client core pinning for 36-box                                             |
| Self-healing monitor           | `textquest/src/client/healing.rs`      | Auto-restarts crashed clients                                                  |
| External TUI dashboard         | `textquest/src/tui/`                   | ratatui-based live monitoring (spawn list, panels, hex dump)                   |
| Cross-platform dev             | macOS stubs + demo data           | UI development without live EQ client                                          |
| Post-login sequencer           | `textquest/src/launcher/post_login.rs` | Group→Buff→Camp automated pipeline                                             |
| DLL injection (custom)         | `textquest/src/inject/`                | Rust-native DLL injection, no MQ2 dependency                                   |

---

## Summary Scorecard

| Area           | ✅ Parity | ⚠️ Partial | ❌ Missing | 🔥 Better | Assessment                                                                  |
| -------------- | --------- | ---------- | ---------- | --------- | --------------------------------------------------------------------------- |
| **Login**      | 4         | 4          | 4          | 5         | Strong foundation; needs kick handling, /switchserver, character scanning   |
| **Combat**     | 3         | 2          | 2          | 8         | Frostreaver dominates with built-in class strategies, pulling, humanization |
| **Navigation** | 1         | 1          | 3          | 6         | Superior humanization/camp/recording; missing navmesh and /stick            |
| **UI/Widgets** | 6         | 3          | 5          | 0         | Solid core; missing deep UI access (inventory, spellbook, context menus)    |
| **IPC**        | 4         | 1          | 0          | 5         | Frostreaver's purpose-built IPC exceeds MQ2's general-purpose routing       |
| **Offsets**    | 5         | 3          | 0          | 2         | Lean and focused; expand as features require more game functions            |

**Overall**: Frostreaver has achieved functional parity or superiority in its core automation loop (combat, navigation, IPC, login). The main gaps are in breadth of game UI access and MQ2's massive offset/function coverage — features that can be added incrementally as M6-M8 milestones require them. The anti-detection features (humanization, staggering, session auth) and modern architecture (Rust type safety, hot-updatable offsets, encrypted credentials) represent genuine advantages over MQ2's C++ plugin ecosystem.
