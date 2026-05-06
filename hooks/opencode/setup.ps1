#Requires -Version 5.1
<#
.SYNOPSIS
    AutoShip OpenCode setup wizard for Windows.

.DESCRIPTION
    PowerShell equivalent of setup.sh for Windows environments.
    Discovers live models, writes .autoship/config.json and .autoship/model-routing.json.

.PARAMETER NoTui
    Run in non-interactive mode (skip prompts).

.PARAMETER MaxAgents
    Set max concurrent agents (default: 20).

.PARAMETER Labels
    Comma-separated labels to monitor (default: agent:ready).

.PARAMETER RefreshModels
    Force refresh model inventory from OpenCode.

.PARAMETER PlannerModel
    Set planner/coordinator/orchestrator/reviewer/lead model.

.PARAMETER WorkerModels
    Comma-separated worker models (default: auto-detect free).

.EXAMPLE
    .\setup.ps1

    # Non-interactive with defaults
    .\setup.ps1 -NoTui

    # Custom configuration
    .\setup.ps1 -NoTui -MaxAgents 10 -Labels "agent:ready,needs-work" -RefreshModels
#>
param(
    [switch]$NoTui,
    [int]$MaxAgents = 20,
    [string]$Labels = "agent:ready",
    [switch]$RefreshModels,
    [string]$PlannerModel = "",
    [string]$WorkerModels = ""
)

$ErrorActionPreference = "Stop"

$AutoshipDir = ".autoship"
$RoutingFile = Join-Path $AutoshipDir "model-routing.json"
$ConfigFile = Join-Path $AutoshipDir "config.json"

function Initialize-AutoshipDirectory {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path -Force

        if (-not $item.PSIsContainer) {
            throw "Path '$Path' exists but is not a directory."
        }

        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing to use '$Path' because it is a symlink or reparse point."
        }

        return
    }

    New-Item -ItemType Directory -Path $Path -Force | Out-Null
}

Initialize-AutoshipDirectory -Path $AutoshipDir

