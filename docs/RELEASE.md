# Package Publish Checklist

Use this checklist from a clean checkout on the release branch. Commands are written so an agent can copy each block, run it, and stop on the first failure.

## Inputs

Set the release version once. `VERSION` includes the `v` prefix; `PACKAGE_VERSION` is plain semver for `package.json`, `package-lock.json`, and npm.

```bash
export VERSION=vX.Y.Z
export PACKAGE_VERSION=${VERSION#v}
```

## Preflight

Verify required tools and repository state before editing release files.

```bash
test -n "$VERSION"
test -n "$PACKAGE_VERSION"
command -v git
command -v gh
command -v node
command -v npm
git status --short
gh auth status
```

The `git status --short` output should be empty, unless the only changes are intentional release edits.

## Update Release Files

Update `VERSION`, `package.json`, and `package-lock.json` together.

```bash
printf '%s\n' "$VERSION" > VERSION
npm version "$PACKAGE_VERSION" --no-git-tag-version
test "$(cat VERSION)" = "$VERSION"
node -e "const pkg=require('./package.json'); if (pkg.version !== process.env.PACKAGE_VERSION) { throw new Error(pkg.version + ' !== ' + process.env.PACKAGE_VERSION); }"
node -e "const lock=require('./package-lock.json'); if (lock.version !== process.env.PACKAGE_VERSION || lock.packages[''].version !== process.env.PACKAGE_VERSION) { throw new Error('package-lock.json version mismatch'); }"
```

Update `CHANGELOG.md` manually so the top entry is `## $VERSION` and summarizes user-visible release changes. Then verify the heading exists.

```bash
grep -n "^## $VERSION$" CHANGELOG.md
```

## Verify Before Packing

Run the repository checks required for AutoShip changes.

```bash
bash hooks/opencode/test-policy.sh
bash -n hooks/opencode/*.sh hooks/*.sh
bash hooks/opencode/smoke-test.sh
npm run typecheck
```

## Npm Pack Dry-Run

Inspect the exact package contents without publishing.

```bash
npm pack --json --dry-run > /tmp/opencode-autoship-pack.json
node - <<'NODE'
const fs = require('node:fs');
const pack = JSON.parse(fs.readFileSync('/tmp/opencode-autoship-pack.json', 'utf8'))[0];
const files = new Set(pack.files.map((file) => file.path));
const required = [
  'dist/index.js',
  'dist/cli.js',
  'INSTALL.md',
  '.opencode/INSTALL.md',
  'hooks/opencode/install.sh',
  'commands/autoship.md',
  'skills/autoship-orchestrate/SKILL.md',
  'AGENTS.md',
  'VERSION',
  'README.md',
  'LICENSE',
];
const missing = required.filter((file) => !files.has(file));
if (missing.length) {
  throw new Error(`Missing package files: ${missing.join(', ')}`);
}
console.log(`Verified ${required.length} required package files in ${pack.filename}`);
NODE
```

## Commit And Tag

Commit the release edits and create the tag only after all verification passes.

```bash
git status --short
git add VERSION package.json package-lock.json CHANGELOG.md
git commit -m "chore: release $VERSION"
git tag -a "$VERSION" -m "$VERSION"
```

## Publish Package

Publish the npm package from the same commit used for the tag. This is the canonical package for the long-term global install path documented as `npm install -g opencode-autoship`.

```bash
npm ci
npm publish --access public
```

If npm prompts with a browser authentication URL, complete the npm CLI auth flow and rerun `npm publish --access public`. If npm publish fails for any other reason, do not move the tag. Fix the release commit, retag only if the failed tag has not been pushed, and rerun verification.

Remove local publish artifacts after publishing if they were created only for the release:

```bash
test -f package.json && test -d hooks/opencode
rm -rf node_modules dist
```

`bunx opencode-autoship install` is the one-time/no-global path. It resolves the same npm package but does not leave the CLI installed on PATH.

GitHub Packages may be used as a secondary registry for provenance, but public docs should keep npm as the primary install path unless the package is renamed/scoped for GitHub Packages.

## GitHub Release

Push the release commit and tag, then create the GitHub release from the changelog entry.

```bash
awk -v version="$VERSION" '
  $0 == "## " version { capture=1; print; next }
  capture && /^## / { exit }
  capture { print }
' CHANGELOG.md > /tmp/opencode-autoship-release-notes.md
test -s /tmp/opencode-autoship-release-notes.md
git push origin HEAD
git push origin "$VERSION"
gh release create "$VERSION" --title "$VERSION" --notes-file /tmp/opencode-autoship-release-notes.md
```

## Install Verification

Verify the published package installs and exposes the expected version in a disposable directory.

```bash
tmpdir=$(mktemp -d)
cd "$tmpdir"
npm init -y
npm install "opencode-autoship@$PACKAGE_VERSION"
node -e "const fs=require('node:fs'); const pkg=JSON.parse(fs.readFileSync('node_modules/opencode-autoship/package.json', 'utf8')); if (pkg.version !== process.env.PACKAGE_VERSION) { throw new Error(pkg.version + ' !== ' + process.env.PACKAGE_VERSION); }"
npx opencode-autoship --help
cd -
rm -rf "$tmpdir"
```

Verify the package installer copies the packaged assets.

```bash
config_dir=$(mktemp -d)
OPENCODE_CONFIG_DIR="$config_dir" npx "opencode-autoship@$PACKAGE_VERSION" install
test "$(cat "$config_dir/.autoship/VERSION")" = "$VERSION"
rm -rf "$config_dir"
```

## Final Checks

Confirm the tag, GitHub release, and npm package agree.

```bash
git rev-parse "$VERSION"
gh release view "$VERSION"
npm view "opencode-autoship@$PACKAGE_VERSION" version
npm view opencode-autoship dist-tags --json
```

The release is complete when all final checks report `$VERSION` or `$PACKAGE_VERSION` as appropriate.
