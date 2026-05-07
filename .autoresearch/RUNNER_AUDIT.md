# AutoShip Hermes Runner Audit — 2026-05-06

## Root Cause: CRLF Line Endings in Status Files

Windows writes status files with `\r\n` (CRLF) line endings. Every bash script that reads these files must strip `\r` before string comparison, or `grep`, `[[ ]]`, and `case` statements fail silently.

### Failure Pattern
```bash
# File contains: "RUNNING\r\n"
status=$(cat status_file)          # status = "RUNNING\r"
[[ "$status" == "RUNNING" ]]       # FALSE — \r at end
[[ "$status" != "RUNNING" ]]       # TRUE — enters wrong branch
grep -l "^QUEUED$" status_file      # FAILS — pattern doesn't match "QUEUED\r"
```

### Impact
- Runner never finds QUEUED workspaces → dispatches 0 workers
- Stuck-cleanup never finds RUNNING workspaces → thinks all are idle
- Status resets to QUEUED while worker is still running → duplicate dispatches
- Multiple workers on same issue → git conflicts, resource contention

## Fixes Applied (all pushed to AutoShip main)

### 1. runner.sh — Batch Mode Status Matching
```bash
# BEFORE (broken)
queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec grep -l "^QUEUED$" {} \;)

# AFTER (fixed)
queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^QUEUED$"' _ {} \; -print)
```

### 2. runner.sh — Single-Issue Mode Status Read
```bash
# BEFORE (broken)
current_status=$(cat "$status_file" 2>/dev/null || echo "unknown")

# AFTER (fixed)
current_status=$(cat "$status_file" 2>/dev/null | tr -d '\r\n' || echo "unknown")
```

### 3. runner.sh — Result Status Check
```bash
# BEFORE (broken)
result_status=$(cat "$workspace_dir/status" 2>/dev/null || echo "unknown")

# AFTER (fixed)
result_status=$(cat "$workspace_dir/status" 2>/dev/null | tr -d '\r\n' || echo "unknown")
```

### 4. stuck-cleanup.sh — Status Read
```bash
# BEFORE (broken)
status=$(cat "$status_file" 2>/dev/null | tr -d '\n')

# AFTER (fixed)
status=$(cat "$status_file" 2>/dev/null | tr -d '\r\n')
```

### 5. Auto-Detect TextQuest Workspaces
```bash
# When runner.sh runs from AutoShip repo, auto-detect TextQuest workspaces
elif [[ "$REPO_ROOT" == *"/AutoShip" ]] && [[ -d "/mnt/c/Users/xmale/Projects/TextQuest/.autoship/workspaces" ]]; then
  WORKSPACES_DIR="/mnt/c/Users/xmale/Projects/TextQuest/.autoship/workspaces"
```

### 6. Hardcode TextQuest Path in Cleanup Script
Cleanup cron runs without env vars — script now hardcodes TextQuest path.

### 7. Remove Timeout
Workers run until they complete. Orchestrator checks process liveness.

### 8. Cleanup Only Resets RUNNING (not STUCK/BLOCKED)
Prevents cleanup from resetting statuses that were intentionally set.

## Verification Checklist

- [x] runner.sh dispatches QUEUED workspaces
- [x] runner.sh counts RUNNING workspaces correctly
- [x] stuck-cleanup finds RUNNING workspaces with alive processes
- [x] stuck-cleanup does NOT reset STUCK/BLOCKED/COMPLETE
- [x] stuck-cleanup resets only RUNNING with no process (died/crashed)
- [x] No duplicate dispatches for same issue
- [x] Status files use consistent LF (not CRLF) after write

## Prevention: Write LF-Only Status Files

All scripts that WRITE status files should use `printf` (not `echo`) to avoid CRLF:
```bash
printf 'RUNNING\n' > "$status_file"    # LF only, no \r
echo "RUNNING" > "$status_file"        # MAY add \r on Windows
```

Audit all write sites in runner.sh:
- Line 69: `printf 'RUNNING\n' >"$status_file"` ✓ LF only
- Line 101: `printf 'BLOCKED\n' >"$status_file"` ✓ LF only
- Line 179: `printf 'STUCK\n' >"$status_file"` ✓ LF only
- Line 181: `printf 'COMPLETE\n' >"$status_file"` ✓ LF only
- Line 208: `printf 'BLOCKED\n' >"$status_file"` ✓ LF only
- Line 261: `printf 'RUNNING\n' >"$status_file"` ✓ LF only

All write sites use `printf` with explicit `\n` — correct.

## Current State (post-fix)

| Metric | Value |
|--------|-------|
| Workers running | 8 |
| Workers falsely STUCK | 0 (fixed) |
| Orphaned RUNNING | 0 |
| Available slots | 2/10 |
| Issues with real commits ready for PR | 9 |

## Recommendations

1. **Add CI test** for runner.sh that creates CRLF status files and verifies dispatch
2. **Standardize** all AutoShip scripts to use `tr -d '\r\n'` when reading status
3. **Consider** using a state database (JSON) instead of plain text status files
4. **Monitor** stuck-cleanup cron output for next 2 cycles to confirm no false resets
