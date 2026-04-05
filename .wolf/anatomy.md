# anatomy.md

> Auto-maintained by OpenWolf. Last scanned: 2026-04-05
> Files: 363 tracked | Anatomy hits: 0 | Misses: 0

## ./

- `Cargo.toml` — Workspace root: defines members dmft, dmft-dll, dmft-common, dmft-web. (~96 tok)

## .github/workflows/

- `agent-close-pr.yml` — Auto-closes stale agent PRs that fail CI or go inactive. (~428 tok)
- `agent-ready.yml` — Watches for `agent:ready` label and dispatches to Claude/Codex workers. (~828 tok)
- `auto-merge.yml` — Auto-merges PRs that pass CI and have approved reviews. (~216 tok)
- `ci.yml` — Main CI pipeline: cargo fmt, clippy, test, build on ubuntu + windows. (~1224 tok)
- `claude-agent.yml` — Dispatches Claude Code agent for issue work via workflow_dispatch. (~208 tok)
- `copilot-ci-dispatch.yml` — Dispatches Copilot CI checks on PR events. (~504 tok)
- `nightly-release.yml` — Nightly release build: cross-compile, package artifacts, GitHub release. (~700 tok)
- `post-merge-sync.yml` — Post-merge sync: updates wiki, metrics, and roadmap after PR merge. (~276 tok)
- `readme-metrics.yml` — Updates README badges with test count, line count, and coverage metrics. (~512 tok)
- `release.yml` — Tagged release workflow: build, sign, and publish release artifacts. (~444 tok)
- `wiki-nightly.yml` — Nightly wiki sync: pushes docs/wiki/ to the GitHub wiki repo. (~356 tok)

## config/

- `accounts.toml` — Account roster: login names, servers, characters, classes, group assignments. (~476 tok)
- `frostreaver.toml` — Main DMFT config: launch settings, retry policy, server config, soul engine, camp defaults. (~1060 tok)
- `hvt_watchlist.toml` — High-value target watchlist: named mobs with priority and zone assignments. (~288 tok)

## config/camps/

- `crescent_reach_newbie.toml` — Camp config for Crescent Reach newbie yard. (~56 tok)
- `crescent_reach_undead.toml` — Camp config for Crescent Reach undead area. (~60 tok)
- `crushbone_entrance.toml` — Camp config for Crushbone entrance. (~60 tok)
- `crushbone_throne.toml` — Camp config for Crushbone throne room. (~60 tok)
- `lguk_dead_side.toml` — Camp config for Lower Guk dead side. (~60 tok)
- `lguk_live_side.toml` — Camp config for Lower Guk live side. (~60 tok)
- `mistmoore_castle.toml` — Camp config for Castle Mistmoore interior. (~60 tok)
- `mistmoore_entrance.toml` — Camp config for Castle Mistmoore entrance. (~60 tok)
- `sebilis_disco.toml` — Camp config for Sebilis disco camp. (~56 tok)
- `unrest_basement.toml` — Camp config for Estate of Unrest basement. (~60 tok)
- `unrest_yard.toml` — Camp config for Estate of Unrest yard. (~60 tok)

## config/classes/

- `bard.toml` — Bard class config: song twist abilities, pull skills, buff priorities. (~96 tok)
- `beastlord.toml` — Beastlord class config: pet commands, slow, melee abilities. (~132 tok)
- `berserker.toml` — Berserker class config: frenzy, rage abilities. (~108 tok)
- `cleric.toml` — Cleric class config: heal tiers, rez, buff priorities. (~136 tok)
- `druid.toml` — Druid class config: heals, nukes, snare, port abilities. (~172 tok)
- `enchanter.toml` — Enchanter class config: mez, haste, charm, stun abilities. (~232 tok)
- `magician.toml` — Magician class config: pet commands, nukes, CoTH. (~112 tok)
- `monk.toml` — Monk class config: melee abilities, flying kick, feign death. (~136 tok)
- `necromancer.toml` — Necromancer class config: DoTs, lifetap, pet, feign death. (~168 tok)
- `paladin.toml` — Paladin class config: heals, stuns, undead nukes, lay hands. (~144 tok)
- `ranger.toml` — Ranger class config: bow, melee, DoTs, tracking. (~112 tok)
- `rogue.toml` — Rogue class config: backstab, evade, hide/sneak. (~140 tok)
- `shadowknight.toml` — Shadow Knight class config: lifetaps, DoTs, snare, FD. (~112 tok)
- `shaman.toml` — Shaman class config: slow, DoTs, heals, canni, buffs. (~184 tok)
- `warrior.toml` — Warrior class config: taunt, discs, defensive abilities. (~108 tok)
- `wizard.toml` — Wizard class config: nukes, harvest mana, evac. (~112 tok)

## config/named_mobs/

- `chardok.toml` — Named mob database for Chardok zone. (~148 tok)
- `crushbone.toml` — Named mob database for Crushbone zone. (~148 tok)
- `karnors.toml` — Named mob database for Karnor's Castle zone. (~184 tok)
- `lowerguk.toml` — Named mob database for Lower Guk zone. (~184 tok)
- `mistmoore.toml` — Named mob database for Castle Mistmoore zone. (~184 tok)
- `sebilis.toml` — Named mob database for Sebilis zone. (~148 tok)
- `unrest.toml` — Named mob database for Estate of Unrest zone. (~148 tok)
- `upperguk.toml` — Named mob database for Upper Guk zone. (~112 tok)

## dmft/

- `Cargo.toml` — Orchestrator crate dependencies: ratatui, crossterm, tokio, tracing, rusqlite, clap, serenity, etc. (~244 tok)

## dmft/src/

