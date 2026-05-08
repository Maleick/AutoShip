# Contributing to AutoShip

AutoShip is an OpenCode issue-to-PR orchestration plugin. It supports both OpenCode and Hermes Agent runtimes.

## Project structure

```text
hooks/opencode/          # OpenCode runtime hooks
hooks/hermes/            # Hermes Agent runtime hooks
skills/                  # OpenCode skills and role definitions
commands/                # OpenCode slash command templates
dist/                    # TypeScript build output
scripts/                 # Build and sync scripts
```

## Development workflow

1. Make changes to source files
2. Run validation:

```bash
npm run typecheck
npm run build
npm run verify:pack
bash hooks/opencode/check.sh
bash -n hooks/opencode/*.sh
```

3. Do not commit `.autoship/` runtime state or generated artifacts.

## Version bumps

Version is managed by semantic-release. Do not manually bump `package.json` version. The release workflow handles:

- `package.json`
- `package-lock.json`
- `VERSION`
- `CHANGELOG.md`

## Validation

Before submitting:

```bash
bash hooks/opencode/check.sh --syntax
bash hooks/opencode/check.sh --policy
bash -n hooks/opencode/*.sh hooks/hermes/*.sh
```
