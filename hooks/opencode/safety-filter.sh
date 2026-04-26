#!/bin/bash
# Safety filter for AutoShip issue planning
# Performs basic safety checks to prevent auto-approval of unsafe code
# Called by: hooks/opencode/plan-issues.sh

set -euo pipefail

# Default exit code: PASS (0)
exit_code=0

# Check for dangerous patterns in staged/modified files
dangerous_patterns=(
    "rm -rf /"              # Destructive filesystem operations
    "sudo.*rm -rf"          # Privileged destructive ops
    "eval.*\\\$"            # Dynamic code execution
    "exec.*\\\$"            # Process replacement with unvalidated input
)

echo "[safety-filter] Running safety checks..."

# Scan git diff for dangerous patterns
if git diff --cached --quiet 2>/dev/null; then
    # No staged changes
    echo "[safety-filter] No staged changes to check"
else
    for pattern in "${dangerous_patterns[@]}"; do
        if git diff --cached | grep -i "$pattern" 2>/dev/null; then
            echo "[safety-filter] WARNING: Potentially dangerous pattern detected: $pattern"
            exit_code=1
        fi
    done
fi

# Check for credentials/secrets in diffs
secret_patterns=(
    "BEGIN.*PRIVATE KEY"
    "password.*="
    "api[_-]key.*="
    "secret.*="
    "token.*="
)

for pattern in "${secret_patterns[@]}"; do
    if git diff --cached | grep -iE "$pattern" 2>/dev/null; then
        echo "[safety-filter] WARNING: Potential credentials detected: $pattern"
        exit_code=1
    fi
done

if [ $exit_code -eq 0 ]; then
    echo "[safety-filter] PASS: No safety violations detected"
else
    echo "[safety-filter] FAIL: Safety checks failed. Manual review required."
fi

exit $exit_code
