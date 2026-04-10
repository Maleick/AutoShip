# Maintaining the Wiki

## Canonical Source

The canonical wiki source lives in:

```text
docs/wiki/
```

Do not treat the GitHub wiki repo as the only source of truth. Reviewable edits belong in the main repo first.

## Standard Update Workflow

### 1. Edit the repo-side page

Update the relevant file under `docs/wiki/`.

Rules:

- keep the directory flat
- keep page names aligned with the published wiki names
- prefer current code and generated docs over older research notes
- call out live-validation gaps explicitly
- keep governance and runbook policy in `docs/wiki/`
- link out to canonical `TextQuest-Ghidra` snapshots or manifests instead of duplicating immutable evidence payloads here

### 2. Validate locally

```bash
python scripts/sync_wiki.py --check
```

This validates:

- required page presence
- flat layout rules
- required special files such as `Home.md` and `_Sidebar.md`
- banned stale references such as deleted local vendor paths

### 3. Preview the publish result

```bash
python scripts/sync_wiki.py --dry-run
```

This materializes a wiki checkout in a temp directory, reports adds, updates, and deletes, and leaves your main repo worktree clean.

### 4. Publish to the GitHub wiki

```bash
python scripts/sync_wiki.py --push
```

What the script does:

- resolves the GitHub repo from `origin`
- gets auth from `GH_TOKEN` or the local `gh auth` session
- checks whether the wiki git remote exists
- clones or initializes a wiki checkout
- syncs all markdown pages from `docs/wiki/`
- commits with `docs: sync wiki from docs/wiki`
- pushes to `<repo>.wiki.git`

## One-Time Bootstrap Case

If the repository has wiki support enabled but the wiki git remote does not exist yet, `--push` will stop with a bootstrap message.

Fix:

1. open the repository Wiki tab in GitHub
2. create the first page in the UI
3. rerun `python scripts/sync_wiki.py --push`

## CI and PR Expectations

- CI runs `python scripts/sync_wiki.py --check` on PRs
- wiki updates should ship in the same PR as the behavior change whenever possible
- README should continue to point contributors at `docs/wiki/` and the sync script commands

## Content Rules

- prefer operator guidance first, internals second
- separate current behavior from roadmap or not-yet-revalidated behavior
- cite checked-in source files or public upstream references
- avoid deleted local vendor paths

## Current Behavior vs Roadmap

### Current behavior

- wiki maintenance stays repo-first and reviewable in normal PRs
- publish still flows through `scripts/sync_wiki.py`

### Future options

- if the team later wants automatic publication on merge, keep `docs/wiki/` as canonical and add automation around the same script rather than editing the wiki repo by hand