- `main.rs` — CLI entry point: parses args (dump/inject/tui/credentials subcommands), sets up tracing, dispatches to cli.rs. (~1312 tok)
- `lib.rs` — Crate root: declares all orchestrator modules (camp, client, combat, config, credentials, discord, eq, inject, ipc, launcher, loot, metrics, nav, orchestrator, process, soul, tui). (~464 tok)
- `cli.rs` — CLI command dispatch: TUI launch, dump mode, injection, credential management, shared-state retry logic. (~6492 tok)
- `config.rs` — TOML config loading: account entries, launch config, retry policy, server config, soul config. (~3268 tok)
- `orchestrator.rs` — Wires camp loop state machine to IPC command delivery. Builds camp snapshots from shared state, drives combat coordinator, camp progression. (~5476 tok)

## dmft/src/bin/

- `import_ghidra.rs` — Binary tool: imports Ghidra JSON harvest (functions, strings, imports) into SQLite GhidraDatabase. (~1000 tok)

## dmft/src/camp/

- `mod.rs` — Camp module re-exports. (~112 tok)
- `state.rs` — Camp loop state machine: drives pull/fight/loot/med cycle. Generates macro-level slash commands, manages group-level flow. CampLoop FSM with CampAction/CampEvent/CampState types. (~5820 tok)
- `buffs.rs` — Buff maintenance: tracks buff durations per member, queues rebuffs during downtime with priority ordering. (~2056 tok)
- `cc.rs` — Crowd control subsystem: mez/stun/charm/snare/root tracking and assignment. CcTracker manages CC state per mob. (~3720 tok)
- `class_config.rs` — Class-specific ability configs loaded from TOML: abilities, cooldowns, priorities per class. (~2648 tok)
- `config.rs` — Camp configuration loaded from TOML: zone, anchor point, pull radius, level range, mob filters. (~1108 tok)
- `hunt.rs` — Hunt mode: tank roams for mobs while group follows at role-appropriate distances. Alternative to stationary camp mode. (~3360 tok)
- `loot.rs` — Loot window automation: FSM-driven corpse targeting, approach, loot window interaction via slash commands. (~2684 tok)
- `personality.rs` — Per-character personality profiles for anti-synchronicity: deterministic timing variance from name hash. (~1120 tok)
- `positioning.rs` — Combat positioning: melee range checks, distance calculations, reposition decisions. (~992 tok)
- `progression.rs` — Camp progression engine: auto-advances camps based on average group level. Loads camp database, checks level thresholds. (~1896 tok)
- `puller.rs` — Pull target selection: picks best mob from nearby spawns considering distance, level, named status, CC state. (~1636 tok)
- `recovery.rs` — Death recovery: detects dead members, requests resurrections, manages rez dialog acceptance, rebuff after rez. (~2200 tok)
- `vendor.rs` — Vendor sell cycle FSM: travel to vendor, open window, sell items by filter, return to camp. (~2740 tok)

## dmft/src/client/

- `mod.rs` — Client module re-exports. (~40 tok)
- `session.rs` — EqSession: represents a single managed EQ client. Tracks SlotLifecycle (Configured→Launching→Injecting→Live→Recovering). (~1484 tok)
- `manager.rs` — ClientManager: discovers, tracks, and manages all EQ client sessions by PID. (~600 tok)
- `affinity.rs` — CPU affinity and process priority settings for EQ clients. Supports per-client core pinning. (~1072 tok)
- `healing.rs` — Self-healing monitor: detects unresponsive clients via ping timeouts, triggers auto-recovery. (~620 tok)

## dmft/src/combat/

- `mod.rs` — Combat module re-exports. (~56 tok)
- `coordinator.rs` — CombatCoordinator: group combat orchestration — assist targeting, CC assignments, camp loop FSM integration. (~2156 tok)
- `heal_coordinator.rs` — Cross-group heal arbitration: prevents double-healing via time-expiring claim locks, priority-ordered target lists. (~3064 tok)
- `ch_chain.rs` — Complete Heal chain coordinator: manages cleric rotation casting CH on main tank with fixed-interval timing. (~2688 tok)
- `camp_loop.rs` — Camp loop FSM: camp → pull → combat → loot → return → repeat cycle. (~1556 tok)
- `spell_db.rs` — Spell info database for orchestrator planning: spell IDs, mana costs, cast times, ranges. (~872 tok)
- `events.rs` — Structured combat event tracking: DPS meters, kill counts, damage aggregation in a bounded ring buffer. (~1360 tok)

## dmft/src/credentials/

- `mod.rs` — Credentials module re-exports. (~40 tok)
- `crypto.rs` — Argon2id key derivation + AES-256-GCM encryption/decryption for credential storage. (~1108 tok)
- `store.rs` — SQLite-backed encrypted credential store: add/get/list/delete accounts with master password. (~1252 tok)
- `prompt.rs` — Terminal password prompts (no echo) for master password and account passwords. (~84 tok)

## dmft/src/discord/

- `mod.rs` — Discord module re-exports. (~52 tok)
- `bot.rs` — Embedded Discord bot: DZ lockout tracker, open world spawn announcements, slash commands (/lockouts, /status, /spawns). Uses serenity. (~2080 tok)
- `bridge.rs` — Command bridge: bidirectional channel between Discord bot and TUI for command relay and status updates. (~664 tok)
- `webhook.rs` — Discord webhook sender: routes alerts to per-category webhook URLs (kills, loot, status, errors). (~1844 tok)

## dmft/src/eq/

- `mod.rs` — EQ module re-exports. (~72 tok)
- `spawn.rs` — Spawn data reader: reads SpawnInfo fields from EQ process memory using field-by-field reads with offset rebasing. (~3008 tok)
- `structs.rs` — EQ data structures: EqClass enum, SpawnInfo, SpawnType, StandState, CastState, GroupInfo, BuffSlot. (~4524 tok)
- `log_parser.rs` — EQ log file parser: extracts chat events, loot events, kill messages, experience gains from log lines. (~3028 tok)
- `log_watcher.rs` — Log file tailer: watches EQ log file for new lines, feeds parsed events into LootDatabase. (~1024 tok)
- `map_parser.rs` — EQ map file parser: reads L (line) and P (point) records from .map files for zone rendering. (~2508 tok)
- `hvt.rs` — High-value target watchlist: loads HVT definitions from TOML, priority-based target matching. (~1280 tok)
- `named_db.rs` — Named mob database: loads per-zone TOML files, stores named mob info with priority and placeholder status. (~1440 tok)
- `named_tracker.rs` — Named spawn tracker: monitors spawn list for named mobs, tracks spawn/despawn times, calculates window timers. (~2792 tok)

