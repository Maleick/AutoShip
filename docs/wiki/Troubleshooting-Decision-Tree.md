# Troubleshooting Decision Tree

An interactive guide to diagnose and resolve common TextQuest automation failures. Use this tree to identify the root cause of your issue, collect diagnostics, and apply the appropriate fix.

## Quick Reference: Diagnostic Commands

Before diving into the tree, familiarize yourself with these tools:

```bash
# Check daemon status
textquest status

# Validate configuration
textquest config check

# View live client status across all processes
textquest client-status-all

# Query a specific client
textquest client-status <PID>

# Export raw event logs for analysis
textquest --dump  # Produces JSON event log

# Check zone navigation state
textquest navmesh diagnostics --pid <PID>

# View running processes
ps aux | grep eqgame.exe
ps aux | grep textquest
```

---

## 1. STARTUP & DAEMON FAILURES

### 1.1 Daemon fails to start (`textquest start` hangs or exits)

```
START
 ├─ Is the error "Permission denied" or "Socket already in use"?
 │  ├─ YES → [1.1.1 Port Conflict]
 │  └─ NO  → [1.1.2 Initialization Error]
 ├─ Does the daemon start but immediately stop?
 │  └─ YES → [1.1.3 Immediate Exit]
 └─ Does it hang indefinitely?
    └─ YES → [1.1.4 Deadlock / Hang]
```

#### 1.1.1 Port Conflict

**Symptom:** Error like `Address already in use` or `bind` failures.

**Root Causes:**

- IPC daemon already running on the port
- Previously crashed daemon left stale socket
- Another service bound to the port

**Diagnostics:**

```bash
# Check running daemons
ps aux | grep textquest | grep -v grep

# Find PIDs listening on IPC port (default 9000)
lsof -i :9000 | grep LISTEN

# Check config for custom IPC port
textquest config show | grep -A5 ipc
```

**Fix:**

```bash
# Option 1: Kill previous daemon
textquest stop

# Option 2: Force remove stale socket (Windows)
# Locate socket file from config/textquest.toml
# Delete ~/.cache/textquest/*.sock

# Option 3: Use different port
textquest start --foreground --ipc-port 9001
```

#### 1.1.2 Initialization Error

**Symptom:** Error in stderr mentioning config, credentials, or file I/O.

**Root Causes:**

- Invalid or missing `config/textquest.toml`
- Corrupted credential store (`credentials/`)
- Missing `data/` directory (offsets)
- Insufficient disk space
- File permissions issue

**Diagnostics:**

```bash
# Validate config syntax
textquest config check --path config/textquest.toml

# Show resolved config (masks sensitive values)
textquest config show

# Check file permissions
ls -la config/ credentials/ data/

# Verify disk space
df -h

# Check logs for specific error
tail -n 50 ~/.cache/textquest/logs/daemon.log
```

**Fix:**

```bash
# Rebuild config from template
cp config/textquest.toml.example config/textquest.toml
# Edit to fill in correct paths and accounts

# Reset credential store if corrupted
rm -rf credentials/
textquest credential add --account Account01  # Rebuild incrementally

# Verify offsets file
ls -la data/eq_client_*.json
# If missing, run offset scanner on live client
```

#### 1.1.3 Immediate Exit

**Symptom:** Daemon starts, no output, exits within 1-2 seconds.

**Root Causes:**

- Fatal panic in initialization
- Unrecoverable configuration error
- Missing required environment variable

**Diagnostics:**

```bash
# Run with full backtraces
RUST_BACKTRACE=1 textquest start --foreground 2>&1 | tee startup.log

# Check if panic happens during config load
textquest config check --path config/textquest.toml
```

**Fix:**

```bash
# Re-run with foreground to see error output
textquest start --foreground

# If panic in dependency loading (EQ offsets), ensure:
# 1. data/eq_client_*.json files exist
# 2. Run offset scanner first: textquest offsets scan --live --pid <PID>
```

#### 1.1.4 Deadlock / Hang

**Symptom:** Process is running but unresponsive (`textquest status` times out).

**Root Causes:**

- Circular lock contention during startup
- Network I/O blocking on unreachable service
- EQ process hook failed to initialize
- Navmesh service blocked on first download

**Diagnostics:**

```bash
# Check if process is alive
ps aux | grep textquest | grep -v grep

# Attempt status query with timeout
timeout 5 textquest status || echo "Timeout"

# Check if threads are stuck
# (requires lldb or gdb on macOS)
# Thread dump not available; check logs instead

# Look for partial initialization in logs
tail -n 100 ~/.cache/textquest/logs/daemon.log | grep -E "Initializ|spawn|hook"
```

**Fix:**

```bash
# Kill hung daemon
pkill -f "textquest start"

# Start with reduced scope (single client only, no navmesh)
textquest start --foreground --disable-navmesh &

# If that succeeds, navmesh is the culprit:
# Manually warm navmesh cache before starting daemon
textquest navmesh reload --zone gfaydark
textquest navmesh reload --zone crescent
# (etc. for all zones in rotation)
```

---

## 2. INJECTION & IPC FAILURES

### 2.1 DLL injection fails

```
START
 ├─ Does the injection command return an error immediately?
 │  └─ YES → [2.1.1 Injection Rejected]
 ├─ Does the process appear injected but unresponsive?
 │  └─ YES → [2.1.2 Pipe Connect Timeout]
 └─ Does it crash the EQ process?
    └─ YES → [2.1.3 DLL Crash / Segfault]
```

#### 2.1.1 Injection Rejected

**Symptom:** Error like `Failed to inject DLL` or `Access denied`.

**Root Causes:**

- Process not found (PID mismatch, exited)
- Administrator privileges required but not granted
- EQ process is in a windowed/suspended state
- Architecture mismatch (32-bit vs. 64-bit)

**Diagnostics:**

```bash
# Verify process exists
ps aux | grep <PID> | grep -v grep

# Check process architecture
file /path/to/eqgame.exe
# or: ldd /path/to/eqgame.exe | head

# Verify DLL is compiled for same arch
file textquest-dll.dll

# Check if process is suspended
# (macOS/Linux: not applicable; Windows-only check)
```

**Fix:**

```bash
# Verify correct PID
textquest client-status-all  # See all EQ processes

# Re-inject with explicit PID
textquest inject --pid <CORRECT_PID>

# If architecture mismatch:
cargo build --release -p textquest-dll --target x86_64-pc-windows-gnu
# Then copy DLL to correct location
```

#### 2.1.2 Pipe Connect Timeout

**Symptom:** Injection appears successful but commands sent to client timeout.

**Root Causes:**

