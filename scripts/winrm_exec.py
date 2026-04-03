#!/usr/bin/env python3
"""Execute commands on frostreaver via WinRM.

Supports two modes:
  - session0: Direct WinRM execution (fast, no GUI access)
  - interactive: Runs in desktop session 1 via scheduled task (GUI-visible)

Usage:
  python3 winrm_exec.py "Get-Process"                    # session 0
  python3 winrm_exec.py --interactive "Start-Process notepad"  # session 1 (GUI)
  python3 winrm_exec.py --interactive --no-capture "Start-Process eqgame.exe"  # fire-and-forget

Required environment variables:
  WINRM_HOST, WINRM_USER, WINRM_PASS
Optional: WINRM_SCHEME (default: https), WINRM_PORT, WINRM_TRANSPORT,
          WINRM_SERVER_CERT_VALIDATION, WINRM_ALLOW_INSECURE_HTTP=1
"""

import argparse
import os
import sys
import time
import uuid

import winrm


def _env(name: str) -> str:
    value = os.getenv(name)
    if not value:
        raise RuntimeError(f"Missing required environment variable: {name}")
    return value


def get_session():
    host = _env("WINRM_HOST")
    user = _env("WINRM_USER")
    password = _env("WINRM_PASS")

    scheme = os.getenv("WINRM_SCHEME", "https").lower()
    if scheme not in {"http", "https"}:
        raise RuntimeError("WINRM_SCHEME must be one of: http, https")
    if scheme == "http" and os.getenv("WINRM_ALLOW_INSECURE_HTTP") != "1":
        raise RuntimeError("Refusing insecure HTTP WinRM. Set WINRM_ALLOW_INSECURE_HTTP=1 to override.")

    port_default = "5986" if scheme == "https" else "5985"
    port = os.getenv("WINRM_PORT", port_default)
    transport = os.getenv("WINRM_TRANSPORT", "ntlm")
    cert_validation = os.getenv("WINRM_SERVER_CERT_VALIDATION", "validate")
    winrm_url = f"{scheme}://{host}:{port}/wsman"

    return winrm.Session(
        winrm_url,
        auth=(user, password),
        transport=transport,
        server_cert_validation=cert_validation,
    )


def run_session0(command: str, use_ps: bool = True) -> tuple[str, str, int]:
    """Run command in WinRM session (session 0). Fast but no GUI access."""
    s = get_session()
    if use_ps:
        r = s.run_ps(command)
    else:
        r = s.run_cmd(command)
    return r.std_out.decode(), r.std_err.decode(), r.status_code


def run_interactive(command: str, capture: bool = True, timeout: int = 10) -> str | None:
    """Run command in interactive desktop session 1 via scheduled task.

    Args:
        command: PowerShell command to execute
        capture: If True, capture stdout to file and return it
        timeout: Seconds to wait for output (only if capture=True)

    Returns:
        Command output if capture=True, else None
    """
    s = get_session()
    task_id = uuid.uuid4().hex[:8]
    task_name = f"FrostExec_{task_id}"

    if capture:
        out_file = f"$env:USERPROFILE\\frost_output_{task_id}.txt"
        bat_content = (
            f'@echo off\n'
            f'powershell -NoProfile -Command "{command}" > %USERPROFILE%\\frost_output_{task_id}.txt 2>&1'
        )
    else:
        out_file = None
        bat_content = (
            f'@echo off\n'
            f'powershell -NoProfile -Command "{command}"'
        )

    ps_script = f"""
$home = $env:USERPROFILE
$bat = "$home\\frost_exec_{task_id}.bat"

@"
{bat_content}
"@ | Out-File -FilePath $bat -Encoding ascii

schtasks /Create /TN "{task_name}" /TR $bat /SC ONCE /ST 00:00 /F /IT /RL HIGHEST 2>&1 | Out-Null
schtasks /Run /TN "{task_name}" 2>&1 | Out-Null
"""

    if capture:
        ps_script += f"""
$elapsed = 0
while ($elapsed -lt {timeout}) {{
    Start-Sleep -Seconds 1
    $elapsed++
    $info = schtasks /Query /TN "{task_name}" /FO LIST 2>&1 | Select-String "Status"
    if ($info -match "Ready") {{ break }}
}}

if (Test-Path {out_file}) {{
    Get-Content {out_file}
    Remove-Item {out_file} -ErrorAction SilentlyContinue
}} else {{
    "ERROR: No output captured"
}}
schtasks /Delete /TN "{task_name}" /F 2>&1 | Out-Null
Remove-Item $bat -ErrorAction SilentlyContinue
"""
    else:
        ps_script += f"""
Start-Sleep -Seconds 2
schtasks /Delete /TN "{task_name}" /F 2>&1 | Out-Null
Remove-Item "$home\\frost_exec_{task_id}.bat" -ErrorAction SilentlyContinue
"""

    r = s.run_ps(ps_script)
    stdout = r.std_out.decode().strip()
    return stdout if capture else None


def main():
    parser = argparse.ArgumentParser(description="Execute commands on frostreaver via WinRM")
    parser.add_argument("command", help="Command to execute")
    parser.add_argument("--interactive", "-i", action="store_true",
                        help="Run in interactive desktop session (session 1)")
    parser.add_argument("--no-capture", action="store_true",
                        help="Don't capture output (fire-and-forget, interactive only)")
    parser.add_argument("--cmd", action="store_true",
                        help="Use cmd.exe instead of PowerShell (session 0 only)")
    parser.add_argument("--timeout", type=int, default=10,
                        help="Seconds to wait for interactive output (default: 10)")
    args = parser.parse_args()

    if args.interactive:
        result = run_interactive(args.command, capture=not args.no_capture, timeout=args.timeout)
        if result is not None:
            print(result)
    else:
        stdout, stderr, code = run_session0(args.command, use_ps=not args.cmd)
        if stdout:
            print(stdout.strip())
        if stderr:
            # Filter out CLIXML progress noise
            clean = [l for l in stderr.split("\n")
                     if l.strip() and "CLIXML" not in l and "Preparing modules" not in l]
            if clean:
                print("\n".join(clean), file=sys.stderr)
        sys.exit(code)


if __name__ == "__main__":
    main()