## dmft/src/inject/

- `mod.rs` — Inject module re-exports. (~32 tok)
- `dll_prep.rs` — DLL staging: copies payload to temp location with randomized legitimate Microsoft DLL name for stealth. (~1448 tok)
- `loader.rs` — Classic DLL injection: CreateRemoteThread + LoadLibraryW into target process by PID. (~1316 tok)
- `reflective.rs` — Reflective DLL injection: maps DLL from raw bytes without LoadLibrary, avoids common detection vectors. PE parsing, section mapping, relocation, import resolution. (~3252 tok)

## dmft/src/ipc/

- `mod.rs` — IPC module re-exports. (~36 tok)
- `pipe.rs` — Named pipe client (orchestrator side): connects to DLL's named pipe, sends commands, receives responses. (~828 tok)
- `shared.rs` — Shared memory reader (orchestrator side): reads GameState published by DLL via named shared memory region. Read-only, least privilege. (~1180 tok)

## dmft/src/launcher/

- `mod.rs` — Launcher module re-exports. (~40 tok)
- `coordinator.rs` — LaunchCoordinator: staggered launching and login of multiple EQ clients with configurable delays and retry policy. (~2100 tok)
- `login_sm.rs` — LoginStateMachine: drives a single client through EQ login flow (launch → connect → server select → char select → enter world). (~2320 tok)
- `post_login.rs` — PostLoginSequencer: sequences post-login actions (group join → buff → navigate to camp → ready). (~1464 tok)
- `spawner.rs` — Process spawner: launches eqgame.exe with login/server arguments. (~360 tok)

## dmft/src/loot/

- `mod.rs` — Loot module re-exports. (~48 tok)
- `store.rs` — SQLite-backed item database: item catalog, loot tables, wishlists, loot history, bid tracking, distribution rules. (~5684 tok)

## dmft/src/metrics/

- `mod.rs` — Metrics module re-exports. (~40 tok)
- `events.rs` — Fleet event capture types: Kill, ZoneChange, LevelUp, Death, LootDrop events flowing from DLL through IPC. (~1056 tok)
- `store.rs` — SQLite-backed fleet metrics store: events, DPS snapshots, loot history, lockouts, session logs. (~2032 tok)

## dmft/src/nav/

- `mod.rs` — Nav module re-exports. (~24 tok)
- `camp.rs` — Camp position management: assigns characters to role-based spots, tracks follow-mode leader anchors. (~2816 tok)
- `mesh.rs` — Navmesh loading pipeline: downloads from mqmesh.com, parses MQ2Nav binary format (DNAV v7), loads into Detour for pathfinding. (~4612 tok)
- `recorder.rs` — Waypoint recorder: captures character movement into replayable paths with minimum distance filtering. (~1116 tok)
- `router.rs` — Zone router: plans multi-zone travel with walk-to and zone-transition steps, coordinates group zone changes. (~1632 tok)

## dmft/src/process/

- `mod.rs` — Process module re-exports. (~24 tok)
- `memory.rs` — ProcessHandle: opens processes by PID for memory reading via ReadProcessMemory. RAII handle management. (~1092 tok)
- `window.rs` — WindowHandle: wraps Win32 HWND for EQ client windows, used for input dispatch and window enumeration. (~268 tok)

## dmft/src/soul/

- `mod.rs` — Soul module re-exports. (~64 tok)
- `config.rs` — Soul Engine config: edginess levels, LLM provider settings, per-character personality config, relationship seeds. (~1960 tok)
- `coordinator.rs` — SoulCoordinator: orchestrates personality engine, idle scheduler, LLM requests, and memory store per character. (~2328 tok)
- `idle.rs` — Idle behavior scheduler: generates ambient actions (emotes, say, sit, stand, look around) based on personality traits and mood. (~3572 tok)
- `memory.rs` — Autobiographical memory store (SQLite): per-character event memory, mood history, relationship snapshots. (~5080 tok)
- `personality.rs` — PersonalityEngine: maps Big Five traits + EQ traits to behavioral decisions. SoulContext provides per-tick personality state. (~4544 tok)
- `social.rs` — Social dynamics: directed relationships with EQ-style faction scores, social tags, relationship decay/growth. (~2512 tok)

## dmft/src/soul/llm/

- `mod.rs` — LLM module: defines LlmProvider trait, LlmPriority, LlmRequest/Response, Situation types. (~904 tok)
- `api_client.rs` — HTTP-based LLM provider: routes to Anthropic/OpenAI/ollama APIs based on config. Falls back to trait-driven responder when no key set. (~1872 tok)
- `fallback.rs` — Trait-driven fallback responder: generates text from personality trait vectors + phrase templates without any LLM API calls. (~3136 tok)
- `priority_queue.rs` — LLM request priority queue with per-hour token budget tracking. Higher priority requests processed first. (~1708 tok)

## dmft/src/tui/

