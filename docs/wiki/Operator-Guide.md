# Operator Guide

This guide is for operators who want to set up, configure, and run TextQuest to control one or more EverQuest clients.

## Table of Contents

1. [Installation Prerequisites](#installation-prerequisites)
2. [Build Instructions](#build-instructions)
3. [Configuration Walkthrough](#configuration-walkthrough)
4. [TUI Navigation and Keybindings](#tui-navigation-and-keybindings)
5. [Common Troubleshooting](#common-troubleshooting)
6. [Next Steps](#next-steps)

---

## Installation Prerequisites

### All Platforms

- **Rust** edition 2024 (install from [rustup.rs](https://rustup.rs))
- Git for cloning the repository
- Your favorite text editor for configuration files

### Windows (Required for Live EQ Control)

- **Windows 10 or later**
- **MSVC toolchain** (part of Visual Studio; see [Rust Windows setup](https://doc.rust-lang.org/1.0.0/book/installing-rust.html))
- **Nightly Rust** on Windows (because the DLL injection dependency `retour` uses unstable features)
  - Install nightly: `rustup install nightly`
  - Override for Windows builds: Create or edit `.cargo/config.toml` with `[build] rustflags = ...` (this is already done in the repo)
- **CMake 3.5+** (for navmesh FFI build)
- **LLVM/Clang** (for bindgen during the navmesh build)
- Running **EverQuest client(s)** for live injection and control

### macOS/Linux (Demo Mode Only)

- **Rust stable** (install from [rustup.rs](https://rustup.rs))
- Running the build and TUI in demo mode (no live EQ client needed)

### Checking Your Environment

Run the preflight script to validate your setup:

```bash
python3 scripts/dev-preflight.py
```

This bundles local formatting with the wiki, lint, test, and Python validation used by the required PR gate.

---

## Build Instructions

### Quick Build

```bash
# Standard debug build
cargo build

# Run in demo mode (no EQ required)
cargo run
```

Expected behavior:

- The TUI opens and displays simulated characters and activity
- All five screens (Characters, Map, Navigation, Debug, Packets) are interactive
- Works on macOS, Linux, or Windows without a live EQ client

### Release Build (Windows)

```powershell
# Build optimized binary
cargo build --release

# The binary is at: target\release\textquest.exe
```

### Running Tests Before Commit

```bash
# Format check
cargo fmt --check

# Linting (Clippy)
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test

# Python tests
python3 -m unittest discover -s tests -p 'test_*.py' -v

# All together (recommended before pushing)
python3 scripts/dev-preflight.py
```

### Platform-Specific Notes

#### macOS/Linux Development

- Windows-specific process APIs are stubbed; demo mode is the normal workflow
- Use `cargo run` to iterate on the TUI and operator workflow
- Does not validate live EQ injection, navigation, or login paths

#### Windows Production

- `cargo run` launches in demo mode if no EQ clients are detected
- To use live EQ:
  1. Start one or more `eqgame.exe` clients
  2. Run `textquest.exe inject` to inject the DLL
  3. Run `textquest.exe tui` to open the dashboard
  4. Commands and navigation execute directly against live EQ memory

---

## Configuration Walkthrough

TextQuest stores configuration in `config/` relative to the working directory (or binary location).

### Main Configuration: `config/textquest.toml`

This is the primary operator configuration file. Key sections:

```toml
# Process discovery
process_name = "eqgame"
max_spawns = 10000

# Server and session settings
[server]
name = "Firiona Vie"         # or your TLP server
auto_discover = true         # Find running EQ clients
polling_interval_ms = 100    # How often to read game state

# Staggered login (used by :login all)
[launch]
stagger_ms = 2000            # Delay between each client launch
max_clients = 6              # Max simultaneous clients

# Group and role definitions
[[group]]
id = 1
name = "Group 1"

[[group]]
id = 2
name = "Group 2"
```

**When to edit:**

- Change `server` name to match your TLP
- Adjust `max_clients` if you run fewer or more clients
- Add group definitions for your multibox composition

### Alerting and Notifications

Operational alert routing is bootstrapped from the `[alerts]` section in `config/textquest.toml`.

```toml
[alerts]
enable_discord = true
discord_webhook_url = "https://discord.com/api/webhooks/..."
enable_email = false
warning_batch_window_secs = 300

[alerts.thresholds]
death_alert = true
stuck_alert = true
memory_warning_mb = 200
ipc_latency_warning_ms = 10
error_rate_warning_per_min = 5
dps_drop_warning_pct = 20
zone_timeout_secs = 60
```

Use this section to define:

- whether Discord webhook delivery is enabled
- whether SMTP email delivery is enabled
- who receives daily summaries
- which warning thresholds generate alerts

Alert history and acknowledgments are stored locally in `data/alerts.db`.
The web dashboard `Alert Routing` panel can change the live alert configuration for the current process and writes an audit alert whenever those settings change.

### Accounts: `config/accounts.toml`

Maps account names to character and group information:

```toml
[[accounts]]
name = "account01"
server = "Firiona Vie"
character = "Warrior01"
class = "WAR"
group = 1

[[accounts]]
name = "account02"
server = "Firiona Vie"
character = "Cleric01"
class = "CLR"
group = 1
```

**When to edit:**

- Add an entry for each account you want to launch
- `name` is what you use in `:login account01`
- `class` should match in-game (used for combat ability selection)
- `group` assigns the character to a combat group (1–6)

**Passwords:** Do NOT add passwords to this file. Use `:login account01` in the TUI; TextQuest will prompt for the password and store it securely.

### Camps: `config/camps/*.toml`

Define camp locations for automated pulling and looting:

```toml
zone = "gfaydark"
camp_center = [200, -100, 50]   # X, Y, Z coordinates
camp_radius = 200               # How far from center for pulls
pull_point = [250, -150, 50]    # Where to stand and pull from
pull_radius = 500               # Max distance to pull mobs
leash_radius = 1000             # Leash distance for CC/melee

# Mana and health thresholds
heal_at_pct = 30                # Request heal if <= 30% HP
mana_regen_secs = 6             # Mana recovery timer

# Optional next/previous for camp chains
next_camp = "gfaydark_camp2"
prev_camp = "qey2hh1_camp"
```

**How to create:**

1. Go to your pull location in EQ
2. Use `:status` to see your coordinates
3. Create `config/camps/zone_name.toml` with those coordinates
4. Adjust `camp_radius` and `pull_radius` as needed
5. Use `:camp start zone_name` to begin pulling

**Available camps:**

```bash
:camp list
```

### Auto-Accept Wards (Web Dashboard)

The web dashboard now exposes an **Auto-Accept Wards** view for unattended multibox prompts that otherwise block automation.

What it controls:

- Group invites
- Trade confirmations
- Task adds
- Dynamic-zone or expedition adds
- Translocate prompts
- Primary or secondary anchor teleports
- Respawn prompts (auto-accepted whenever the master ward is on; not currently configurable per-type)

How to configure it:

1. Open the dashboard and switch to **Auto-Accept Wards** from the left sidebar.
2. Toggle the **Master Ward** on to arm the feature.
3. Enable or disable each prompt type independently.
4. Choose **Anyone** to accept enabled prompts from all senders, or **Trust List** to restrict acceptance to named characters only.
5. When using **Trust List**, enter one player name per line or separate names with commas.
6. Save the ward profile.

Operational notes:

- Trust-list matching is case-insensitive.
- In trust-list mode, requests without a detected sender are rejected instead of being auto-accepted.
- Turning the master toggle off disables all auto-accept behavior without clearing the saved per-type settings.

### Tradeskill Trophy (Web Dashboard)

The web dashboard now exposes a **Tradeskill Trophy** panel for MQ2TSTrophy-style
crafting support.

What it controls:

- master enable or disable for trophy automation
- the exact trophy item name to pick up and equip before crafting
- live status for any injected client currently reporting trophy activity

What it does at runtime:

1. Detects supported world crafting containers such as forges, looms, ovens,
   brew barrels, and other MQ2TSTrophy parity stations.
2. Equips the configured trophy in the ammo slot for most skills, or the main
   hand slot when the skill requires it.
3. Restores the displaced item after the crafting session ends.
4. Tracks remaining trophy charges when the client can read them from the
   equipped slot or cursor choreography.

Operator notes:

- Enter the trophy item name exactly as it appears in game.
- The panel applies settings to live injected clients immediately.
- Status cards show the active container, chosen slot, displaced item restore
  target, and any observed remaining charges.
- These dashboard settings are currently runtime-only and reset if the web
  process restarts.

### Vendor Watch (Web Dashboard)

The web dashboard now exposes a **Vendor Item Watch** panel under **Economy**
for MQ2Vendors-style merchant browse alerts.

What it does:

- lets you maintain a watched item list in the dashboard
- stores that watch list in `config/textquest.toml` under `[vendor_watch]`
- polls live `MerchantWnd` rows from injected clients while merchant windows are open
- records an alert when a watched item appears on a merchant during normal browsing
- shows expected-vs-actual price comparison whenever the merchant row exposes a price

Operational notes:

- matches are case-insensitive on the merchant item name
- alerts are deduped while the same merchant row remains visible, then reset when
  the merchant window closes or the row changes
- merchant rows with `--` quantity in EQ are shown as `qty Infinite` in the dashboard
- the dashboard websocket pushes new vendor-watch alerts live to connected browsers

### Classes: `config/classes/*.toml`

Define combat ability rotations for each class. TextQuest includes pre-configured rotations for all 16 classes (Bard, Beastlord, Berserker, Cleric, Druid, Enchanter, Magician, Monk, Necromancer, Paladin, Ranger, Rogue, Shadowknight, Shaman, Warrior, Wizard).

Example (Warrior):

```toml
class_name = "warrior"
role = "tank"
rest_command = "/sit"

[[level_overrides]]
name = "warrior-live-62"
min_level = 62
max_level = 62

[[level_overrides.combat_abilities]]
name = "Deflection Discipline"
command = "/disc Deflection Discipline"
cooldown_secs = 900.0
priority = 10

[[ability_sets]]
name = "BurnPrimary"

[[ability_sets.candidates]]
name = "Spirit of Rage Discipline"
min_level = 61
spell_id = 4689
cooldown_ticks = 36000
shared_cooldown_key = "warrior-offensive-disc"
shared_cooldown_ticks = 36000

[[rotation_groups]]
name = "Burn"
target_selector = "AutoTarget"
combat_state_req = "Combat"

[[rotation_groups.entries]]
name = "BurnPrimary"
action_type = { Disc = "BurnPrimary" }
condition = { And = [ { TargetHpAbove = 25.0 }, { EnduranceAbove = 40.0 } ] }
```

**When to edit:**

- Per-class rotation tuning happens in the class file
- Per-character overrides go in `config/toons/<character>.toml`
- Live-safe Warrior tuning is split between operator-facing `level_overrides`
  and runtime `ability_sets` / `rotation_groups` in the same class file
- Changes take effect after re-injection of the DLL

### Per-Character Overrides: `config/toons/<character>.toml`

Override the class rotation for a specific character:

```toml
[[spells]]
name = "Cure Poison"
cooldown_secs = 12

[[disciplines]]
name = "Holy Aura"
cooldown_secs = 300
```

### Web Strategy Tuning: Resurrection Offers

The web strategy tuning panel includes a per-character **Resurrection Offers**
section for MQ2Rez-style popup handling. Each character can store:

- `enabled` — turn automatic resurrection handling on or off
- `min_xp_pct` — minimum resurrection percentage required before TextQuest accepts
- `trusted_casters` — explicit character names allowed to offer a resurrection
- `decline_if_untrusted` — automatically decline offers that fail the policy
- `delay_ms` — wait window before acting so an operator can manually override the popup

TextQuest only auto-accepts a resurrection offer when both checks pass:

1. the offer meets or exceeds the configured XP percentage
2. the caster appears in the trusted-caster list

If auto-decline is enabled, offers that fail either check are declined after the
configured delay. If auto-decline is disabled, TextQuest leaves the popup open
for manual handling when the policy does not match.

### Web Strategy Tuning: Window Identity

The same per-character tuning panel now includes a **Window Identity** field
that controls the EverQuest window title shown to the operating system.

This is intended for multi-box operators who need to distinguish many EQ
clients from the taskbar or Alt-Tab list. The value is stored in
`config/character-configs.json` as `window_title_format`.

Supported tokens include:

- `{server}`
- `{character}`
- `{level}`
- `{class}`
- `{class_short}`
- `{zone}`
- `{zone_long}`
- `{zone_short}`

Recommended starting format:

```text
[{server}] {character} ({level} {class_short})
```

TextQuest reapplies the title when the character finishes loading and after a
zone transition so the window stays identifiable without manual renaming.

### HVT Watchlist: `config/hvt_watchlist.toml`

Define high-value target mobs for named tracking and alerts:

```toml
[[named]]
name = "Frostcreeper King"
zone = "gfaydark"
discord_alert = true      # Ping Discord when spawned

[[named]]
name = "Lady Vox"
zone = "karnor"
```

### Discord Routing: `config/textquest.toml` or Web Dashboard

Discord alert routing now has two operator surfaces:

- static config in `config/textquest.toml`
- the web dashboard `Security Wards` panel for live webhook URL and policy edits

Use `[discord.channels]` for category feeds (`kills`, `loot`, `timers`,
`feats`, `status`) and `[discord.notification_routes.<route>]` for operational
alert policy (`death`, `status`, `hvt`, `crash`, `mass_failure`).

Important defaults:

- `death` uses `CRITICAL` severity with `mention_policy = "everyone"`
- `status` uses `INFO` severity with no mention
- route webhook overrides can be left blank to fall back to the category or
  default webhook
- Discord delivery is rate-limited to 30 requests per minute per webhook URL

### Named Mobs: `config/named_mobs/<zone>.toml`

Define zone-specific named mobs for tracking:

```toml
[[named]]
name = "Enraged Griffon"
x = 100
y = 200
z = 50
respawn_secs = 3600
```

---

## TUI Navigation and Keybindings

### The Five Screens

Launch the TUI with `cargo run` (demo mode) or `textquest.exe tui` (live mode).

| Screen         | Key | Purpose                                       |
| -------------- | --- | --------------------------------------------- |
| **Characters** | `1` | Roster, group focus, selected character state |
| **Map**        | `2` | Zone map, spawn overlays, named tracking      |
| **Navigation** | `3` | Route status, waypoints, stuck recovery       |
| **Debug**      | `4` | Spawn list, hex dump, EQ internals browser    |
| **Packets**    | `5` | Packet monitor UI for captured opcode events  |

### Global Keybindings

| Key           | Action                                       |
| ------------- | -------------------------------------------- |
| `1`–`5`       | Switch screens                               |
| `Shift+1`–`6` | Focus groups 1–6                             |
| `Shift+0`     | Clear group focus                            |
| `Tab`         | Cycle panes on current screen                |
| `[` / `]`     | Cycle between connected clients              |
| `/`           | Open spawn search                            |
| `f`           | Cycle spawn filter                           |
| `g`           | Toggle group section                         |
| `v`           | Toggle scope section                         |
| `z`           | Collapse focused section                     |
| `+` / `-`     | Adjust map Z slice                           |
| `m`           | Maximize map                                 |
| `F8`          | Open or close the alert history overlay      |
| `p`           | Toggle privacy mode (redacts names)          |
| `T`           | Cycle theme (Dark, Dracula, Classic, Neriak) |
| `:`           | Enter command mode                           |
| `?`           | Toggle help overlay                          |
| `q`           | Quit                                         |

### Alert History and Acknowledgment

Press `F8` from any TUI screen to open the operational alert overlay.

- The header shows unread count plus quick controls.
- The left pane shows the most recent 100 alerts with severity and acknowledgment state.
- The right pane shows the full message, actor, zone, and source for the selected alert.
- Press `Enter` or `a` to acknowledge the selected alert.
- Press `Shift+A` to acknowledge every unread alert.

### Command Mode (`:`)

Commands start with `:` and are auto-completed. Press `Enter` to execute.

#### Status and Inspection

```text
:status               Show overview of all clients
:commands             List all available commands
:help                 Show command help
:help camp            Help for a specific command
```

#### Camp Control

```text
:camp list            List available camps
:camp start zone      Start pulling at a camp
:camp stop            Stop pulling
:camp next            Go to next camp in chain
:camp prev            Go to previous camp in chain
:camp add camp_name   Mark a current location as a camp
```

#### Navigation

```text
:nav zone             Navigate all focused clients to zone
:nav x y z            Navigate to coordinates (x, y, z)
:nav camp_name        Navigate to a saved camp
```

#### Combat and Groups

```text
:ma warrior01         Make Warrior01 main assist
:mt mage01            Make Mage01 main tank
:engage               Engage current target
:disengage            Stop attacking
:invite warrior01     Invite to group
:accept               Accept group invite
```

#### Corrective Healing (CH) Chain

```text
:ch status            Show CH chain state
:ch start pid1,pid2 5 Healing spells every 5 seconds
:ch stop              Stop CH chain
:ch add pid           Add client to CH chain
:ch remove pid        Remove from CH chain
```

#### Login and Lifecycle

```text
:login                Login one account (interactive)
:login all            Login all configured accounts
:login account01      Login specific account
:login G1             Login all accounts in group 1
:stop account01       Stop one account
:stop all             Stop all clients
```

#### Utility

```text
:config               Open configuration editor
:theme                Cycle color themes
:privacy              Toggle privacy mode
:quit                 Exit TextQuest
```

### Screen Details

#### Characters Screen (1)

Shows your roster with:

- Health and mana bars
- Buffs/debuffs
- Cast bars for spells and abilities
- Selected character highlighted
- Group assignments

**Use this for:**

- Quick health checks
- Group composition review
- Focusing on specific clients with `Shift+1`–`6`

#### Map Screen (2)

Shows:

- Zone geometry (from `config/maps/*.txt`)
- Spawn positions and types
- Named mob tracking and timers
- Navigation target
- Map controls (`+`/`-` for Z slice, `m` to maximize)

**Use this for:**

- Tactical awareness
- Monitoring pull distance
- Named mob respawn timers

#### Navigation Screen (3)

Shows per-client:

- Current destination
- Waypoint queue
- Movement state (moving, stuck, arrived)
- Recovery actions if stuck

**Use this for:**

- Confirming navigation is active
- Spotting stuck clients
- Verifying arrival

#### Debug Screen (4)

Shows:

- Full spawn list (searchable with `/`)
- Target details (click to highlight)
- Raw hex dump of spawn data
- EQ internals browser

**Use this for:**

- Troubleshooting spawn reads
- Validating offsets after patches
- Inspecting target data

#### Packets Screen (5)

Shows:

- Captured packet rows when the current DLL build emits packet events
- Opcode decode
- Filtering by opcode
- Pause/resume capture

**Use this for:**

- Inspecting whichever packet events the attached DLL build produces
- Debugging packet-monitor output during attended runs
- Cross-checking opcode rows against other client state

Current limitation:

- Packet-monitor activation and zone-transition validation are still tracked by
  issue `#1270`. The screen is real, but the current repo does not treat it as
  validated proof that live packet capture is active on every build.

---

## Common Troubleshooting

### "No EQ clients found" or TUI starts in demo mode

**Symptoms:**

- TUI displays simulated characters instead of real ones
- `client-status-all` shows no clients

**Checks:**

1. Are you on **Windows**? (Live mode only works on Windows)
2. Is `eqgame.exe` actually running?
3. Is the process name correct? Check `config/textquest.toml`:
   ```toml
   process_name = "eqgame"
   ```

**Fix:**

```powershell
# On Windows:
Start-Process "C:\path\to\eqgame.exe"
timeout /t 10  # Wait for client to fully load
cargo run  # or textquest.exe tui
```

### "Cannot open shared memory" or "No session token"

**Symptoms:**

- Commands execute but nothing happens
- Status commands fail

**What it means:**

- DLL was not injected successfully, or the session token is missing

**Fix:**

```powershell
# Step 1: Inject the DLL
textquest.exe inject

# Step 2: Verify injection succeeded
# Check that %TEMP%\textquest\ contains token files:
dir %TEMP%\textquest\
# Look for: login_token_*.bin

# Step 3: Retry commands
textquest.exe tui
```

### DLL injection fails

**Symptoms:**

- `textquest.exe inject` completes but no token file is created
- DLL log is not created

**Checks:**

1. Are you running as **Administrator**? (Required for injection)
2. Is the DLL binary present? (Built with `cargo build --release`)
3. Are EQ clients actually running?
4. Check Windows Event Viewer for access denied errors

**Fix:**

```powershell
# Run as Administrator
runas /user:Administrator "textquest.exe inject"

# Or rebuild and retry
cargo build --release
textquest.exe inject
```

### Navigation doesn't work or mobs won't move

**Symptoms:**

- `:nav coordinates` executes but character doesn't move
- Movement commands appear in logs but no in-game action

**Checks:**

1. Is the DLL injected and running? (Check `client-status-all`)
2. Are you **in combat**? Navigation is disabled during combat
3. Check the **DLL log**: `%TEMP%\textquest\textquest-dll.log`
4. Is the navmesh loaded? Run:
   ```powershell
   textquest.exe navmesh diagnostics gfaydark
   ```

**Common causes:**

- Offset data is stale (after EQ patches)
- Client is stuck or unresponsive
- Navigation target is unreachable (try waypoint instead)

**Fix:**

```powershell
# 1. Clear nav state
textquest.exe cmd <pid> "/follow off"

# 2. Reload navmesh for the zone
textquest.exe navmesh reload gfaydark

# 3. Try simpler nav command
textquest.exe nav <pid> x y z

# 4. If still failing, update offsets (see next section)
```

### "Offsets are stale" after EQ patches

EverQuest patches often change memory layouts. TextQuest won't work correctly until offsets are revalidated.

**Symptoms:**

- Characters move erratically or crash the client
- Spawn data is corrupted or missing
- Login automation fails to click the right buttons

**Temporary workaround:**

- Use `:camp stop` and manual `/follow` commands
- Avoid login automation until offsets are confirmed

**Next step:**

- See `docs/wiki/Research-Patch-Day-Reproduction.md` for offset revalidation steps
- Or wait for an issue to be filed on GitHub with the latest offsets

### Login automation not working

**Symptoms:**

- `:login account01` starts but gets stuck at character select
- Login widgets are not being clicked

**Checks:**

1. Confirm account exists in `config/accounts.toml`
2. Confirm password was stored (first `:login` run prompts for password)
3. Check the **DLL log** for widget path errors
4. After **EQ patches**, login widgets often move (offset change)

**Fix:**

```bash
# 1. Check DLL log for widget errors
cat %TEMP%\textquest\textquest-dll.log | tail -20

# 2. If widgets changed after a patch, file an issue on GitHub
# 3. Manually trigger calibration
textquest.exe calibrate

# 4. Retry login
:login account01
```

### TUI crashes or is unresponsive

**Symptoms:**

- TUI freezes or exits unexpectedly
- Terminal shows a panic message

**Checks:**

1. Check the **orchestrator log**: `logs/textquest.log`
2. Look for error messages (permission denied, file not found, etc.)
3. Confirm `config/textquest.toml` is valid TOML (no syntax errors)

**Fix:**

```bash
# 1. Validate config syntax
python3 -c "import toml; toml.load(open('config/textquest.toml'))"

# 2. Clear temp files
rm -r %TEMP%\textquest\  # Windows: rmdir /s %TEMP%\textquest

# 3. Rebuild and restart
cargo build
cargo run
```

### Map is empty or showing the wrong zone

**Symptoms:**

- Map displays blank or gridlines only
- Named tracking shows no mobs

**Checks:**

1. Is a map file present? Check `config/maps/gfaydark.txt`
2. Is the zone ID correct in `config/maps/`?
3. Are you actually in that zone? Check `:status`

**Fix:**

```bash
# 1. Verify map files exist
ls config/maps/ | grep -i gfaydark

# 2. If missing, add a map file or use the wiki
# TextQuest includes maps from RedGuides

# 3. Clear map cache if one exists
rm -r data/navmesh/
```

---

## Next Steps

1. **Read the Quick Start** for the fastest path: [Quick Start](Quick-Start.md)
2. **Explore the Command Reference** for all available commands: [Command Reference](Command-Reference.md)
3. **Learn camp and combat setup**: [Combat and Camp Loop](Combat-and-Camp-Loop.md)
4. **Understand login automation**: [Login Automation](Login-Automation.md)
5. **Master navigation**: [Navigation and Maps](Navigation-and-Maps.md)
6. **Check the Architecture** to understand how TextQuest works: [Architecture Overview](Architecture-Overview.md)

For developers or those extending TextQuest, see the [Developer Guide](Developer-Guide.md).
