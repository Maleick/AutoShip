#!/usr/bin/env pwsh
# AutoShip Loop Runner for Windows
# Keeps processing queued workspaces until all are done

$ErrorActionPreference = "Continue"
$repoRoot = $env:AUTOSHIP_REPO_ROOT
if (-not $repoRoot) {
    $repoRoot = (git rev-parse --show-toplevel 2>$null)
    if (-not $repoRoot) {
        Write-Error "Not in a git repository and AUTOSHIP_REPO_ROOT not set"
        exit 1
    }
}
$autoshipDir = "$repoRoot\.autoship"
$workspacesDir = "$autoshipDir\workspaces"
$hooksDir = "$env:USERPROFILE\.config\opencode\.autoship\hooks\opencode"

function Fix-WorkspaceGitPath($workspace) {
    $gitFile = "$workspace\.git"
    if (Test-Path $gitFile) {
        $content = Get-Content $gitFile -Raw
        if ($content -match '/mnt/c/') {
            $fixed = $content -replace '/mnt/c/', 'C:/' -replace '/Users/\w+/Projects/', 'C:/Users/$env:USERNAME/Projects/'
            Set-Content $gitFile $fixed -NoNewline
        }
    }
}

function Process-Workspace($workspacePath) {
    $issueKey = Split-Path $workspacePath -Leaf
    $statusFile = "$workspacePath\status"
    $promptFile = "$workspacePath\AUTOSHIP_PROMPT.md"
    $modelFile = "$workspacePath\model"
    
    if (-not (Test-Path $statusFile) -or -not (Test-Path $promptFile)) {
        return $false
    }
    
    $status = (Get-Content $statusFile -Raw).Trim()
    if ($status -ne "QUEUED") {
        return $false
    }
    
    Fix-WorkspaceGitPath $workspacePath
    
    $model = if (Test-Path $modelFile) { (Get-Content $modelFile -Raw).Trim() } else { "opencode/nemotron-3-super-free" }
    
    Write-Host "Processing $issueKey with $model" -ForegroundColor Cyan
    Set-Content $statusFile "RUNNING"
    
    # Run opencode in the workspace
    $originalDir = Get-Location
    try {
        Set-Location $workspacePath
        $prompt = Get-Content AUTOSHIP_PROMPT.md -Raw
        opencode run --model $model $prompt 2>&1 | Tee-Object -FilePath AUTOSHIP_RUNNER.log
    }
    catch {
        Write-Host "Error processing ${issueKey}: $_" -ForegroundColor Red
        Set-Content $statusFile "STUCK"
        return $false
    }
    finally {
        Set-Location $originalDir
    }
    
    # Check if agent completed
    $finalStatus = if (Test-Path $statusFile) { (Get-Content $statusFile -Raw).Trim() } else { "STUCK" }
    
    if ($finalStatus -eq "COMPLETE") {
        Write-Host "$issueKey COMPLETE" -ForegroundColor Green
        return $true
    }
    elseif ($finalStatus -eq "BLOCKED") {
        Write-Host "$issueKey BLOCKED" -ForegroundColor Yellow
        return $true
    }
    else {
        # Check if there are uncommitted changes to salvage
        Fix-WorkspaceGitPath $workspacePath
        Set-Location $workspacePath
        $gitStatus = git status --short 2>$null
        if ($gitStatus) {
            git add -A 2>$null
            git reset -- model started_at status worker.pid AUTOSHIP_RUNNER.log .autoship-event-* 2>$null
            git commit -m "wip: $issueKey (agent timeout)" 2>$null
            Write-Host "$issueKey changes committed" -ForegroundColor Yellow
        }
        Set-Location $originalDir
        Set-Content $statusFile "STUCK"
        return $false
    }
}

function Dispatch-NewIssues() {
    # Get eligible issues from GitHub
    $eligible = wsl bash "/mnt/c/Users/xmale/.config/opencode/.autoship/hooks/opencode/plan-issues.sh" --limit 20 2>$null
    if ($LASTEXITCODE -eq 0) {
        # Parse and dispatch
        # This is simplified - actual implementation would parse JSON
        Write-Host "Checked for new issues" -ForegroundColor Gray
    }
}

# Main loop
$iteration = 0
while ($true) {
    $iteration++
    Write-Host "`n=== AutoShip Loop Iteration $iteration ===" -ForegroundColor Magenta
    
    # Fix .git paths in all workspaces
    Get-ChildItem $workspacesDir -Directory | ForEach-Object {
        Fix-WorkspaceGitPath $_.FullName
    }
    
    # Process queued workspaces (up to max concurrent)
    $running = 0
    $maxConcurrent = 5
    Get-ChildItem $workspacesDir -Directory | ForEach-Object {
        if ($running -ge $maxConcurrent) { return }
        $workspace = $_.FullName
        $statusFile = "$workspace\status"
        if (Test-Path $statusFile) {
            $status = (Get-Content $statusFile -Raw).Trim()
            if ($status -eq "QUEUED") {
                if (Process-Workspace $workspace) {
                    $running++
                }
            }
        }
    }
    
    # Check if any queued remain
    $queued = @(Get-ChildItem $workspacesDir -Directory | Where-Object {
        $statusFile = "$($_.FullName)\status"
        Test-Path $statusFile -and ((Get-Content $statusFile -Raw).Trim() -eq "QUEUED")
    })
    
    $stuck = @(Get-ChildItem $workspacesDir -Directory | Where-Object {
        $statusFile = "$($_.FullName)\status"
        Test-Path $statusFile -and ((Get-Content $statusFile -Raw).Trim() -eq "STUCK")
    })
    
    Write-Host "Queued: $($queued.Count), Stuck: $($stuck.Count)" -ForegroundColor Gray
    
    # Retry stuck issues
    if ($stuck.Count -gt 0 -and $running -lt $maxConcurrent) {
        $stuck | ForEach-Object {
            if ($running -ge $maxConcurrent) { return }
            $workspace = $_.FullName
            Set-Content "$workspace\status" "QUEUED"
            Write-Host "Retrying $($_.Name)" -ForegroundColor Yellow
            if (Process-Workspace $workspace) {
                $running++
            }
        }
    }
    
    if ($queued.Count -eq 0 -and $stuck.Count -eq 0) {
        Write-Host "All workspaces processed. Checking for new issues..." -ForegroundColor Green
        Dispatch-NewIssues
        
        # Double check
        $queued = @(Get-ChildItem $workspacesDir -Directory | Where-Object {
            $statusFile = "$($_.FullName)\status"
            Test-Path $statusFile -and ((Get-Content $statusFile -Raw).Trim() -eq "QUEUED")
        })
        
        if ($queued.Count -eq 0) {
            Write-Host "AutoShip complete!" -ForegroundColor Green
            break
        }
    }
    
    Write-Host "Waiting 30s before next iteration..." -ForegroundColor Gray
    Start-Sleep -Seconds 30
}
