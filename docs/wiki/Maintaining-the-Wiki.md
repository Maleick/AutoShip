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

### 2. Validate locally

```bash
python3 scripts/sync_wiki.py --check
```

This validates:

- required page presence
- flat layout rules
- required special files such as `Home.md` and `_Sidebar.md`
- banned stale references such as old pre-submodule paths

### 3. Preview the publish result

```bash
python3 scripts/sync_wiki.py --dry-run
```

This materializes a wiki checkout in a temp directory, reports adds/updates/deletes, and leaves your main repo worktree clean.

### 4. Publish to the GitHub wiki

```bash
python3 scripts/sync_wiki.py --push
```

What the script does:

- resolves the GitHub repo from `origin`
- gets auth from `GH_TOKEN` or `gh auth token`
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
3. rerun `python3 scripts/sync_wiki.py --push`

## CI and PR Expectations

- CI runs `python3 scripts/sync_wiki.py --check` on PRs.
- Wiki updates should ship in the same PR as the behavior change whenever possible.
- README should continue to point contributors at `docs/wiki/` and the sync script commands.

## Content Rules

- Prefer operator guidance first, internals second.
- Separate current behavior from roadmap or not-yet-revalidated behavior.
- Use `third_party/eqlib` as the canonical eqlib reference path.
- Avoid stale references to old pre-submodule layouts.

## Current Behavior vs Roadmap

### Current behavior

- Wiki maintenance is intentionally manual-publish plus automated validation.
- This keeps wiki content reviewable in normal PRs without adding auto-publish risk on every merge.

### Future options

- If the team later wants automatic publication on merge, keep `docs/wiki/` as canonical and add automation around the same script rather than editing the wiki repo by hand.