function Test-Command {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-OpencodeModels {
    try {
        $output = opencode models 2>$null
        if ($LASTEXITCODE -ne 0 -or -not $output) { return $null }
        return $output
    }
    catch {
        return $null
    }
}

function Get-ModelIds {
    param([string]$ModelsOutput)
    $ModelsOutput -split "`r?`n" |
        ForEach-Object { $_.Trim() } |
        Where-Object { $_ -match '^[a-z0-9._-]+/.+' } |
        Sort-Object -Unique
}

function Test-FreeModel {
    param([string]$Model)
    $m = $Model.ToLower()
    return ($m -like "*:free*" -or $m -like "*/free*" -or $m -like "*-free*" -or
            $m -eq "opencode/big-pickle" -or $m -eq "opencode/gpt-5-nano")
}

function Test-GoModel {
    param([string]$Model)
    return $Model.ToLower().StartsWith("opencode-go/")
}

function Get-FreeModelRank {
    param([string]$Model)
    $m = $Model.ToLower()
    $score = 40

    switch -Regex ($m) {
        "nemotron-3-super" { $score = 95 }
        "kimi.*k2\.6|kimi-2\.6" { $score = 94 }
        "gpt-oss-120b" { $score = 92 }
        "gpt-5-nano" { $score = 90 }
        "llama-3\.3-70b" { $score = 88 }
        "big-pickle" { $score = 86 }
        "minimax-m2\.5" { $score = 84 }
        "qwen|glm|kimi|mimo" { $score = 80 }
        "gemma-3-27b|gemma-4-31b" { $score = 72 }
        "mistral|devstral" { $score = 68 }
        "ling" { $score = 62 }
        "hy3" { $score = 56 }
    }

    if ($m.StartsWith("opencode/")) { $score += 6 }
    elseif ($m.StartsWith("openrouter/")) { $score += 3 }

    return $score
}

function Get-DefaultFreeModels {
    param([array]$AvailableIds)
    $models = $AvailableIds |
        Where-Object { Test-FreeModel $_ } |
        ForEach-Object { [PSCustomObject]@{ Score = (Get-FreeModelRank $_); Model = $_ } } |
        Sort-Object -Property Score -Descending |
        Select-Object -ExpandProperty Model

    return ($models -join ",")
}

function Get-DefaultRoleModel {
    param([array]$AvailableIds)

    $preferred = $AvailableIds | Where-Object { $_ -match '^opencode-go/(kimi|kimmy).*2\.6' } | Select-Object -First 1
    if ($preferred) { return $preferred }

    $preferred = $AvailableIds |
        Where-Object { Test-FreeModel $_ } |
        ForEach-Object { [PSCustomObject]@{ Score = (Get-FreeModelRank $_); Model = $_ } } |
        Sort-Object -Property Score -Descending |
        Select-Object -ExpandProperty Model -First 1
    if ($preferred) { return $preferred }

    $preferred = $AvailableIds | Where-Object { Test-GoModel $_ } | Select-Object -First 1
    if ($preferred) { return $preferred }

    $preferred = $AvailableIds | Where-Object { $_ -match 'gpt-5\.5|gpt-5\.3-spark' } | Select-Object -First 1
    if ($preferred) { return $preferred }

    return $AvailableIds | Select-Object -First 1
}

function Get-ModelStrength {
    param([string]$Model)
    $m = $Model.ToLower()
    if ($m -like "*:free*" -or $m -like "*free*") { $base = 45 } else { $base = 90 }

    if ($m -like "*nemotron-3-super*") { return 80 }
    if ($m -like "*minimax-m2.5*") { return 75 }
    if ($m -like "*gpt-oss-120b*") { return 78 }
    if ($m -like "*llama-3.3-70b*") { return 70 }
    if ($m -like "*gemma-3-27b*" -or $m -like "*gemma-4-31b*") { return 65 }
    if ($m -like "*ling-2.6*") { return 60 }
    if ($m -like "*hy3*") { return 55 }
    return $base
}

function Get-TaskTypes {
    param([string]$Model)
    $m = $Model.ToLower()
    if ($m -match "nemotron-3-super|gpt-oss-120b|llama-3.3-70b") {
        return @("docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe")
    }
    if ($m -match "minimax|qwen|glm|kimi|mimo") {
        return @("docs", "simple_code", "medium_code", "mechanical", "ci_fix", "rust_unsafe")
    }
    if ($m -match "ling|gemma|mistral|devstral") {
        return @("docs", "simple_code", "mechanical", "ci_fix")
    }
    return @("docs", "simple_code", "mechanical")
}

function Test-ForbiddenModel {
    param([string]$Model)
    return $Model -eq "openai/gpt-5.5-fast"
}

function Find-MissingModels {
    param([array]$AvailableIds, [string]$Requested)
    $requestedList = $Requested -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    $missing = @()
    foreach ($model in $requestedList) {
        if ($AvailableIds -notcontains $model) {
            $missing += $model
        }
    }
    return $missing | Select-Object -Unique
}

# ─── Main ────────────────────────────────────────────────────────────────────

if (-not (Test-Command "gh")) {
    Write-Error "GitHub CLI (gh) is required. Install from https://cli.github.com/"
    exit 1
}

try {
    gh auth status >$null 2>&1
} catch {
    Write-Error "GitHub authentication required. Run 'gh auth login' or set GH_TOKEN."
    exit 1
}

if (-not (Test-Command "opencode")) {
    Write-Error "OpenCode CLI is required for AutoShip workers. Install from https://opencode.ai/"
    exit 1
}

$availableModels = Invoke-OpencodeModels
if (-not $availableModels) {
    Write-Error "Unable to list OpenCode models. Ensure opencode is authenticated."
    exit 1
}

$availableIds = Get-ModelIds -ModelsOutput $availableModels
if ($availableIds.Count -eq 0) {
    Write-Error "No OpenCode model IDs found in model list."
    exit 1
}

Write-Host "Found $($availableIds.Count) available models." -ForegroundColor Green

if ($RefreshModels -and (Test-Path $RoutingFile)) {
    Remove-Item $RoutingFile -Force
    Remove-Item $ConfigFile -Force -ErrorAction SilentlyContinue
}

if ((Test-Path $RoutingFile) -and -not $WorkerModels -and -not $RefreshModels -and -not $PlannerModel) {
    try {
        $existing = Get-Content $RoutingFile -Raw | ConvertFrom-Json
        if ($existing.models -and $existing.models.Count -gt 0) {
            if (-not (Test-Path $ConfigFile)) {
                $labelsList = $Labels -split "," | ForEach-Object { $_.Trim() }
                $config = @{
                    runtime = "opencode"
                    maxConcurrentAgents = $MaxAgents
                    max_agents = $MaxAgents
                    models = @()
                    labels = $labelsList
                    refreshModels = $false
                    policyProfile = "default"
                    cargoConcurrencyCap = 8
                    cargoTargetIsolationThreshold = 8
                    cargoTimeoutSeconds = 120
                    mergeStrategy = "safe"
                    quotaRouting = $true
                    workerCwdLock = $true
                    truncationSalvage = $true
                    workflowRunnerDefault = ""
                }
                $config | ConvertTo-Json -Depth 10 | Set-Content $ConfigFile -Encoding UTF8
            }
            Write-Host "AutoShip OpenCode setup already configured" -ForegroundColor Cyan
            Write-Host "Model routing preserved: $RoutingFile"
            Write-Host "Set -RefreshModels to regenerate from current opencode models."
            exit 0
        }
    }
    catch {
        # Invalid existing file, continue with setup
    }
}

$selectedModels = if ($WorkerModels) { $WorkerModels } else { Get-DefaultFreeModels -AvailableIds $availableIds }

$defaultRoleModel = Get-DefaultRoleModel -AvailableIds $availableIds
$plannerModel = if ($PlannerModel) { $PlannerModel } else { $defaultRoleModel }
$coordinatorModel = $plannerModel
$orchestratorModel = $plannerModel
$reviewerModel = $plannerModel
$leadModel = $plannerModel

if (-not $NoTui -and [Environment]::UserInteractive) {
    Write-Host "Running in interactive mode. Use -NoTui for non-interactive." -ForegroundColor Yellow
    $prompt = Read-Host "Orchestrator model [$orchestratorModel]"
    if ($prompt) { $orchestratorModel = $prompt }
    $prompt = Read-Host "Reviewer model [$reviewerModel]"
    if ($prompt) { $reviewerModel = $prompt }
}

$allModels = @($selectedModels, $plannerModel, $coordinatorModel, $orchestratorModel, $reviewerModel, $leadModel) -join ","
foreach ($model in ($allModels -split ",")) {
    $model = $model.Trim()
    if (Test-ForbiddenModel $model) {
        Write-Error "openai/gpt-5.5-fast is not allowed for AutoShip. Use openai/gpt-5.5 instead."
        exit 1
    }
}

if (-not $selectedModels) {
    Write-Error "No free OpenCode models found. Set -WorkerModels to choose models explicitly."
    exit 1
}

$missing = Find-MissingModels -AvailableIds $availableIds -Requested $selectedModels
if ($missing) {
    Write-Error "Selected worker models are not currently available:`n$($missing -join "`n")"
    exit 1
}

$missingRole = Find-MissingModels -AvailableIds $availableIds -Requested "$plannerModel,$coordinatorModel,$orchestratorModel,$reviewerModel,$leadModel"
if ($missingRole) {
    Write-Error "Role models are not currently available:`n$($missingRole -join "`n")"
    exit 1
}

# Build entries
$modelsList = $selectedModels -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ }
$entries = @()
foreach ($model in $modelsList) {
    $entries += @{
        id = $model
        cost = if (Test-FreeModel $model) { "free" } else { "selected" }
        strength = Get-ModelStrength $model
        max_task_types = Get-TaskTypes $model
    }
}

