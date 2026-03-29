# ============================================================
#  DMFT Automated Test Loop
#  Pulls latest, builds, injects, and checks results
#  Usage: powershell -ExecutionPolicy Bypass -File test_loop.ps1
#         powershell -ExecutionPolicy Bypass -File test_loop.ps1 -SkipBuild
# ============================================================
param(
    [switch]$SkipBuild,
    [switch]$SkipInject,
    [string]$LogDir = "C:\Users\xmale\Projects\DMFT\test-results"
)

$ErrorActionPreference = "Continue"
$ProjectDir = "C:\Users\xmale\Projects\DMFT"
$Timestamp = Get-Date -Format "yyyy-MM-dd_HH-mm-ss"
$LogFile = Join-Path $LogDir "test_$Timestamp.log"

# Ensure log directory exists
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
}

function Log {
    param([string]$Message, [string]$Level = "INFO")
    $line = "[$Level] $(Get-Date -Format 'HH:mm:ss') $Message"
    Write-Host $line
    Add-Content -Path $LogFile -Value $line
}

function Log-Section {
    param([string]$Title)
    $sep = "=" * 50
    Log $sep
    Log $Title
    Log $sep
}

# --- Header ---
Log-Section "DMFT Test Run: $Timestamp"
Log "Host: $env:COMPUTERNAME"
Log "Log: $LogFile"

$Results = @{
    GitPull   = "SKIP"
    Build     = "SKIP"
    UnitTests = "SKIP"
    EqRunning = "SKIP"
    Inject    = "SKIP"
    DllLog    = "SKIP"
}

# --- Step 1: Git Pull ---
Log-Section "Step 1: Git Pull"
Set-Location $ProjectDir
try {
    $gitOutput = git pull 2>&1 | Out-String
    Log $gitOutput.Trim()
    if ($LASTEXITCODE -eq 0) {
        $Results.GitPull = "PASS"
        Log "Git pull succeeded" "OK"
    } else {
        $Results.GitPull = "FAIL"
        Log "Git pull failed (exit $LASTEXITCODE)" "ERROR"
    }
} catch {
    $Results.GitPull = "FAIL"
    Log "Git pull exception: $_" "ERROR"
}

# --- Step 2: Build ---
if (-not $SkipBuild) {
    Log-Section "Step 2: Build (release)"
    try {
        $buildOutput = cargo build --release 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            $Results.Build = "PASS"
            Log "Build succeeded" "OK"
        } else {
            $Results.Build = "FAIL"
            Log "Build failed (exit $LASTEXITCODE)" "ERROR"
            Log $buildOutput
        }
    } catch {
        $Results.Build = "FAIL"
        Log "Build exception: $_" "ERROR"
    }

    # --- Step 2b: Unit Tests ---
    Log-Section "Step 2b: Unit Tests"
    try {
        $testOutput = cargo test 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            # Extract test summary line
            $summary = $testOutput -split "`n" | Select-String "test result:"
            $Results.UnitTests = "PASS"
            Log "Tests passed: $($summary -join '; ')" "OK"
        } else {
            $Results.UnitTests = "FAIL"
            Log "Tests failed (exit $LASTEXITCODE)" "ERROR"
            # Log failed test names
            $failures = $testOutput -split "`n" | Select-String "FAILED|failures:"
            foreach ($f in $failures) { Log "  $f" "ERROR" }
        }
    } catch {
        $Results.UnitTests = "FAIL"
        Log "Test exception: $_" "ERROR"
    }
} else {
    Log "Skipping build (--SkipBuild)" "INFO"
}

# --- Step 3: Check EQ ---
Log-Section "Step 3: EQ Process Check"
$eqProcs = Get-Process -Name "eqgame" -ErrorAction SilentlyContinue
if ($eqProcs) {
    $Results.EqRunning = "PASS"
    $pids = ($eqProcs | ForEach-Object { $_.Id }) -join ", "
    Log "EQ running: $($eqProcs.Count) client(s), PIDs: $pids" "OK"
} else {
    $Results.EqRunning = "FAIL"
    Log "No eqgame.exe processes found" "WARN"
    Log "Skipping injection (no EQ)" "INFO"
}

# --- Step 4: Inject DLL ---
if ($eqProcs -and -not $SkipInject) {
    Log-Section "Step 4: DLL Injection"

    $dmftExe = Join-Path $ProjectDir "target\release\dmft.exe"
    if (-not (Test-Path $dmftExe)) {
        $dmftExe = Join-Path $ProjectDir "target\debug\dmft.exe"
    }

    if (Test-Path $dmftExe) {
        try {
            $injectOutput = & $dmftExe --inject 2>&1 | Out-String
            Log $injectOutput.Trim()
            if ($LASTEXITCODE -eq 0) {
                $Results.Inject = "PASS"
                Log "Injection succeeded" "OK"
            } else {
                $Results.Inject = "FAIL"
                Log "Injection failed (exit $LASTEXITCODE)" "ERROR"
            }
        } catch {
            $Results.Inject = "FAIL"
            Log "Injection exception: $_" "ERROR"
        }

        # Wait for DLL to initialize
        Start-Sleep -Seconds 3

        # --- Step 5: Check DLL Log ---
        Log-Section "Step 5: DLL Log Check"
        $checkScript = Join-Path $ProjectDir "scripts\check_dll_log.ps1"
        if (Test-Path $checkScript) {
            $logCheck = powershell -ExecutionPolicy Bypass -File $checkScript 2>&1 | Out-String
            Log $logCheck.Trim()
            if ($logCheck -match "OVERALL: PASS") {
                $Results.DllLog = "PASS"
            } else {
                $Results.DllLog = "FAIL"
            }
        } else {
            Log "check_dll_log.ps1 not found, checking manually" "WARN"
            $dllLogDir = "$env:TEMP\dmft"
            $today = Get-Date -Format "yyyy-MM-dd"
            $dllLog = Get-ChildItem "$dllLogDir\dmft-dll.log*" -ErrorAction SilentlyContinue |
                      Sort-Object LastWriteTime -Descending | Select-Object -First 1
            if ($dllLog) {
                $tail = Get-Content $dllLog.FullName -Tail 10
                $hasSuccess = $tail | Select-String "initialized successfully"
                $hasError = $tail | Select-String "error|panic" -CaseSensitive:$false
                if ($hasSuccess) {
                    $Results.DllLog = if ($hasError) { "WARN" } else { "PASS" }
                } else {
                    $Results.DllLog = "FAIL"
                }
                Log "DLL log tail:" "INFO"
                foreach ($line in $tail) { Log "  $line" }
            } else {
                $Results.DllLog = "FAIL"
                Log "No DLL log file found" "ERROR"
            }
        }
    } else {
        $Results.Inject = "FAIL"
        Log "dmft.exe not found" "ERROR"
    }
}

# --- Summary ---
Log-Section "TEST RESULTS SUMMARY"
$allPass = $true
foreach ($key in $Results.Keys | Sort-Object) {
    $status = $Results[$key]
    $icon = switch ($status) {
        "PASS" { "[OK]  " }
        "FAIL" { "[FAIL]" }
        "WARN" { "[WARN]" }
        "SKIP" { "[SKIP]" }
    }
    Log "$icon $key`: $status"
    if ($status -eq "FAIL") { $allPass = $false }
}

$overall = if ($allPass) { "PASS" } else { "FAIL" }
Log ""
Log "OVERALL: $overall"
Log "Log saved to: $LogFile"

# Return exit code for scripting
if (-not $allPass) { exit 1 }
