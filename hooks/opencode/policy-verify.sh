#!/usr/bin/env bash
set -euo pipefail

WORKTREE_PATH="${1:?Worktree path required}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
POLICY_JSON=$(bash "$SCRIPT_DIR/policy.sh" json)
runner=$(jq -r '.workflowRunnerDefault // empty' <<< "$POLICY_JSON")

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

if [[ -n "$runner" ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    if grep -Eq 'runs-on:[[:space:]]*ubuntu-latest' "$WORKTREE_PATH/$file"; then
      fail "$file uses ubuntu-latest; policy requires runs-on: $runner"
    fi
  done < <(git -C "$WORKTREE_PATH" ls-files '.github/workflows/*.yml' '.github/workflows/*.yaml' 2>/dev/null || true)
fi

while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  if [[ "$(grep -Ec '^[[:space:]]*pub mod [A-Za-z0-9_]+;' "$WORKTREE_PATH/$file" || true)" -gt "0" ]]; then
    dup=$(grep -E '^[[:space:]]*pub mod [A-Za-z0-9_]+;' "$WORKTREE_PATH/$file" | sort | uniq -d | head -1 || true)
    [[ -z "$dup" ]] || fail "$file contains duplicate module declaration: $dup"
  fi
done < <(git -C "$WORKTREE_PATH" ls-files 'src/lib.rs' '*/src/lib.rs' 2>/dev/null || true)

printf 'PASS\n'