- DLL initialization stalled (waiting for hook/memory scan)
- Named pipe creation failed silently
- High process load causing thread scheduling delays
- EQ memory layout changed (offset scan mismatch)

**Diagnostics:**

```bash
# Attempt to query injected client
timeout 5 textquest client-status --pid <PID> || echo "Timeout"

# Check DLL log output (if available)
# DLL logs typically written to:
# C:\Users\<USER>\AppData\Local\TextQuest\dll.log

# Monitor memory reads for signs of hanging
# (requires memory debugger; advisory only)

# Force sampling of client response
textquest cmd <PID> "/echo test"  # Should return immediately
```

**Fix:**

```bash
# Option 1: Restart injection with longer timeout
# (Internal; may require code change)

# Option 2: Ensure offset database is up-to-date
ls -la data/eq_client_*.json
# If outdated (before last EQ patch date), re-scan:
textquest offsets scan --live --pid <PID>

# Option 3: Re-inject with debug output
RUST_LOG=debug textquest inject --pid <PID>

# Option 4: If persistent, terminate and re-spawn
pkill -f eqgame.exe
sleep 2
# Re-spawn fresh and re-inject
```

#### 2.1.3 DLL Crash / Segfault

**Symptom:** EQ process crashes immediately after injection, or crashes on first command.

**Root Causes:**

- Invalid offset in DLL memory access
- Hook function signature mismatch
- Unhandled exception in DLL initialization
- Stack overflow in injection stub

**Diagnostics:**

```bash
# Check Windows event logs (Event Viewer)
# Look for: "eqgame.exe" → Application Error

# If using minidump: analyze crash file
# Typical location: C:\Users\<USER>\AppData\Local

# Verify offsets are plausible (sanity check)
grep "0x[a-f0-9]" data/eq_client_*.json | head -5

# Check DLL build logs for warnings
cargo build -p textquest-dll 2>&1 | grep -i "warning\|error"
```

**Fix:**

```bash
# Re-scan offsets from a known-good baseline
textquest offsets scan --live --pid <PID> --force

# If offsets are recent (< 1 week), the DLL code is likely at fault:
# File a GitHub issue with EQ version + crash info

# As a workaround, downgrade to last known-good DLL build:
git log --oneline -p textquest-dll/src | head -20
git checkout <LAST_GOOD_COMMIT> -- textquest-dll/
cargo build --release -p textquest-dll
```

---

## 3. LOGIN FAILURES

### 3.1 Autologin fails

```
START
 ├─ Does the credential lookup fail?
 │  └─ YES → [3.1.1 Missing Account]
 ├─ Does login hang at splash screen?
 │  └─ YES → [3.1.2 UI Interaction Timeout]
 ├─ Does login return auth error?
 │  └─ YES → [3.1.3 Auth Failure / Ban]
 └─ Does login succeed but character select fails?
    └─ YES → [3.1.4 Character Selection Error]
```

#### 3.1.1 Missing Account

**Symptom:** Error like `Account not found` or `Unknown account name`.

**Root Causes:**

- Account not defined in `accounts.toml`
- Account name case mismatch
- Typo in account reference
- Credentials file encrypted but master password not provided

**Diagnostics:**

```bash
# List all defined accounts
grep "\[account" config/accounts.toml

# Check if specific account exists
grep "Frostreaver01" config/accounts.toml || echo "Not found"

# Verify credential store
textquest credential show --account Frostreaver01 2>&1
# (This reveals if credential is missing)
```

**Fix:**

```bash
# Add missing account to config
# Edit config/accounts.toml:
# [account.Frostreaver01]
# server = "Firiona Vie"
# group = 1

# Add credential if missing
textquest credential add --account Frostreaver01 --password "***"
# (Or set TEXTQUEST_PASSWORD env var before autologin)
```

#### 3.1.2 UI Interaction Timeout

**Symptom:** Login hangs at splash screen or character select; no error message.

**Root Causes:**

- UI element not found (window layout changed)
- Timing too aggressive (click before UI rendered)
- Click coordinates incorrect for this resolution
- EQ process frozen or heavily lagged

**Diagnostics:**

```bash
# Monitor login attempt with verbose logging
RUST_LOG=debug textquest login --account Frostreaver01 2>&1 | tee login.log

# Check if EQ process is responsive
textquest cmd <PID> "/echo test"

# Verify screen resolution matches login config
# (Check testquest.toml: [login.ui_coordinates])
textquest config show | grep -A20 "login.ui_coordinates"

# Take screenshot if possible
# (Requires X11 or Mac screencap)
```

**Fix:**

```bash
# Option 1: Increase login delay
textquest autologin --account Frostreaver01 --inject-delay 5

# Option 2: Manually identify correct UI coordinates
# 1. Manually log in to capture coords
# 2. Edit config/textquest.toml:
#    [login.splash_username_input] = { x = 640, y = 350 }
#    (Adjust based on screenshots)

# Option 3: Use live calibration
textquest calibrate  # Auto-scan active EQ for UI offsets
```

#### 3.1.3 Auth Failure / Ban

**Symptom:** Error like `Invalid username or password` or `Account locked`.

**Root Causes:**

- Incorrect credentials
- Account temporarily locked (2FA, failed attempts)
- Account suspended or banned
- Daybreak account mismatch

**Diagnostics:**

```bash
# Verify credentials are correct
textquest credential show --account Frostreaver01

# Check if account exists on Daybreak's system
# (Manual: Log in via official launcher)

# Look for pattern of failed attempts in logs
grep "Auth" ~/.cache/textquest/logs/*.log | tail -20

# Query event logs for ban patterns
textquest --dump | jq '.fleet_events[] | select(.event_type == "AccountLocked")'
```

**Fix:**

```bash
# Option 1: Verify account on official launcher first
# (Use official Daybreak launcher; ensure it works there)

# Option 2: Reset account password
# (Via Daybreak account portal; then update TextQuest credential)
textquest credential add --account Frostreaver01 --password "NEW_PASSWORD"

# Option 3: Wait for lockout period to expire
# (Typically 15-30 minutes after failed attempts)
sleep 1800
textquest autologin --account Frostreaver01

# Option 4: If account is banned, escalation required
# Contact Daybreak support; document details in GitHub issue
```

#### 3.1.4 Character Selection Error

**Symptom:** Login succeeds, but character select hangs or clicks wrong character.

**Root Causes:**

- Character name not in config
- UI coordinates stale (resolution changed)
- Multiple characters with same name prefix
- Character list sorting changed

**Diagnostics:**

