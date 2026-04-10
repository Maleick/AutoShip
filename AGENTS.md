# TextQuest Agent Workflow

## Repository Ownership

- Live is the primary product target going forward.
- Test is historical and reference-only.
- `TextQuest-Ghidra` is canonical for immutable snapshots, manifests, baseline selection, curated Ghidra evidence, and Ghidra intake and analysis flow.
- `TextQuest` is canonical for code, `docs/wiki/`, runbooks, automation, and lightweight references that point at canonical evidence.
- `docs/wiki/` is the canonical documentation surface. The GitHub wiki is a lightweight landing page only.

## Master-Safe Integration

- Base every branch on fresh `origin/master`.
- Keep PRs focused and honest. Prefer multiple small PRs over one large migration branch.
- Never merge active Test defaults, Test offsets, or Test-only workflow guidance into `master`.
- Keep Test references historical-only when they still help explain current behavior or evidence lineage.
- If work is not solved in the current branch, open or update a strict GitHub issue instead of burying the gap in docs, memory, or thread context.

## GitHub Tracking Policy

- Issues are the default unit of work.
- Milestones are release and initiative grouping buckets.
- PRs are the implementation and review unit.
- GitHub Projects are retired and historical-only.
- Sub-issues are only for true epics or release buckets.
- Parent epic issues stay open as coordination shells until all child issues are complete.
- Mark an issue `agent:ready` only when the body is concrete enough to execute and includes an explicit `Verify` section.
- Use `agent:blocked` or `human:required` when required evidence, access, or policy decisions are missing.

## Evidence And Docs

- Link to canonical `TextQuest-Ghidra` snapshot and manifest paths instead of copying immutable evidence into this repo.
- Treat repo-local caches such as `data/ghidra.db` or local export folders as mutable runtime and debug state, not canonical evidence.
- Update the matching `docs/wiki/` page in the same PR whenever behavior or operator workflow changes.
