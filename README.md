# TextQuest

[![CI](https://github.com/Maleick/TextQuest/actions/workflows/ci.yml/badge.svg)](https://github.com/Maleick/TextQuest/actions/workflows/ci.yml)
[![Release](https://github.com/Maleick/TextQuest/actions/workflows/release.yml/badge.svg)](https://github.com/Maleick/TextQuest/actions/workflows/release.yml)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange?style=flat-square)](https://www.rust-lang.org/)
[![Rust LOC](https://img.shields.io/badge/Rust%20LOC-108%2C448-blue?style=flat-square)](#testing)
[![Tests](https://img.shields.io/badge/Tests-2%2C569%20exact-brightgreen?style=flat-square)](#testing)
[![Status](https://img.shields.io/badge/status-Active-green?style=flat-square)](#roadmap)
[![License](https://img.shields.io/badge/license-Private-red?style=flat-square)](#license)

External process memory reader, DLL injector, and multibox controller for EverQuest, built in Rust.

TextQuest reads live game state from EQ client memory, injects a DLL for direct control via internal function calls (InterpretCmd), and orchestrates up to 36 characters across a TLP multibox setup.

If you plan to do offset, struct, or MacroQuest reference work, clone with submodules:

```bash
git clone --recurse-submodules https://github.com/Maleick/TextQuest.git
cd TextQuest

# Existing clone
git submodule update --init --recursive
```

Routine `cargo build` / `cargo test` work does not require the reference trees, but `third_party/eqlib` and `third_party/macroquest` are the canonical local sources for reference work. See `third_party/README.md` for the layout.

## Status

**DLL injection + command execution confirmed working on live eqgame.exe** (March 2026 build). Login automation (Phases 1-3) working end-to-end: credential entry, server select, character select, Enter World. Two characters successfully grouped, following, sitting/standing via remote commands.

## Features

### Core

- **DLL Injection** — Rust `cdylib` injected via CreateRemoteThread + LoadLibraryW, staged with randomized names
- **InterpretCmd** — Calls EQ's internal `CEverQuest::InterpretCmd` to execute any slash command invisibly
- **Game State Publishing** — DLL reads HP/mana/target/nearby spawns every tick, publishes via shared memory
- **IPC Pipeline** — Named pipes (commands) + shared memory (game state) with current-user DACL security
- **Render Mode System** — Three modes: Normal (full render), Strobe (1 frame per 5 sec, ~97% GPU savings), NullRender (zero rendering). DX11 hooks use DXGI Present vtable approach to intercept `ID3D11Device`, replacing textures with 1×1 and buffers with 256 bytes, saving ~500 MB per background client

### TUI Dashboard (5 screens, 4 themes)

| Screen         | Key | Description                                                                                                                           |
| -------------- | --- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Characters     | `1` | Operator roster, selected character detail with class emblem sprites, toggleable group/scope panels                                   |
| Map            | `2` | Zone geometry (Brewall maps), group markers, HP status overlay, spawn overlay, named mob tracker with respawn timers, Z-slice control |
| Navigation     | `3` | Per-character Zone, Status, and Destination, with route progress, recovery state, and waypoint queue                                  |
| Debug          | `4` | Full spawn list with live search, type filter (All/PC/NPC/Named), EQ Internals, hex dump with annotations, target detail              |
| Packet Monitor | `5` | Opcode sniffer with live filtering, protocol decode, send/recv separation                                                             |

**Themes:** Dark Modern (default), Dracula, Classic, Neriak Third Gate — cycle with `T`

**Widget library:** Sparklines, gauge bars, scrollable lists, tooltips, badges, notification area, inline hints, multi-option selectors, and scrollbar indicators. Context-sensitive help overlay with per-screen keybinding hints, did-you-mean suggestions for commands, and a comprehensive scrollable reference.

### TUI Controls

| Key         | Action                                                |
| ----------- | ----------------------------------------------------- |
| `1-5`       | Switch screens                                        |
| `Shift+1-6` | Focus group G1-G6                                     |
| `Shift+0`   | All groups (clear group focus)                        |
| `Tab`       | Cycle focused pane                                    |
| `[` / `]`   | Cycle between EQ clients                              |
| `/`         | Search spawns (live typing)                           |
| `f`         | Cycle spawn type filter                               |
| `g`         | Toggle group section (Characters screen)              |
| `v`         | Toggle scope section (Characters screen)              |
| `z`         | Collapse focused section                              |
| `+` / `-`   | Adjust Tactical Z slice                               |
| `m`         | Maximize Tactical map                                 |
| `p`         | Privacy mode (redacts names + server for screenshots) |
| `T`         | Cycle theme                                           |
| `:`         | Command mode                                          |
| `?`         | Help overlay (scrollable, context-sensitive)          |
| `q`         | Quit                                                  |

**Status Glyphs:** `⚔`/`✚`/`✦` Fight/Heal/Cast · `➜`/`✓`/`!` Navigate/Arrived/Stuck · `☾`/`⇣`/`⌕` Sit/Feign/Loot

### Command Bar (`:` mode)

```text
:<name> /sit             Send slash command to character
:G1-G6 /cmd             Send to group
:all /sit                Broadcast to all clients
:camp start|stop|list|status|next|prev  Camp loop control
:camp add|remove        Add/remove camp config
:nav <dest>              Navigate to camp, coords, or slash fallback
:track <name>            Track a spawn
:ma <name>               Set Main Assist
:mt <name>               Set Main Tank
:engage / :disengage     Start/stop combat
:invite <name>           Group invite
:accept                  Accept group invite
:mode camp|hunt          Set operating mode
:ch start <pids> <int>   Start CH chain
:ch stop|add|remove      CH chain management (`rm` also works)
:ch adaptive on|off      Adaptive CH timing
:help                    Show all commands
```

### Camp Loop Automation

- **6-phase state machine**: Idle -> Pulling -> Fighting -> Looting -> Medding -> Buffing
- **Smart decisions** from real game state (HP/mana-driven, not timers)
- **16 class ability configs** (TOML) with cooldowns, priorities, conditions
- **CC system**: Charm/mez tracking, Tash->Malo debuff chain, charm break emergency response
- **Rogue backstab positioning**: Calculates behind-target position using EQ heading math
- **Intelligent pull target selection**: Filters by distance/type, prefers HVT watchlist targets
- **Buff maintenance**: Tracks durations, auto-rebuffs during idle/med
- **Death recovery**: Detects deaths, cleric rez commands, rebuff sequence
- **Sell/bank cycle**: Navigate to vendor, sell, return to camp

### Soul Engine

- **Personality system** — Per-character mood, traits, and behavioral profiles with deterministic personality engine
- **Persistent memory** — SQLite-backed memory database for long-term character state
- **Social dynamics** — Social graph tracking relationships between characters
- **Idle behavior** — Personality-driven actions during downtime
- **LLM integration** — Async request queue for provider-backed character responses (tracked under `M10` in the canonical roadmap)

### Login Automation

- **Credential store** — Argon2id + AES-256-GCM encrypted credentials in SQLite, with CLI management (`--add`, `--list`, `--password`/`--master-password` flags)
- **Login FSM** — Full end-to-end chain: credential entry → server select → character select → Enter World
- **Launch coordinator** — Staggered multi-client launch with post-login sequencing
- **Process spawner** — Spawns and manages EQ client processes
- **Daemon CLI** — Headless orchestrator mode for scripted/remote operation

### Navigation

- **Navmesh pathfinding** — Detour-based pathfinding via C++ FFI shim
- **888-zone BFS routing** — Zone-to-zone route planning across the full EQ world
- **Navigator FSM** — Waypoint following with stuck detection and recovery
- **Movement humanization** — Natural-looking movement patterns
- **Waypoint recording** — RDP simplification for path recording
- **Travel diagnostics** — Navigation screen calls out fallback routing, stuck recovery, and pending zone-match blockers

### Combat

- **17 class strategies** — ClassStrategy trait with per-class implementations including generic DPS fallback
- **Puller FSM** — Automated pull cycle with target selection and aggro management
- **HolyShit system** — Emergency response conditions (low HP, charm break, adds)
- **GCD tracker + mana governor** — Intelligent ability timing and resource management
- **CH chain** — Coordinated Complete Heal rotation with adaptive timing
- **Skill cooldown tracking** — Per-ability cooldown management across all classes

### Anti-Detection

- **CSPRNG session tokens** (not PID-derived)
- **Randomized DLL staging names** (CSPRNG filename, not static)
- **Restrictive pipe DACL** (current user only)
- **Session-derived IPC naming** — active helpers derive names from the per-session token rather than a simple fixed public prefix
- **Human-like command jitter** (triangle distribution + hesitation spikes)
- **Per-character personality profiles** (reaction speed, aggression, discipline variation)
- **GM flag detection** (alerts on GM spawns)
- **Render mode system** (Normal / Strobe / NullRender with DX11 null device hooks)

### Named Spawn Tracker

- Detects named mobs (filters generic "a goblin" names)
- HVT watchlist (`config/hvt_watchlist.toml`) with Discord alert support
- Tracks up/down status with respawn timer estimation
- Map shows `!` for live named, `X` for dead with countdown

### EQ Log Parser

- Parses loot, kills, money, XP, deaths, zone changes from EQ log files
- Live session stats on TUI dashboard (XP/hr, plat/hr, top items)
- Feeds into LootDatabase for economy analysis

## Architecture

```text
TextQuest Workspace (3 crates, ~108K lines of Rust)
├── textquest/           — Orchestrator: TUI, camp loop, process reading, injection, soul engine
├── textquest-dll/       — Injected DLL: hooks, game state reader, IPC, render strobing, combat
└── textquest-common/    — Shared types: IPC, offsets, combat/nav/soul types
```

### Command Pipeline

```text
TUI :command  →  Orchestrator  →  Named Pipe  →  DLL  →  InterpretCmd  →  EQ
     or  (invisible to game)
Discord msg
```

### Camp Loop

```text
Orchestrator ticks camp loop → reads game state from shared memory →
generates (pid, slash_command) pairs per role → sends via IPC pipe →
DLL executes InterpretCmd with human-like jitter delay
```

## Quick Start

### Developer Preflight (optional, recommended)

```bash
python3 scripts/dev-preflight.py
python3 scripts/dev-preflight.py --require-reference-trees
python3 scripts/dev-preflight.py --init-submodules --require-reference-trees
```

Use the default run for routine `cargo build` / `cargo test` work. Add
`--require-reference-trees` when you plan to inspect or cite
`third_party/eqlib` or `third_party/macroquest`. On Windows, use `py -3`
instead of `python3`, or run `scripts\setup-windows.ps1` for full machine setup.

### GitHub Actions Self-hosted Runner (Windows)

For workflows that now target `self-hosted` Windows runners, use:

```powershell
.\scripts\setup-self-hosted-runner.ps1 -Token "<NEW_GITHUB_TOKEN>" -InstallService
```

Run `setup-self-hosted-runner.ps1` from an elevated PowerShell session for automatic service install.
If `svc.cmd` is not present in that runner package, the script prints `sc.exe` fallback commands.
For unattended PR merges and nightly jobs, keep this `textquest` runner on a dedicated
always-on Windows box or VM instead of a personal laptop. The canonical bootstrap flow
is [`scripts/setup-self-hosted-runner.ps1`](scripts/setup-self-hosted-runner.ps1).

CI and nightly automation:

- `.github/workflows/wiki-nightly.yml` validates `docs/wiki/` and publishes the GitHub wiki at 3 AM America/Chicago using runner-local `gh auth`
- `.github/workflows/nightly-release.yml` builds a rolling nightly prerelease containing `textquest.exe` and `textquest_dll.dll`
- `.github/workflows/ci.yml` keeps the required `PR gate (fmt + clippy + test + python)` on the self-hosted runner for same-repo PRs, pushes to `master`, and manual dispatches; fork PRs use GitHub-hosted Windows instead
- self-hosted CI/wiki jobs use runner-local `python` / `py -3` when available, otherwise they fall back to the official Python 3.12.10 embeddable ZIP with a pinned SHA-256 check before extraction
- `.github/workflows/wiki-nightly.yml` validates `docs/wiki/` and publishes the GitHub wiki at 3 AM America/Chicago using the workflow-provided `GH_TOKEN` (`secrets.GITHUB_TOKEN`) for `gh`
- `.github/workflows/nightly-release.yml` builds a rolling nightly prerelease containing `textquest.exe` and `textquest_dll.dll`; `wiki-nightly` follows that run against the same built commit SHA
- `.github/workflows/ci.yml` keeps the required `PR gate (fmt + clippy + test + python)` on the self-hosted runner for same-repo PRs, pushes to `master`, and manual dispatches; fork PRs use GitHub-hosted Windows instead
- self-hosted CI/wiki jobs use runner-local `python` / `py -3` when available, otherwise they fall back to the official Python 3.12.10 embeddable ZIP with a pinned SHA-256 check before extraction
- `.github/workflows/copilot-ci-dispatch.yml` runs on GitHub-hosted Linux from `master`, dispatches `CI` on same-repo Copilot PR heads when GitHub leaves the PR-triggered run in `action_required`, and skips PRs that edit workflow files so approval-sensitive changes still require manual review

If this runner will also mirror GitHub Projects, refresh the CLI scopes on the runner account:

```powershell
gh auth status
gh auth refresh -s project -s read:project
```

### Git Hygiene (PRs + stale branches)

```bash
# Preview cleanup operations (default: dry-run)
scripts/git_prune.sh

# Apply local cleanup. By default this protects main/master, release/*,
# hotfix/*, codex/*, copilot/*, dependabot/*, the current branch, and the base branch.
scripts/git_prune.sh --apply

# Add extra protected globs for long-lived branches
scripts/git_prune.sh --apply --protect 'feature/keep-*'

# Also delete merged remote PR branches (requires gh auth)
scripts/git_prune.sh --apply --include-remote
```

The script auto-detects the base branch from local `main`, local `master`, then
`origin/HEAD` unless you pass `--base`.

Stale local branches are only deleted by default when they are already merged into the
base branch or their upstream has disappeared. Use `--force-stale` if you really want
age-only pruning.

### Protected `master` workflow

`master` remains the protected release branch for TextQuest.

1. Branch from `master` into a short-lived topic branch (`feature/*`, `hotfix/*`, `codex/*`, etc.).
2. Push that branch. The expected path is that Codex or Claude opens the pull request back into `master`, though you can still open one manually if needed.
3. GitHub requires `PR gate (fmt + clippy + test + python)` on every PR, including README-only and docs-only changes.
4. Keep the PR up to date with `master`, address review comments in the PR thread, and merge once the required gate is green.
5. Let GitHub auto-delete the merged topic branch. Auto-merge can stay enabled when the gate is already satisfied.

TextQuest-specific notes:

- The required merge blocker remains `PR gate (fmt + clippy + test + python)`.
- Same-repo PRs, pushes to `master`, and manual `CI` dispatches run that gate on runner labels `self-hosted`, `Windows`, `X64`, and `textquest`.
- Fork or otherwise untrusted PRs run the same visible gate name on GitHub-hosted `windows-latest` instead of the self-hosted runner.
- The GitHub-hosted fork path uses `actions/setup-python@v6`; the self-hosted path stays cmd-safe and verifies any fallback Python ZIP before extraction.
- That Windows gate currently boots the nightly MSVC Rust toolchain, because the Windows hook stack still depends on nightly-only `retour`.
- The scheduled `TextQuest PR manager` Codex cloud automation is expected to open missing PRs, address straightforward review feedback, and merge eligible branches into `master`.
- Manual `CI` workflow dispatch is the place to get the heavier `Windows release build (manual)` validation on a topic branch before merge.
- Trusted agent PRs should carry `merge:auto` by default unless the PR or linked issue is labeled `human:required`, `risk:high`, or `agent:blocked`.
- The scheduled `TextQuest issue executor` opens trusted agent PRs into `master`, adds automation labels, and should default `merge:auto` on those PRs when the linked issue is not explicitly blocked from unattended merge.
- The scheduled `TextQuest PR manager` Codex cloud automation is expected to address straightforward review feedback, resolve clearly addressed bot review threads, merge eligible agent-authored PRs into `master`, and close stale or superseded agent-authored PRs when the queue has moved on.
- `.github/workflows/agent-ready.yml` keeps the `agent:ready` and `agent:skip-ready` labels aligned on issue events plus an hourly sweep, suppresses `agent:ready` while an issue already has an open linked PR or active `agent:working` / `agent:blocked` state, and treats roadmap-container titles that start with `M<number>` or `Mx` as skip-ready epics.
- `scripts/reconcile-agent-queue.sh` plus the scheduled TextQuest issue-queue reconciler automation add missing open issues to the `TextQuest Roadmap` project, set `Agent Status`, strip stale `agent:ready` / `agent:working` labels from non-ready items, and promote every other open non-epic issue to `Ready for Agent`.
- The `agent:close` label lets repo automation close only agent-authored PRs (`codex/*`, `claude/*`, or PRs carrying the `codex-automation` label) without touching unrelated human PRs.
- Manual `CI` workflow dispatch can opt into the heavier `Windows release build (manual)` validation on a topic branch before merge.
- `Release`, `Nightly Release`, `README Metrics`, and `Wiki Nightly` are not required merge gates.
- `README Metrics` should now be run on a topic branch and merged via PR instead of pushing directly into `master`.
- If the single Windows runner starts queueing behind nightly or release work, add a second runner with the same labels instead of redesigning the workflow.

### Development (any platform — demo mode)

```bash
cargo build              # Debug build
cargo run                # TUI with demo data (auto-detected on non-Windows)
cargo test               # Run the full workspace test suite
cargo clippy --all-targets --all-features -- -D warnings
```

**Demo mode** activates automatically when no live EQ process is found (always on macOS/Linux, on Windows when EQ isn't running). It populates the TUI with 18 simulated characters across 3 groups covering all 16 EQ classes:

| Group | Zone           | Classes                      |
| ----- | -------------- | ---------------------------- |
| G1    | Permafrost     | WAR, CLR, ENC, BRD, RNG, WIZ |
| G2    | Eastern Wastes | SK, SHM, DRU, ROG, NEC, MAG  |
| G3    | Great Divide   | PAL, MNK, BST, BER, CLR, WIZ |

Each zone has NPC spawns (including named bosses like Lady Vox, Wuoshi, Garudon), corpses, and realistic HP/mana values. This lets you develop and test all TUI screens without a live EQ client.

### Production (Windows — live EQ)

```powershell
# Build
cargo build --release

# Run TUI dashboard (live mode with connected EQ clients)
target\release\textquest.exe

# One-shot CLI dump of player, target, and spawn data
target\release\textquest.exe --dump

# Inject DLL into all running EQ clients
target\release\textquest.exe --inject

# Inject DLL into a specific client by PID
target\release\textquest.exe --inject-pid <pid>

# Start login automation for a specific client
target\release\textquest.exe --login-pid <pid> <account> <password> [server] [character]

# Query shared memory state for a single client
target\release\textquest.exe --status <pid>

# Summary table of all connected EQ clients
target\release\textquest.exe --statusall

# Send a slash command to a specific client
target\release\textquest.exe --cmd <pid> "/sit"
```

### Log Files

- **Orchestrator:** `./logs/textquest.log` (daily rolling)
- **DLL:** `%TEMP%/textquest/textquest-dll.log` (daily rolling)

## Testing

Current workspace totals: 108,448 Rust lines and 2,569 exact tests. This line and the badges above are auto-refreshed by `scripts/update_readme_metrics.py`. The required PR gate keeps a single visible check name across trusted and untrusted PRs:

| Trigger                | Jobs                                                                   |
| ---------------------- | ---------------------------------------------------------------------- |
| Same-repo pull request | self-hosted `PR gate (fmt + clippy + test + python)`                   |
| Fork pull request      | GitHub-hosted `PR gate (fmt + clippy + test + python)` on Windows      |
| Push to master         | self-hosted `PR gate (fmt + clippy + test + python)`                   |
| Manual `CI` dispatch   | required PR gate, with optional `Windows release build (manual)` input |

Tag-triggered releases (`v*`) build Windows binaries and create GitHub Releases automatically.

Release and wiki automation now run separately on the self-hosted Windows runner:

- wiki auto-publish via `scripts/sync_wiki.py --check`
- wiki auto-publish via `scripts/sync_wiki.py --push`
- rolling nightly prerelease build and artifact upload

## Configuration

### Accounts (`config/accounts.toml`)

```toml
[[accounts]]
name = "textquest01"
server = "Firiona Vie"
character = "Camrene"
class = "WAR"
group = 1
```

### Camp Configs (`config/camps/*.toml`)

```toml
name = "crushbone_entrance"
zone = "crushbone"
camp_center = [500.0, -200.0, 3.0]
pull_point = [550.0, -180.0, 3.0]
pull_radius = 150.0
camp_radius = 30.0
rest_mana_pct = 20
pull_mana_pct = 60
```

### Class Ability Configs (`config/classes/*.toml`)

16 classes: WAR, CLR, PAL, RNG, SK, DRU, MNK, BRD, ROG, SHM, NEC, WIZ, MAG, ENC, BST, BER

- Optional `[[level_overrides]]` blocks gate alternate combat/buff/emergency/cc/debuff ability lists by level range; categories omitted inside an override fall back to the base class lists, and the base profile is used when no override matches.

### HVT Watchlist (`config/hvt_watchlist.toml`)

```toml
[[targets]]
name = "Emperor Crush"
zone = "crushbone"
priority = "high"
alert_discord = true
```

### EQ Client Optimization

Run `scripts\optimize_ini.ps1` to apply minimal settings:

- StickFigures=1, Shadows=0, MaxBGFPS=10, AllLuclinPcModelsOff=1
- Expected: ~500MB RAM per client (down from ~2GB)

## Roadmap

Canonical roadmap source:

- `docs/implementation-roadmap.md`

Historical milestones already implemented in the repository:

- [x] **M1** — External memory reading + TUI dashboard
- [x] **M2** — DLL injection + function hooking + IPC + self-healing monitor
- [x] **M2.5** — Login automation + encrypted credential store + launch coordinator
- [x] **M3** — Navigation — navmesh pathfinding (Detour), 888-zone BFS routing, movement humanization
- [x] **M4** — Combat automation — 17 class strategies, puller FSM, HolyShit system, CH chain

Canonical active roadmap order:

- [ ] **M5** (~95%) — Anti-Cheat — stealth stack shipped (PoolParty injection, stack spoofing, fingerprint spoofing, sleep obfuscation, page encryption, ETW blinding, stealth allocator). 1 open issue (#355 launchpad token RE)
- [ ] **M6** (~55%) — Web Dashboard + TUI — EQ Internals, packet monitor, map rework, DPS bars, Neriak theme shipped; web dashboard scaffold (Axum + React/Vite/Tailwind), fleet metrics (SQLite), Discord webhooks in progress
- [ ] **M7** — Zoning/Movement
- [ ] **M8** — Orchestrator
- [ ] **M9** — Learning/RL
- [ ] **M10** — Soul Engine + LLM
- [ ] **M11** — Economy

Execution rules:

- external research can add milestone slices, but it cannot reorder milestones on its own
- `docs/implementation-roadmap.md` is the source of truth for milestone gates and evidence states
- GitHub Projects mirror the roadmap; they do not replace the repo docs as the source of truth

## Research Docs

- `docs/implementation-roadmap.md` — canonical roadmap, evidence model, milestone gates
- `docs/external-research/automation-source-ledger.md` — primary, secondary, and low-confidence source ledger
- `docs/external-research/packet-zoning-send-path-and-state-ledger.md` — curated `M5`/`M6` control-path ledger that separates in-process defaults from packet candidates and blocked protocol gaps
- `docs/external-research/kissassist-gap-and-tui-translation.md` — KissAssist capability audit and native TextQuest TUI translation targets
- `docs/external-research/daybreak-detection-digest.md` — official Daybreak policy anchors, `M5`-`M8` risk gates, and operator hygiene inputs
- `docs/external-research/zoning-queue-and-safe-coord-validation.md` — curated `M6` checkpoint note for queue flush, timeout, and safe-coordinate recovery
- `docs/research-imports/2026-04-02-packet-zoning/` — raw packet and zoning evidence archive
- `docs/orchestration-design.md` — 7-phase plan, group model, camp loop design
- `docs/anti-detection.md` — evidence-based anti-detection posture, gate matrix, and operator-risk rules
- `docs/redguides-automation-research.md` — KissAssist, CWTN, camp loop patterns
- `docs/mq2-deep-dive.md` — MQ2Nav, combat, stick/follow analysis
- `docs/eq-maps-research.md` — Brewall format, coordinate transform
- `docs/eq-ini-optimization.md` — 4-tier INI settings, memory budgets
- `docs/dll-injection-plan.md` — Injection sequence, integration loop
- `docs/wineq-research.md` — Render strobing, window management
- `docs/roadmap-review.md` — Milestone priorities, risk assessment
- `docs/code-review-session3.md` — Code audit findings

## Wiki

The long-lived operator and developer wiki is source-controlled in `docs/wiki/` and published to
the GitHub wiki with `scripts/sync_wiki.py`.

```bash
python scripts/sync_wiki.py --check
python scripts/sync_wiki.py --dry-run
python scripts/sync_wiki.py --push
```

Update the repo-side source files in `docs/wiki/` in the same PRs that change behavior, then
publish the wiki snapshot after review.

The nightly wiki publish workflow uses runner-local `gh auth`, not a repository secret token.

## Requirements

- **Rust** (edition 2024; nightly MSVC toolchain currently required on Windows because `retour` uses unstable features)
- **Windows** for live EQ interaction (macOS/Linux for development only)
- **EverQuest** client (March 2026 build confirmed)

## License

Private project.
