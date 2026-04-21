# Wiki Auto-Sync on Merge

**Workflow**: `.github/workflows/automation.yml` (post-merge job)
**Scope**: Automated docs publishing (issue #1237 Gap #1)

## Purpose

Any merge to `master` that touches `docs/wiki/**` triggers an immediate push to the paired `TextQuest.wiki` repository. Replaces the prior manual cadence where wiki changes waited until the next `nightly-release.yml` run (up to 7 days).

## Trigger

Runs automatically after label cleanup and merged-branch deletion in the post-merge job. No manual action required.

## Mechanics

Three added steps in the post-merge job:

1. **Checkout merged branch** — `actions/checkout@v4`
2. **Setup Python** — 3.12 via `actions/setup-python@v5`
3. **Publish** — `python3 scripts/sync_wiki.py --push` with `GH_TOKEN=secrets.GITHUB_TOKEN`

No new secrets or dependencies. Uses existing `sync_wiki.py` script.

## Operator Workflow

Merge any PR that updates `docs/wiki/*.md`. Within seconds, the change is visible on the GitHub wiki. No follow-up action required.

## Related

- Issue: #1237 (Gap #1 of 3 from 2026-04-03 pipeline audit)
- Follow-up candidates: project board auto-update (Gap #2), scheduled queue reconcile (Gap #3)