```bash
# Check character config
textquest config show | grep -A30 "character_select"

# Verify character name spelling
# (Check against actual character list on launcher)

# Test manual character selection
textquest cmd <PID> "/targetself"  # Verify client is responsive

# Take screenshot of character select screen
# (Manual inspection needed)
```

**Fix:**

```bash
# Update config with correct character name
# Edit config/textquest.toml:
# [[client]]
# account = "Frostreaver01"
# character = "Druid99"  # Must match exactly

# Recalibrate UI coordinates for this character
textquest calibrate --pid <PID>

# Verify by running login again
textquest login --account Frostreaver01 --character "Druid99"
```

---

## 4. CAMP LOOP FAILURES

### 4.1 Camp loop stops or hangs

```
START
 ├─ Does the client get stuck on a single action?
 │  └─ YES → [4.1.1 Action Stuck / Timeout]
 ├─ Does the client spam commands without responding?
 │  └─ YES → [4.1.2 Rapid-Fire Loop]
 ├─ Does the client sit idle doing nothing?
 │  └─ YES → [4.1.3 State Machine Deadlock]
 └─ Does the client disconnect or zone unexpectedly?
    └─ YES → [4.2 Zoning Issues]
```

#### 4.1.1 Action Stuck / Timeout

**Symptom:** Camp loop appears to hang on a single action (e.g., `/cast`, `/sit`, `/camp`).

**Root Causes:**

- Cast-time too long (mana insufficient, spell resist)
- Sit command never completes (already sitting or standing prevented)
- IPC timeout on command dispatch
- Action not recognized (command syntax error)

**Diagnostics:**

```bash
# Inspect live camp state
textquest client-status --pid <PID>

# Check if action is actually being sent
textquest cmd <PID> "/echo Test"  # Quick sanity check

# Query event logs for stuck action pattern
textquest --dump | jq '.camp_events[-10:] | .[]'

# Monitor memory for last action executed
# (Advisory; requires EQ internals knowledge)

# Check combat state (rotation may be paused)
textquest client-status --pid <PID> | grep -A5 "rotation\|combat"
```

**Fix:**

```bash
# Option 1: Interrupt and reset
textquest cmd <PID> "/sit"
sleep 1
textquest cmd <PID> "/stand"
# Restart camp loop via TUI

# Option 2: Skip problematic ability
# Edit config/textquest.toml → [rotation]
# Remove or comment out the stuck action

# Option 3: Increase action timeout
# Edit config/textquest.toml:
# [camp.action_timeout_ms] = 10000  # 10 seconds

# Option 4: Check ability prerequisites
# If casting: Verify mana, reagents, distance
# Run: textquest client-status --pid <PID>
```

#### 4.1.2 Rapid-Fire Loop

**Symptom:** Client is spamming commands rapidly without executing them (IPC queue backlog).

**Root Causes:**

- Camp loop logic not waiting for command completion
- IPC queue growing unbounded
- Client unresponsive (hung in EQ code)
- High network latency preventing state updates

**Diagnostics:**

```bash
# Check IPC latency
textquest client-status --pid <PID> | grep -i "latency\|pending"

# Monitor command queue growth
# (Requires internal metrics; advisory)

# Check for memory growth
textquest client-status --pid <PID> | grep -i "memory"

# Inspect DLL health
# (May require crash dump analysis)
```

**Fix:**

```bash
# Option 1: Restart client injection
textquest inject --pid <PID> --force-reinject

# Option 2: Slow down camp loop
# Edit config/textquest.toml:
# [camp.action_delay_ms] = 500  # Wait 500ms between actions

# Option 3: Check for unresponsive EQ process
# Kill and re-spawn
pkill -f "eqgame.exe"
sleep 2
# Re-inject and restart

# Option 4: Disable problematic rotation entry
# Edit config → rotation, comment out entries causing loop
```

#### 4.1.3 State Machine Deadlock

**Symptom:** Client sits idle; camp loop is "running" but never progresses to next action.

**Root Causes:**

- State machine stuck in intermediate state (e.g., waiting for condition that never occurs)
- Condition evaluation infinite loop
- Encounter or spell in invalid state
- Combat rotation blocked on missing resource

**Diagnostics:**

```bash
# Query client state
textquest client-status --pid <PID> | tee state.log

# Inspect last evaluated conditions
textquest --dump | jq '.camp_events[-5:] | .[] | {action, condition_result}'

# Check for stuck encounter
textquest --dump | jq '.encounter[] | select(.status == "stuck")'

# Look for error patterns in metrics
textquest --dump | jq '.client_metrics[] | select(.error_kind == "HealthCheck")'
```

**Fix:**

```bash
# Option 1: Manually transition state
textquest cmd <PID> "/stand"
textquest cmd <PID> "/sit"
sleep 1

# Option 2: Reset rotation
# Stop camp loop via TUI, restart

# Option 3: Simplify rotation (remove blocking conditions)
# Edit config/textquest.toml:
# Temporarily disable problematic condition checks

# Option 4: Check for stuck spell/ability
# Edit combat/class_config.rs, verify rotation entries
# ensure no circular dependencies or missing flags
```

---

## 5. NAVIGATION & ZONING FAILURES

### 5.1 Navigation fails or goes off-course

```
START
 ├─ Does client refuse to move?
 │  └─ YES → [5.1.1 Navigation Blocked]
 ├─ Does client move but in wrong direction?
 │  └─ YES → [5.1.2 Incorrect Path]
 ├─ Does client get stuck at zone boundary?
 │  └─ YES → [5.1.3 Zone Line Issue]
 └─ Does navmesh fail to load?
    └─ YES → [5.1.4 Navmesh Download / Validation]
```

#### 5.1.1 Navigation Blocked

**Symptom:** Client ignores navigation commands or responds with "stuck" status.

**Root Causes:**

- Navmesh not loaded for current zone
- Navigation goal inside geometry or unreachable
- Client is rooted or crowd-controlled
- Navigation service unresponsive

**Diagnostics:**

```bash
# Query navigation state
textquest navmesh diagnostics --pid <PID>

# Check if navmesh is loaded
textquest navmesh diagnostics --pid <PID> | grep -i "loaded\|missing"

# Verify current zone
textquest client-status --pid <PID> | grep -i "zone"

# Check for CC or root
textquest --dump | jq '.buff_events[-5:] | .[] | select(.buff_type == "root" or .buff_type == "stun")'

# Test navigation with explicit coordinates
textquest nav <PID> 100 200 50  # Test nav command

# Check navmesh service connectivity
timeout 5 textquest navmesh diagnostics --pid <PID> || echo "Service unresponsive"
```

**Fix:**

