# Release Checklist

Step-by-step checklist for performing a TextQuest release.

## Prerequisites

- [ ] You have push access to the main repository
- [ ] All work is committed and pushed to `master` branch
- [ ] You have a local checkout with clean working tree

## Pre-Release Validation

### 1. Run Full Test Suite

```bash
# Unit and integration tests
cargo test --all --lib

# Python tests (if applicable)
python3 -m unittest discover -s tests -p 'test_*.py' -v

# Pre-push gate (comprehensive)
python3 scripts/dev-preflight.py
```

- [ ] All tests pass locally
- [ ] No new warnings or errors introduced

### 2. Code Quality Checks

```bash
# Formatting
cargo fmt --all -- --check

# Linting
cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] All code is properly formatted
- [ ] No clippy warnings

### 3. Documentation Review

- [ ] README.md is current and accurate
- [ ] API documentation is up-to-date
- [ ] Wiki pages reflect new features/changes
- [ ] Breaking changes documented (if applicable)
- [ ] Migration guides created (if MAJOR version bump)

## Version Bump

### 4. Update Version Numbers

```bash
# Determine new version (major, minor, or patch)
# E.g., for PATCH release: 0.6.0 → 0.6.1

scripts/bump-version.sh patch
```

- [ ] Version updated in all `Cargo.toml` files
- [ ] Commit message format: `chore: bump version to vX.Y.Z`

## Changelog Update

### 5. Update CHANGELOG.md

1. Add new version section at top of `CHANGELOG.md`:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Breaking Changes

- (if any)

### Features

- Feature description (#issue or #PR)

### Bug Fixes

- Bug description (#issue)

### Performance

- Performance improvement description

### Security

- Security fix description

### Documentation

- Documentation updates

### Known Issues

- Known issues or limitations
```

2. Review and consolidate `[Unreleased]` section into new release section
3. Update release date (YYYY-MM-DD format)
4. Include issue/PR numbers for traceability

- [ ] Changelog is updated and accurate
- [ ] `[Unreleased]` section is empty or contains only planned work
- [ ] Release date is correct
- [ ] All contributors mentioned (if applicable)

## Tagging & Release

### 6. Create Git Tag

```bash
# Create annotated tag
git tag -a vX.Y.Z -m "Release vX.Y.Z"

# Push tag to origin (triggers release workflow)
git push origin vX.Y.Z
```

- [ ] Tag created with correct version format (`vX.Y.Z`)
- [ ] Tag message is clear and concise
- [ ] Tag pushed to origin

### 7. Verify Release Workflow

GitHub Actions workflow `.github/workflows/release.yml` will automatically:

1. Run full test suite on self-hosted Windows runner
2. Build release binaries
3. Create GitHub Release with artifacts
4. Generate release notes

Expected status:
- [ ] GitHub Actions "Release" workflow triggered
- [ ] Workflow completes successfully (green checkmark)
- [ ] Release artifacts appear in GitHub Releases page
- [ ] Release notes populated (if `generate_release_notes: true`)

## Post-Release

### 8. Verify Release Artifacts

Navigate to [GitHub Releases](https://github.com/TextQuest/textquest/releases):

- [ ] Release page exists for new version
- [ ] Binaries downloaded and checksummed:
  - `textquest.exe` (orchestrator)
  - `textquest_dll.dll` (injected DLL)
  - `VERSION.txt` (version metadata)
- [ ] Release notes are visible and accurate
- [ ] Prerelease flag is correct (unset for stable, set for `-rc`/`-alpha`/`-beta`)

### 9. Communication & Documentation

- [ ] Release announcement posted to project Discord channel (if applicable)
- [ ] Wiki updated with new version availability
- [ ] Update project roadmap if applicable
- [ ] Notify team members/collaborators

### 10. Post-Release Cleanup

```bash
# If this was a hotfix, merge back to master
git checkout master
git pull origin master
git merge --no-ff hotfix/X.Y.(Z+1)   # (if applicable)
git push origin master
```

- [ ] Hotfix branches merged back to master (if applicable)
- [ ] Local tags cleaned up
- [ ] CI/CD pipeline remains stable

## Hotfix Release Workflow

For critical production bugs requiring immediate release:

### 1. Create Hotfix Branch

```bash
# Check out from release tag
git checkout vX.Y.Z
git checkout -b hotfix/X.Y.(Z+1)
```

### 2. Apply Fixes

- [ ] Make only targeted bug fixes (no feature work)
- [ ] Commit with message: `fix: description of the issue (#issue-number)`
- [ ] Test changes thoroughly

### 3. Bump Patch Version Only

```bash
# PATCH version only
scripts/bump-version.sh patch
```

- [ ] Version bumped correctly (PATCH only)

### 4. Update Changelog

- [ ] Add hotfix section to `CHANGELOG.md`
- [ ] Note the hotfix nature in release notes

### 5. Tag & Release

```bash
git tag -a vX.Y.(Z+1) -m "Hotfix release vX.Y.(Z+1)"
git push origin vX.Y.(Z+1)
```

- [ ] Tag created and pushed
- [ ] Release workflow triggers and completes

### 6. Backport to Master

```bash
git checkout master
git pull origin master
git cherry-pick hotfix/X.Y.(Z+1)
git push origin master

# Cleanup
git branch -d hotfix/X.Y.(Z+1)
git push origin :hotfix/X.Y.(Z+1)
```

- [ ] Fixes backported to master
- [ ] Hotfix branch deleted

## Rollback Procedure

If a release needs to be yanked/rolled back:

```bash
# Delete tag locally and remotely
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z

# Delete GitHub Release (via web UI)
# - Go to Releases → [version] → Edit → Delete
```

- [ ] Tag deleted
- [ ] GitHub Release deleted
- [ ] Team notified of issue

## Troubleshooting

### Tag Already Exists

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

### Wrong Commit Tagged

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git checkout <correct-commit-hash>
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

### Release Workflow Failed

1. Go to GitHub Actions → Release workflow run
2. Review error logs from self-hosted runner
3. Common issues:
   - Tests failed: Run `cargo test` locally and fix
   - Version mismatch: Verify `Cargo.toml` was committed before tagging
   - Missing changelog: Ensure `CHANGELOG.md` was updated

## Related Documentation

- `docs/dev/versioning.md` — Versioning strategy and format
- `docs/dev/release-process.md` — Detailed release workflow and procedures
- `CHANGELOG.md` — Full release history
- `RELEASE_TEMPLATE.md` — Release notes template
- `.github/workflows/release.yml` — Release automation workflow
