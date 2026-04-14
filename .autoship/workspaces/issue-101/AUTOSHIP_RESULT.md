# Result: #101 — rust_unsafe routing override

## Status: DONE

## Changes Made
- hooks/init.sh: Added `rust_unsafe` task type to `DEFAULT_ROUTING`, implemented `rust_windows` project profile detection and routing override to prefer Claude.
- skills/dispatch/SKILL.md: Added "Routing Overrides" section documenting `rust_unsafe`, keyword-based promotion (unsafe, winapi, etc.), and `rust_windows` profile behavior.
- AUTOSHIP.md: Added `rust_unsafe` to the front matter routing table for documentation and default usage.

## Tests
- Command: bash -n hooks/init.sh
- Result: PASS

## Notes
The `rust_windows` detection uses `grep -qr` on `src/` to identify OS-specific code. Routing is automatically overridden to Claude for all task types when this profile is active to ensure reliable results on platforms where Gemini/Codex have historically struggled.
