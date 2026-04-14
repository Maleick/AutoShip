# Result: #96 — Dispatch: document and enforce 20-agent dynamic cap

## Status: DONE

## Changes Made

- `skills/dispatch/SKILL.md`: Updated Codex app-server note to "unlimited (when working)". (Most other changes were already present from a previous commit).
- `skills/orchestrate/SKILL.md`: Updated Step 7 to explicitly mention dispatching up to 20 issues concurrently and removing any throttle to 6.
- `AUTOSHIP.md`: Updated `max_concurrent_agents` from 6 to 20 in the configuration front matter.
- `hooks/init.sh`: Updated `DEFAULT_ROUTING` to use 20 as the default `max_concurrent_agents`.
- `README.md`: Updated documentation to reflect "up to 20 issues in flight simultaneously".
- `docs/index.html`: Updated landing page stats and feature descriptions to reflect the 20-agent cap.

## Tests

- Command: N/A (documentation/skill update)
- Result: PASS
- New tests added: no

## Notes

Commit 50945ae had already partially addressed the changes in `skills/dispatch/SKILL.md`, but other files and the orchestration skill still required updates to fully implement and document the 20-agent cap.