- `mod.rs` — TUI module re-exports. (~180 tok)
- `app.rs` — Main App struct: holds all TUI state (screens, clients, spawns, groups, nav, combat, config). Drives the TUI event loop and screen transitions. (~21912 tok)
- `run.rs` — TUI runner: terminal setup/teardown, main render loop, demo data loading, live EQ process polling. (~5448 tok)
- `state.rs` — Per-screen state structs: SpawnsState, MapState, GroupsState, NavigationState with selection and filtering. (~5840 tok)
- `theme.rs` — Theme system: semantic colors/styles for all UI elements. Supports multiple themes (Neriak, Classic, etc.). (~3908 tok)
- `event.rs` — Keyboard event handler: processes key input for screen navigation, selection, commands, and modal interactions. (~3328 tok)
- `command.rs` — Command metadata: help sections, command definitions for TUI command bar and help overlay. (~2860 tok)
- `demo_data.rs` — Demo spawn data for TUI demo mode: curated spawn lists per zone mixing PCs, NPCs, corpses. (~7904 tok)
- `sprites.rs` — Pixel-art sprite system: 10x10 class emblems rendered with Unicode half-blocks, state-based animation (idle, casting, dead). (~4224 tok)
- `menu.rs` — Dropdown menu bar: top-level categorized commands navigable with arrow keys, toggled with F10/Alt. (~2528 tok)
- `toast.rs` — Toast notification system: ephemeral messages for achievements, warnings, status with auto-dismiss. (~888 tok)
- `wizard.rs` — Onboarding wizard: first-run setup for EQ client detection, character assignment, camp config, class roles. (~2588 tok)
- `cast.rs` — Cast display model: shared presentation for spell cast bars with progress, labels, and remaining time. (~1356 tok)
- `client.rs` — ClientState: per-client TUI state (PID, EQ base, local player, target, group info, lifecycle). (~616 tok)
- `companion.rs` — Tamagotchi-style fleet companion: virtual EQ creature that evolves with fleet performance (kills grow, wipes sadden). (~1576 tok)
- `config_panel.rs` — Configuration panel: tree view with inline editing for browsing/modifying config hierarchy. (~2044 tok)
- `dps.rs` — DPS tracker: rolling-window damage-per-second calculation for group members. (~904 tok)
- `group_builder.rs` — Group builder: dynamic templates, slot assignment, auto-fill for 6-man groups and raid compositions. (~1832 tok)
- `live_cast_capture.rs` — Live cast capture: logs spell cast events to file for debugging, with quantized progress bucketing. (~1428 tok)
- `session_monitor.rs` — Session monitor: fleet overview tracking connections, zone changes, deaths, level-ups, loot across all clients. (~1656 tok)
- `achievements.rs` — Achievement system: tracks milestones (platinum, raid firsts, level caps, DZ clears, session uptime). (~1208 tok)

## dmft/src/tui/ui/

- `mod.rs` — TUI renderer entry point: global chrome (header, status bar, help overlay), dispatches to per-screen renderers. (~5588 tok)
- `dashboard.rs` — Character screen renderer: operator roster grid, group scope selector, selected character detail panel. (~5848 tok)
- `spawns.rs` — Spawns screen renderer: filterable/searchable spawn list with hex dump viewer. (~2432 tok)
- `map.rs` — Map screen renderer: zone map with line/point rendering, spawn position overlay, named tracker panel, navmesh overlay. (~7976 tok)
- `groups.rs` — Groups screen renderer: dynamic grid of group panels showing per-slot HP/mana bars with buff timers. (~2880 tok)
- `navigation.rs` — Navigation screen renderer: per-character nav status table with commands reference panel. (~1644 tok)
- `widgets.rs` — Shared widget helpers: panel builder, themed headers, HP/mana color functions, cast bar renderer, spawn row styling. (~10420 tok)
- `ch_chain.rs` — CH Chain panel: Complete Heal chain configuration and real-time monitoring with cast bars and timing display. (~3024 tok)
- `dps_bars.rs` — DPS bar widget: horizontal bar chart showing per-member DPS within a group panel. (~664 tok)
- `eq_internals.rs` — EQ Internals panel: offset browser for compiled EQ memory offsets with hex dump auto-scroll. (~520 tok)
- `explorer.rs` — Ghidra function/offset explorer panel for the Debug screen. (~540 tok)
- `packets.rs` — Packet monitor panel: scrolling log of captured EQ network opcodes with direction and payload display. (~896 tok)

## dmft-common/

- `Cargo.toml` — Shared crate dependencies: serde, bincode, rusqlite, anyhow, thiserror. (~60 tok)

## dmft-common/src/

- `lib.rs` — Crate root: declares all shared modules (combat, ghidra_db, ipc, login, nav, offset_db, offsets, packet, protocol, routing, soul, types). (~120 tok)
- `combat.rs` — Combat shared types: CombatStatus FSM states, CombatRole, SpellEntry, HolyShitCondition/Action, CombatConfig, AbilitySet, RotationEntry definitions. (~5128 tok)
- `ipc.rs` — IPC command/response enums: Command variants (MoveTo, CastSpell, SetTarget, StartLogin, SetRenderMode, etc.), Response variants, CorrelationIdGenerator, SessionToken. (~5100 tok)
- `offsets.rs` — EQ memory addresses: preferred-base pointers (pinstLocalPlayer, pinstTarget, SpawnManager, etc.) and struct field offset modules (player_base, player_zone, spawn_manager, etc.). (~4424 tok)
- `nav.rs` — Navigation shared types: Waypoint, CampSpot, NavStatus FSM, StickConfig, FollowConfig, ZoneGraph, IndexedQueue, Xorshift32 PRNG. (~5832 tok)
- `login.rs` — Login shared types: AccountInfo, LoginPhase FSM, LoginError, ServerInfo, RetryPolicy with exponential backoff and jitter. (~4440 tok)
- `types.rs` — Common types: ClientId, GameState snapshot, SpawnData, SharedStateFrame, HookStatus, SlotLifecycle. (~2780 tok)
- `protocol.rs` — Wire protocol: length-prefixed bincode framing (encode/decode), 64KB max message size, DecodeBuffer for streaming. (~1320 tok)
- `ghidra_db.rs` — SQLite-backed Ghidra database: stores functions, globals, strings, imports, opcodes, call graphs from EQ binary analysis. (~2784 tok)
- `offset_db.rs` — Hot-updatable offset database (JSON): runtime EQ offset overrides without recompilation. Falls back to compile-time constants. (~1964 tok)
- `packet.rs` — Packet capture types: direction-aware filtering, opcode whitelist/blacklist, ring-buffered capture sessions, binary save/load, JSON export. (~2832 tok)
- `routing.rs` — Routing scope types: OneToon, OneGroup, AllClients for cross-client command dispatch (M8 orchestrator). (~704 tok)
- `soul.rs` — Soul Engine shared types: PersonalityTraits (Big Five + EQ traits), MoodState, SoulEvent, SoulAction, SpeechStyle, SocialTag. (~2588 tok)

