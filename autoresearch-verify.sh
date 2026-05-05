#!/usr/bin/env bash
# autoresearch-verify.sh — count shellcheck-clean hooks
cd "$(dirname "$0")"
total=0
clean=0
for sub in opencode hermes; do
  for f in hooks/"$sub"/*.sh; do
    [[ -f "$f" ]] || continue
    total=$((total + 1))
    count=$(shellcheck -f json "$f" 2>/dev/null | jq 'length')
    if [[ "$count" == "0" ]]; then
      clean=$((clean + 1))
    fi
  done
done
echo "METRIC: clean_hooks_count=$clean"
echo "TOTAL: $total"