$defaultFallback = ($entries | Where-Object { $_.cost -eq "free" } | Select-Object -First 1).id
if (-not $defaultFallback) { $defaultFallback = $entries[0].id }

# Write model-routing.json
$poolModels = $entries | ForEach-Object { $_.id }
$frontendModels = $entries | Where-Object { ($_.max_task_types -contains "frontend") -or ($_.max_task_types -contains "docs") } | ForEach-Object { $_.id }
if (-not $frontendModels) { $frontendModels = $poolModels }

$backendModels = $entries | Where-Object { ($_.max_task_types -contains "medium_code") -or ($_.max_task_types -contains "complex") } | ForEach-Object { $_.id }
if (-not $backendModels) { $backendModels = $poolModels }

$docsModels = $entries | Where-Object { $_.max_task_types -contains "docs" } | ForEach-Object { $_.id }
if (-not $docsModels) { $docsModels = $poolModels }

$mechanicalModels = $entries | Where-Object { $_.max_task_types -contains "mechanical" } | ForEach-Object { $_.id }
if (-not $mechanicalModels) { $mechanicalModels = $poolModels }

$routing = @{
    roles = @{
        planner = $plannerModel
        coordinator = $coordinatorModel
        orchestrator = $orchestratorModel
        reviewer = $reviewerModel
        lead = $leadModel
    }
    pools = @{
        default = @{
            description = "Default worker pool for general tasks"
            models = $poolModels
        }
        frontend = @{
            description = "Frontend development tasks"
            models = $frontendModels
        }
        backend = @{
            description = "Backend development tasks"
            models = $backendModels
        }
        docs = @{
            description = "Documentation tasks"
            models = $docsModels
        }
        mechanical = @{
            description = "Mechanical/boilerplate tasks"
            models = $mechanicalModels
        }
    }
    defaultFallback = $defaultFallback
    models = $entries
}