## dmft-dll/

- `Cargo.toml` — DLL crate dependencies: windows, retour, tracing, dmft-common. cdylib output type. (~168 tok)

## dmft-dll/src/

- `lib.rs` — DLL entry point: cdylib loaded via reflective injection. Declares modules, EQ_BASE atomic, initialization via PoolParty thread pool. (~1388 tok)
- `dialog.rs` — Auto-accept dialog handling: scans allowlist of EQ dialogs (trade, task, rez) and auto-clicks accept/yes. (~736 tok)

## dmft-dll/src/eq/

- `mod.rs` — EQ function bindings: internal function offset constants (MainLoop, MovePlayer, CastSpell, SetTarget), combat function wrappers via transmuted fn pointers. (~2228 tok)
- `widgets.rs` — EQ UI widget primitives: CXWndManager window scan, CXStr read/write, button click via vtable, child window lookup. Confirmed working patterns for login and IPC. (~3700 tok)

## dmft-dll/src/hooks/

- `mod.rs` — Hook management: install_all/remove_all for hardware breakpoint hooks. (~92 tok)
- `game_loop.rs` — Game loop hook: intercepts CEverQuest::MainLoop via DR0 hardware breakpoint. VEH handler runs combat/nav/IPC ticks, manages stealth wake/sleep cycle. (~6348 tok)
- `hwbp.rs` — Hardware breakpoint engine: uses x86_64 debug registers (DR0-DR3) for execution breakpoints. VEH dispatcher, zero code modification, invisible to memory scans. (~1236 tok)
- `casting.rs` — Spell casting API: invokes EQ's internal CastSpell/UseAbility functions. Currently logs intent pending offset resolution. (~724 tok)
- `movement.rs` — Movement control: writes heading/speed to PlayerClient struct fields. Calc_heading (EQ 0-512 system), arrival distance, MovementController trait. (~732 tok)
- `targeting.rs` — Targeting control: writes to pinstTarget pointer. Set/clear/query current target by spawn ID. (~944 tok)
- `render.rs` — Render mode hook: intercepts CDisplay::RealRender_World. Three modes — Normal, Strobe (1 frame/5s), NullRender (zero GPU). (~852 tok)
- `dx11_null.rs` — DX11 null device hook: vtable-hooks ID3D11Device to replace textures with 1x1 stubs and buffers with 256-byte stubs in NullRender mode. Cuts GPU memory ~500MB→0. (~2088 tok)
- `fingerprint.rs` — Hardware fingerprint spoofing: intercepts SystemFingerprint to return unique per-client values for VideoCard/NetworkCard/HardDrive/ComputerName. Deterministic from session token. (~1452 tok)

## dmft-dll/src/ipc/

- `mod.rs` — DLL-side IPC: shared memory writer + named pipe listener. Commands buffered in channel, drained each game tick via poll_commands(). (~1544 tok)
- `pipe.rs` — Named pipe server (DLL side): creates pipe, listens for orchestrator commands. Persistent connections with session token auth. (~1500 tok)
- `shared.rs` — Shared memory writer (DLL side): creates named shared memory region, publishes GameState snapshots for orchestrator to read. (~1644 tok)

## dmft-dll/src/login/

- `mod.rs` — DLL-side login automation: FSM drives EQ login UI (credential entry, server select, character select, enter world). Reports progress via IPC. (~3296 tok)
- `eqmain.rs` — eqmain.dll discovery: finds eqmain.dll base address in process for login UI pointer resolution. (~708 tok)
- `widgets.rs` — Login UI widget helpers: direct memory writes to EQLogin char arrays for credentials, SIDL window lookup for splash/error dialogs. (~5384 tok)

## dmft-dll/src/nav/

- `mod.rs` — Navigation module: global Navigator singleton, init/tick/stop/command functions. (~448 tok)
- `state.rs` — Navigator FSM: Idle→Moving→Arrived→Idle with Stick/Follow/Camp modes. Inline stuck detection, warp gating, humanized movement. (~2196 tok)
- `stick.rs` — Stick-to-target engine (MQ2MoveUtils /stick equivalent): maintains configured distance from target. Supports hold, always, id, moveback modifiers. (~3716 tok)
- `stuck.rs` — Stuck detection: tracks position per tick, detects low movement, applies escalating recovery maneuvers (angle turns). (~1008 tok)
- `humanize.rs` — Movement humanization: per-character speed jitter, heading wobble, occasional path deviations. Seeded from client_id for consistency. (~800 tok)
- `warp.rs` — Warp detection: tracks target position deltas, pauses navigation on teleport/rubber-band, resumes after stable ticks. (~856 tok)
- `waypoint.rs` — Waypoint queue: ordered path storage with advance/peek/reset, backed by IndexedQueue. (~648 tok)
- `zone_graph.rs` — Zone adjacency graph reader: reads ZoneGuideManagerClient from EQ memory (888 zones, connection entries). (~1044 tok)

## dmft-dll/src/combat/

