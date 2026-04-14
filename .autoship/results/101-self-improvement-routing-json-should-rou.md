# Result: #101 — rust_unsafe routing override

## Status: DONE

## Changes Made
- hooks/init.sh: Added `rust_unsafe` task type to `DEFAULT_ROUTING`. Added project detection for Rust/Windows (`Cargo.toml` + `#[cfg(windows)]`) to set `project_profile` and `project_language` in `state.json`. When `rust_windows` profile is detected, `DEFAULT_ROUTING` is overridden to prefer Claude agents.
- skills/dispatch/SKILL.md: Documented the routing override rule for Rust unsafe/Windows keywords and project profiles.
- AUTOSHIP.md: Added `rust_unsafe` to the front matter routing section.

## Tests
- Command: bash -n hooks/init.sh
- Result: PASS

## Notes
The routing override in `hooks/init.sh` for `rust_windows` profile proactively swaps Gemini/Codex for Claude in the default routing table if no user-defined routing is found in `AUTOSHIP.md`. The documentation in `SKILL.md` guides the dispatch agent to use the `rust_unsafe` task type when relevant keywords are present.
