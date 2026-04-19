# Frostreaver Checkout Refresh Runbook

**Issue**: [#1910 - Validation env: Refresh Frostreaver runner checkout and launch surfaces](https://github.com/Maleick/TextQuest/issues/1910)

**Date**: 2026-04-18

**Purpose**: Refresh the Frostreaver TextQuest runner checkout from a stale detached state to a current master branch revision that includes the #1722 camp/wiki/template files.

---

## Overview

The Frostreaver Windows build machine maintains a self-hosted GitHub Actions runner with a TextQuest checkout at:

```
C:\actions-runner\_work\TextQuest\TextQuest
```

As of the #1722 retry audit, this checkout is:

- **Detached** on commit `6361f280fb97be26f6a4634449e6c43157c0c33e`
- **Missing** the #1722 Foundation Underquarry camp definition and validation files
- **Broken** launch/control surface paths preventing live validation setup

This runbook provides the steps to refresh the environment and validate readiness.

---

## Prerequisites

- SSH access to Frostreaver (Windows 11, AD domain member)
- PowerShell execution policy allows running local scripts
- Git for Windows installed and available on PATH
- GitHub credentials configured (SSH key or PAT) for pulling current master

---

## Step 1: SSH into Frostreaver

From macOS or Linux:

```bash
ssh maleick@frostreaver.local
```

Or use the configured SSH key from `.claude.local.md` if available.

---

## Step 2: Run the Refresh Script

Navigate to the TextQuest repository root and execute the refresh script:

```powershell
cd C:\actions-runner\_work\TextQuest\TextQuest

# Dry-run: Validate current state without modifying
PowerShell -ExecutionPolicy Bypass -File scripts/refresh-frostreaver-checkout.ps1 -ValidateOnly

# Full refresh: Update checkout and validate
PowerShell -ExecutionPolicy Bypass -File scripts/refresh-frostreaver-checkout.ps1

# Verbose output for debugging
PowerShell -ExecutionPolicy Bypass -File scripts/refresh-frostreaver-checkout.ps1 -VerboseOutput
```

### Script Behavior

The PowerShell script (`scripts/refresh-frostreaver-checkout.ps1`):

1. **Validates checkout existence** — Confirms `C:\actions-runner\_work\TextQuest\TextQuest` is present
2. **Records before state** — Captures current commit hash and file presence
3. **Updates checkout** (unless `-ValidateOnly`):
   - `git fetch origin master`
   - `git checkout master`
   - `git merge --ff-only origin/master`
4. **Records after state** — Captures new commit hash
5. **Validates #1722 files**:
   - `config\camps\foundation_underquarry_scouts.toml`
   - `docs\wiki\Camp-Runbooks.md`
   - `docs\wiki\assets\foundation-underquarry-validation-template.csv`
6. **Checks scheduled tasks**:
   - Validates `LaunchEQ`, `LaunchEQ1`, `StartRunners` tasks exist and are accessible
7. **Generates report** — Saves evidence file for audit trail

---

## Step 3: Review the Report

The script generates a report file:

```
C:\actions-runner\_work\frostreaver-refresh-report-YYYYMMDD-HHMMSS.txt
```

**Key sections to verify**:

- ✓ Update Status: SUCCESS
- ✓ Commit changed from `6361f280...` to current master
- ✓ All three #1722 files present
- ✓ Scheduled tasks exist and are accessible

Example successful report:

```
[2026-04-18 15:30:45] [SUCCESS] Checkout update completed successfully
...
## File Validation
- #1722 Camp Files Present: YES

## Scheduled Tasks
- LaunchEQ: EXISTS (State: Ready)
- LaunchEQ1: EXISTS (State: Ready)
- StartRunners: EXISTS (State: Ready)

✓ Environment refresh successful and validated.
```

---

## Step 4: Validate Launch Surface

After the script succeeds, test the launch surface manually:

### Check File System Paths

```powershell
# Verify binaries exist
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\target\debug\textquest.exe"
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\target\debug\deps\textquest_dll.dll"

# Verify config
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\config\accounts.toml"

# Verify #1722 files
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\config\camps\foundation_underquarry_scouts.toml"
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\docs\wiki\Camp-Runbooks.md"
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest\docs\wiki\assets\foundation-underquarry-validation-template.csv"
```

All should return `True`.

### Check Scheduled Task Configuration

```powershell
# View LaunchEQ task details
Get-ScheduledTask -TaskName "LaunchEQ" | Get-ScheduledTaskAction

# View startup action
Get-ScheduledTask -TaskName "LaunchEQ" | Get-ScheduledTaskAction | Select -ExpandProperty Execute
```

The task action should reference the correct checkout path without breaking.

---

## Step 5: Record Evidence and Update Issue

Once validation passes, document the refresh in the issue tracker:

### Evidence to Capture

1. **Commit Hash** — From the script report

   ```
   git -C "C:\actions-runner\_work\TextQuest\TextQuest" rev-parse HEAD
   ```

2. **File Presence Proof** — List files to confirm

   ```powershell
   dir "C:\actions-runner\_work\TextQuest\TextQuest\config\camps\foundation_underquarry_scouts.toml"
   dir "C:\actions-runner\_work\TextQuest\TextQuest\docs\wiki\Camp-Runbooks.md"
   dir "C:\actions-runner\_work\TextQuest\TextQuest\docs\wiki\assets\foundation-underquarry-validation-template.csv"
   ```

3. **Script Report** — Copy-paste from the generated report file

4. **Task Status** — Output of scheduled task verification

### Update #1722 Issue

Reply to the #1722 issue or create a sub-issue with:

```markdown
## Frostreaver Environment Refresh Complete (#1910)

Checkout refreshed and validated.

**Before State**

- Commit: 6361f280fb97be26f6a4634449e6c43157c0c33e (detached)
- #1722 files: MISSING

**After State**

- Commit: [NEW COMMIT HASH]
- #1722 files: PRESENT
- Task status: ALL READY

[Attach script report as comment]

#1722 live validation can now proceed with current repository content.
```

---

## Troubleshooting

### Git Checkout Fails with "Already on master"

The checkout may already be on master but detached. Try:

```powershell
cd C:\actions-runner\_work\TextQuest\TextQuest
git status  # Verify detached HEAD
git fetch origin master
git checkout -b temp-master origin/master
git branch -m temp-master master
git branch -u origin/master master
```

### "Permission Denied" on Scheduled Tasks

The runner service account may lack read permission. Try:

```powershell
# Check runner service account
Get-Service Actions.Runner* | Select-Object -ExpandProperty ServiceName

# Verify scheduled task ownership
Get-ScheduledTask -TaskName "LaunchEQ" | Select -ExpandProperty Principal
```

Contact Frostreaver admin for task permission repair.

### Camp Files Still Missing After Update

The git checkout succeeded but files are not present. This indicates:

1. The current master doesn't yet include the files (upstream issue)
2. Git filter or sparse-checkout is blocking the files

Verify upstream:

```bash
git ls-remote origin master | head -1
git show origin/master:config/camps/foundation_underquarry_scouts.toml
```

If the files don't exist on master, the issue must be escalated to #1722.

### Report File Not Generated

The script may have exited early. Check:

```powershell
$ErrorActionPreference = "Stop"
& ".\scripts\refresh-frostreaver-checkout.ps1" -VerboseOutput 2>&1
```

---

## Rollback (if needed)

If the refresh causes issues, revert to the previous state:

```powershell
cd C:\actions-runner\_work\TextQuest\TextQuest

# Return to detached state
git checkout 6361f280fb97be26f6a4634449e6c43157c0c33e

# Or reattach to stable tag if known
git checkout v1.2.3  # (adjust version as needed)
```

---

## Related Issues & References

- **#1910** — This issue (environment refresh)
- **#1722** — Underquarry camp live validation (blocked by this work)
- **docs/wiki/Research-Remote-Control-Setup.md** — Runner architecture overview
- **scripts/remote_api.ps1** — Remote API launch helper

---

## Acceptance Criteria Checklist

- [ ] Script executes without errors on Frostreaver
- [ ] Commit advances from `6361f280...` to current master
- [ ] All three #1722 files present in refreshed checkout
- [ ] Scheduled tasks verified as accessible
- [ ] Report file generated with evidence
- [ ] Evidence documented in #1722 issue or linked workpad
- [ ] #1722 live validation can proceed without environment rediscovery

---

**Last Updated**: 2026-04-18  
**Status**: READY FOR DEPLOYMENT
