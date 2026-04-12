param(
    [string]$RepoRoot = (Get-Location).Path,
    [switch]$CopyToDesktop
)

$ErrorActionPreference = "Stop"

$targetRoot = Join-Path $RepoRoot "target\release"
$recastBuildRoot = Join-Path $targetRoot "build"
$desktopDrop = Join-Path $env:USERPROFILE "Desktop\TextQuest-Test"
$verifyScript = Join-Path $RepoRoot "scripts\verify_injection.bat"
$repoConfigMaps = Join-Path $RepoRoot "config\maps"
$repoDataMeshes = Join-Path $RepoRoot "data\meshes"
$repoGhidraDb = Join-Path $RepoRoot "data\ghidra.db"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " TextQuest Windows Release Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " RepoRoot: $RepoRoot"
Write-Host ""

if (-not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    throw "Repo root does not contain Cargo.toml: $RepoRoot"
}

Write-Host "[1/3] Clearing stale recast build state..." -ForegroundColor Yellow
Get-ChildItem $recastBuildRoot -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "recastnavigation-sys-*" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[2/3] Refreshing Rust source mtimes for synced files..." -ForegroundColor Yellow
$refreshRoots = @(
    (Join-Path $RepoRoot "textquest-common\src"),
    (Join-Path $RepoRoot "textquest\src"),
    (Join-Path $RepoRoot "textquest-dll\src")
)
$now = Get-Date
foreach ($root in $refreshRoots) {
    if (Test-Path $root) {
        Get-ChildItem $root -Recurse -File -Include *.rs |
            ForEach-Object { $_.LastWriteTime = $now }
    }
}

Write-Host "[3/3] Building release artifacts..." -ForegroundColor Yellow
Push-Location $RepoRoot
try {
    $env:CMAKE_GENERATOR = "Visual Studio 17 2022"
    cargo +nightly build --release -p textquest -p textquest-dll
} finally {
    Pop-Location
}

$exe = Join-Path $targetRoot "textquest.exe"
$dll = Join-Path $targetRoot "textquest_dll.dll"

if (-not (Test-Path $exe)) {
    throw "Missing build artifact: $exe"
}
if (-not (Test-Path $dll)) {
    throw "Missing build artifact: $dll"
}

Write-Host "[4/4] Release artifacts ready:" -ForegroundColor Yellow
Get-Item $exe, $dll |
    Select-Object FullName, Length, @{Name="LastWriteTime"; Expression = { $_.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss") }} |
    Format-Table -AutoSize

if ($CopyToDesktop) {
    $desktopExe = Join-Path $desktopDrop "textquest.exe"
    $desktopDll = Join-Path $desktopDrop "textquest_dll.dll"
    $desktopConfig = Join-Path $desktopDrop "config"
    $desktopConfigMaps = Join-Path $desktopConfig "maps"
    $desktopData = Join-Path $desktopDrop "data"
    $desktopDataMeshes = Join-Path $desktopData "meshes"
    $desktopGhidraDb = Join-Path $desktopData "ghidra.db"

    Write-Host ""
    Write-Host "Copying artifacts to desktop drop..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $desktopDrop | Out-Null
    Remove-Item $desktopExe, $desktopDll -Force -ErrorAction SilentlyContinue
    Remove-Item $desktopConfig, $desktopData -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $desktopDrop "*.cmd") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $desktopDrop "README.txt") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $desktopDrop "TEST-ORDER.txt") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $desktopDrop "verify_injection.bat") -Force -ErrorAction SilentlyContinue
    Copy-Item $exe $desktopExe -Force
    Copy-Item $dll $desktopDll -Force
    if (Test-Path $repoConfigMaps) {
        New-Item -ItemType Directory -Force -Path $desktopConfig | Out-Null
        Copy-Item $repoConfigMaps $desktopConfigMaps -Recurse -Force
    }
    if (Test-Path $repoDataMeshes) {
        New-Item -ItemType Directory -Force -Path $desktopData | Out-Null
        Copy-Item $repoDataMeshes $desktopDataMeshes -Recurse -Force
    }
    if (Test-Path $repoGhidraDb) {
        New-Item -ItemType Directory -Force -Path $desktopData | Out-Null
        Copy-Item $repoGhidraDb $desktopGhidraDb -Force
    } else {
        Write-Warning "Missing data\\ghidra.db in repo root; Debug Explorer will show 'No ghidra.db loaded' in the desktop drop."
    }

    if (Test-Path $verifyScript) {
        Copy-Item $verifyScript (Join-Path $desktopDrop "verify_injection.bat") -Force
    }

    $dumpCmd = @"
@echo off
cd /d "$desktopDrop"
"$desktopExe" --dump
pause
"@

    $injectCmd = @"
@echo off
cd /d "$desktopDrop"
"$desktopExe" --inject
pause
"@

    $uiCmd = @"
@echo off
cd /d "$desktopDrop"
"$desktopExe"
"@

    Set-Content -Path (Join-Path $desktopDrop "01-dump.cmd") -Value $dumpCmd -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "02-inject.cmd") -Value $injectCmd -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "03-ui.cmd") -Value $uiCmd -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "04-ui-workspace.cmd") -Value $uiCmd -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "05-inject-workspace.cmd") -Value $injectCmd -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "06-dump-workspace.cmd") -Value $dumpCmd -Encoding Ascii

    $testOrder = @"
TextQuest Test Retest (fresh desktop artifacts)

1. Launch EverQuest and get fully in game before using TextQuest.
2. Run: $desktopDrop\02-inject.cmd
3. If injection says FAILED, stop there.
4. Run: $desktopDrop\01-dump.cmd
5. Run: $desktopDrop\03-ui.cmd
6. Optional verification: $desktopDrop\verify_injection.bat

Logs to review:
- $desktopDrop\logs\textquest-dump.log.*
- $desktopDrop\logs\textquest.log.*
- %TEMP%\textquest\textquest-dll.log*
- %TEMP%\textquest\textquest-dll-init-*.log

Important:
- Use the desktop drop or target\release artifacts only.
- If you want the Debug Explorer, make sure $desktopDrop\data\ghidra.db is present.
- Do NOT launch $RepoRoot\textquest.exe or $RepoRoot\textquest_dll.dll directly. Those root-level copies can be stale.
"@
    Set-Content -Path (Join-Path $desktopDrop "TEST-ORDER.txt") -Value $testOrder -Encoding Ascii
    Set-Content -Path (Join-Path $desktopDrop "README.txt") -Value $testOrder -Encoding Ascii

    Get-Item $desktopExe, $desktopDll |
        Select-Object FullName, @{Name="LastWriteTime"; Expression = { $_.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss") }} |
        Format-Table -AutoSize
}
