# Release Process & Versioning Strategy

This document outlines the release management workflow, versioning strategy, and procedures for TextQuest.

## Semantic Versioning

TextQuest follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH
```

- **MAJOR** (e.g., 1.0.0): Incompatible API changes, breaking changes to user-facing features, significant architecture shifts
- **MINOR** (e.g., 0.6.0): New backwards-compatible functionality, new features, non-breaking enhancements
- **PATCH** (e.g., 0.6.1): Bug fixes, performance improvements, documentation updates — no new features

### Current Version

Current version: **v0.6.0** (maintained in `textquest/Cargo.toml` and all workspace crates)

All workspace crates (`textquest`, `textquest-common`, `textquest-dll`, `textquest-soul`, `textquest-web`) maintain the same version number for consistency.

## Release Checklist

Before creating a release tag, complete the following checklist:

### 1. Pre-Release Validation

- [ ] All tests pass locally
  ```bash
  cargo test --all
  python3 -m unittest discover -s tests -p 'test_*.py' -v
  ```
- [ ] Code passes linting and formatting
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo fmt --check
  python3 scripts/dev-preflight.py
  ```
- [ ] Documentation is up-to-date (API docs, README, wiki)
- [ ] Breaking changes documented in migration guide (if applicable)

### 2. Changelog Update

- [ ] Update `CHANGELOG.md` with:
  - New version number
  - Release date (YYYY-MM-DD)
  - Categories: Breaking Changes, Features, Bug Fixes, Performance, Security, Documentation
  - Contributor credits
- [ ] Link issues/PRs where applicable (e.g., "#1234")
- [ ] Verify `[Unreleased]` section is empty

### 3. Version Bumping

- [ ] Update version in `Cargo.toml` (all crates synchronized)
  ```toml
  [package]
  version = "X.Y.Z"
  ```
- [ ] Commit version bump and changelog
  ```bash
  git add Cargo.toml Cargo.lock CHANGELOG.md
  git commit -m "chore: bump version to vX.Y.Z"
  ```

### 4. Tag Creation

- [ ] Create annotated git tag
  ```bash
  git tag -a vX.Y.Z -m "Release vX.Y.Z"
  ```
- [ ] Push tag to origin
  ```bash
  git push origin vX.Y.Z
  ```

### 5. Release Automation

- [ ] Release workflow triggers automatically on `v*` tag push
  - Runs on self-hosted Windows (Frostreaver) and Linux (DigitalOcean) runners
  - Builds binaries, runs full test suite, publishes artifacts
  - Nightly release has 3am CT time gate (PowerShell check in workflow)

### 6. Post-Release

- [ ] Verify release artifacts in GitHub Releases
- [ ] Announce release in project channels (Discord, docs, wiki)
- [ ] Update project roadmap if applicable

## Hotfix Workflow

For critical production bugs requiring immediate release (e.g., security patches):

### 1. Create Hotfix Branch

```bash
git checkout vX.Y.Z                    # Check out from the release tag
git checkout -b hotfix/X.Y.(Z+1)       # Create hotfix branch
```

### 2. Apply Fixes

- Make targeted bug fixes only (no feature work)
- Commit with clear messages: `fix: description of the issue (#issue-number)`
- Keep changes minimal and focused

### 3. Update Version and Changelog

```bash
# Bump PATCH version only
# E.g., v0.6.0 → v0.6.1
sed -i '' 's/version = "0.6.0"/version = "0.6.1"/' Cargo.toml
```

- Update `CHANGELOG.md` with hotfix notes
- Commit: `chore: bump version to vX.Y.(Z+1)`

### 4. Tag and Release

```bash
git tag -a vX.Y.(Z+1) -m "Hotfix release vX.Y.(Z+1)"
git push origin vX.Y.(Z+1)
```

### 5. Backport to Main Branch

After hotfix is released:

```bash
git checkout master
git pull origin master
git cherry-pick hotfix/X.Y.(Z+1)       # Cherry-pick fix commits
git push origin master
```

Or open a PR to merge hotfix changes back to main:

```bash
git checkout master
git pull origin master
git merge --no-ff hotfix/X.Y.(Z+1)
git push origin master
```

Then delete the hotfix branch:

```bash
git branch -d hotfix/X.Y.(Z+1)
git push origin :hotfix/X.Y.(Z+1)
```

## Breaking Changes & Migration Guides

When introducing breaking changes, create a migration guide:

### File: `docs/migration/vX.Y.Z-MIGRATION.md`

Template:

