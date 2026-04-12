# Result: #27 — Implement quota parsing for Codex and Gemini CLIs

## Status: DONE

## Changes Made

- `hooks/detect-tools.sh`: Rewrote to output `quota_pct` for every tool. Claude is hardcoded to 100. Codex is split into two separate entries (`codex-spark`, `codex-gpt`) each with `quota_pct: -1` (unknown). Gemini gets `quota_pct: -1` (unknown). Quota helpers (`quota_codex_spark`, `quota_codex_gpt`, `quota_gemini`) are isolated functions — easy to update if the CLIs ever expose a machine-readable quota command.

- `hooks/beacon-init.sh`: Added `SCRIPT_DIR` resolution and calls `detect-tools.sh` at init time. Transforms detect-tools output (uses `available: bool`) to state.json format (`status: "available"|"unavailable"`) via jq. Uses `jq -n` to write the full state.json (replaces heredoc) so tools data is embedded cleanly. On re-run (state.json already exists), refreshes only the `tools` section without touching other state (`issues`, `stats`, etc.) and updates `updated_at`.

## Tests

- Test command: `bash hooks/detect-tools.sh` (smoke test)
- Result: PASS — outputs valid JSON with `quota_pct` for all four tool keys
- Test command: `bash hooks/beacon-init.sh` (init + refresh)
- Result: PASS — creates state.json on first run, refreshes tools section on re-run
- New tests added: no

## Notes

- Neither `codex` (v0.120.0) nor `gemini` (v0.37.1) expose a quota or status command that outputs machine-readable quota data. Both default to `quota_pct: -1` (unknown).
- `codex` has a `cloud` subcommand that could theoretically surface task/quota info, but it requires interactive stdin and has no structured quota output.
- `gemini` has no quota-related subcommands in its help output.
- The quota helper functions (`quota_codex_spark`, etc.) are intentionally separated — when the CLIs add quota commands, only those functions need updating.
- jq is required (available at /opt/homebrew/bin/jq on macOS). If jq is absent, beacon-init.sh falls back to a hardcoded tools JSON with conservative defaults (claude: available/100, others: unavailable/-1).
