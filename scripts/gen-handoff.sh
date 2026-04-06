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

MacroQuest reference code now lives in local git submodules at \`third_party/eqlib\`
and \`third_party/macroquest\`. Routine \`cargo build\` / \`cargo test\` work does not
require them, but offset or struct work does. After checkout, run
\`git submodule update --init --recursive\` before working against those trees.

## Repository Stats

EOF

# --- Git stats ---
BRANCH=$(git branch --show-current)
TOTAL_COMMITS=$(git rev-list --count HEAD)

echo "- **Branch:** \`$BRANCH\`"
echo "- **Total commits:** $TOTAL_COMMITS"

# Line counts from tracked workspace Rust sources (excludes submodules automatically)
RUST_FILES=$(git ls-files -- '*.rs')
if [ -n "$RUST_FILES" ]; then
    LINES=$(printf '%s\n' "$RUST_FILES" | xargs cat | wc -l | awk '{print $1}')
else
    LINES=0
fi
echo "- **Rust lines:** ~${LINES}"

echo ""

# --- Reference trees ---
echo "## Reference Trees"
echo ""
git submodule status --recursive | while IFS= read -r line; do
    status_char=${line:0:1}
    rest=${line:1}
    sha=${rest%% *}
    rest=${rest#"$sha "}
    path=${rest%% *}

    case "$status_char" in
        ' ')
            state="ready"
            ;;
        '-')
            state="not initialized"
            ;;
        '+')
            state="checked out at a different commit"
            ;;
        'U')
            state="merge conflict"
            ;;
        *)
            state="unknown"
            ;;
    esac

    echo "- \`$path\` — $state (\`$sha\`)"
done
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
TEST_OUTPUT=$(CARGO_TERM_COLOR=never cargo test 2>&1 || true)
TEST_SUMMARY=$(printf '%s\n' "$TEST_OUTPUT" | grep -E '(^running |^test result:)' || true)
TEST_RESULTS=$(printf '%s\n' "$TEST_OUTPUT" | grep '^test result:' || true)

echo '```'
if [ -n "$TEST_SUMMARY" ]; then
    printf '%s\n' "$TEST_SUMMARY"
else
    echo "(no test summary lines captured)"
fi
echo '```'
echo ""

# Summarize pass/fail per crate
echo "### Per-crate summary"
echo ""
echo "| Crate | Passed | Failed |"
echo "|-------|--------|--------|"
if [ -n "$TEST_RESULTS" ]; then
    while IFS= read -r line; do
        passed=$(printf '%s\n' "$line" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+')
        failed=$(printf '%s\n' "$line" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")
        # Best-effort crate name from context — cargo test prints "Running unittests" lines
        echo "| — | ${passed:-0} | ${failed:-0} |"
    done <<< "$TEST_RESULTS"
else
    echo "| — | 0 | 0 |"
fi
echo ""

# --- Key offsets ---
echo "## Key Offsets (from textquest-common/src/offsets.rs)"
echo ""
echo "| Constant | Value |"
echo "|----------|-------|"
grep -E '^pub const' textquest-common/src/offsets.rs | sed 's/pub const \([A-Z0-9_]*\):[^=]*= \(0x[0-9A-Fa-f_]*\);/| \1 | `\2` |/' | head -30
echo ""

# --- Build requirements ---
cat <<'BUILDEOF'
## Build Requirements

```bash
# Optional reference trees (only for offset/struct work)
git submodule update --init --recursive

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

- Rust (edition 2024, nightly MSVC toolchain on Windows for live/release validation)
- CMake 3.5+ (for navmesh C++ FFI shim)
- LLVM/Clang (Windows, for bindgen)

BUILDEOF

# --- Key references ---
cat <<'REFEOF'
## Key References

- Local eqlib reference: `third_party/eqlib`
- Local MacroQuest reference: `third_party/macroquest`
- MacroQuest login code: `third_party/macroquest/src/login`
- MacroQuest routing code: `third_party/macroquest/src/routing`
- MQ2Nav: https://github.com/brainiac/MQ2Nav
- mqmesh.com — navmesh downloads + updater.json manifest
REFEOF