```markdown
# Migration Guide: vX.Y.Z

## Overview

Brief description of breaking changes in this release.

## Breaking Changes

### 1. Configuration Format Change

**What changed:** Old format → New format

**Before:**
\`\`\`toml
[config_section]
old_key = "value"
\`\`\`

**After:**
\`\`\`toml
[config_section]
new_key = "value"
\`\`\`

**Migration:** Replace `old_key` with `new_key` in your config files.

### 2. API Changes (if applicable)

Document any CLI, IPC, or programmatic API changes.

## Upgrade Steps

1. Backup your configuration files
2. Update TextQuest to vX.Y.Z
3. Apply configuration changes per above
4. Test in staging before production

## Support

For issues during migration, open a GitHub issue with:

- Your current version
- Migration step where you encountered the issue
- Error logs from `logs/textquest.log` and `%TEMP%/textquest/textquest-dll.log`
```

Include migration guides in the `CHANGELOG.md` release notes:

```markdown
## [vX.Y.Z] - YYYY-MM-DD

### Breaking Changes

- Configuration format changed (see [Migration Guide](docs/migration/vX.Y.Z-MIGRATION.md))
- IPC protocol v2 (incompatible with v0.5.x)

See [Migration Guide: vX.Y.Z](docs/migration/vX.Y.Z-MIGRATION.md) for detailed upgrade instructions.
```

## Release Cadence

- **Regular releases**: Monthly (1st week of each month, pending project milestones)
- **Nightly releases**: Automated, runs at 3am CT on Windows runner
- **Hotfixes**: Ad-hoc, for critical security or stability issues
- **Major releases**: Tied to milestone completion (M1, M2, M3, etc.)

## CI/CD Pipeline

### Release Workflow Triggers

1. **Tag-based**: Push `v*` tag → GitHub Actions triggers release workflow
2. **Artifacts**: Windows binaries built on Frostreaver, Linux on DigitalOcean
3. **Windows Release Requirements**:
   - Nightly toolchain (due to `retour` dependency)
   - Link with nightly MSVC
4. **Tests**: Full workspace test suite runs before artifact publication

### Self-Hosted Runners

| Runner                                  | OS      | Purpose                                   |
| --------------------------------------- | ------- | ----------------------------------------- |
| `dmft-ci-service-01` (Frostreaver)      | Windows | Release builds, nightly builds, wiki sync |
| `dmft-ci-service-02` (Frostreaver)      | Windows | Release builds, parallel testing          |
| `textquest-gha-linux-01` (DigitalOcean) | Linux   | Merge gate, secrets scan, CI checks       |
| `textquest-gha-linux-02` (DigitalOcean) | Linux   | Merge gate, parallel testing              |

## Version Pinning in Config

In `config/textquest.toml`, optional field for config schema versioning:

```toml
[metadata]
config_version = "0.6"      # Matches release version for breaking changes tracking
```

This helps users detect stale configs after major updates.

## Release Notes Template

See `scripts/prepare-release.sh` for automated changelog template generation.

When writing release notes manually, follow this structure:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Breaking Changes

- (if any)

### Features

- New feature description (#PR or #issue)
- Another feature (#PR or #issue)

### Bug Fixes

- Bug description, impact (#issue)
- Another bug fix (#issue)

### Performance

- Performance improvement description (% improvement if known)

### Security

- Security fix description (CVE number if applicable, though this project is not in a CVE registry)

### Documentation

- Doc updates, API reference changes

### Known Issues

- Known issues or limitations in this release

### Contributors

- @username (First contribution)
- @username (contributions)
```

## Related Files

- `CHANGELOG.md` — Full release history and notes
- `scripts/prepare-release.sh` — Automated release validation and tag creation
- `.github/workflows/release.yml` — CI/CD release workflow
- `.github/workflows/nightly.yml` — Nightly release automation
- `Cargo.toml` — Version source of truth (all workspace crates)

## Troubleshooting

### Tag Already Exists

```bash
git tag -d vX.Y.Z                     # Delete local tag
git push origin :refs/tags/vX.Y.Z     # Delete remote tag
git tag -a vX.Y.Z -m "Release vX.Y.Z" # Recreate with correct message
git push origin vX.Y.Z
```

### Wrong Commit Tagged

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git checkout <correct-commit>
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

### CI/CD Pipeline Failure

Check GitHub Actions logs:

1. Go to repo → Actions → Release workflow run
2. Review error logs from Windows/Linux runners
3. Common issues:
   - Tests failed: Run `cargo test` locally and fix
   - Version mismatch: Ensure Cargo.toml was updated and committed before tagging
   - Missing changelog: Ensure `CHANGELOG.md` was updated

---

**Last Updated:** 2026-04-14
