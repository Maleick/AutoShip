# =============================================================================
# TextQuest — Self-hosted GitHub Actions runner bootstrap
# =============================================================================
# This script installs/updates the runner in C:\actions-runner and configures it for
# this repository with labels [self-hosted, Windows, X64, textquest].
#
# USAGE:
#   .\scripts\setup-self-hosted-runner.ps1 -Token "<YOUR_REGISTRATION_TOKEN>"
#
# Optional:
#   -Token "<token>"            New registration token from GitHub UI
#   -RepositoryUrl "<url>"       Defaults to https://github.com/Maleick/TextQuest
#   -Version "latest"|"2.333.1"  Runner package version (defaults to latest)
#   -RunnerName "MyRunner"      Runner display name
#   -RunnerRoot "C:\actions-runner"
#   -InstallService             Install and start as a Windows service
#   -Force                     Remove existing runner config before reconfigure
#   -ServiceName "MyService"    Windows service name when svc.cmd is not available
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Token,

    [string]$RepositoryUrl = "https://github.com/Maleick/TextQuest",
    [string]$Version = "latest",
    [string]$RunnerRoot = "C:\actions-runner",
    [string]$RunnerName = "",
    [string[]]$Labels = @("textquest", "Windows", "X64"),
    [switch]$InstallService,
    [string]$ServiceName = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

function Add-SafeGitDirectory {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return
    }

    if (-not (Test-Path $Path)) {
        Write-Host "Skipping non-existent safe.directory path: $Path" -ForegroundColor Yellow
        return
    }

    git config --global --add safe.directory $Path
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to mark $Path as safe.directory for git."
    }

    Write-Host "Marked Git safe.directory: $Path" -ForegroundColor Green
}

function Ensure-NightlyRustToolchain {
    param(
        [string]$Toolchain = "nightly-x86_64-pc-windows-msvc"
    )

    $cargoHome = $env:CARGO_HOME
    if ([string]::IsNullOrWhiteSpace($cargoHome)) {
        $cargoHome = Join-Path $env:USERPROFILE ".cargo"
    }

    $rustup = Join-Path $cargoHome "bin\rustup.exe"
    if (-not (Test-Path $rustup)) {
        throw "Rustup was not found under $rustup. Install rustup manually from https://rustup.rs before running this script."
    }

    $env:PATH = "${cargoHome}\bin;${env:PATH}"
    try {
        & $rustup show | Out-Null
        & $rustup toolchain install $Toolchain | Out-Null
        & $rustup default $Toolchain | Out-Null
        & $rustup component add rustfmt clippy --toolchain $Toolchain | Out-Null
        Write-Host "Rust nightly toolchain configured: $Toolchain" -ForegroundColor Green
    } catch {
        throw "Failed to configure rustup toolchain '$Toolchain'."
    }
}

if ([string]::IsNullOrWhiteSpace($Token)) {
    throw "A runner registration token is required. Generate one from GitHub Settings → Actions → Runners."
}

if (-not $RunnerName) {
    $RunnerName = "textquest-$(hostname)"
}

if ($Version -eq "latest") {
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/actions/runner/releases/latest" -Headers @{ "User-Agent" = "textquest-self-hosted-runner-setup" }
        $Version = ($release.tag_name -replace "^v", "")
    } catch {
        throw "Could not resolve latest runner release. Re-run with -Version set explicitly."
    }
}

$Version = $Version -replace "^v", ""
$ZipName = "actions-runner-win-x64-$Version.zip"
$DownloadUrl = "https://github.com/actions/runner/releases/download/v$Version/$ZipName"

Write-Host "Runner root: $RunnerRoot" -ForegroundColor Cyan
New-Item -Path $RunnerRoot -ItemType Directory -Force | Out-Null
Set-Location $RunnerRoot

if (Test-Path ".runner") {
    if (-not $Force) {
        throw ".runner already exists. Stop existing runner and delete/recreate this directory, or rerun with -Force."
    }

    Write-Host "Removing existing runner configuration..." -ForegroundColor Yellow
    if (Test-Path ".\svc.cmd") {
        try { .\svc.cmd stop | Out-Null } catch {}
        try { .\svc.cmd uninstall | Out-Null } catch {}
    }
    Remove-Item -Path ".runner", ".credentials", ".credentials_openssl", ".service", "svc.sh", "svc.cmd" -ErrorAction SilentlyContinue
}

$ZipPath = Join-Path $RunnerRoot $ZipName
if (Test-Path $ZipPath) {
    Write-Host "Runner zip already present: $ZipName" -ForegroundColor Green
} else {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
}

for ($attempt = 1; $attempt -le 2; $attempt++) {
    try {
        Write-Host "Extracting $ZipName (attempt $attempt)..." -ForegroundColor Yellow
        Expand-Archive -LiteralPath $ZipPath -DestinationPath $RunnerRoot -Force
        break
    } catch {
        if ($attempt -ge 2) { throw }
        Write-Host "Zip extraction failed. Re-downloading and retrying..." -ForegroundColor Yellow
        Remove-Item -Path $ZipPath -Force
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
    }
}

if (-not (Test-Path "$RunnerRoot\config.cmd")) {
    throw "Extraction completed but config.cmd is missing. Check archive contents for version $Version."
}

if (-not (Test-Path "$RunnerRoot\run.cmd")) {
    throw "Extraction completed but run.cmd is missing. Check archive contents for version $Version."
}