$routing | ConvertTo-Json -Depth 10 | Set-Content $RoutingFile -Encoding UTF8

# Write config.json
$labelsList = $Labels -split "," | ForEach-Object { $_.Trim() }
$config = @{
    runtime = "opencode"
    maxConcurrentAgents = $MaxAgents
    max_agents = $MaxAgents
    plannerModel = $plannerModel
    coordinatorModel = $coordinatorModel
    orchestratorModel = $orchestratorModel
    reviewerModel = $reviewerModel
    leadModel = $leadModel
    models = $modelsList
    labels = $labelsList
    refreshModels = [bool]$RefreshModels
    policyProfile = "default"
    cargoConcurrencyCap = 8
    cargoTargetIsolationThreshold = 8
    cargoTimeoutSeconds = 120
    mergeStrategy = "safe"
    quotaRouting = $true
    workerCwdLock = $true
    truncationSalvage = $true
    workflowRunnerDefault = ""
}

$config | ConvertTo-Json -Depth 10 | Set-Content $ConfigFile -Encoding UTF8

# Write .onboarded timestamp
$timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$timestamp | Set-Content (Join-Path $AutoshipDir ".onboarded") -Encoding UTF8

Write-Host "`nAutoShip OpenCode setup complete" -ForegroundColor Green
Write-Host "Configured models: $selectedModels"
Write-Host "Max agents: $MaxAgents"
Write-Host "Labels: $Labels"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  opencode-autoship doctor    # Diagnose AutoShip installation"
Write-Host "  /autoship-setup             # Re-run setup wizard (OpenCode)"
Write-Host "  /autoship                   # Start orchestration"
