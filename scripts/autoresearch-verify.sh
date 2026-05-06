#!/usr/bin/env bash
set -euo pipefail

# AutoResearch verifier: count shell scripts with zero ShellCheck issues.
# Scope intentionally includes hooks/, scripts/, and bin/ when present.
if ! command -v shellcheck >/dev/null 2>&1; then
  echo "ERROR: shellcheck is required" >&2
  exit 1
fi

clean=0
total=0
while IFS= read -r -d '' file; do
  total=$((total + 1))
  if shellcheck "$file" >/dev/null 2>&1; then
    clean=$((clean + 1))
  fi
done < <(find hooks scripts bin -type f -name "*.sh" -print0 2>/dev/null | sort -z)

printf 'clean_hooks_count=%s\n' "$clean"
printf 'total_shell_scripts=%s\n' "$total" >&2