Write-Host "Configuring runner for $RepositoryUrl" -ForegroundColor Yellow
$runnerConfigArgs = @("--unattended", "--url", $RepositoryUrl, "--token", $Token, "--name", $RunnerName, "--labels", ($Labels -join ","), "--work", "_work")
if ($InstallService) { $runnerConfigArgs += "--runasservice" }
& "$RunnerRoot\config.cmd" @runnerConfigArgs
    if ($LASTEXITCODE -ne 0) {
    throw "config.cmd returned exit code $LASTEXITCODE"
}

Write-Host "Runner configured with labels: self-hosted, $($Labels -join ', ')." -ForegroundColor Green
Write-Host "Seeding Git safe.directory entries for this runner..." -ForegroundColor Yellow

$repoName = "TextQuest"
try {
    $repoName = ([System.Uri]$RepositoryUrl).Segments[-1].TrimEnd("/")
} catch {
    Write-Host "Could not parse repository name from RepositoryUrl, using fallback 'TextQuest'." -ForegroundColor Yellow
}
if ([string]::IsNullOrWhiteSpace($repoName)) {
    $repoName = "TextQuest"
}

$workRoot = Join-Path $RunnerRoot "_work"
Add-SafeGitDirectory -Path (Join-Path $workRoot "$repoName\$repoName")
Add-SafeGitDirectory -Path (Join-Path $workRoot $repoName)
Add-SafeGitDirectory -Path "C:\actions-runner\_work\$repoName\$repoName"
Add-SafeGitDirectory -Path "C:\actions-runner\_work\$repoName"
Write-Host "Completed git safe.directory seeding." -ForegroundColor Green
Write-Host ""
Write-Host "Ensuring nightly Rust toolchain for release workflows..." -ForegroundColor Yellow
Ensure-NightlyRustToolchain -Toolchain "nightly-x86_64-pc-windows-msvc"

if (-not $ServiceName) {
    $ServiceName = "GitHubActionsRunner-$RunnerName"
}
$ServiceName = $ServiceName -replace "[^\w\-\.]", "_"

if ($InstallService) {
    $isAdmin = $false
    try {
        $current = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = New-Object System.Security.Principal.WindowsPrincipal($current)
        $isAdmin = $principal.IsInRole([System.Security.Principal.WindowsBuiltinRole]::Administrator)
    } catch {}

    if (Test-Path "$RunnerRoot\svc.cmd") {
        Write-Host "Installing runner as Windows service..." -ForegroundColor Yellow
        & "$RunnerRoot\svc.cmd" install
        if ($LASTEXITCODE -ne 0) { throw "svc.cmd install failed with exit code $LASTEXITCODE" }

        Write-Host "Starting runner service..." -ForegroundColor Yellow
        & "$RunnerRoot\svc.cmd" start
        if ($LASTEXITCODE -ne 0) { throw "svc.cmd start failed with exit code $LASTEXITCODE" }

        Write-Host "Checking service status..." -ForegroundColor Yellow
        & "$RunnerRoot\svc.cmd" status
    } else {
        if (-not $isAdmin) {
            Write-Host "svc.cmd was not found and this session is not elevated. Service install requires admin." -ForegroundColor Red
            Write-Host "  1) Open an elevated PowerShell / Command Prompt." -ForegroundColor Yellow
            Write-Host "  2) Run this setup script again with -InstallService." -ForegroundColor Yellow
            Write-Host "  3) Or run the fallback commands below manually." -ForegroundColor Yellow
            Write-Host "     sc.exe delete `"$ServiceName`"" -ForegroundColor Yellow
            Write-Host "     sc.exe create `"$ServiceName`" binPath= `"`"$env:SystemRoot\System32\cmd.exe`" /c `"$RunnerRoot\run.cmd`"`" start= auto DisplayName= `"GitHub Actions Runner - $RunnerName`"" -ForegroundColor Yellow
            Write-Host "     sc.exe start `"$ServiceName`"" -ForegroundColor Yellow
            Write-Host "     sc.exe query `"$ServiceName`"" -ForegroundColor Yellow
            Write-Host "If a Service with same name exists, replace `$ServiceName." -ForegroundColor Yellow
        } else {
            $binPath = "`"$env:SystemRoot\System32\cmd.exe`" /c `"$RunnerRoot\run.cmd`""
            Write-Host "Creating service '$ServiceName'..." -ForegroundColor Yellow
            sc.exe delete "$ServiceName" | Out-Null
            sc.exe create "$ServiceName" binPath= $binPath start= auto DisplayName= "GitHub Actions Runner - $RunnerName" | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "sc.exe create failed with exit code $LASTEXITCODE" }

            Write-Host "Starting service '$ServiceName'..." -ForegroundColor Yellow
            sc.exe start "$ServiceName" | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "sc.exe start failed with exit code $LASTEXITCODE" }

            Write-Host "Checking service status..." -ForegroundColor Yellow
            sc.exe query "$ServiceName"
        }
    }
} else {
    Write-Host "Skipped service install. Run .\run.cmd now to validate one-shot execution."
}

Write-Host ""
Write-Host "Done. Verify in GitHub under Settings → Actions → Runners that the runner is online." -ForegroundColor Green
Write-Host "Self-hosted CI/wiki workflows use runner-local Python when available and otherwise fall back to a checksum-verified embeddable Python ZIP." -ForegroundColor Cyan
Write-Host "If this machine will run wiki-nightly or GitHub Project tooling, also verify local GitHub CLI auth:" -ForegroundColor Cyan
Write-Host "  gh auth status" -ForegroundColor Yellow
Write-Host "  gh auth refresh -s project -s read:project" -ForegroundColor Yellow
Write-Host "The nightly wiki workflow uses runner-local gh auth for scripts/sync_wiki.py --push." -ForegroundColor Cyan