```bash
# Option 1: Reload navmesh for zone
textquest navmesh reload --pid <PID>

# Option 2: If reload fails, re-download cache
rm -rf ~/.cache/textquest/navmesh/*
textquest navmesh reload --zone gfaydark
textquest navmesh reload --zone crescent
# (For all zones in rotation)

# Option 3: Adjust navigation goal to valid location
# Ensure X, Y, Z coordinates are within zone bounds
# and not inside walls or geometry

# Option 4: Clear CC / root
textquest cmd <PID> "/cast Cure <target>"
# Or equivalent dispel for your class

# Option 5: Restart navigation service
# (May require daemon restart)
textquest stop
sleep 2
textquest start
```

#### 5.1.2 Incorrect Path

**Symptom:** Client takes a long or impossible route to destination.

**Root Causes:**

- Navmesh geometry out of date (zone changed)
- Pathfinding algorithm misconfiguration
- Waypoint network broken or incomplete
- Coordinate system offset

**Diagnostics:**

```bash
# Compare pathfinding with manual route
# (Manual inspection: watch client movement)

# Inspect navmesh coverage
textquest navmesh diagnostics --pid <PID> | grep -i "coverage\|nodes"

# Check for known geometry issues
# (Requires zone-specific knowledge; manual inspection)

# Verify coordinate system
textquest client-status --pid <PID> | grep -i "pos\|coord"

# Sample navigation performance
time textquest nav <PID> 0 0 0  # Nav to origin
time textquest nav <PID> 100 100 0  # Nav short distance
```

**Fix:**

```bash
# Option 1: Re-download and validate navmesh
textquest navmesh reload --zone <ZONE> --force

# Option 2: Verify waypoint network is complete
textquest navmesh diagnostics --zone <ZONE> | grep -i "waypoint"

# Option 3: Adjust pathfinding parameters
# Edit config/textquest.toml:
# [navigation.pathfinding]
# max_iterations = 1000
# heuristic_weight = 1.5

# Option 4: Use manual waypoint sequence instead
# For critical routes, define explicit waypoints in config
# [camp.manual_routes.gfaydark_to_crescent]
# waypoints = [ {x=100, y=200, z=50}, {x=150, y=250, z=60} ]
```

#### 5.1.3 Zone Line Issue

**Symptom:** Client gets stuck at zone boundary; unable to cross into adjacent zone.

**Root Causes:**

- Zone line coordinates incorrect in adjacency graph
- Navmesh doesn't cover zone line
- Permission check failing (e.g., level requirement)
- Zone line instance mismatch (wrong server/instance)

**Diagnostics:**

```bash
# Query zone adjacency graph
textquest zones --pid <PID>

# Check zone line coordinates
textquest zones --pid <PID> | grep -i "zone_line\|boundary"

# Verify permission to enter zone
# (Check character level vs. zone min level)
textquest client-status --pid <PID> | grep -i "level"

# Inspect navmesh at zone line
textquest navmesh diagnostics --pid <PID> | grep -A5 "zone_line"

# Test manual zone crossing
textquest cmd <PID> "/zone Crescent"  # Manual zone command
```

**Fix:**

```bash
# Option 1: Verify zone adjacency configuration
# Edit config/textquest.toml:
# [[zone_adjacency]]
# from = "gfaydark"
# to = "crescent"
# line_x = 0
# line_y = 0
# line_z = 0

# Option 2: Recalibrate zone line coordinates
# Position client on zone line, note coords:
textquest client-status --pid <PID>
# Update config with actual coordinates

# Option 3: Force zone transition if level-blocked
# (Only if character is adequate level)
# Add special-case handling in camp config

# Option 4: Use manual zone command
# If auto-zoning fails, use explicit zone command:
textquest cmd <PID> "/zone DestinationZone"
```

#### 5.1.4 Navmesh Download / Validation

**Symptom:** Navmesh reload fails; error like `Download failed` or `Validation error`.

**Root Causes:**

- Network unavailable or latency high
- Navmesh server unreachable
- Downloaded file corrupted
- Zone not supported (no navmesh available)

**Diagnostics:**

```bash
# Check network connectivity
ping 8.8.8.8

# Verify navmesh server is reachable
# (Server URL in config; default: internal service)
textquest config show | grep -i "navmesh.*url"

# Inspect download error
textquest navmesh reload --zone gfaydark 2>&1 | tee navmesh.log

# Check for corrupted cache
ls -lh ~/.cache/textquest/navmesh/*.json

# Verify zone is supported
textquest navmesh reload --zone invalid_zone 2>&1  # Should error for invalid zone
```

**Fix:**

```bash
# Option 1: Retry download
textquest navmesh reload --zone gfaydark --force

# Option 2: Clear cache and retry
rm -rf ~/.cache/textquest/navmesh/
textquest navmesh reload --zone gfaydark

# Option 3: Check network connectivity
# Verify internet access and latency
# If behind firewall, check egress rules

# Option 4: Pre-warm all zones before running
for zone in gfaydark crescent sebilis; do
  textquest navmesh reload --zone $zone
done

# Option 5: If navmesh server is down
# (Check GitHub status page or Discord announcements)
# Use manual waypoint sequences as fallback
```

---

## 6. COMBAT & ROTATION FAILURES

### 6.1 Rotation not executing

```
START
 ├─ Is combat even initiated?
 │  └─ NO → [6.1.1 Combat Not Starting]
 ├─ Does rotation start but stop after first action?
 │  └─ YES → [6.1.2 Rotation Halts Prematurely]
 ├─ Are some abilities never cast?
 │  └─ YES → [6.1.3 Ability Skipped]
 └─ Is damage low or ineffective?
    └─ YES → [6.1.4 Rotation Effectiveness]
```

#### 6.1.1 Combat Not Starting

**Symptom:** No combat commands sent; client remains passive even when in camp zone with mobs.

**Root Causes:**

- Combat enabled/disabled flag is off
- No valid target selected
- Target is out of range or invalid
- Puller not bringing mobs to camp
- Encounter state not initialized

**Diagnostics:**

```bash
# Check if combat is enabled
textquest client-status --pid <PID> | grep -i "combat\|enabled"

# Verify client has a target
textquest cmd <PID> "/target self"
textquest cmd <PID> "/target"  # Echo current target

# Query encounter state
textquest --dump | jq '.encounter_state[] | select(.client_id == <PID>)'

# Check rotation configuration
textquest config show | grep -A20 "rotation"

# Inspect puller behavior
textquest --dump | jq '.camp_events[] | select(.event == "Puller")'
```

**Fix:**

