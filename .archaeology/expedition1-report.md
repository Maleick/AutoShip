# Expedition 1: Dead Code Excavation

Mode: survey

## Findings

- `knip` reported `dist/cli.*`, `dist/index.*`, and `dist/types.*` as unused. These are build outputs used by package publishing and are not removal candidates.
- `knip` reported `plugins/autoship.ts` as unused from static imports. This is installed as an OpenCode plugin asset, so static import analysis cannot prove it dead.
- `semantic-release` appears as an unlisted binary in `.github/workflows/release.yml` because it is intentionally invoked through `npx --package` rather than a package dependency.

## Action

- No code removed.
- Keep future dead-code tooling configured with package/runtime asset exclusions before using restore mode.
