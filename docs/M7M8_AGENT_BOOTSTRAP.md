# M7/M8 Sprint Agent Bootstrap

**Target**: New agents onboarding in M7/M8 feature work. Read this first (5 min), then CLAUDE.md.

## Architecture TL;DR

- **4 crates**: `textquest` (orchestrator/TUI) | `textquest-dll` (Windows DLL hooks) | `textquest-common` (shared types) | `textquest-web` (React dashboard)
- **One source of truth**: Windows live EQ → DLL reads game state via `ReadProcessMemory` + injects via named pipes → orchestrator TUI displays state
- **Dev loop**: Code on macOS (demo mode works), test on Windows (live EQ), push to CI (all platforms)

## Must-Know Patterns (M7/M8 Blockers)

1. **Offset rebasing** (CRITICAL): Every pointer in `offsets.rs` needs `rebase(preferred_base, actual_base)` before use. Skipped rebasing = crash or silent corruption.
2. **Field-by-field reads** (CRITICAL): `SpawnInfo` read field-at-a-time (not whole struct). MQ2 layouts have gaps. Whole-struct reads = offset misalignment.
3. **IPC naming** (CRITICAL): Pipe/shared-mem names from `pipe_name(session_id, client_id)`. Never hardcode `textquest_cmd_` prefixes. Hardcoded = multi-session collision.
4. **Platform gates** (CRITICAL): Windows APIs behind `#[cfg(windows)]` + macOS stubs. Never `#[cfg(target_os)]`. Stubs return dummy; don't debug stub code.
5. **const fn** (CRITICAL): No allocations (`Vec`, `String`, `Box`), no `&mut self`, no non-const calls. Violations = compiler error.

## Before You Code

- [ ] Read `.wolf/buglog.json` (known fixes, repeat patterns)
- [ ] Read `.wolf/cerebrum.md` (do-not-repeat list from prior sessions)
- [ ] Run `python3 scripts/dev-preflight.py` locally (same as CI: fmt → clippy → test → python)
- [ ] Check open PRs for related work (avoid duplicate effort)

## Build & Test

```bash
cargo build                          # Debug (macOS OK)
cargo test -p textquest             # Orchestrator only
cargo test -p textquest-common       # Shared types
cargo test -p textquest-dll          # DLL (Windows or stubs)
python3 scripts/dev-preflight.py     # Full CI sequence
```

## CI Gate

All pushes check: `fmt` → `clippy` → `test` → `python3 tests`. All must pass. Branch protection enforced.

## Model Strategy (Effort Levels)

- **Default (`/effort high`)**: 16k thinking tokens. Covers M7/M8 complexity.
- **Low (`/effort low`)**: 8k tokens. Trivial edits only.
- **Ultra (`/effort ultra`)**: 32k tokens (override). Use only when stuck on novel domain.

## Subagent Routing

- **Haiku** (default): Quick work, TaskList polling, triage
- **Codex** (override per-task): M7/M8 feature dev, multi-file refactors, complex bugs. Use `Agent(subagent_type: "...", model: "codex", ...)`
- **Opus** (strategic only): Architecture reviews, T.O.P. launch decisions

## Hooks & Behavior Prevention

After major sprints, `/hookify` captures behavioral patterns and creates preventive rules:

- Run after 2-3 repeated fixes in same area
- Review patterns, create rules if robust
- Test with `/verify`

## Next Steps

1. Read "Critical for M7/M8" section in CLAUDE.md → `## Gotchas`
2. Read "Patterns & Conventions" section (10 established practices)
3. Read "Configuration Files" (where to find config for camps, classes, accounts)
4. Start with assigned task; reference CLAUDE.md as needed

---

**Questions?** Check `.wolf/cerebrum.md` preferences and learnings from prior sessions, or ask team lead.