```bash
# Option 1: Enable combat
# Edit config/textquest.toml:
# [camp.combat_enabled] = true

# Option 2: Verify rotation is configured
# Ensure [[rotation]] entries exist:
# [[rotation.entry]]
# ability = "Kick"
# action_type = "melee"

# Option 3: Manually select target and test
textquest cmd <PID> "/assist <puller_name>"
sleep 1
# Camp loop should engage

# Option 4: Check puller is functional
# Ensure puller is pulling mobs to camp
# (Monitor TUI for puller status)
```

#### 6.1.2 Rotation Halts Prematurely

**Symptom:** Rotation starts (first 1-2 actions execute) then stops.

**Root Causes:**

- Encounter ended prematurely
- Mana or resources exhausted
- Combat condition evaluation error
- Defensive ability triggered (run/kite initiated)

**Diagnostics:**

```bash
# Check if encounter is active
textquest --dump | jq '.encounter_state[] | select(.client_id == <PID> and .status == "active")'

# Monitor resource levels
textquest client-status --pid <PID> | grep -i "mana\|endurance"

# Inspect last condition evaluation
textquest --dump | jq '.rotation_events[-10:] | .[] | {ability, condition_met}'

# Check for defensive triggers
textquest --dump | jq '.camp_events[] | select(.event_type == "Defensive")'

# Monitor encounter duration
textquest --dump | jq '.encounter_state[] | {duration_ms, mobs_killed}'
```

**Fix:**

```bash
# Option 1: Verify mana is sufficient
# Check client status; if out of mana:
textquest cmd <PID> "/cast Mana Recovery <target>"
# or rest for mana

# Option 2: Remove resource prerequisites from rotation
# Edit config → rotation.entry
# Change mana_cost threshold or remove check

# Option 3: Extend encounter timeout
# Edit config/textquest.toml:
# [encounter.timeout_ms] = 60000  # 60 seconds

# Option 4: Disable defensive triggers temporarily
# Edit config/textquest.toml:
# [camp.defensive_enabled] = false
# (For testing; re-enable after verification)

# Option 5: Check rotation logic for unmet conditions
# Review combat/class_config.rs for condition_met() logic
```

#### 6.1.3 Ability Skipped

**Symptom:** Specific ability never executes, even when conditions should be met.

**Root Causes:**

- Ability cooldown not reset (tracker bug)
- Condition evaluation returns false
- Ability level/class check failing
- Ability name/syntax incorrect

**Diagnostics:**

```bash
# Inspect cooldown state
textquest --dump | jq '.ability_cooldowns[] | select(.ability_id == "<ABILITY>")'

# Check condition for this ability
textquest --dump | jq '.rotation_events[] | select(.ability == "<ABILITY>") | .condition_met'

# Verify ability is recognized
grep "<ABILITY>" config/textquest.toml

# Check ability prerequisites (level, skill)
textquest client-status --pid <PID> | grep -i "level\|skill"

# Test ability manually
textquest cmd <PID> "/cast <ABILITY>"
# Should execute immediately if conditions met
```

**Fix:**

```bash
# Option 1: Reset ability cooldown
# (Requires internal method; advisory)
# In rotation, remove or reset cooldown_key

# Option 2: Verify ability name and syntax
# Check: textquest --dump | jq '.spell_book[]'
# Ensure rotation entry matches exactly

# Option 3: Adjust condition threshold
# Edit config/textquest.toml:
# [[rotation.entry]]
# ability = "<ABILITY>"
# condition = { mana_pct: 30 }  # Lower threshold if too high

# Option 4: Add explicit logging
# Edit combat/class_config.rs
# Add: tracing::debug!("Evaluating {}", ability_name);
# Rebuild and inspect logs

# Option 5: Remove condition check temporarily
# [[rotation.entry]]
# ability = "<ABILITY>"
# # condition = ... (comment out)
# Test if ability executes; if yes, condition is the issue
```

#### 6.1.4 Rotation Effectiveness

**Symptom:** Rotation executes but damage is low; combat takes much longer than expected.

**Root Causes:**

- Ability rotation order suboptimal
- Cooldown configuration too conservative (long gaps between attacks)
- Mana management preventing high-damage abilities
- Wrong target being attacked

**Diagnostics:**

```bash
# Query combat round metrics
textquest --dump | jq '.combat_rounds[-10:] | .[] | {dps: .damage_dealt / (.duration_ms/1000), rotation_used}'

# Compare to baseline
# (Requires manual testing; record baseline DPS)

# Inspect ability sequence
textquest --dump | jq '.rotation_events[-20:] | .[] | .ability'

# Check if rotation is repeating efficiently
textquest --dump | jq '.rotation_events[-20:] | .[] | select(.duration_since_last_ms > 5000)'
# High gaps indicate cooldown issues

# Verify target selection
textquest cmd <PID> "/tar"  # Show current target
textquest client-status --pid <PID> | grep -i "target"
```

**Fix:**

```bash
# Option 1: Optimize rotation order
# In config/textquest.toml, reorder [[rotation.entry]] by DPS
# High-damage abilities should have lower cooldowns

# Option 2: Reduce cooldown durations
# Edit rotation entries:
# [[rotation.entry]]
# ability = "<ABILITY>"
# cooldown_ticks = 4  # Lower from default

# Option 3: Adjust mana threshold to allow high-damage spam
# Edit config/textquest.toml:
# [combat.mana_threshold] = 15  # Cast at 15% mana, not 50%

# Option 4: Add hot-swap rotation for high-mana scenarios
# [[rotation.entry]]
# condition = { mana_pct: 80 }
# ability = "ExpensiveHighDamageAbility"
# cooldown_ticks = 1  # Spam when mana is full

# Option 5: Test against known encounter
# Use Sebilis camp (predictable mob HP/level)
# Compare DPS to expected baseline for your class
```

---

## 7. CIRCUIT BREAKER & ERROR ACCUMULATION

### 7.1 Client marked as failed / circuit breaker triggered

```
START
 ├─ Is the error "HealthCheck"?
 │  └─ YES → [7.1.1 Health Check Failure]
 ├─ Is the error "LaunchFailure"?
 │  └─ YES → [7.1.2 Launch / Spawn Failure]
 ├─ Is the error "PipeConnect"?
 │  └─ YES → [7.1.3 IPC Pipe Reconnect]
 └─ Is the error "IpcDispatch"?
    └─ YES → [7.1.4 Command Dispatch Timeout]
```

#### 7.1.1 Health Check Failure

**Symptom:** Client status shows repeated `HealthCheck` errors; eventually marked as `Failed`.

**Root Causes:**

- DLL unresponsive to status queries
- Network latency causing timeouts
- Process memory corruption
- EQ process in suspended state

