# TypeScript & Linting Configuration

This document outlines the code quality setup for the TextQuest frontend.

## Overview

The frontend uses the following tools for code quality:

- **TypeScript**: Strict type checking with ES2023 target
- **ESLint**: JavaScript/TypeScript linting with React rules
- **Prettier**: Automatic code formatting
- **Husky**: Git hooks to enforce linting before commits
- **Lint-Staged**: Run linters only on staged files

## TypeScript Configuration

### Strict Mode Enabled

The `tsconfig.app.json` includes strict mode with comprehensive checks:

```json
{
  "compilerOptions": {
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  }
}
```

- **strict**: Enables all strict checking options
- **noUnusedLocals**: Error on unused local variables
- **noUnusedParameters**: Error on unused function parameters
- **noFallthroughCasesInSwitch**: Error on switch cases without break
- **noUncheckedSideEffectImports**: Warning for imports with side effects

## ESLint Configuration

### Plugins

- **typescript-eslint**: TypeScript linting rules
- **react-hooks**: React Hooks rules
- **react-refresh**: Vite React refresh plugin rules

### Rules

All ESLint recommended rules are enabled plus:

- React Hooks best practices
- React refresh component export validation

## Prettier Configuration

### Settings

```json
{
  "semi": true,
  "singleQuote": false,
  "tabWidth": 2,
  "trailingComma": "all",
  "printWidth": 100
}
```

## Pre-commit Hooks

The `pre-commit` hook (managed by Husky) runs:

1. **ESLint Auto-fix**: Automatically fixes linting issues
2. **Lint-Staged**: Runs linters only on staged files
3. **Prettier**: Formats staged code

### Setup

Husky is automatically initialized via `npm install`. To set up hooks manually:

```bash
cd textquest-web/frontend
npm run prepare
```

## Available Commands

```bash
# Run ESLint
npm run lint

# Auto-fix ESLint issues
npm run lint:fix

# Format code with Prettier
npm run format

# Check formatting without changes
npm run format:check

# TypeScript compilation check
npm run build

# Development server
npm run dev
```

## CI/CD Integration

GitHub Actions workflow (`.github/workflows/lint.yml`) runs on all PRs and pushes to `master`/`main`:

1. **ESLint Check**: Ensures code passes linting rules
2. **TypeScript Compilation**: Verifies TypeScript builds successfully
3. **Prettier Format Check**: Confirms code is properly formatted

## Acceptance Criteria

- ✓ TypeScript strict mode enabled
- ✓ ESLint configured with React rules
- ✓ Prettier auto-formatting on save
- ✓ Pre-commit hooks enforce linting
- ✓ CI checks linting pass

## Troubleshooting

### Pre-commit hooks not running

```bash
cd textquest-web/frontend
npm run prepare
```

### Fix all linting and formatting issues

```bash
cd textquest-web/frontend
npm run lint:fix
npm run format
```

### Clear husky cache

```bash
rm -rf .husky
npm run prepare
```