- `mod.rs` — Combat module: global Combatant singleton, init/tick/status/handle_command interface. CombatCommand enum (Engage, Disengage, SetAssistTarget). (~352 tok)
- `state.rs` — Combatant FSM: per-character combat state machine (Idle→Engaging→Casting→OnGcd loop). Integrates HolyShit evaluator, class strategy, GCD/mana/dot trackers. (~4752 tok)
- `strategy.rs` — ClassStrategy trait + CombatContext: per-class combat decision interface. build_strategy() factory dispatches to 17 class implementations. (~3660 tok)
- `rotation.rs` — Data-driven rotation engine: executes ordered action lists with conditions (rgmercs-style). RotationGroup, RotationEntry, condition evaluation. (~3276 tok)
- `holyshit.rs` — HolyShit emergency evaluator: fires before normal rotation. Priority-sorted rules with condition expressions. (~1652 tok)
- `classes/mod.rs` — Class strategy module declarations for all 17 EQ classes. (~68 tok)
- `classes/bard.rs` — Bard strategy: song twist rotation via TwistEngine, mez weaving, pull support. (~3044 tok)
- `classes/beastlord.rs` — Beastlord strategy: pet DPS + melee + slow debuff. (~820 tok)
- `classes/berserker.rs` — Berserker strategy: pure melee DPS with frenzy/rage abilities. (~820 tok)
- `classes/cleric.rs` — Cleric strategy: prioritized heal tiers (emergency/moderate), rez, heal cancel threshold, buff support. (~3132 tok)
- `classes/druid.rs` — Druid strategy: hybrid healer/nuker, emergency heals, snare on fleeing mobs. (~1144 tok)
- `classes/enchanter.rs` — Enchanter strategy: CC priority, mez off-targets, nuke when single enemy. (~1256 tok)
- `classes/generic_dps.rs` — Generic DPS strategy: fallback for any DPS class, assists MA, uses highest-priority spell. (~848 tok)
- `classes/magician.rs` — Magician strategy: pet-based DPS + nukes, pet management via /pet commands. (~656 tok)
- `classes/monk.rs` — Monk strategy: melee DPS + puller, flying kick/round kick priority, feign death. (~744 tok)
- `classes/necromancer.rs` — Necromancer strategy: DoT-focused DPS with pet, lifetap sustain, feign death escape. (~1380 tok)
- `classes/paladin.rs` — Paladin strategy: off-tank + healer hybrid, stuns, cures, emergency/moderate heals, undead nukes. (~1516 tok)
- `classes/ranger.rs` — Ranger strategy: ranged/melee hybrid, bow pulling, DoTs, stance switching by distance. (~1496 tok)
- `classes/rogue.rs` — Rogue strategy: melee DPS, backstab from behind mob, evade/hide. (~744 tok)
- `classes/shadow_knight.rs` — Shadow Knight strategy: off-tank with lifetap DPS, disease/poison DoTs, snare. (~1204 tok)
- `classes/shaman.rs` — Shaman strategy: hybrid healer/slower/DoT. Prioritizes slow, heals on low group HP, DoTs otherwise. (~1868 tok)
- `classes/warrior.rs` — Warrior strategy: main tank, rgmercs-style rotation (HateTools, Emergency, Defenses, DPS burns). (~2320 tok)
- `classes/wizard.rs` — Wizard strategy: pure nuke DPS, mana-aware spell selection. (~740 tok)
- `aggro.rs` — Heuristic aggro detection: heading + distance approximation (EQ has no explicit aggro flag). (~524 tok)
- `ability_cooldowns.rs` — Cooldown tracking for activated abilities (disciplines, AA clicks) with availability checks. (~788 tok)
- `dot_tracker.rs` — DoT tracker: prevents wasteful recasts by tracking active DoTs per target with expiration ticks. (~972 tok)
- `gcd.rs` — GCD tracker: tracks global cooldown (~1.5s / ~30 frames) between spell casts. (~520 tok)
- `humanize.rs` — Combat humanization: per-character jitter on assist timing, cast delay, med threshold. Seeded from client_id. (~604 tok)
- `loot.rs` — Loot automation: detects nearby corpses by spawn_type, uses /loot and /lootall slash commands. (~784 tok)
- `mana.rs` — Mana governor: enforces mana floor percentage to prevent OOM. Healer-aware (healers get lower floor). (~508 tok)
- `mez_queue.rs` — Mez queue: multi-target CC tracking for enchanters/bards. Auto-refresh before break, resist retry tracking. (~1060 tok)
- `positioning.rs` — Melee positioning: tank facing, rogue backstab angle, camp range enforcement. (~1408 tok)
- `puller.rs` — Puller FSM: Ready→Pulling→Waiting→Returning cycle. Chain pull cooldown, return-to-camp after pull. (~1396 tok)
- `skill_cooldowns.rs` — Per-skill melee cooldown tracking: individual timers for kick, bash, backstab, taunt, etc. (~900 tok)
- `twist.rs` — Bard TwistEngine: custom song rotation with category priority (Haste, SpellFocus, Tank, etc.) and hold support. (~3508 tok)

## dmft-dll/src/stealth/

- `mod.rs` — Stealth module: three-layer per-frame sleep obfuscation (page_guard + text_encrypt + stack_spoof). Wake/sleep toggle functions. (~544 tok)
- `alloc.rs` — Stealth memory allocator: RtlAllocateHeap/NtCreateSection instead of VirtualAlloc to avoid detection signatures. (~1468 tok)
- `etw_blind.rs` — Patchless ETW blinding: hardware breakpoint on NtTraceEvent, VEH returns STATUS_SUCCESS and skips function body. Zero code modification. (~916 tok)
- `page_encrypt.rs` — Nighthawk-style page-level encryption: XOR-encrypts code pages, VEH decrypts on access. Only one page readable at any time (~2% exposure). (~1444 tok)
- `page_guard.rs` — VirtualProtect toggle: flips .text between RW and RX. Layer 1 of per-frame sleep obfuscation. (~204 tok)
- `pe_erase.rs` — PE header erasure: zeroes DOS and NT headers of the loaded DLL. (~164 tok)
- `peb_unlink.rs` — PEB module unlinking: removes DLL from the three PEB module lists (InLoadOrder, InMemoryOrder, InInitializationOrder). (~368 tok)
- `section_remap.rs` — Section remapping: triggers copy-on-write to convert MEM_IMAGE to MEM_PRIVATE, avoiding image-based detection. (~192 tok)
- `stack_spoof.rs` — Call stack spoofing: overwrites return addresses with legitimate ntdll/kernel32 addresses before sensitive API calls. (~1292 tok)
- `text_encrypt.rs` — SIMD XOR encryption of .text section: SSE2 128-bit XOR with random 16-byte key refreshed each cycle. Layer 2 of sleep obfuscation. (~1324 tok)
- `thread_pool.rs` — PoolParty-style thread pool execution: uses CreateThreadpoolWork instead of CreateThread/CreateRemoteThread. No suspicious thread creation events. (~404 tok)

