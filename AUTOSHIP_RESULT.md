Issue `#1733` is blocked.

Findings:
- The repository does not contain the prerequisite `textquest-admin` CLI target described by the issue.
- The repository does not contain the expected shared admin client/module under `textquest/src/admin/`.
- The repository does not contain admin backup API wiring or shared backup response models to reuse.
- Searches across the worktree, local branches, and remote branches did not find merged scaffolding for issues `#1729` or `#1730`.

Evidence gathered:
- `find textquest/src -maxdepth 2 -type d | rg '/admin$'` returned no matches.
- `cargo metadata --no-deps --format-version 1` listed no `textquest-admin` target.
- `rg -n "/api/admin|textquest-admin|backup/create|backup/restore|backup/list" textquest textquest-web -g '*.rs' -g '*.md'` returned no implementation matches.
- GitHub issues `#1566` and `#1733` reference work that is not present on `origin/master`.

Action taken:
- No out-of-scope scaffolding was added.
- `feature-list.json` was updated to mark this feature as `blocked`.

Recommended next step:
- Land or rebase the prerequisite work for `#1729` and `#1730`, then re-run `#1733` against the branch that contains the admin CLI scaffold and backup client/models.
