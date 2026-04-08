# ============================================================
#  TextQuest Remote API Server
#  Minimal HTTP server for remote control from macOS
#  Usage: powershell -ExecutionPolicy Bypass -File remote_api.ps1
#         powershell -ExecutionPolicy Bypass -File remote_api.ps1 -Port 8080
#
#  Endpoints:
#    GET  /status       - EQ process status + DLL log tail
#    POST /run          - Disabled (was command execution; removed for security)
#    POST /screenshot   - Capture screen, return base64 PNG
#    GET  /test-results - Latest test_loop results
#    GET  /dll-log      - Latest DLL log tail
# ============================================================
param(
    [int]$Port = 8080,
    [string]$ApiKey = $env:DMFT_API_KEY
)

$ErrorActionPreference = "Stop"
$ProjectDir = "C:\Users\xmale\Projects\TextQuest"

Write-Host "Starting TextQuest Remote API on port $Port..."

# Create HTTP listener (localhost-only to avoid remote exposure)
$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://localhost:$Port/")

try {
    $listener.Start()
} catch {
    Write-Host "Failed to start listener on localhost:$Port."
    exit 1
}

Write-Host "Listening on http://localhost:$Port/"
Write-Host "Endpoints: /status, /run (disabled), /screenshot, /test-results, /dll-log"
Write-Host "Press Ctrl+C to stop."

function Get-EqStatus {
    $procs = Get-Process -Name "eqgame" -ErrorAction SilentlyContinue
    $result = @{
        eq_running = ($null -ne $procs)
        eq_count   = if ($procs) { $procs.Count } else { 0 }
        eq_pids    = if ($procs) { $procs | ForEach-Object { $_.Id } } else { @() }
        timestamp  = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    }

    # DLL log check
    $logFiles = Get-ChildItem "$env:TEMP\textquest\textquest-dll.log*" -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending
    if ($logFiles) {
        $latest = $logFiles[0]
        $tail = Get-Content $latest.FullName -Tail 10 -ErrorAction SilentlyContinue
        $result.dll_log_file = $latest.Name
        $result.dll_log_age_minutes = [math]::Round(((Get-Date) - $latest.LastWriteTime).TotalMinutes, 1)
        $result.dll_log_tail = $tail -join "`n"
        $result.dll_initialized = ($tail | Select-String "initialized successfully").Count -gt 0
        $result.dll_errors = ($tail | Select-String " ERROR ").Count
    } else {
        $result.dll_log_file = $null
        $result.dll_initialized = $false
    }

    return $result
}

function Get-DllLogTail {
    param([int]$Lines = 50)
    $logFiles = Get-ChildItem "$env:TEMP\textquest\textquest-dll.log*" -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending
    if ($logFiles) {
        $content = Get-Content $logFiles[0].FullName -Tail $Lines -ErrorAction SilentlyContinue
        # Convert to plain strings to avoid PS object serialization bloat
        $plainLines = @($content | ForEach-Object { $_.ToString() })
        return @{
            file    = $logFiles[0].Name
            lines   = $plainLines
            count   = $plainLines.Count
        }
    }
    return @{ file = $null; lines = @(); count = 0 }
}

function Get-LatestTestResult {
    $resultDir = Join-Path $ProjectDir "test-results"
    $files = Get-ChildItem "$resultDir\test_*.log" -ErrorAction SilentlyContinue |
             Sort-Object LastWriteTime -Descending
    if ($files) {
        $content = Get-Content $files[0].FullName -ErrorAction SilentlyContinue
        $overall = ($content | Select-String "^OVERALL:") | Select-Object -Last 1
        return @{
            file    = $files[0].Name
            overall = if ($overall) { $overall.ToString().Trim() } else { "UNKNOWN" }
            content = $content -join "`n"
        }
    }
    return @{ file = $null; overall = "NO RESULTS"; content = "" }
}

