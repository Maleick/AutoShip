# ============================================================
#  DMFT DLL Log Checker
#  Reads latest DLL log and checks for success/failure markers
#  Usage: powershell -ExecutionPolicy Bypass -File check_dll_log.ps1
#         powershell -ExecutionPolicy Bypass -File check_dll_log.ps1 -Lines 50
# ============================================================
param(
    [int]$Lines = 30,
    [string]$LogDir = "$env:TEMP\dmft"
)

$ErrorActionPreference = "Continue"

function Write-Status {
    param([string]$Status, [string]$Message)
    $color = switch ($Status) {
        "PASS" { "Green" }
        "FAIL" { "Red" }
        "WARN" { "Yellow" }
        "INFO" { "Cyan" }
    }
    Write-Host "[$Status] " -ForegroundColor $color -NoNewline
    Write-Host $Message
}

# --- Find latest log file ---
$logFiles = Get-ChildItem "$LogDir\dmft-dll.log*" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending

if (-not $logFiles) {
    Write-Status "FAIL" "No DLL log files found in $LogDir"
    Write-Host "OVERALL: FAIL"
    exit 1
}

$latestLog = $logFiles[0]
$logAge = (Get-Date) - $latestLog.LastWriteTime
Write-Status "INFO" "Latest log: $($latestLog.Name) (modified $([math]::Round($logAge.TotalMinutes, 1)) min ago)"

# --- Read log tail ---
$content = Get-Content $latestLog.FullName -Tail $Lines

if (-not $content) {
    Write-Status "FAIL" "Log file is empty"
    Write-Host "OVERALL: FAIL"
    exit 1
}

# --- Check markers ---
$checks = @{
    Initialized   = @{ Pattern = "initialized successfully"; Found = $false; Lines = @() }
    GameLoop      = @{ Pattern = "Game loop hook installed"; Found = $false; Lines = @() }
    IPC           = @{ Pattern = "IPC started|IPC listener thread started"; Found = $false; Lines = @() }
    Phase3        = @{ Pattern = "Phase 3:"; Found = $false; Lines = @() }
    EnterWorld    = @{ Pattern = "EnterWorld"; Found = $false; Lines = @() }
    Errors        = @{ Pattern = " ERROR "; Found = $false; Lines = @() }
    Panics        = @{ Pattern = "panic|PANIC|thread.*panicked"; Found = $false; Lines = @() }
}

foreach ($line in $content) {
    foreach ($key in $checks.Keys) {
        if ($line -match $checks[$key].Pattern) {
            $checks[$key].Found = $true
            $checks[$key].Lines += $line
        }
    }
}

# --- Report ---
Write-Host ""
Write-Host "=== DLL Health Checks ==="
Write-Host ""

# Required markers (PASS if found)
$requiredPass = $true
foreach ($key in @("Initialized", "GameLoop", "IPC")) {
    if ($checks[$key].Found) {
        Write-Status "PASS" "$key`: found"
    } else {
        Write-Status "FAIL" "$key`: NOT found in last $Lines lines"
        $requiredPass = $false
    }
}

# Optional markers (INFO)
foreach ($key in @("Phase3", "EnterWorld")) {
    if ($checks[$key].Found) {
        Write-Status "INFO" "$key`: found ($($checks[$key].Lines.Count) occurrence(s))"
        foreach ($l in $checks[$key].Lines | Select-Object -Last 2) {
            Write-Host "         $l"
        }
    } else {
        Write-Status "INFO" "$key`: not present (may not apply to current test)"
    }
}

# Bad markers (FAIL/WARN if found)
$hasErrors = $false
foreach ($key in @("Errors", "Panics")) {
    if ($checks[$key].Found) {
        $severity = if ($key -eq "Panics") { "FAIL" } else { "WARN" }
        if ($key -eq "Panics") { $hasErrors = $true }
        Write-Status $severity "$key`: $($checks[$key].Lines.Count) found"
        foreach ($l in $checks[$key].Lines | Select-Object -Last 5) {
            Write-Host "         $l"
        }
    } else {
        Write-Status "PASS" "$key`: none found"
    }
}

# --- Overall ---
Write-Host ""
$overall = if ($requiredPass -and -not $hasErrors) { "PASS" } else { "FAIL" }
Write-Host "OVERALL: $overall"

if ($overall -eq "FAIL") { exit 1 }
