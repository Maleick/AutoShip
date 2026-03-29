# ============================================================
#  DMFT Remote API Server
#  Minimal HTTP server for remote control from macOS
#  Usage: powershell -ExecutionPolicy Bypass -File remote_api.ps1
#         powershell -ExecutionPolicy Bypass -File remote_api.ps1 -Port 8080
#
#  Endpoints:
#    GET  /status       - EQ process status + DLL log tail
#    POST /run          - Execute PowerShell command (body = command string)
#    POST /screenshot   - Capture screen, return base64 PNG
#    GET  /test-results - Latest test_loop results
#    GET  /dll-log      - Latest DLL log tail
# ============================================================
param(
    [int]$Port = 8080
)

$ErrorActionPreference = "Stop"
$ProjectDir = "C:\Users\xmale\Projects\DMFT"

Write-Host "Starting DMFT Remote API on port $Port..."

# Create HTTP listener
$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://+:$Port/")

try {
    $listener.Start()
} catch {
    Write-Host "Failed to start listener. Try running as Administrator, or use:"
    Write-Host "  netsh http add urlacl url=http://+:$Port/ user=$env:USERNAME"
    exit 1
}

Write-Host "Listening on http://localhost:$Port/"
Write-Host "Endpoints: /status, /run, /screenshot, /test-results, /dll-log"
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
    $logFiles = Get-ChildItem "$env:TEMP\dmft\dmft-dll.log*" -ErrorAction SilentlyContinue |
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
    $logFiles = Get-ChildItem "$env:TEMP\dmft\dmft-dll.log*" -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending
    if ($logFiles) {
        $content = Get-Content $logFiles[0].FullName -Tail $Lines -ErrorAction SilentlyContinue
        return @{
            file    = $logFiles[0].Name
            lines   = $content
            count   = $content.Count
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
                 "Get-ChildItem", "Test-Path", "dir", "type", "systeminfo", "hostname")
    $firstWord = ($Command -split '\s+')[0]
    $isAllowed = $allowed | Where-Object { $firstWord -like "$_*" }
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
    $outFile = Join-Path $env:TEMP "dmft_screenshot.png"
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

# --- Main loop ---
while ($listener.IsListening) {
    try {
        $context = $listener.GetContext()
        $request = $context.Request
        $response = $context.Response
        $path = $request.Url.AbsolutePath
        $method = $request.HttpMethod

        Write-Host "$(Get-Date -Format 'HH:mm:ss') $method $path"

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
                    $reader = New-Object System.IO.StreamReader($request.InputStream)
                    $body = $reader.ReadToEnd()
                    $reader.Close()

                    # Parse JSON body if present, otherwise treat as raw command
                    try {
                        $parsed = $body | ConvertFrom-Json
                        $cmd = $parsed.command
                    } catch {
                        $cmd = $body
                    }

                    if (-not $cmd) {
                        Send-JsonResponse $response @{ error = "No command provided" } 400
                    } else {
                        $result = Invoke-RemoteCommand -Command $cmd
                        Send-JsonResponse $response $result
                    }
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
            default {
                $help = @{
                    endpoints = @(
                        "GET  /status       - EQ process status + DLL log summary"
                        "POST /run          - Execute command (body: {`"command`":`"...`"})"
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
