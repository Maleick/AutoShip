# =============================================================================
# DMFT — Windows Setup Script
# =============================================================================
# Installs Rust toolchain, VS Build Tools, clones repo, and builds all crates.
#
# USAGE:
#   1. Open PowerShell as Administrator
#   2. Run: Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
#   3. Run: .\scripts\setup-windows.ps1
#
# If the repo is already cloned, run from the repo root.
# If not, run from any directory — the script will clone into .\DMFT
# =============================================================================

$ErrorActionPreference = "Stop"
$RepoUrl = "https://github.com/Maleick/DMFT.git"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " DMFT — Windows Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ---------------------------------------------------------------------------
# Step 1: Check for Git
# ---------------------------------------------------------------------------
Write-Host "[1/6] Checking for Git..." -ForegroundColor Yellow

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host "  Git not found. Installing via winget..." -ForegroundColor Red
    try {
        winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
    } catch {
        Write-Host "  ERROR: Could not install Git. Please install manually from https://git-scm.com" -ForegroundColor Red
        exit 1
    }
}
$gitVersion = git --version
Write-Host "  OK: $gitVersion" -ForegroundColor Green

# ---------------------------------------------------------------------------
# Step 2: Check for Visual Studio Build Tools (MSVC)
# ---------------------------------------------------------------------------
Write-Host "[2/6] Checking for Visual Studio Build Tools..." -ForegroundColor Yellow

$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasBuildTools = $false

if (Test-Path $vsWhere) {
    $installed = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($installed) {
        $hasBuildTools = $true
    }
}

if (-not $hasBuildTools) {
    Write-Host "  VS Build Tools not found. Installing via winget..." -ForegroundColor Red
    Write-Host "  This may take 5-15 minutes..." -ForegroundColor Yellow
    try {
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget `
            --accept-package-agreements --accept-source-agreements `
            --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    } catch {
        Write-Host "  ERROR: Could not install VS Build Tools." -ForegroundColor Red
        Write-Host "  Install manually: https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor Red
        Write-Host "  Select 'Desktop development with C++' workload." -ForegroundColor Red
        exit 1
    }
    Write-Host "  VS Build Tools installed. You may need to restart PowerShell." -ForegroundColor Green
} else {
    Write-Host "  OK: VS Build Tools found" -ForegroundColor Green
}

# ---------------------------------------------------------------------------
# Step 3: Check for Rust
# ---------------------------------------------------------------------------
Write-Host "[3/6] Checking for Rust..." -ForegroundColor Yellow

if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host "  Rust not found. Installing via rustup..." -ForegroundColor Red
    try {
        $rustupInit = "$env:TEMP\rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit
        & $rustupInit -y --default-toolchain stable-x86_64-pc-windows-msvc
        Remove-Item $rustupInit -ErrorAction SilentlyContinue
        # Refresh PATH
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
        $env:Path += ";$env:USERPROFILE\.cargo\bin"
    } catch {
        Write-Host "  ERROR: Could not install Rust. Please install manually from https://rustup.rs" -ForegroundColor Red
        exit 1
    }
}
$rustVersion = rustc --version
$cargoVersion = cargo --version
Write-Host "  OK: $rustVersion" -ForegroundColor Green
Write-Host "  OK: $cargoVersion" -ForegroundColor Green

# ---------------------------------------------------------------------------
# Step 4: Clone or update repo
# ---------------------------------------------------------------------------
Write-Host "[4/6] Setting up repository..." -ForegroundColor Yellow

# If we're already in the DMFT repo, just pull
if (Test-Path "Cargo.toml") {
    $cargoContent = Get-Content "Cargo.toml" -Raw
    if ($cargoContent -match 'members.*=.*\[.*"dmft"') {
        Write-Host "  Already in DMFT repo. Pulling latest..." -ForegroundColor Green
        git pull
    }
} elseif (Test-Path "DMFT\Cargo.toml") {
    Write-Host "  DMFT directory exists. Pulling latest..." -ForegroundColor Green
    Set-Location DMFT
    git pull
} else {
    Write-Host "  Cloning repository..." -ForegroundColor Green
    git clone $RepoUrl
    Set-Location DMFT
}

Write-Host "  OK: Repository ready at $(Get-Location)" -ForegroundColor Green

# Configure a repo-local nightly override to match CI and release builds.
Write-Host "  Configuring repo-local nightly MSVC toolchain..." -ForegroundColor Green
try {
    rustup toolchain install nightly-x86_64-pc-windows-msvc | Out-Null
    rustup override set nightly-x86_64-pc-windows-msvc | Out-Null
    Write-Host "  OK: rustup override set to nightly-x86_64-pc-windows-msvc" -ForegroundColor Green
} catch {
    Write-Host "  ERROR: Could not configure repo-local nightly toolchain." -ForegroundColor Red
    Write-Host "  Run manually in the repo:" -ForegroundColor Yellow
    Write-Host "    rustup toolchain install nightly-x86_64-pc-windows-msvc" -ForegroundColor Yellow
    Write-Host "    rustup override set nightly-x86_64-pc-windows-msvc" -ForegroundColor Yellow
    exit 1
}

# ---------------------------------------------------------------------------
# Step 5: Build debug
# ---------------------------------------------------------------------------
Write-Host "[5/6] Building debug..." -ForegroundColor Yellow

$buildResult = cargo build 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  OK: Debug build succeeded" -ForegroundColor Green
} else {
    Write-Host "  ERROR: Debug build failed!" -ForegroundColor Red
    Write-Host $buildResult
    Write-Host ""
    Write-Host "  Common fixes:" -ForegroundColor Yellow
    Write-Host "  - Restart PowerShell after installing VS Build Tools" -ForegroundColor Yellow
    Write-Host "  - Run: rustup override set nightly-x86_64-pc-windows-msvc" -ForegroundColor Yellow
    exit 1
}

# ---------------------------------------------------------------------------
# Step 6: Build release
# ---------------------------------------------------------------------------
Write-Host "[6/6] Building release..." -ForegroundColor Yellow

$buildResult = cargo build --release 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  OK: Release build succeeded" -ForegroundColor Green
} else {
    Write-Host "  ERROR: Release build failed!" -ForegroundColor Red
    Write-Host $buildResult
    exit 1
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " Setup Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Run TUI demo:    cargo run" -ForegroundColor White
Write-Host "  2. Run tests:       .\scripts\test-windows.ps1" -ForegroundColor White
Write-Host "  3. Run with EQ:     cargo run  (with eqgame.exe running)" -ForegroundColor White
Write-Host ""
Write-Host "Binaries built to:" -ForegroundColor Cyan
Write-Host "  Debug:   target\debug\dmft.exe" -ForegroundColor White
Write-Host "  Release: target\release\dmft.exe" -ForegroundColor White
Write-Host "  DLL:     target\release\dmft_dll.dll" -ForegroundColor White
Write-Host ""