**Diagnostics:**

```bash
# Check circuit breaker state
textquest client-status --pid <PID> | grep -i "circuit\|error\|health"

# Query error history
textquest --dump | jq '.client_errors[] | select(.client_id == <PID>) | .error_kind'

# Attempt direct health check
timeout 5 textquest cmd <PID> "/echo health_check" || echo "Timeout"

# Monitor IPC latency
textquest client-status --pid <PID> | grep -i "latency"

# Check process memory
ps aux | grep "<PID>" | awk '{print $6}'  # RSS memory
```

**Fix:**

```bash
# Option 1: Reset health check counter
# (Requires internal reset; advisory)
# Restart injection:
textquest inject --pid <PID> --force-reinject

# Option 2: Increase health check timeout
# Edit config/textquest.toml:
# [monitoring.health_check_timeout_ms] = 10000  # 10 seconds

# Option 3: Reduce health check frequency
# Edit config/textquest.toml:
# [monitoring.health_check_interval_ms] = 5000  # 5 seconds

# Option 4: Restart daemon
# If widespread health check failures:
textquest stop
sleep 2
textquest start

# Option 5: Check for process suspension
# (Windows-specific)
# Ensure EQ process is not suspended/paused
```

#### 7.1.2 Launch / Spawn Failure

**Symptom:** New client fails to launch or inject; circuit breaker prevents retry.

**Root Causes:**

- EQ installation not found or corrupted
- Insufficient disk space for new process
- License check failing (already logged in elsewhere)
- Process spawning logic broken

**Diagnostics:**

```bash
# Check spawn logs
textquest --dump | jq '.launch_events[] | select(.status == "Failed")'

# Verify EQ installation
ls -la "C:\Program Files\EverQuest\eqgame.exe"

# Check disk space
df -h /  # Root filesystem
du -h ~/.cache/textquest  # Cache size

# Inspect spawner error details
tail -n 50 ~/.cache/textquest/logs/launch.log | grep -i "error\|failed"

# Try manual spawn
textquest autologin --account Frostreaver01 --spawn --inject-delay 5
```

**Fix:**

```bash
# Option 1: Verify EQ installation integrity
# Repair via official launcher:
# 1. Start EverQuest launcher
# 2. Settings → Verify Files
# 3. Wait for verification

# Option 2: Ensure EQ can run
# Manually start eqgame.exe; verify it launches
# If manual launch fails, EQ installation issue (not TextQuest)

# Option 3: Reset circuit breaker
# Clear error history:
# (Restart daemon to clear; no manual reset available)
textquest stop
sleep 2
textquest start

# Option 4: Check license
# If error mentions "already logged in":
# Log out from all other EQ sessions
# Wait 30 seconds
# Retry spawn

# Option 5: Increase retry delay
# Edit config/textquest.toml:
# [orchestrator.spawn_delay_seconds] = 10  # Space out spawns
```

#### 7.1.3 IPC Pipe Reconnect

**Symptom:** Repeated `PipeConnect` errors; IPC connection drops and re-establishes frequently.

**Root Causes:**

- Named pipe broken (DLL crashed and revived)
- Network issues between daemon and client
- DLL memory pressure causing GC/pause
- Firewall blocking IPC

**Diagnostics:**

```bash
# Monitor pipe connection stability
textquest client-status --pid <PID> | grep -i "pipe\|connect\|ipc"

# Check for DLL crashes
# (Look for segfault signals or exit codes)
ps aux | grep "<PID>"  # Check process state

# Inspect DLL logs if available
# C:\Users\<USER>\AppData\Local\TextQuest\dll.log

# Monitor memory pressure
textquest client-status --pid <PID> | grep -i "memory"
```

**Fix:**

```bash
# Option 1: Restart injection
textquest inject --pid <PID> --force-reinject

# Option 2: Reduce IPC command frequency
# Edit config/textquest.toml:
# [camp.command_batch_interval_ms] = 500  # Batch commands

# Option 3: Increase IPC timeout
# Edit config/textquest.toml:
# [ipc.command_timeout_ms] = 10000  # 10 seconds

# Option 4: Check firewall rules
# Ensure IPC port is not blocked (usually local-only)

# Option 5: If DLL crashes repeatedly
# File GitHub issue with crash dump
# Downgrade to last known-good build as workaround
```

#### 7.1.4 Command Dispatch Timeout

**Symptom:** IPC commands time out repeatedly; client becomes unresponsive.

**Root Causes:**

- Command queue saturated
- DLL processing loop blocked
- EQ frame rate very low (lag)
- Incompatible command syntax

**Diagnostics:**

```bash
# Monitor command queue depth
textquest client-status --pid <PID> | grep -i "queue\|pending"

# Check IPC latency trend
textquest client-status --pid <PID> | grep -i "latency"

# Inspect stuck commands in event log
textquest --dump | jq '.command_dispatch[-10:] | .[] | {command, latency_ms}'

# Check EQ frame rate (if visible)
# (Requires EQ console or external FPS monitor)

# Sample a few commands to verify responsiveness
textquest cmd <PID> "/echo test1"
textquest cmd <PID> "/echo test2"
# Should return quickly
```

**Fix:**

```bash
# Option 1: Reduce command frequency
# Edit config/textquest.toml:
# [camp.action_delay_ms] = 1000  # 1 second between actions

# Option 2: Skip commands if queue is full
# Edit config/textquest.toml:
# [ipc.max_pending_commands] = 5  # Drop if > 5 pending

# Option 3: Use simpler command syntax
# Replace multi-line commands with single-line equivalents

# Option 4: Restart daemon
# Force reconnect and flush command queue:
textquest stop
sleep 2
textquest start

# Option 5: Check EQ performance
# If EQ FPS very low (< 10), performance issue
# (Not TextQuest; may need EQ client tuning)
```

---

## 8. ACCOUNT LOCKOUT & BAN DETECTION

### 8.1 Account locked or banned

```
START
 ├─ Do login attempts fail with auth error?
 │  └─ YES → [8.1.1 Temporary Lockout]
 ├─ Does login fail with "Account suspended"?
 │  └─ YES → [8.1.2 Permanent Ban / Suspension]
 └─ Does client get kicked mid-session?
    └─ YES → [8.1.3 Session Ban / Disconnect]
```

#### 8.1.1 Temporary Lockout

**Symptom:** Auth returns error; account is temporarily locked (typically 15-30 minutes).

**Root Causes:**

- Too many failed login attempts (wrong password)
- 2FA failure
- Account security check triggered
- IP address flagged as suspicious

**Diagnostics:**

