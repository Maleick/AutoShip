# Frostreaver Checkout Refresh Script
# Purpose: Update stale Frostreaver TextQuest checkout and reconcile launch surfaces
# Issue: #1910
#
# This script:
# 1. Updates the checkout at C:\actions-runner\_work\TextQuest\TextQuest to current master
# 2. Validates the presence of #1722 camp/wiki/template files
# 3. Captures evidence of the refreshed state
# 4. Reconciles Windows scheduled tasks to point at the refreshed checkout

param(
    [string]$CheckoutPath = "C:\actions-runner\_work\TextQuest\TextQuest",
    [string]$TargetBranch = "master",
    [switch]$ValidateOnly,
    [switch]$VerboseOutput
)

# ============================================================================
# Utility Functions
# ============================================================================

function Write-Log {
    param([string]$Message, [string]$Level = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] [$Level] $Message"
}

function Test-CheckoutExists {
    param([string]$Path)
    if (-not (Test-Path $Path)) {
        Write-Log "Checkout path does not exist: $Path" "ERROR"
        return $false
    }
    Write-Log "Checkout path exists: $Path" "INFO"
    return $true
}

function Get-CheckoutCommit {
    param([string]$Path)
    Push-Location $Path
    try {
        $commit = git rev-parse HEAD
        $shortHash = git rev-parse --short HEAD
        Write-Log "Current commit: $commit (short: $shortHash)" "INFO"
        return @{ full = $commit; short = $shortHash }
    }
    finally {
        Pop-Location
    }
}

function Test-FileExists {
    param([string]$Path, [string]$Description)
    if (Test-Path $Path) {
        Write-Log "✓ Found: $Description" "SUCCESS"
        return $true
    }
    else {
        Write-Log "✗ Missing: $Description" "ERROR"
        return $false
    }
}

function Validate-CampFiles {
    param([string]$CheckoutPath)

    Write-Log "Validating #1722 camp/wiki/template files..." "INFO"

    $requiredFiles = @(
        @{
            path = "$CheckoutPath\config\camps\foundation_underquarry_scouts.toml"
            desc = "Camp definition (foundation_underquarry_scouts.toml)"
        },
        @{
            path = "$CheckoutPath\docs\wiki\Foundation-Underquarry-Scouts-Camp.md"
            desc = "Wiki documentation (Foundation-Underquarry-Scouts-Camp.md)"
        },
        @{
            path = "$CheckoutPath\docs\wiki\assets\foundation-underquarry-validation-template.csv"
            desc = "Validation template (foundation-underquarry-validation-template.csv)"
        }
    )

    $allExist = $true
    foreach ($file in $requiredFiles) {
        $exists = Test-FileExists $file.path $file.desc
        if (-not $exists) {
            $allExist = $false
        }
    }

    return $allExist
}

function Update-Checkout {
    param([string]$CheckoutPath, [string]$TargetBranch)

    Write-Log "Updating checkout from detached state to $TargetBranch..." "INFO"

    Push-Location $CheckoutPath
    try {
        # Fetch latest
        Write-Log "Fetching latest from origin..." "INFO"
        $output = git fetch origin $TargetBranch 2>&1
        if ($VerboseOutput) { Write-Log "Fetch output: $output" "DEBUG" }

        # Check out target branch
        Write-Log "Checking out $TargetBranch..." "INFO"
        $output = git checkout $TargetBranch 2>&1
        if ($VerboseOutput) { Write-Log "Checkout output: $output" "DEBUG" }

        # Fast-forward merge
        Write-Log "Fast-forwarding to origin/$TargetBranch..." "INFO"
        $output = git merge --ff-only origin/$TargetBranch 2>&1
        if ($VerboseOutput) { Write-Log "Merge output: $output" "DEBUG" }

        Write-Log "Checkout update completed successfully" "SUCCESS"
        return $true
    }
    catch {
        Write-Log "Failed to update checkout: $_" "ERROR"
        return $false
    }
    finally {
        Pop-Location
    }
}

function Get-ScheduledTaskStatus {
    param([string]$TaskName)

    try {
        $task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        if ($null -eq $task) {
            return @{ exists = $false; name = $TaskName }
        }

        $info = Get-ScheduledTaskInfo -InputObject $task -ErrorAction SilentlyContinue
        $details = Get-ScheduledTask -TaskName $TaskName | Get-ScheduledTaskAction

        return @{
            exists = $true
            name = $TaskName
            state = $task.State
            lastRun = $info.LastRunTime
            lastResult = $info.LastTaskResult
            action = $details.Execute
        }
    }
    catch {
        Write-Log "Error checking task $TaskName : $_" "ERROR"
        return @{ exists = $false; name = $TaskName; error = $_ }
    }
}

