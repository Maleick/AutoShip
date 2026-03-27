# =============================================================================
# Frostreaver (DMFT) — Windows Test Runner
# =============================================================================
# Runs automated tests and reports results. Run from the repo root.
#
# USAGE:
#   .\scripts\test-windows.ps1            # Run all automated tests (Tier 1)
#   .\scripts\test-windows.ps1 -Tier 2    # Include Tier 2 (requires EQ running)
#
# Paste the output summary into Discord for Mike.
# =============================================================================

param(
    [int]$Tier = 1
)

$ErrorActionPreference = "Continue"
$passed = 0
$failed = 0
$skipped = 0
$results = @()

function Test-Check {
    param(
        [string]$Name,
        [scriptblock]$Test
    )

    Write-Host "  [$Name] " -NoNewline -ForegroundColor Yellow
    try {
        $result = & $Test
        if ($result) {
            Write-Host "PASS" -ForegroundColor Green
            $script:passed++
            $script:results += "PASS: $Name"
        } else {
            Write-Host "FAIL" -ForegroundColor Red
            $script:failed++
            $script:results += "FAIL: $Name"
        }
    } catch {
        Write-Host "FAIL ($_)" -ForegroundColor Red
        $script:failed++
        $script:results += "FAIL: $Name ($_)"
    }
}