function Invoke-RemoteCommand {
    param([string]$Command)
    # Allowlist: only permit known-safe commands
    $allowed = @("tasklist", "netstat", "cargo", "git", "Get-Process", "Get-Content",
                 "Get-ChildItem", "Test-Path", "dir", "type", "systeminfo", "hostname",
                 "Start-Process", "Stop-Process", "powershell")

    # Reject command separator characters to prevent multi-command injection.
    if ($Command -match "[;|&]" -or $Command.Contains("`n") -or $Command.Contains("`r")) {
        return @{
            success = $false
            output  = "Command contains disallowed separators"
            exit_code = -1
        }
    }

    $trimmed = $Command.Trim()
    if (-not $trimmed) {
        return @{
            success = $false
            output  = "No command provided"
            exit_code = -1
        }
    }

    $firstWord = ($trimmed -split '\s+', 2)[0]
    $isAllowed = $allowed -contains $firstWord
    if (-not $isAllowed) {
        return @{
            success = $false
            output  = "Command not in allowlist. Allowed: $($allowed -join ', ')"
            exit_code = -1
        }
    }
    try {
        $pinfo = New-Object System.Diagnostics.ProcessStartInfo
        $pinfo.FileName = "powershell.exe"
        $pinfo.Arguments = "-NoProfile -Command `"$Command`""
        $pinfo.RedirectStandardOutput = $true
        $pinfo.RedirectStandardError = $true
        $pinfo.UseShellExecute = $false
        $pinfo.CreateNoWindow = $true
        $proc = [System.Diagnostics.Process]::Start($pinfo)
        $stdout = $proc.StandardOutput.ReadToEnd()
        $stderr = $proc.StandardError.ReadToEnd()
        $proc.WaitForExit(30000)
        return @{
            success   = ($proc.ExitCode -eq 0)
            output    = ($stdout + $stderr).Trim()
            exit_code = $proc.ExitCode
        }
    } catch {
        return @{
            success = $false
            output  = $_.ToString()
            exit_code = -1
        }
    }
}

function Get-Screenshot {
    # Use built-in Windows screenshot tool to avoid AV triggers
    $outFile = Join-Path $env:TEMP "textquest_screenshot.png"
    $snippingArgs = "/clip"
    try {
        # Use nircmd if available, otherwise fall back to info message
        $nircmd = "C:\tools\nircmd.exe"
        if (Test-Path $nircmd) {
            & $nircmd savescreenshot $outFile
            if (Test-Path $outFile) {
                $bytes = [System.IO.File]::ReadAllBytes($outFile)
                Remove-Item $outFile -Force
                return $bytes
            }
        }
        return $null
    } catch {
        return $null
    }
}

function Send-JsonResponse {
    param($Response, $Data, [int]$StatusCode = 200)
    $json = $Data | ConvertTo-Json -Depth 5 -Compress
    $buffer = [System.Text.Encoding]::UTF8.GetBytes($json)
    $Response.StatusCode = $StatusCode
    $Response.ContentType = "application/json"
    $Response.ContentLength64 = $buffer.Length
    $Response.OutputStream.Write($buffer, 0, $buffer.Length)
    $Response.OutputStream.Close()
}

function Send-BinaryResponse {
    param($Response, [byte[]]$Data, [string]$ContentType)
    $Response.StatusCode = 200
    $Response.ContentType = $ContentType
    $Response.ContentLength64 = $Data.Length
    $Response.OutputStream.Write($Data, 0, $Data.Length)
    $Response.OutputStream.Close()
}

function Test-AuthorizedRequest {
    param($Request)
    $providedKey = $Request.Headers["X-DMFT-API-Key"]
    return -not [string]::IsNullOrWhiteSpace($providedKey) -and ($providedKey -eq $ApiKey)
}

# --- Main loop ---
while ($listener.IsListening) {
    try {
        $context = $listener.GetContext()
        $request = $context.Request
        $response = $context.Response
        $path = $request.Url.AbsolutePath
        $method = $request.HttpMethod

        Write-Host "$(Get-Date -Format 'HH:mm:ss') $method $path"

        if (-not (Test-AuthorizedRequest -Request $request)) {
            Send-JsonResponse $response @{ error = "Unauthorized" } 401
            continue
        }

        switch ($path) {
            "/status" {
                $status = Get-EqStatus
                Send-JsonResponse $response $status
            }
            "/dll-log" {
                $lines = 50
                if ($request.QueryString["lines"]) {
                    $lines = [int]$request.QueryString["lines"]
                }
                $log = Get-DllLogTail -Lines $lines
                Send-JsonResponse $response $log
            }
            "/test-results" {
                $results = Get-LatestTestResult
                Send-JsonResponse $response $results
            }
            "/run" {
                if ($method -ne "POST") {
                    Send-JsonResponse $response @{ error = "POST required" } 405
                } else {
                    Send-JsonResponse $response @{
                        error = "The /run endpoint has been disabled for security reasons."
                    } 403
                }
            }
            "/screenshot" {
                try {
                    $pngBytes = Get-Screenshot
                    if ($pngBytes) {
                        Send-BinaryResponse $response $pngBytes "image/png"
                    } else {
                        Send-JsonResponse $response @{ error = "Screenshot not available (install nircmd to C:\tools\)" } 501
                    }
                } catch {
                    Send-JsonResponse $response @{ error = $_.ToString() } 500
                }
            }
            "/launch-eq" {
                # Dedicated EQ launcher — no allowlist needed
                if ($method -ne "POST") {
                    Send-JsonResponse $response @{ error = "POST required" } 405
                } else {
                    $reader = New-Object System.IO.StreamReader($request.InputStream)
                    $body = $reader.ReadToEnd()
                    $reader.Close()
                    try {
                        $parsed = $body | ConvertFrom-Json
                    } catch {
                        $parsed = @{ account = "frostmale001" }
                    }
                    $account = if ($parsed.account) { $parsed.account } else { "frostmale001" }
                    $eqPath = "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"
                    try {
                        Start-Process -FilePath "$eqPath\eqgame.exe" `
                            -ArgumentList "patchme","/login:$account" `
                            -WorkingDirectory $eqPath
                        Send-JsonResponse $response @{ success = $true; account = $account; message = "EQ launched" }
                    } catch {
                        Send-JsonResponse $response @{ success = $false; error = $_.ToString() } 500
                    }
                }
            }
            "/inject" {
                # Inject DLL into running EQ
                if ($method -ne "POST") {
                    Send-JsonResponse $response @{ error = "POST required" } 405
                } else {
                    try {
                        $result = & "$ProjectDir\target\release\textquest.exe" --inject 2>&1
                        Send-JsonResponse $response @{ success = $true; output = ($result -join "`n") }
                    } catch {
                        Send-JsonResponse $response @{ success = $false; error = $_.ToString() } 500
                    }
                }
            }
            "/kill-eq" {
                # Kill all EQ processes
                if ($method -ne "POST") {
                    Send-JsonResponse $response @{ error = "POST required" } 405
                } else {
                    $procs = Get-Process -Name "eqgame" -ErrorAction SilentlyContinue
                    if ($procs) {
                        $procs | Stop-Process -Force
                        Send-JsonResponse $response @{ success = $true; killed = $procs.Count }
                    } else {
                        Send-JsonResponse $response @{ success = $true; killed = 0; message = "No EQ processes" }
                    }
                }
            }
            "/restart" {
                # Pull latest code and restart the API
                if ($method -ne "POST") {
                    Send-JsonResponse $response @{ error = "POST required" } 405
                } else {
                    Send-JsonResponse $response @{ success = $true; message = "Restarting..." }
                    Set-Location $ProjectDir
                    & git pull
                    # Stop listener and re-launch
                    $listener.Stop()
                    Start-Process powershell -ArgumentList "-ExecutionPolicy Bypass -File $ProjectDir\scripts\remote_api.ps1 -Port $Port"
                    exit 0
                }
            }
            default {
                $help = @{
                    endpoints = @(
                        "GET  /status       - EQ process status + DLL log summary"
                        "POST /run          - Disabled for security"
                        "POST /launch-eq    - Launch EQ (body: {`"account`":`"name`"})"
                        "POST /inject       - Inject DLL into running EQ"
                        "POST /kill-eq      - Kill all EQ processes"
                        "POST /restart      - Pull latest + restart API"
                        "POST /screenshot   - Capture screen (returns PNG)"
                        "GET  /test-results - Latest test loop results"
                        "GET  /dll-log      - DLL log tail (?lines=N)"
                    )
                }
                Send-JsonResponse $response $help
            }
        }
    } catch {
        Write-Host "Error: $_" -ForegroundColor Red
    }
}
