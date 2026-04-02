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
python scripts/sync_wiki.py --check
```

This validates:

- required page presence
- flat layout rules
- required special files such as `Home.md` and `_Sidebar.md`
- banned stale references such as old pre-submodule paths

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
- gets auth from `GH_TOKEN` or falls back to `gh auth token`
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

## Nightly Publish

The repository also has a nightly wiki publish workflow:

- `.github/workflows/wiki-nightly.yml`

Behavior:

- runs on the self-hosted runner labeled `[self-hosted, Windows, X64, dmft]`
- runs on the configured UTC schedule, on manual dispatch, and after a successful `Nightly Release` workflow
- validates with `python scripts/sync_wiki.py --check`
- publishes with `python scripts/sync_wiki.py --push`

Auth model:

- the workflow exports `GH_TOKEN` from GitHub Actions and uses the same `scripts/sync_wiki.py` auth path as local runs
- if `gh auth status` fails or the wiki remote has not been initialized, the job should fail clearly rather than silently skipping work

This nightly job mirrors the repo-side canonical pages. It does not replace the requirement to update `docs/wiki/` in normal PRs.

## Content Rules

- prefer operator guidance first, internals second
- separate current behavior from roadmap or not-yet-revalidated behavior
- use `third_party/eqlib` as the canonical eqlib reference path
- avoid stale references to old pre-submodule layouts

## Current Behavior vs Roadmap

### Current behavior

- wiki maintenance stays repo-first and reviewable in normal PRs
- a nightly auto-publish job now mirrors the reviewed repo state to the GitHub wiki

### Future options

- if the team later wants automatic publication on merge, keep `docs/wiki/` as canonical and add automation around the same script rather than editing the wiki repo by hand