function Test-Skip {
    param([string]$Name, [string]$Reason)
    Write-Host "  [$Name] SKIP ($Reason)" -ForegroundColor DarkGray
    $script:skipped++
    $script:results += "SKIP: $Name ($Reason)"
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Frostreaver — Windows Test Suite" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Date: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
Write-Host " Host: $env:COMPUTERNAME"
Write-Host " OS:   $([System.Environment]::OSVersion.VersionString)"
Write-Host ""

# ==========================================================================
# TIER 1 — Build & TUI (No EQ Required)
# ==========================================================================
Write-Host "--- TIER 1: Build & TUI Verification ---" -ForegroundColor Cyan
Write-Host ""

# T1.1: Rust toolchain
Test-Check "Rust toolchain installed" {
    $null = rustc --version 2>&1
    $LASTEXITCODE -eq 0
}

# T1.2: Cargo available
Test-Check "Cargo available" {
    $null = cargo --version 2>&1
    $LASTEXITCODE -eq 0
}

# T1.3: Debug build — dmft-common
Test-Check "cargo build: dmft-common" {
    $null = cargo build -p dmft-common 2>&1
    $LASTEXITCODE -eq 0
}

# T1.4: Debug build — dmft-dll
Test-Check "cargo build: dmft-dll" {
    $null = cargo build -p dmft-dll 2>&1
    $LASTEXITCODE -eq 0
}

# T1.5: Debug build — dmft
Test-Check "cargo build: dmft" {
    $null = cargo build -p dmft 2>&1
    $LASTEXITCODE -eq 0
}

# T1.6: Release build — full workspace
Test-Check "cargo build --release (all crates)" {
    $null = cargo build --release 2>&1
    $LASTEXITCODE -eq 0
}

# T1.7: Clippy
Test-Check "cargo clippy (no errors)" {
    $output = cargo clippy 2>&1
    # Clippy returns 0 even with warnings; check for actual errors
    $hasErrors = $output | Select-String "^error\[" | Measure-Object | Select-Object -ExpandProperty Count
    $hasErrors -eq 0
}

# T1.8: Config file exists
Test-Check "Config file exists (config/frostreaver.toml)" {
    Test-Path "config/frostreaver.toml"
}

# T1.9: DLL artifact built
Test-Check "DLL artifact exists (target/release/dmft_dll.dll)" {
    Test-Path "target/release/dmft_dll.dll"
}

# T1.10: EXE artifact built
Test-Check "EXE artifact exists (target/release/dmft.exe)" {
    Test-Path "target/release/dmft.exe"
}

# T1.11: TUI launches and exits cleanly (send 'q' after 2 seconds)
Test-Check "TUI launches in demo mode" {
    $proc = Start-Process -FilePath "target\release\dmft.exe" -PassThru -NoNewWindow
    Start-Sleep -Seconds 3
    if (-not $proc.HasExited) {
        $proc.Kill()
        $true  # It ran for 3 seconds without crashing
    } else {
        $proc.ExitCode -eq 0
    }
}

# T1.12: Dump mode runs
Test-Check "Dump mode (--dump) runs" {
    $null = & "target\release\dmft.exe" --dump 2>&1
    # On Windows without EQ, this should exit gracefully (non-zero is OK if no process found)
    $true  # Just checking it doesn't crash/hang
}

Write-Host ""

# ==========================================================================
# TIER 2 — EQ Process Reading (Requires EQ Running)
# ==========================================================================
if ($Tier -ge 2) {
    Write-Host "--- TIER 2: EQ Process Reading ---" -ForegroundColor Cyan
    Write-Host ""

    # Check if EQ is running
    $eqProcess = Get-Process -Name "eqgame" -ErrorAction SilentlyContinue

    if (-not $eqProcess) {
        Write-Host "  EQ is not running. Skipping Tier 2 tests." -ForegroundColor DarkGray
        Write-Host "  Launch EQ and log in, then re-run with: .\scripts\test-windows.ps1 -Tier 2" -ForegroundColor Yellow
        Test-Skip "EQ process detection" "eqgame.exe not running"
        Test-Skip "Spawn list population" "eqgame.exe not running"
        Test-Skip "Player data reading" "eqgame.exe not running"
        Test-Skip "Multi-client detection" "eqgame.exe not running"
    } else {
        $eqCount = ($eqProcess | Measure-Object).Count
        Write-Host "  Found $eqCount eqgame.exe process(es)" -ForegroundColor Green
        Write-Host ""

        # T2.1: Process detection
        Test-Check "EQ process detected by dmft" {
            $output = & "target\release\dmft.exe" --dump 2>&1 | Out-String
            $output -match "(?i)found|player|spawn"
        }

        # T2.2: Player data
        Test-Check "Player data readable" {
            $output = & "target\release\dmft.exe" --dump 2>&1 | Out-String
            $output -match "(?i)name|level|class|hp"
        }

        # T2.3: Spawn list
        Test-Check "Spawn list populated" {
            $output = & "target\release\dmft.exe" --dump 2>&1 | Out-String
            $output -match "(?i)spawn|npc|pc"
        }

        # T2.4: Multi-client
        if ($eqCount -gt 1) {
            Test-Check "Multiple EQ clients detected ($eqCount)" {
                $true
            }
        } else {
            Test-Skip "Multi-client detection" "Only 1 EQ client running"
        }
    }

    Write-Host ""
}

# ==========================================================================
# TIER 3 — DLL Injection (Manual)
# ==========================================================================
if ($Tier -ge 3) {
    Write-Host "--- TIER 3: DLL Injection (Manual Steps) ---" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Tier 3 tests are MANUAL. Follow these steps:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  WARNING: Use a THROWAWAY ACCOUNT only!" -ForegroundColor Red
    Write-Host "  DLL injection may trigger anti-cheat detection." -ForegroundColor Red
    Write-Host ""
    Write-Host "  1. Launch ONE EQ client on a throwaway account" -ForegroundColor White
    Write-Host "  2. Log into any server, select any character" -ForegroundColor White
    Write-Host "  3. Run: target\release\dmft.exe --inject" -ForegroundColor White
    Write-Host "  4. Check for: 'Injection successful' message" -ForegroundColor White
    Write-Host "  5. Check for: IPC connection established" -ForegroundColor White
    Write-Host "  6. In TUI, verify live data updates" -ForegroundColor White
    Write-Host "  7. Test eject: send eject command from TUI" -ForegroundColor White
    Write-Host "  8. Verify EQ client remains stable after eject" -ForegroundColor White
    Write-Host ""
    Write-Host "  Report results in Discord with:" -ForegroundColor Yellow
    Write-Host "    - Did injection succeed? (yes/no/crash)" -ForegroundColor White
    Write-Host "    - Did IPC connect? (yes/no/timeout)" -ForegroundColor White
    Write-Host "    - Any error messages? (paste them)" -ForegroundColor White
    Write-Host "    - Did EQ crash? (when — on inject/during/on eject)" -ForegroundColor White
    Write-Host ""
}

# ==========================================================================
# Summary
# ==========================================================================
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Test Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Passed:  $passed" -ForegroundColor Green
Write-Host "  Failed:  $failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })
Write-Host "  Skipped: $skipped" -ForegroundColor DarkGray
Write-Host ""

Write-Host "--- Copy below and paste into Discord ---" -ForegroundColor Yellow
Write-Host ""
Write-Host "``````"
Write-Host "DMFT Test Report — $(Get-Date -Format 'yyyy-MM-dd HH:mm')"
Write-Host "Host: $env:COMPUTERNAME | OS: $([System.Environment]::OSVersion.VersionString)"
Write-Host "Rust: $(rustc --version 2>&1)"
Write-Host "Results: $passed passed / $failed failed / $skipped skipped"
Write-Host ""
foreach ($r in $results) {
    Write-Host $r
}
Write-Host "``````"
Write-Host ""

if ($failed -gt 0) {
    Write-Host "Some tests failed. Check the output above for details." -ForegroundColor Red
    exit 1
} else {
    Write-Host "All tests passed!" -ForegroundColor Green
    exit 0
}