```bash
# Inspect auth error message
textquest --dump | jq '.login_events[] | select(.status == "Failed") | .error_message'

# Check when lockout expires
# (Typically auto-expires after 15-30 minutes)

# Verify password is correct
# (Test via official launcher first)

# Check 2FA status
# (Manual verification via Daybreak account portal)

# Query IP reputation if suspicious activity
# (Advisory; check Daybreak forums for IP bans)
```

**Fix:**

```bash
# Option 1: Wait for lockout to expire
# (Typically 15-30 minutes)
sleep 1800

# Option 2: Verify correct password
# Re-test via official launcher first
# If official launcher works, update TextQuest credential:
textquest credential add --account Frostreaver01 --password "CORRECT_PASSWORD"

# Option 3: Re-enable 2FA if needed
# (Manual via Daybreak account portal)

# Option 4: Attempt login from different IP
# (If IP is flagged, use VPN or different network)

# Option 5: Contact Daybreak support
# If lockout persists beyond 30 minutes, escalate
```

#### 8.1.2 Permanent Ban / Suspension

**Symptom:** Auth fails with "Account suspended" or "Account banned"; error persists indefinitely.

**Root Causes:**

- Account permanently banned by Daybreak
- Account suspended for ToS violation
- Account compromised and recovered by Daybreak
- Account flagged for RMT or real-money trading

**Diagnostics:**

```bash
# Check Daybreak account portal
# (Manual: Log in to Daybreak account site)
# Look for account status or suspension message

# Inspect ban notification in event logs
textquest --dump | jq '.account_events[] | select(.event_type == "Suspended")'

# Check Daybreak support tickets
# (If you filed a support ticket previously)

# Verify account is not compromised
# (Check Daybreak password reset history)
```

**Fix:**

```bash
# Option 1: Contact Daybreak support
# File a support ticket explaining the issue
# Provide account details and any relevant logs

# Option 2: If account is compromised:
# 1. Change password via Daybreak account portal
# 2. Enable 2FA
# 3. Request account review by support

# Option 3: Accept the account loss
# (If ban is permanent and valid)
# Remove account from config/accounts.toml
# Create a new account if necessary

# Option 4: Document for escalation
# Save all logs, errors, and Daybreak responses
# File GitHub issue with [BAN] tag
```

#### 8.1.3 Session Ban / Disconnect

**Symptom:** Client is logged in, then suddenly disconnects with "Banned" or kicked.

**Root Causes:**

- Detected automation/cheating activity
- Daybreak's anti-cheat system triggered
- Account compromised (someone else logged in)
- Network issue misreported as ban

**Diagnostics:**

```bash
# Check disconnect reason
textquest --dump | jq '.session_events[] | select(.event_type == "Disconnect") | .reason'

# Look for ban confirmation in chat log
grep -i "banned\|suspended\|kick" ~/.cache/textquest/chat.log

# Check if other characters on same account are playable
# (Manual: Try logging in via official launcher)

# Inspect orchestrator logs for ban detection
grep -i "ban\|detect" ~/.cache/textquest/logs/*.log | tail -20

# Query event system for anomalies
textquest --dump | jq '.anomaly_events[-10:] | .[]'
```

**Fix:**

```bash
# Option 1: Verify it's actually a ban
# Try logging in manually via official launcher
# If manual login succeeds, disconnect was likely network-related

# Option 2: Stop automation and wait
# Stop TextQuest for 24-48 hours
# Daybreak's automated systems may lift temporary bans

# Option 3: Review logs for triggering activity
# Look for: excessive casting, stamina drain, botting detection
# Edit rotation to be more "human-like"

# Option 4: Contact Daybreak support
# If ban is permanent, escalate with evidence
# Provide full logs and rotations used

# Option 5: Adjust automation patterns
# If avoiding re-ban:
# - Add random delays between actions
# - Vary rotation slightly
# - Add breaks/rest periods
# - Disable high-speed camping patterns
```

---

## 9. SYSTEM & ENVIRONMENT ISSUES

### 9.1 General system failures

```
START
 ├─ Do commands error with "File not found"?
 │  └─ YES → [9.1.1 Missing Configuration / Files]
 ├─ Do commands error with "Permission denied"?
 │  └─ YES → [9.1.2 File Permissions]
 ├─ Is the daemon consuming excessive resources?
 │  └─ YES → [9.1.3 Resource Exhaustion]
 └─ Are timestamps wrong or inconsistent?
    └─ YES → [9.1.4 Time Sync / Clock Issues]
```

#### 9.1.1 Missing Configuration / Files

**Symptom:** Error like `File not found: config/textquest.toml` or `Missing offset database`.

**Root Causes:**

- File not in expected location
- Symlink broken
- Path misconfiguration
- Installation incomplete

**Diagnostics:**

```bash
# Check if file exists
ls -la config/textquest.toml
ls -la credentials/
ls -la data/

# Check working directory
pwd

# Verify PATH includes TextQuest bin
echo $PATH | grep -i textquest

# Check if config path is absolute
grep "config_path\|path" ~/.config/textquest/rc

# Inspect log for path info
tail ~/.cache/textquest/logs/*.log | grep -i "path\|load"
```

**Fix:**

```bash
# Option 1: Create missing files
cp config/textquest.toml.example config/textquest.toml
mkdir -p credentials/ data/

# Option 2: Fix path in config
# Edit config/textquest.toml or environment:
export TEXTQUEST_CONFIG_PATH="/full/path/to/config/textquest.toml"

# Option 3: Initialize directory structure
# Run installer or setup script:
./scripts/init_textquest.sh

# Option 4: Verify symlinks
# If using symlinks, ensure targets exist:
ls -l config/
# If symlink points to non-existent file, fix:
rm config/textquest.toml
ln -s /actual/path/textquest.toml config/textquest.toml
```

#### 9.1.2 File Permissions

**Symptom:** Error like `Permission denied` when reading config or writing logs.

**Root Causes:**

- File owned by different user
- Directory lacks write permissions
- SELinux or AppArmor policy blocking access
- File is read-only

**Diagnostics:**

```bash
# Check file ownership and permissions
ls -la config/ credentials/ data/
ls -la ~/.cache/textquest/

# Identify permission issues
stat config/textquest.toml | grep "Access: "

# Check current user
whoami

# Verify group membership
groups

# Check SELinux status
getenforce  # (Linux only)

# Check AppArmor status
aa-status   # (Linux only)
```

**Fix:**

