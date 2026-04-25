# Handoff — Oldest Open Issues

Paste into fresh Claude Code session. Targets top 10 oldest open issues on Maleick/TextQuest.

---

## Context

Repo: `Maleick/TextQuest` at `/Users/maleick/Projects/TextQuest`. Default branch: `master`.

Architecture (2026-04-24): `textquest-web/frontend/` = primary config UI; TUI (`textquest` crate) = gameplay; `textquest-web` axum backend on :3001.

Prior session just drained the 2026-04-24 polish queue (13 PRs opened — #2512, #2526, #2527, #2539, #2557 + 5 in-flight worker PRs not yet reviewed). Oldest issues remain untouched since mid-April 2026.

Caveman ultra mode active per user pref. Auto mode on — execute autonomously, minimize interruptions.

## Top 10 oldest open issues

| #   | Created    | Title                                                                  | Size         |
| --- | ---------- | ---------------------------------------------------------------------- | ------------ |
| 713 | 2026-04-10 | [Runtime Proof] Archive frostreaver/Test artifacts or downgrade claims | S (docs)     |
| 717 | 2026-04-10 | Selective debugger and packet migration from legacy test branch        | M            |
| 722 | 2026-04-11 | Map EQ's anti-cheat detection functions in Ghidra                      | L (research) |
| 723 | 2026-04-11 | Stack spoofing implementation (Hypnus-style)                           | L            |
| 724 | 2026-04-11 | VM detection evasion — counter EQ's VM fingerprinting                  | M            |
| 725 | 2026-04-11 | Machine fingerprint analysis — understand what EQ collects and sends   | M (research) |
| 727 | 2026-04-11 | Per-patch detection function diffing via MemoryQuest pipeline          | L            |
| 728 | 2026-04-11 | [EPIC] Detour evasion — eliminate JMP hook signatures                  | EPIC         |
| 730 | 2026-04-11 | INT3/hardware breakpoint hooking to replace JMP hooks                  | L            |
| 731 | 2026-04-11 | VEH-based hooking framework (Vectored Exception Handler)               | L            |

## Suggested execution order

1. **#713** — small docs/archival task. Good warmup, no code risk. `scripts/sync_wiki.py --check` gates it.
2. **#717** — selective migration. Read `git log legacy-test..master -- textquest-dll/src/net/` to scope what's missing. Scope carefully; prior migration already landed mechanical parts.
3. **#722 / #725** — research-mode (label `mode:research`). Delegate to codex-rescue or general-purpose agent with Ghidra API access. Frostreaver machine runs GhidraMCP (see memory).
4. **#728 EPIC** — decompose into sub-issues before touching. Likely children already exist (#730 INT3, #731 VEH). Verify with `gh issue list --search "parent:728"`.
5. **#723/#724/#727/#730/#731** — all anti-cheat stealth work. HIGH risk: touches `textquest-dll` unsafe Rust. Use `unsafe-reviewer` subagent after any change. Serialize these — don't parallelize.

## Commands you'll need

```bash
# Browse oldest unblocked
gh issue list --state open --search "sort:created-asc -label:agent:blocked" --limit 20

# Per-issue full body
gh issue view <N> --json title,body,labels,comments

# Spawn worker (isolated worktree)
# Use Agent tool with subagent_type: "general-purpose" + isolation: "worktree"

# Frostreaver Windows build (cross-compile alt)
ssh frostreaver "cd /c/TextQuest && cargo build -p textquest-dll"
```

## Key conventions (from user memory)

- **Never cache codex quota** — check live each dispatch
- **Conventional commit prefixes** — `fix(scope):`, `feat(scope):` (required for PR Title Validation CI)
- **Worktree commit required** — workers using `isolation: "worktree"` MUST commit before exit or work is deleted
- **Codex for workers, Sonnet for orchestration** — codex-gpt medium/complex, codex-spark simple, claude-sonnet = coordinator only
- **Parallel cap 10** — max concurrent AutoShip workers
- **Issue checks first** — verify `gh issue view N --json state` before dispatching; 2 agents last session wasted runs on already-closed issues
- **Check `.wolf/buglog.json`** before fixing any bug (known fixes)
- **Check `.wolf/anatomy.md`** before reading project files
- **After bug fix** — append to `.wolf/buglog.json` + `.wolf/memory.md`
- **Stealth work (#722-#731)** — check `~/.claude/rules/core-invariants.md` invariant 2 (no credentials in commits) + use `unsafe-reviewer` subagent
- **Don't skip hooks** (`--no-verify`) unless explicitly asked

## In-flight from prior session (don't re-dispatch)

Workers still running when worktree was torn down:

- #2488 opcode lookup — verify `gh pr list --search "is:open #2488"` landed
- #2489 help per-screen — same
- #2509 TUI status pills — same
- #2510 panel title casing — same
- #2544 dashboard → roster rename — same

Check PR status first; if not merged, leave alone.

## Success criteria

- Top 3 oldest (#713, #717, #722) have PRs opened OR are reclassified (agent:blocked → human:required if scope unclear)
- No new issues filed this round unless genuinely discovered
- Each PR body references its issue with `Closes #N`
- All PRs pass CI (fast PR gate minimum; coverage can follow)

## Start here

```
Read HANDOFF_OLDEST_ISSUES.md. Begin with #713 — it's the smallest
and least risky. Verify issue state via gh, then implement and PR.
Report PR URL, then move to #717. Pause before tackling anti-cheat
work (#722+) to confirm scope with user.
```
