# Remote Control Setup — frostreaver

## Overview

Three methods are available for remote command execution on frostreaver from macOS:

| Method | Session | GUI Apps | Speed | Setup |
|--------|---------|----------|-------|-------|
| SSH | Session 0 | No | Fast | Already working |
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
- **User**: maleick / 1118

### Session isolation finding

**WinRM has the same session 0 problem as SSH.** Commands run via WinRM execute in session 0 (services), not the interactive desktop session 1. GUI apps launched from session 0 are invisible to the logged-in user.

### Solution: Scheduled task bridge

Using `schtasks /IT` (interactive-only flag) with `/RU` and `/RP` creates a task that runs in the interactive desktop session 1. This is the **proven method** for launching GUI-visible apps remotely.

Pattern:
1. Write a .bat file via WinRM (session 0)
2. Create a scheduled task with `/IT` flag
3. Run the task — it executes in session 1
4. Optionally capture output via file redirect
5. Clean up task and bat file

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
sshpass -p '1118' ssh maleick@frostreaver "winrm set winrm/config/service @{AllowUnencrypted=\"true\"} && winrm set winrm/config/service/auth @{Basic=\"true\"}"
```
