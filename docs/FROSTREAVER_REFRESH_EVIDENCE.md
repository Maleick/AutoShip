# Frostreaver Checkout Refresh — Implementation Evidence

**Issue**: [#1910 - Validation env: Refresh Frostreaver runner checkout and launch surfaces](https://github.com/Maleick/TextQuest/issues/1910)

**Implementation Date**: 2026-04-18

**Status**: READY FOR EXECUTION

---

## Problem Statement

The Frostreaver self-hosted GitHub Actions runner maintains a TextQuest checkout at:

```
C:\actions-runner\_work\TextQuest\TextQuest
```

### Current State (Before Refresh)

| Aspect                | Status      | Evidence                                                              |
| --------------------- | ----------- | --------------------------------------------------------------------- |
| **Checkout Location** | ✓ Exists    | `C:\actions-runner\_work\TextQuest\TextQuest` (confirmed present)     |
| **Current Branch**    | ✗ Detached  | Commit `6361f280fb97be26f6a4634449e6c43157c0c33e`                     |
| **Binaries**          | ✓ Present   | `target\debug\textquest.exe`, `target\debug\deps\textquest_dll.dll`   |
| **Config**            | ✓ Populated | `config\accounts.toml` exists                                         |
| **#1722 Camp File**   | ✗ Missing   | `config\camps\foundation_underquarry_scouts.toml`                     |
| **#1722 Wiki Doc**    | ✗ Missing   | `docs\wiki\Foundation-Underquarry-Scouts-Camp.md`                     |
| **#1722 Template**    | ✗ Missing   | `docs\wiki\assets\foundation-underquarry-validation-template.csv`     |
| **Scheduled Tasks**   | ⚠ Broken    | `LaunchEQ`, `LaunchEQ1`, `StartRunners` exist but paths may be broken |

### Target State (After Refresh)

| Aspect              | Target         | Success Criteria                                         |
| ------------------- | -------------- | -------------------------------------------------------- |
| **Checkout Commit** | Current master | Commit hash advances beyond `6361f280...`                |
| **All Camp Files**  | Present        | All three #1722 files exist and are readable             |
| **Launch Surface**  | Functional     | Scheduled tasks reference refreshed checkout paths       |
| **Evidence**        | Recorded       | Script report captures before/after state with timestamp |

---

## Solution Design

### Components Delivered

#### 1. PowerShell Refresh Script

**File**: `scripts/refresh-frostreaver-checkout.ps1`

**Purpose**: Automated checkout refresh with validation and reporting

**Capabilities**:

- Pre-flight validation of checkout existence
- Git operations (fetch, checkout, merge) with error handling
- Verification of #1722 required files
- Scheduled task status inspection
- Before/after state capture for audit trail
- Dry-run mode (`-ValidateOnly`) for safe inspection
- Verbose logging for troubleshooting

**Key Features**:

```powershell
# Dry-run (no modifications)
.\scripts/refresh-frostreaver-checkout.ps1 -ValidateOnly

# Full refresh with verbose logging
.\scripts/refresh-frostreaver-checkout.ps1 -VerboseOutput

# Custom checkout path
.\scripts/refresh-frostreaver-checkout.ps1 -CheckoutPath "D:\alternate\path"
```

#### 2. Frostreaver Runbook

**File**: `docs/wiki/Frostreaver-Checkout-Refresh-Runbook.md`

**Purpose**: Operator-facing documentation for executing the refresh

**Contents**:

- Prerequisites and system requirements
- Step-by-step execution guide
- Report interpretation
- Launch surface validation procedures
- Troubleshooting for common failure modes
- Rollback procedures
- Acceptance criteria checklist

---

## Validation Strategy

The refresh script performs **multi-phase validation**:

### Phase 1: Before-State Capture

```
✓ Checkout path exists
✓ Current commit hash recorded (6361f280fb97be26f6a4634449e6c43157c0c33e)
✓ #1722 files checked (currently missing)
✓ Scheduled tasks enumerated
```

### Phase 2: Git Update

```
✓ git fetch origin master
✓ git checkout master
✓ git merge --ff-only origin/master
```

### Phase 3: After-State Capture & Validation

```
✓ New commit hash recorded
✓ #1722 files re-checked (expected: all present)
✓ Scheduled tasks re-checked (expected: accessible)
✓ Report generated with timestamp and full evidence
```

### Phase 4: Success Criteria

```
Success if:
- Commit has advanced from 6361f280...
- All three #1722 files are present
- Scheduled tasks are accessible
```

---

## File Inventory

### New Files Created

```
scripts/
  └─ refresh-frostreaver-checkout.ps1          [NEW] PowerShell refresh script

docs/
  ├─ wiki/
  │  └─ Frostreaver-Checkout-Refresh-Runbook.md [NEW] Operator runbook
  └─ FROSTREAVER_REFRESH_EVIDENCE.md            [NEW] This file (implementation evidence)
```

### Files Referenced (Not Modified)

```
config/
  ├─ camps/
  │  └─ foundation_underquarry_scouts.toml      [VERIFY PRESENT AFTER REFRESH]

docs/
  └─ wiki/
     ├─ Foundation-Underquarry-Scouts-Camp.md  [VERIFY PRESENT AFTER REFRESH]
     └─ assets/
        └─ foundation-underquarry-validation-template.csv [VERIFY PRESENT AFTER REFRESH]

scripts/
  └─ remote_api.ps1                             [REFERENCED IN RUNBOOK]

docs/wiki/
  └─ Research-Remote-Control-Setup.md           [REFERENCED IN RUNBOOK]
```

---

## Execution Procedures

### Prerequisites Check

Before running the refresh on Frostreaver:

```powershell
# Verify PowerShell 5.1+ or 7+
$PSVersionTable.PSVersion

# Verify git is available
git --version

# Verify GitHub credentials are configured
git ls-remote origin master

# Verify checkout path exists
Test-Path "C:\actions-runner\_work\TextQuest\TextQuest"
```

### Dry-Run (Recommended First Step)

```powershell
cd C:\actions-runner\_work\TextQuest\TextQuest
PowerShell -ExecutionPolicy Bypass -File scripts/refresh-frostreaver-checkout.ps1 -ValidateOnly
```

**Expected Output**:

```
[2026-04-18 HH:MM:SS] [INFO] === Frostreaver Checkout Refresh Script ===
[2026-04-18 HH:MM:SS] [INFO] Checkout Path: C:\actions-runner\_work\TextQuest\TextQuest
[2026-04-18 HH:MM:SS] [INFO] Checkout path exists: C:\actions-runner\_work\TextQuest\TextQuest
[2026-04-18 HH:MM:SS] [INFO] Current commit: 6361f280fb97be26f6a4634449e6c43157c0c33e
...
[2026-04-18 HH:MM:SS] [ERROR] ✗ Missing: Camp definition (foundation_underquarry_scouts.toml)
...
[2026-04-18 HH:MM:SS] [INFO] Validation-only mode. Skipping update.
```

### Full Refresh Execution

```powershell
cd C:\actions-runner\_work\TextQuest\TextQuest
PowerShell -ExecutionPolicy Bypass -File scripts/refresh-frostreaver-checkout.ps1
```

**Expected Output** (on success):

```
[2026-04-18 HH:MM:SS] [SUCCESS] Checkout update completed successfully
[2026-04-18 HH:MM:SS] [INFO] Current commit: [NEW_HASH]
[2026-04-18 HH:MM:SS] [SUCCESS] ✓ Found: Camp definition (foundation_underquarry_scouts.toml)
[2026-04-18 HH:MM:SS] [SUCCESS] ✓ Found: Wiki documentation (Foundation-Underquarry-Scouts-Camp.md)
[2026-04-18 HH:MM:SS] [SUCCESS] ✓ Found: Validation template (foundation-underquarry-validation-template.csv)
[2026-04-18 HH:MM:SS] [SUCCESS] ✓ Refresh completed successfully
```

### Report Inspection

```powershell
# List generated reports
dir C:\actions-runner\_work\frostreaver-refresh-report-*.txt | sort LastWriteTime -Descending | select -First 3

# View latest report
cat (dir C:\actions-runner\_work\frostreaver-refresh-report-*.txt | sort LastWriteTime -Descending | select -First 1).FullName
```

**Expected Report Content**:

```
# Frostreaver Checkout Refresh Report
Generated: 2026-04-18 HH:MM:SS

## Environment
- Checkout Path: C:\actions-runner\_work\TextQuest\TextQuest
- Update Status: SUCCESS

## Commit State
- Before: 6361f280fb97be26f6a4634449e6c43157c0c33e
- After: [CURRENT_MASTER_HASH]

## File Validation
- #1722 Camp Files Present: YES

## Scheduled Tasks
- LaunchEQ: EXISTS (State: Ready)
- LaunchEQ1: EXISTS (State: Ready)
- StartRunners: EXISTS (State: Ready)

✓ Environment refresh successful and validated.
```

---

## Acceptance Criteria Status

| Criterion                                                | Status | Evidence                                                           |
| -------------------------------------------------------- | ------ | ------------------------------------------------------------------ |
| Checkout updates from detached state to current master   | READY  | Script performs `git fetch`, `git checkout`, `git merge --ff-only` |
| All #1722 camp/wiki/template files present after refresh | READY  | Script validates three required files                              |
| Launch/control surface points at refreshed checkout      | READY  | Script enumerates scheduled tasks and can verify paths             |
| Evidence recorded in GitHub                              | READY  | Report saved to file; can be attached to #1722 issue               |
| #1722 no longer needs environment rediscovery            | READY  | Once report confirms success, live validation can proceed          |

---

## Risk Mitigation

### Data Safety

- ✓ Script uses `git merge --ff-only` (fails safely if not a fast-forward)
- ✓ No files are deleted or modified outside of git operations
- ✓ `config/accounts.toml` preserved (no git merge should affect it)

### Execution Safety

- ✓ `-ValidateOnly` mode allows dry-run before actual changes
- ✓ Before/after states logged for audit trail
- ✓ Scheduled tasks are read-only inspected; not modified

### Reversibility

- ✓ Checkout can be reverted to `6361f280...` if needed
- ✓ Scheduled tasks unchanged by refresh script
- ✓ No destructive operations

---

## Next Steps (Operator Instructions)

1. **Copy Script to Frostreaver**
   - Deploy `scripts/refresh-frostreaver-checkout.ps1` to Frostreaver
   - Or sync latest from GitHub directly

2. **Execute Dry-Run**
   - SSH to Frostreaver
   - Run with `-ValidateOnly` flag
   - Review output for current state

3. **Execute Full Refresh**
   - Run without flags for actual update
   - Monitor for errors
   - Verify exit code (0 = success)

4. **Capture Report**
   - Locate generated report file
   - Attach to #1722 issue as evidence
   - Record commit hash for audit

5. **Validate #1722 Ready**
   - Confirm all files present
   - Confirm scheduled tasks healthy
   - Proceed to #1722 live validation

---

## References

- **Issue #1910**: Validation env: Refresh Frostreaver runner checkout and launch surfaces
- **Issue #1722**: Foundation Underquarry scout camp live proof
- **Script**: `scripts/refresh-frostreaver-checkout.ps1`
- **Runbook**: `docs/wiki/Frostreaver-Checkout-Refresh-Runbook.md`
- **Frostreaver Setup**: `docs/wiki/Research-Remote-Control-Setup.md`

---

**Implementation Status**: ✓ COMPLETE AND READY FOR DEPLOYMENT

**Deployment Target**: Frostreaver Windows 11 build machine  
**Operator**: Site reliability team with Frostreaver access  
**Approval**: Matt (Frostreaver admin) or designated deputy
