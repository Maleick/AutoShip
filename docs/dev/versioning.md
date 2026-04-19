# Versioning Strategy

TextQuest uses [Semantic Versioning 2.0.0](https://semver.org/) for release version management.

## Version Format

```
MAJOR.MINOR.PATCH
```

### Incrementing Rules

- **MAJOR** (e.g., `1.0.0`): Incompatible API changes, breaking changes to user-facing features, significant architecture shifts
- **MINOR** (e.g., `0.6.0`): New backwards-compatible functionality, new features, non-breaking enhancements
- **PATCH** (e.g., `0.6.1`): Bug fixes, performance improvements, documentation updates — no new features

## Current Version

**v0.6.0**

All workspace crates maintain the same version number for consistency:
- `textquest` (orchestrator binary)
- `textquest-common` (shared protocol)
- `textquest-dll` (Windows DLL)
- `textquest-soul` (LLM/AI layer)
- `textquest-web` (Axum web backend)
- `textquest-web-sdk` (SDK/API)

Version is tracked in:
- `Cargo.toml` (root workspace)
- Individual crate `Cargo.toml` files
- `CHANGELOG.md` (release notes)

## Version Bumping Workflow

Use `scripts/bump-version.sh` to automate version updates across all crates:

```bash
# Bump MINOR version (0.6.0 → 0.7.0)
scripts/bump-version.sh minor

# Bump PATCH version (0.6.0 → 0.6.1)
scripts/bump-version.sh patch

# Bump MAJOR version (0.6.0 → 1.0.0)
scripts/bump-version.sh major
```

The script:
1. Updates version in all `Cargo.toml` files
2. Commits with message: `chore: bump version to vX.Y.Z`

## Release Cadence

- **Regular releases**: Monthly (1st week of each month, pending project milestones)
- **Hotfixes**: Ad-hoc, for critical security or stability issues
- **Nightly releases**: Automated on Windows runner at 3am CT (see `.github/workflows/nightly-release.yml`)
- **Major releases**: Tied to milestone completion (M1, M2, M3, etc.)

## Breaking Changes

When introducing incompatible changes (MAJOR version bump):

1. Create `docs/migration/vX.Y.Z-MIGRATION.md` with upgrade steps
2. Document breaking changes in `CHANGELOG.md`
3. Link migration guide in release notes
4. Consider providing migration tools or scripts if practical

Example:
```markdown
## Migration Guide: v1.0.0

### Breaking Change: Configuration Format

**Before:**
```toml
[config]
old_key = "value"
```

**After:**
```toml
[config]
new_key = "value"
```

**Migration:** Update all config files with `old_key` → `new_key`.
```

## Related Documentation

- `docs/dev/release-checklist.md` — Pre-release validation steps
- `docs/dev/release-process.md` — Detailed release workflow and hotfix procedures
- `CHANGELOG.md` — Full release history
- `RELEASE_TEMPLATE.md` — Release notes template
- `.github/workflows/release.yml` — CI/CD release automation
