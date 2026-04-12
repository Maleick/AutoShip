# Result: #15 — CronCreate polling: 10-minute GitHub issue sync

## Status: DONE

## Changes Made

- `skills/beacon-poll/SKILL.md`: New skill implementing the full 10-minute GitHub issue polling protocol. Covers all 7 steps: load known state, fetch live issues, diff for new/closed/changed, handle each category, update state, and log results.
- `skills/beacon/SKILL.md` (Step 6): Updated CronCreate call to delegate to the new `beacon-poll` skill instead of embedding inline logic. Added bullet summary of what the skill covers and a note about re-establishing the cron after context compaction.

## Tests

- Test command: none (skill files are markdown protocols, not executable code)
- Result: N/A — verified by reading protocol against all 6 acceptance criteria

## Acceptance Criteria Verification

- [x] CronCreate fires every 10 minutes — `schedule: "*/10 * * * *"` in Step 6 of beacon/SKILL.md
- [x] Fetch open issues and diff against known state — Steps 2 & 3 of beacon-poll/SKILL.md
- [x] New issues: run UltraPlan to integrate into existing plan — Step 4 classifies complexity and appends to plan phases
- [x] Closed issues: cancel running agents if applicable — Step 5 kills tmux panes and removes worktrees
- [x] Changed issues: update local state — Step 6 handles label reconciliation and metadata sync
- [x] Log poll results — Step 7 writes timestamped summary to `.beacon/poll.log` (bounded to 500 lines)

## Notes

- The CronCreate prompt now references `skills/beacon-poll/SKILL.md` directly so the protocol can evolve independently of the orchestrator
- Step 5 (closed issues) reports cancellations via stdout; Opus reads the poll output and can take additional action if needed
- The `beacon:in-progress` label is removed from cancelled issues to keep GitHub labels as the durable source of truth
- Context compaction note added to Step 6 and already present in the Recovery section (line 398) — both now mention re-establishing the cron after compaction