```bash
# Option 1: Fix file ownership
sudo chown $USER:$USER config/textquest.toml
sudo chown -R $USER:$USER credentials/ data/
sudo chown -R $USER:$USER ~/.cache/textquest/

# Option 2: Fix file permissions
chmod 644 config/textquest.toml
chmod 755 config/ credentials/ data/
chmod 755 ~/.cache/textquest/

# Option 3: Disable SELinux if enforced
# (Requires sudo; not recommended for production)
sudo setenforce 0

# Option 4: Create AppArmor exception
# (Requires detailed AppArmor knowledge; skip unless necessary)

# Option 5: Run as owner
# If file is owned by another user, run as that user:
sudo -u file_owner textquest status
```

#### 9.1.3 Resource Exhaustion

**Symptom:** Daemon very slow; high CPU or memory usage; commands timeout.

**Root Causes:**

- Too many clients in fleet (memory leak)
- Event log or cache growing unbounded
- Navmesh cache huge
- Metrics collection overhead

**Diagnostics:**

```bash
# Monitor resource usage
ps aux | grep textquest
# Check RSS (memory) and %CPU

# Check disk usage
du -h ~/.cache/textquest/
du -h ./data/
du -h ./credentials/

# Monitor open file descriptors
lsof -p $(pgrep -f "textquest start") | wc -l

# Check event log size
wc -l ~/.cache/textquest/logs/*.log

# Sample system metrics
top -b -n 1 | head -n 20
```

**Fix:**

```bash
# Option 1: Clear event logs
rm ~/.cache/textquest/logs/*
# (Or rotate logs via config)

# Option 2: Clear navmesh cache
rm -rf ~/.cache/textquest/navmesh/
textquest navmesh reload --zone gfaydark
# (Pre-warm only zones in use)

# Option 3: Reduce client count
# Edit config/textquest.toml:
# [fleet.max_clients] = 12  # Lower from default

# Option 4: Disable metrics collection
# Edit config/textquest.toml:
# [metrics.enabled] = false

# Option 5: Restart daemon
# Forces cleanup of in-memory caches:
textquest stop
sleep 2
textquest start

# Option 6: Cleanup old backups/snapshots
rm -rf ~/.cache/textquest/snapshots/*.tar.gz
```

#### 9.1.4 Time Sync / Clock Issues

**Symptom:** Timestamps in logs are wrong; events out of order; cooldown tracking wrong.

**Root Causes:**

- System clock is incorrect
- Timezone misconfigured
- NTP not syncing
- DST transition

**Diagnostics:**

```bash
# Check system time
date
date -u  # UTC

# Check timezone
timedatectl status  # (systemd systems)
cat /etc/timezone
echo $TZ

# Check NTP sync status
timedatectl show | grep NTP
ntpq -p  # (if ntpd running)

# Compare system time to NTP server
ntpdate -q pool.ntp.org

# Check log timestamps
tail ~/.cache/textquest/logs/*.log | grep -o "[0-9]{4}-[0-9]{2}-[0-9]{2}" | sort | uniq
```

**Fix:**

```bash
# Option 1: Sync time immediately
# systemd systems:
sudo timedatectl set-ntp true
sudo systemctl restart systemd-timesyncd

# Option 2: Set time manually (if NTP unavailable)
sudo date -s "2026-04-18 09:30:00"

# Option 3: Fix timezone
# systemd systems:
sudo timedatectl set-timezone America/Chicago  # Example

# Option 4: Set timezone in environment
export TZ=America/Chicago  # For current session

# Option 5: Force NTP sync
sudo ntpdate -s pool.ntp.org  # (if ntpd not running)

# Option 6: Restart after time sync
textquest stop
sleep 2
textquest start
```

---

## 10. ESCALATION & SUPPORT

If you've worked through this decision tree and still can't resolve your issue:

1. **Collect diagnostics:**

   ```bash
   textquest --dump > full_dump.json
   textquest config show > config_dump.txt
   textquest client-status-all > client_status.txt
   grep -r "ERROR\|WARN" ~/.cache/textquest/logs/ > error_log.txt
   ```

2. **Create a GitHub issue with:**
   - Title: Clear description of symptom
   - Category tag: `[LOGIN]`, `[CAMP]`, `[NAV]`, `[ROTATION]`, `[BAN]`, etc.
   - TextQuest version: `textquest --version`
   - EQ server / TLP name
   - Class/character affected
   - Reproduction steps
   - Diagnostic output from above
   - Relevant config snippets (mask passwords)

3. **Contact support channels:**
   - Discord: Post in #help or #troubleshooting
   - Email: Include `[TextQuest-<VERSION>]` in subject
   - Include GitHub issue link

4. **For bans / account issues:**
   - Document all automation patterns used
   - Save Daybreak disconnect messages
   - File with Daybreak support in parallel
   - Tag GitHub issue with `[BAN]` for visibility

---

## Decision Tree Summary (ASCII)

```
START
├─ [1] Startup Failures
│  ├─ Port Conflict
│  ├─ Config Error
│  ├─ Immediate Exit
│  └─ Deadlock/Hang
├─ [2] Injection Failures
│  ├─ Injection Rejected
│  ├─ Pipe Timeout
│  └─ DLL Crash
├─ [3] Login Failures
│  ├─ Missing Account
│  ├─ UI Timeout
│  ├─ Auth Failure
│  └─ Char Select Error
├─ [4] Camp Loop Failures
│  ├─ Action Stuck
│  ├─ Rapid-Fire Loop
│  └─ State Machine Deadlock
├─ [5] Navigation Failures
│  ├─ Nav Blocked
│  ├─ Wrong Path
│  ├─ Zone Line Issue
│  └─ Navmesh Download
├─ [6] Combat / Rotation Failures
│  ├─ Combat Not Starting
│  ├─ Rotation Halts
│  ├─ Ability Skipped
│  └─ Low Effectiveness
├─ [7] Circuit Breaker / Errors
│  ├─ Health Check Failure
│  ├─ Launch Failure
│  ├─ Pipe Reconnect
│  └─ Command Timeout
├─ [8] Account / Ban Issues
│  ├─ Temporary Lockout
│  ├─ Permanent Ban
│  └─ Session Ban
└─ [9] System / Environment
   ├─ Missing Files
   ├─ Permissions
   ├─ Resource Exhaustion
   └─ Time Sync Issues
```

---

## Related Documentation

- [Command Reference](Command-Reference.md) — All TextQuest CLI commands
- [Configuration](Configuration.md) — Detailed config options
- [Combat and Camp Loop](Combat-and-Camp-Loop.md) — Rotation mechanics
- [DLL Injection and IPC](DLL-Injection-and-IPC-Pipeline.md) — Injection details
- [Developer Guide](Developer-Guide.md) — For contributing fixes