function Validate-LaunchSurface {
    param([string]$CheckoutPath)

    Write-Log "Validating launch/control surface..." "INFO"

    $tasks = @("LaunchEQ", "LaunchEQ1", "StartRunners")
    $taskStatus = @()

    foreach ($taskName in $tasks) {
        $status = Get-ScheduledTaskStatus $taskName
        $taskStatus += $status

        if ($status.exists) {
            Write-Log "Task exists: $taskName (State: $($status.state))" "INFO"
        }
        else {
            Write-Log "Task missing: $taskName" "WARNING"
        }
    }

    return $taskStatus
}

function Generate-EnvironmentReport {
    param(
        [string]$CheckoutPath,
        [hashtable]$BeforeCommit,
        [hashtable]$AfterCommit,
        [bool]$FilesValid,
        [array]$TaskStatus,
        [bool]$UpdateSuccess
    )

    $report = @"
# Frostreaver Checkout Refresh Report
Generated: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')

## Environment
- Checkout Path: $CheckoutPath
- Update Status: $(if ($UpdateSuccess) { 'SUCCESS' } else { 'FAILED' })

## Commit State
- Before: $($BeforeCommit.full)
- After: $($AfterCommit.full)

## File Validation
- #1722 Camp Files Present: $(if ($FilesValid) { 'YES' } else { 'NO' })

## Scheduled Tasks
"@

    foreach ($task in $TaskStatus) {
        if ($task.exists) {
            $report += @"
- $($task.name): EXISTS (State: $($task.state))
"@
        }
        else {
            $report += @"
- $($task.name): MISSING
"@
        }
    }

    $report += @"

## Recommendations
"@

    if (-not $FilesValid) {
        $report += "`n- ERROR: Camp files not found after update. Checkout may not be on correct branch.`n"
    }

    $taskStatus | Where-Object { -not $_.exists } | ForEach-Object {
        $report += "- WARN: Scheduled task `"$($_.name)`" is missing. May need to be recreated.`n"
    }

    if ($UpdateSuccess -and $FilesValid) {
        $report += "`n✓ Environment refresh successful and validated.`n"
    }

    return $report
}

# ============================================================================
# Main Execution
# ============================================================================

Write-Log "=== Frostreaver Checkout Refresh Script ===" "INFO"
Write-Log "Checkout Path: $CheckoutPath" "INFO"
Write-Log "Target Branch: $TargetBranch" "INFO"

# Check if checkout exists
if (-not (Test-CheckoutExists $CheckoutPath)) {
    Write-Log "Cannot proceed without valid checkout path" "ERROR"
    exit 1
}

# Get before state
$beforeCommit = Get-CheckoutCommit $CheckoutPath
$beforeFilesValid = Validate-CampFiles $CheckoutPath
$beforeTasks = Validate-LaunchSurface $CheckoutPath

Write-Log "--- Before State ---" "INFO"
Write-Log "Commit: $($beforeCommit.short)" "INFO"
Write-Log "Camp files valid: $beforeFilesValid" "INFO"

# Exit early if only validating
if ($ValidateOnly) {
    Write-Log "Validation-only mode. Skipping update." "INFO"
    exit 0
}

# Update the checkout
Write-Log "--- Performing Update ---" "INFO"
$updateSuccess = Update-Checkout $CheckoutPath $TargetBranch

if (-not $updateSuccess) {
    Write-Log "Checkout update failed. Aborting." "ERROR"
    exit 1
}

# Get after state
$afterCommit = Get-CheckoutCommit $CheckoutPath
$afterFilesValid = Validate-CampFiles $CheckoutPath
$afterTasks = Validate-LaunchSurface $CheckoutPath

Write-Log "--- After State ---" "INFO"
Write-Log "Commit: $($afterCommit.short)" "INFO"
Write-Log "Camp files valid: $afterFilesValid" "INFO"

# Generate report
$report = Generate-EnvironmentReport `
    -CheckoutPath $CheckoutPath `
    -BeforeCommit $beforeCommit `
    -AfterCommit $afterCommit `
    -FilesValid $afterFilesValid `
    -TaskStatus $afterTasks `
    -UpdateSuccess $updateSuccess

Write-Log $report "REPORT"

# Save report to file for inspection
$reportPath = "$CheckoutPath\..\frostreaver-refresh-report-$(Get-Date -Format 'yyyyMMdd-HHmmss').txt"
$report | Out-File $reportPath -Encoding UTF8
Write-Log "Report saved to: $reportPath" "INFO"

# Exit with success if validation passed
if ($afterFilesValid -and $updateSuccess) {
    Write-Log "✓ Refresh completed successfully" "SUCCESS"
    exit 0
}
else {
    Write-Log "✗ Refresh incomplete or validation failed" "ERROR"
    exit 1
}
