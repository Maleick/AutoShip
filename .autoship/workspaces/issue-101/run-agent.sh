#!/usr/bin/env bash
cd "$(dirname "$0")"
cat AUTOSHIP_PROMPT.md | gemini -y -p "See the task description above. Work in the current directory. Implement all acceptance criteria, commit your changes, and write AUTOSHIP_RESULT.md."
for i in $(seq 1 5); do
  [[ -f AUTOSHIP_RESULT.md ]] && break
  sleep 1
done
[[ -f AUTOSHIP_RESULT.md ]] && echo COMPLETE || echo STUCK
