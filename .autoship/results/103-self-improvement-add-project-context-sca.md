# Result: #103 — project-context scaffolding

## Status: DONE

## Changes Made
- `hooks/extract-context.sh`: New script to extract project conventions from CLAUDE.md, AGENTS.md, and config.json.
- `hooks/init.sh`: Call `extract-context.sh` after config initialization.
- `skills/dispatch/SKILL.md`: Added `## Project Context` section to all agent dispatch templates (Gemini, Codex, Claude Haiku, Claude Sonnet).
- `AUTOSHIP.md`: Documented the new Project Context feature.

## Tests
- Command: bash -n hooks/extract-context.sh
- Result: PASS

## Notes
The extraction script uses a simple `awk` logic to find headers containing "Patterns", "Conventions", or "Gotchas" and extracts up to 40 lines after each. It also includes all content from `AGENTS.md` and key-value pairs from `.autoship/config.json`. The final output is capped at 3000 characters.