## dmft-dll/src/syscall/

- `mod.rs` — Indirect syscall layer (RecycledGate pattern): DJB2 hashing + TartarusGate SSN extraction + assembly gate stubs. (~420 tok)
- `gate.rs` — RecycledGate indirect syscall invocation: assembly stubs that JMP to ntdll's syscall;ret gadget for legitimate-looking call stacks. (~1280 tok)
- `hash.rs` — DJB2 hash function: const-evaluable hash for NT API name resolution without plaintext strings in binary. (~292 tok)
- `table.rs` — TartarusGate syscall table: maps fresh ntdll from KnownDlls, walks exports to extract SSNs, finds syscall;ret gadgets. (~2628 tok)

## dmft-web/

- `Cargo.toml` — Web dashboard crate dependencies: axum, tokio, tower-http, serde_json. (~56 tok)

## dmft-web/src/

- `main.rs` — Web dashboard server: Axum backend serving React SPA, REST API, WebSocket endpoint for live monitoring. (~224 tok)
- `api.rs` — REST API handlers: health check endpoint. Placeholder for credentials, group config, loot table APIs. (~244 tok)
- `ws.rs` — WebSocket handler: upgrades HTTP to WebSocket for real-time session event streaming. (~224 tok)

## docs/

- `README.md` — Docs index and navigation guide. (~varies tok)
- `anti-detection.md` — Anti-detection strategy: HWBP hooks, reflective injection, sleep obfuscation, ETW blinding. (~varies tok)
- `audit-2026-03-29.md` — Code audit results from March 29, 2026. (~varies tok)
- `autonomous-agent-pipeline.md` — Agent pipeline docs: Claude/Codex issue workers, routing, CI gates. (~varies tok)
- `class-combat-rotations.md` — Combat rotation documentation for all 17 EQ classes. (~varies tok)
- `claude-issue-worker.md` — Claude Code issue worker protocol and workflow. (~varies tok)
- `code-review-session3.md` — Code review notes from session 3. (~varies tok)
- `dll-injection-plan.md` — DLL injection design: classic vs reflective, PoolParty, stealth staging. (~varies tok)
- `eq-ini-optimization.md` — EQ client INI optimization for multibox performance. (~varies tok)
- `eq-maps-research.md` — EQ map file format research and rendering strategy. (~varies tok)
- `frostreaver-farming-guide.md` — Frostreaver farming guide for the Frostreaver camp. (~varies tok)
- `implementation-roadmap.md` — Full milestone roadmap (M1-M11) with status and priorities. (~varies tok)
- `local-claude-research.md` — Local Claude/LLM research notes. (~varies tok)
- `mq2-comparison.md` — DMFT vs MacroQuest2 feature comparison. (~varies tok)
- `mq2-deep-dive.md` — Deep dive into MQ2 internals for reference. (~varies tok)
- `orchestration-design.md` — M8 orchestrator design: session relay, group coordination. (~varies tok)
- `p99-zone-guide-detailed.md` — P99/TLP zone guide with level ranges and camp spots. (~varies tok)
- `rdp-automation-research.md` — RDP automation research for remote control. (~varies tok)
- `redguides-automation-research.md` — RedGuides automation ecosystem analysis. (~varies tok)
- `remote-control-setup.md` — Remote control setup guide (WinRM, VNC, RDP). (~varies tok)
- `roadmap-review.md` — Roadmap review and prioritization notes. (~varies tok)
- `security-audit-2026-03-27.md` — Security audit from March 27, 2026. (~varies tok)
- `tui-map-navmesh-audit-2026-03-31.md` — TUI map and navmesh integration audit. (~varies tok)
- `tui-mockup.txt` — ASCII TUI mockup for dashboard layout. (~varies tok)
- `vnc-research.md` — VNC research for remote EQ client control. (~varies tok)
- `wineq-research.md` — WinEQ2 research for multibox window management. (~varies tok)

## docs/external-research/

- `README.md` — External research index. (~varies tok)
- `ability-packet-coverage-and-targetability-validation.md` — EQ ability packet structures and validation. (~varies tok)
- `automation-source-ledger.md` — Automation source tracking ledger. (~varies tok)
- `daybreak-detection-digest.md` — Daybreak anti-cheat detection methods digest. (~varies tok)
- `eq-protocol-public-research.md` — EQ network protocol public research compilation. (~varies tok)
- `jmb-session-and-relay-comparison.md` — JMB vs ISBoxer session/relay model comparison. (~varies tok)
- `kissassist-gap-and-tui-translation.md` — KissAssist feature gap analysis and TUI translation. (~varies tok)
- `m8-cross-client-control-model.md` — M8 cross-client control architecture. (~varies tok)
- `mq2-parity-matrix.md` — MQ2 feature parity tracking matrix. (~varies tok)
- `packet-engine-send-receive-pipeline.md` — EQ packet engine send/receive pipeline analysis. (~varies tok)
- `packet-zoning-send-path-and-state-ledger.md` — Zoning packet send paths and state tracking. (~varies tok)
- `runeq-headless-eq-analysis.md` — RunEQ headless EQ client analysis. (~varies tok)
- `syscall-evasion-and-ntdll-unhooking.md` — Syscall evasion techniques and ntdll unhooking. (~varies tok)
- `syscall-evasion-hellsgate-halosgate.md` — HellsGate/HalosGate syscall evasion patterns. (~varies tok)
- `tui-map-and-polish-research.md` — TUI map rendering and polish research. (~varies tok)
- `zoning-queue-and-safe-coord-validation.md` — Zone transition queue and coordinate validation. (~varies tok)

