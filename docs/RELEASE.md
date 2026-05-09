# Release Checklist

AutoShip releases are automated from protected `main` with semantic-release. Release commits must still go through pull requests; the release workflow publishes npm and creates the GitHub Release after a qualifying commit lands on `main`.

## Preflight

Run from a clean checkout:

```bash
git status --short
gh auth status
npm view opencode-autoship version dist-tags.latest
gh release list --limit 5
```

## Change Requirements

Use conventional commits so semantic-release can decide the next version:

- `fix(...)` creates a patch release.
- `feat(...)` creates a minor release.
- Breaking changes create a major release.
- `chore(...)` alone does not create a release.

Do not add `@semantic-release/git` or any step that pushes release-artifact commits directly to `main`. Branch protection requires changes through pull requests.

## Verify Before PR

```bash
npm run typecheck
npm run build
npm run verify:pack
bash hooks/opencode/check.sh
bash -n hooks/opencode/*.sh hooks/*.sh hooks/hermes/*.sh
```

## Merge Flow

1. Push a feature/fix branch.
2. Open a PR against `main`.
3. Wait for the AutoShip checks to pass.
4. Merge through GitHub.
5. Wait for the `Release` workflow on `main`.

## Release Workflow

The workflow in `.github/workflows/release.yml` runs:

```bash
npm ci
npm run build
npm run verify:pack
semantic-release
```

Semantic-release publishes to npm with provenance and creates the GitHub Release through `@semantic-release/github`. The tracked `CHANGELOG.md`, `VERSION`, `package.json`, and `package-lock.json` may be updated manually by follow-up PRs when needed, but the canonical published release notes are the GitHub Release notes.

## Final Checks

Confirm the GitHub Release and npm package agree:

```bash
gh release list --limit 5
npm view opencode-autoship version dist-tags.latest
gh run list --limit 5 --json databaseId,workflowName,displayTitle,status,conclusion,url
```

The release is complete when the latest GitHub Release tag and npm `latest` version match, and the `Release` workflow completed successfully.
