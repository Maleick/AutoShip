# Result: #104 — codex fast-fail + stuck tracking

## Status: DONE

## Changes Made
- hooks/quota-update.sh: added tool_stuck_count and exhausted fields to quota.json, and implemented the 'stuck' subcommand to increment stuck count and mark tool exhausted if >= 3. Added TOOL_DEGRADED event emission.
- hooks/dispatch-codex-appserver.sh: added a fast-fail health check (codex --version) at the top of the script. Added a call to quota-update.sh stuck when a tool is stuck (either health check or end of script).
- skills/orchestrate/SKILL.md: added health check prose to Step 5 (UltraPlan) and updated the Event Reactions table to include calling quota-update.sh stuck on STUCK events.

## Tests
- Command: bash -n hooks/dispatch-codex-appserver.sh && bash -n hooks/quota-update.sh
- Result: PASS

## Notes
<merge dependency: #93 must merge first>