## docs/research/

- `rgmercs-analysis.md` — RGMercs automation analysis: rotation engine, class configs. (~varies tok)

## docs/research-imports/2026-04-02-packet-zoning/

- `EQ_Ability_Packet_Structures.md` — EQ ability packet structure documentation. (~varies tok)
- `EQ_AntiCheat_HookDetection_Notes.md` — EQ anti-cheat hook detection notes. (~varies tok)
- `EQ_Network_Architecture.md` — EQ network architecture documentation. (~varies tok)
- `EQ_Zoning_System.md` — EQ zoning system documentation. (~varies tok)
- `README.md` — Research import index for 2026-04-02 packet/zoning batch. (~varies tok)
- `movement-validation.raw.txt` — Raw movement validation research data. (~varies tok)
- `zoning-fingerprint-results.raw.txt` — Raw zoning fingerprint test results. (~varies tok)

## docs/superpowers/plans/

- `2026-03-24-m2-input-dispatch.md` — M2 input dispatch plan. (~varies tok)
- `2026-03-27-m2-dll-injection.md` — M2 DLL injection plan. (~varies tok)
- `2026-03-27-m3-navigation.md` — M3 navigation plan. (~varies tok)
- `2026-03-31-fix-copilot-branch-push.md` — Fix for Copilot branch push issue. (~varies tok)

## docs/superpowers/specs/

- `2026-03-29-auto-login-design.md` — Auto-login system design specification. (~varies tok)

## docs/wiki/

- `Home.md` — Wiki home page. (~varies tok)
- `_Sidebar.md` — Wiki sidebar navigation. (~varies tok)
- `Architecture-Overview.md` — System architecture overview. (~varies tok)
- `Combat-and-Camp-Loop.md` — Combat and camp loop documentation. (~varies tok)
- `Command-Reference.md` — TUI command reference. (~varies tok)
- `Configuration.md` — Configuration guide. (~varies tok)
- `DLL-Injection-and-IPC-Pipeline.md` — DLL injection and IPC pipeline docs. (~varies tok)
- `Development-Workflow.md` — Development workflow guide. (~varies tok)
- `Installation-and-Build.md` — Installation and build instructions. (~varies tok)
- `Login-Automation.md` — Login automation documentation. (~varies tok)
- `Maintaining-the-Wiki.md` — Wiki maintenance guide. (~varies tok)
- `Navigation-and-Maps.md` — Navigation and map system docs. (~varies tok)
- `Offsets-EQ-Internals-and-MacroQuest-References.md` — EQ offsets and MQ reference docs. (~varies tok)
- `Operating-the-TUI.md` — TUI operation guide. (~varies tok)
- `Quick-Start.md` — Quick start guide. (~varies tok)
- `Roadmap-and-Known-Gaps.md` — Roadmap and known gaps. (~varies tok)
- `Security-and-Anti-Detection-Notes.md` — Security and anti-detection notes. (~varies tok)
- `Soul-Engine.md` — Soul Engine documentation. (~varies tok)
- `Troubleshooting.md` — Troubleshooting guide. (~varies tok)

## scripts/

- `check_dll_log.ps1` — PowerShell: checks DLL injection log output for errors. (~468 tok)
- `cmd_all.bat` — Batch: sends a command to all running EQ clients. (~160 tok)
- `dev-preflight.py` — Python: pre-flight checks before development (format, clippy, test). (~1704 tok)
- `fix_stickfigures.ps1` — PowerShell: fixes stick figure sprite rendering issues. (~72 tok)
- `gen-handoff.sh` — Shell: generates Cowork handoff prompt with task context. (~748 tok)
- `git_prune.sh` — Shell: prunes stale git branches and worktrees. (~1076 tok)
- `launch_and_login.bat` — Batch: launches EQ and runs login automation. (~276 tok)
- `launch_group1.bat` — Batch: launches group 1 EQ clients. (~456 tok)
- `launch_group1.ps1` — PowerShell: launches group 1 with staggered timing. (~412 tok)
- `mark-roadmap-containers-skip-ready.sh` — Shell: marks roadmap container issues as skip-ready. (~368 tok)
- `optimize_ini.ps1` — PowerShell: optimizes EQ INI settings for multibox performance. (~140 tok)
- `reconcile-agent-queue.sh` — Shell: reconciles agent issue queue status with GitHub project. (~1912 tok)
- `remote_api.ps1` — PowerShell: remote API server for WinRM-based control. (~1340 tok)
- `setup-self-hosted-runner.ps1` — PowerShell: sets up GitHub Actions self-hosted runner. (~1004 tok)
- `setup-windows.ps1` — PowerShell: Windows development environment setup. (~940 tok)
- `start_remote_api.bat` — Batch: starts the remote API server. (~16 tok)
- `sync_project.py` — Python: syncs GitHub project board issues and labels. (~1900 tok)
- `sync_wiki.py` — Python: syncs docs/wiki/ to GitHub wiki repository. (~1620 tok)
- `test-windows.ps1` — PowerShell: runs Windows-specific test suite. (~1052 tok)
- `test_single_login.bat` — Batch: tests single-client login flow. (~276 tok)
- `update_readme_metrics.py` — Python: updates README with test counts and line metrics. (~416 tok)
- `validate_roadmap_unknowns.py` — Python: validates roadmap items for unknown/missing fields. (~1012 tok)
- `verify_injection.bat` — Batch: verifies DLL injection succeeded. (~496 tok)
- `winrm_exec.py` — Python: executes commands on remote Windows host via WinRM. (~688 tok)
