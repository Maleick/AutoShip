# Release Notes Template

This template is used when creating release notes for TextQuest. Customize the sections based on the release content.

---

# TextQuest Release: vX.Y.Z

**Release Date:** YYYY-MM-DD

## Overview

Brief summary of this release. What are the major highlights? 1-2 sentences.

Example:
> This release introduces the M7 feature parity milestone with soul engine integration, party-level AI orchestration, and async combat rotation system.

## What's New

### Major Features

- **Feature Name**: Short description of the feature and its impact. (#issue or #PR)
  - Sub-feature or variant if applicable
- **Feature Name**: Description

### Enhancements

- Performance improvement: Description of what was optimized and measured impact
- Quality-of-life improvement: What was made easier or more efficient

## Bug Fixes

- **Critical**: Description of critical bug and impact (Affects all users)
- **High**: Description of high-priority bug (Affects many users)
- **Medium**: Description of medium-priority bug (Affects some workflows)
- **Low**: Description of low-priority bug (Edge cases, rare scenarios)

Example format:
```
- Fixed spawn linked list traversal crash when client count exceeds 100
  - Root cause: Off-by-one in iteration limit check
  - Impact: Affected multibox users with >100 clients
  - PR: #1223
```

## Security Fixes

- **CVE-XXXX-XXXXX**: Description (though TextQuest may not be in CVE registry)
- DLL injection now validates EQ process memory layout before hook installation
- Named pipe access control improved to prevent cross-client data leakage

## Performance Improvements

- Combat ability evaluation: 40% faster via precomputed cooldown keys
- Memory reads batched to reduce IPC round-trips by 50%
- TUI rendering optimized for 60+ concurrent spawn updates per second

## Breaking Changes

If this is a MAJOR version bump:

```markdown
### ⚠️ Breaking Changes

This release introduces breaking changes. See [Migration Guide: vX.Y.Z](docs/migration/vX.Y.Z-MIGRATION.md) for detailed upgrade instructions.

- Configuration file format changed (toml restructure)
  - **Before:** `[config] old_key = "value"`
  - **After:** `[config] new_key = "value"`
  - **Migration:** Update config files, validate with `textquest --validate-config`

- IPC protocol upgraded to v2
  - Older clients (v0.x) cannot connect to v1.0+ orchestrator
  - **Migration:** Upgrade all clients, stop all running instances, start fresh
```

## Deprecations

List any deprecated features that will be removed in a future release:

- `--legacy-rotation-format` CLI flag: Use `--rotation-file` with new TOML format (removal planned for v1.0)
- `DMFT_CONFIG_PATH` environment variable: Use `TEXTQUEST_CONFIG_PATH` instead (removal planned for v1.0)

## Known Issues

- EQ patch detection requires manual offset revalidation post-patch
  - **Workaround:** Run `textquest --scan-offsets` after EQ updates on Wednesdays
  - **Status:** Pattern scanner provided in toolchain; full auto-detection planned for v0.8
  
- Nightly releases trigger 3am CT only on Windows runner
  - **Impact:** macOS/Linux nightly builds may be delayed
  - **Status:** Cross-platform nightly CI under development

- Self-hosted runner workspaces persist between test runs
  - **Workaround:** Use skipTest guards in integration tests
  - **Status:** Cleanup hooks planned for next CI refactor

## Download & Installation

### Binaries

Download release artifacts from [GitHub Releases](https://github.com/TextQuest/textquest/releases/tag/vX.Y.Z):

- `textquest.exe` — Orchestrator binary (Windows)
- `textquest_dll.dll` — Injected DLL for EQ client (Windows)
- `VERSION.txt` — Version metadata

### Source Code

```bash
git clone https://github.com/TextQuest/textquest.git
cd textquest
git checkout vX.Y.Z

# Build locally
cargo build --release
```

### Installation Steps

1. **Backup existing installation**
   ```bash
   cp -r C:\TextQuest C:\TextQuest.backup
   ```

2. **Extract release artifacts**
   ```bash
   # Download vX.Y.Z release zip from GitHub
   # Extract to TextQuest installation directory
   ```

3. **Verify installation**
   ```bash
   textquest.exe --version
   # Expected: TextQuest vX.Y.Z
   ```

4. **Update configuration (if breaking changes)**
   - See [Migration Guide](docs/migration/vX.Y.Z-MIGRATION.md) if applicable
   - Test configuration: `textquest.exe --validate-config`

## Migration & Upgrade Notes

**For users upgrading from vX.Y.(Z-1):**

- Configuration files remain backward compatible (PATCH release)
- No breaking changes in this release
- Simply replace binaries and restart

**For users upgrading from vX.(Y-1).Z or earlier:**

- See [Migration Guide](docs/migration/vX.Y.Z-MIGRATION.md) for breaking changes (if MINOR or MAJOR bump)

## Contributors

Thanks to the following contributors for this release:

- [@username](https://github.com/username) — Feature X, Documentation
- [@username](https://github.com/username) — Bug fixes Y, Z

## Testing & Validation

This release was validated on:

- **Windows**: Server 2019, Windows 10, Windows 11 (all x64)
- **EverQuest**: Truebox TLP (Teek), Agnarr (no True Box)
- **Configurations**: 1 client, 36-box multibox, mixed class groups

## Related Documentation

- [Installation & Build Guide](docs/wiki/Installation-and-Build.md)
- [Configuration Guide](docs/wiki/Configuration.md)
- [Developer Guide](docs/wiki/Developer-Guide.md)
- [Release Process](docs/dev/release-process.md)
- [Versioning Strategy](docs/dev/versioning.md)

## Reporting Issues

Found a bug? Please open an issue on GitHub with:

- TextQuest version (output of `textquest --version`)
- Steps to reproduce
- Error logs from `logs/textquest.log` and `%TEMP%/textquest/textquest-dll.log`
- Your system details (Windows version, EQ client version)

## Support

For questions or issues:

- **GitHub Issues**: [Report a bug or request a feature](https://github.com/TextQuest/textquest/issues)
- **Documentation**: Check the [wiki](docs/wiki/) for troubleshooting guides
- **Community**: Join the Discord for real-time discussion (if applicable)

---

**Last Updated:** vX.Y.Z Release Date

For a complete history of changes, see [CHANGELOG.md](CHANGELOG.md).
