#!/usr/bin/env bash
# gen-handoff.sh — Auto-generates HANDOFF.md from actual repo state.
# Run from the repo root: ./scripts/gen-handoff.sh > HANDOFF.md
#
# Requires: cargo, git, wc, grep (standard Unix + Rust toolchain)

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

TIMESTAMP=$(date -u '+%Y-%m-%d %H:%M UTC')

cat <<EOF
# Session Handoff — $TIMESTAMP

> **Auto-generated** by \`scripts/gen-handoff.sh\`. Do not edit manually.

## Start Here

Read this file + check memories (\`MEMORY.md\`) for full project context.

## Repository Stats

EOF

# --- Git stats ---
BRANCH=$(git branch --show-current)
TOTAL_COMMITS=$(git rev-list --count HEAD)

echo "- **Branch:** \`$BRANCH\`"
echo "- **Total commits:** $TOTAL_COMMITS"

# Line counts (exclude target/ and mq2-reference/)
LINES=$(find . -name '*.rs' -not -path './target/*' -not -path './mq2-reference/*' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')
echo "- **Rust lines:** ~${LINES}"

echo ""

# --- Crate structure ---
echo "## Workspace Crates"
echo ""
echo '```'
# Parse members from single-line or multi-line format
grep 'members' Cargo.toml | grep -oE '"[^"]+"' | tr -d '"' | while read -r crate; do
    if [ -f "$crate/Cargo.toml" ]; then
        ver=$(grep '^version' "$crate/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
        echo "$crate  (v$ver)"
    else
        echo "$crate"
    fi
done
echo '```'
echo ""

# --- Recent commits ---
echo "## Recent Commits (last 20)"
echo ""
echo '```'
git log --oneline -20
echo '```'
echo ""

# --- Test results ---
echo "## Test Results"
echo ""

# Run cargo test and capture output
export CMAKE_POLICY_VERSION_MINIMUM=3.5
TEST_OUTPUT=$(cargo test 2>&1 || true)

echo '```'
# Extract the "test result:" lines per crate
echo "$TEST_OUTPUT" | grep -E '(^running |^test result:)' | while read -r line; do
    echo "$line"
done
echo '```'
echo ""

# Summarize pass/fail per crate
echo "### Per-crate summary"
echo ""
echo "| Crate | Passed | Failed |"
echo "|-------|--------|--------|"
echo "$TEST_OUTPUT" | grep '^test result:' | while read -r line; do
    passed=$(echo "$line" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+')
    failed=$(echo "$line" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")
    # Best-effort crate name from context — cargo test prints "Running unittests" lines
    echo "| — | ${passed:-0} | ${failed:-0} |"
done
echo ""

# --- Key offsets ---
echo "## Key Offsets (from dmft-common/src/offsets.rs)"
echo ""
echo "| Constant | Value |"
echo "|----------|-------|"
grep -E '^pub const' dmft-common/src/offsets.rs | sed 's/pub const \([A-Z_]*\):[^=]*= \(0x[0-9A-Fa-f]*\);/| \1 | `\2` |/' | head -30
echo ""

# --- Build requirements ---
cat <<'BUILDEOF'
## Build Requirements

```bash
# macOS/Linux (development — demo mode)
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build
cargo run        # TUI with demo data
cargo test

# Windows (production — live EQ)
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build --release
```

### Dependencies

- Rust (edition 2024, stable MSVC toolchain on Windows)
- CMake 3.5+ (for navmesh C++ FFI shim)
- LLVM/Clang (Windows, for bindgen)

BUILDEOF

# --- Key references ---
cat <<'REFEOF'
## Key References

- MQ2 Login: https://github.com/macroquest/macroquest/tree/master/src/login
- MQ2 Routing: https://github.com/macroquest/macroquest/tree/master/src/routing
- MQ2Nav: https://github.com/brainiac/MQ2Nav
- eqlib: https://github.com/macroquest/eqlib
- mqmesh.com — navmesh downloads + updater.json manifest
REFEOF
