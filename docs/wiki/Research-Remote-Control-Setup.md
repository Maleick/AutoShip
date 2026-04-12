# Remote Control Setup — frostreaver

## Overview

Three methods are available for remote command execution on frostreaver from macOS:

| Method | Session | GUI Apps | Speed | Setup |
|--------|---------|----------|-------|-------|
| SSH | Session 0 | No | Fast | Already working |
| SSH + interactive scheduled task | Session 1 | **Yes** | ~3-5s overhead | Proven 2026-04-09 |
| WinRM (direct) | Session 0 | No | Fast | Configured |
| WinRM + schtasks /IT | Session 1 | **Yes** | ~3-5s overhead | Configured |

## WinRM Setup (completed 2026-03-29)

### What was configured

1. **WinRM service**: Enabled, set to auto-start
2. **Basic auth**: Enabled (`winrm set winrm/config/service/auth @{Basic="true"}`)
3. **AllowUnencrypted**: Enabled (safe — Tailscale provides encryption)
4. **Firewall**: WinRM exception enabled (port 5985)

### Connection details

- **URL**: `http://frostreaver:5985/wsman`
- **Transport**: basic
- **Credentials**: Use your local WinRM username/password (do not commit secrets)

### Session isolation finding

**WinRM has the same session 0 problem as SSH.** Commands run via WinRM execute in session 0 (services), not the interactive desktop session 1. GUI apps launched from session 0 are invisible to the logged-in user.

### Solution: Scheduled task bridge

Using `schtasks /IT` (interactive-only flag) with `/RU` and `/RP` creates a task that runs in the interactive desktop session 1. This is the **proven method** for launching GUI-visible apps remotely.

The same pattern also works over SSH by calling the PowerShell `ScheduledTasks`
module directly. This matters because the current day-to-day control path from
macOS is SSH, not WinRM.

Pattern:
1. Write a .bat file via WinRM (session 0)
2. Create a scheduled task with `/IT` flag
3. Run the task — it executes in session 1
4. Optionally capture output via file redirect
5. Clean up task and bat file

## Proof: 2026-04-09 live checks on frostreaver

Observed active desktop session:

- `query user` reported `maleick` in `console` session `1`

Direct SSH launch proof:

- `Start-Process eqgame.exe patchme` launched `eqgame.exe` in `SessionId = 0`
- That confirms direct SSH launch is still unsuitable for visible EQ automation

Interactive task proof:

- An SSH-created interactive scheduled task launched `notepad.exe` in `SessionId = 1`
- An SSH-created interactive scheduled task launched `eqgame.exe patchme` in `SessionId = 1`

Operational conclusion:

- Do **not** launch EverQuest directly via raw SSH or raw WinRM if the test needs
  the visible desktop session
- Use an interactive scheduled task bridge to launch:
  - `eqgame.exe patchme /login:<account>`
  - or a desktop-session TextQuest entrypoint such as `textquest.exe autologin --spawn ...`

## Recommended TextQuest path

For fully autonomous remote testing, the best current shape is:

1. Trigger a desktop-session task on `frostreaver`
2. That task runs `textquest.exe autologin --spawn ...` locally
3. TextQuest then handles:
   - EQ spawn
   - DLL injection
   - `StartLogin`
   - DLL-side UI automation

Only the task trigger should happen over SSH/WinRM. The actual `autologin` run
must execute in session 1.

### Important: Home directory

WinRM session uses `C:\Users\xmale` (not `C:\Users\maleick`). Always use `$env:USERPROFILE` in scripts.

## Helper script: `scripts/winrm_exec.py`

Requires: `pip install pywinrm`

### Usage

```bash
# Session 0 (fast, no GUI)
python3 scripts/winrm_exec.py "Get-Process"
python3 scripts/winrm_exec.py --cmd "whoami"

# Interactive session 1 (GUI-visible)
python3 scripts/winrm_exec.py -i "Start-Process notepad"
python3 scripts/winrm_exec.py -i "Start-Process eqgame.exe" --no-capture

# With custom timeout for slow commands
python3 scripts/winrm_exec.py -i "some-slow-command" --timeout 30
```

### API (for use in other Python scripts)

```python
from scripts.winrm_exec import run_session0, run_interactive

# Fast session 0 command
stdout, stderr, code = run_session0("Get-Service WinRM")

# Interactive session 1 with output capture
output = run_interactive("Get-Process | Select Name, Id", capture=True)

# Fire-and-forget GUI launch
run_interactive("Start-Process notepad", capture=False)
```

## Existing remote API

The Frostreaver remote API (`scripts/remote_api.ps1`) runs on port 9999 and provides EQ-specific endpoints (`/launch-eq`, `/inject`, `/kill-eq`, `/restart`). It auto-starts via the Windows startup folder.

For EQ operations, prefer the remote API. Use WinRM for general system administration and ad-hoc commands.

## Re-applying WinRM config after reboot

WinRM service is set to auto-start, but if config resets:

```bash
ssh frostreaver "winrm set winrm/config/service @{AllowUnencrypted=\"true\"} && winrm set winrm/config/service/auth @{Basic=\"true\"}"
```
