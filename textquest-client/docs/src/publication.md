# Publication Metadata

This document describes the registry publication configuration for TextQuest SDK packages.

## Published Packages

| Package | Registry | Crate ID | Status |
|---------|----------|----------|--------|
| `textquest-common` | crates.io | `textquest-common` | Published (v0.6.0) |
| Python SDK | PyPI | `textquest` | Not published |
| TypeScript SDK | npm | `@textquest/client` | Not published |

## Crates.io (Rust)

### Current Status

`textquest-common` is published to crates.io at version 0.6.0.

### Publication Workflow

The Rust SDK is published via the `release.yml` workflow with the following configuration:

**Required Secrets:**

| Secret | Description |
|--------|-------------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token with publish permissions |

**Workflow:** `.github/workflows/release.yml`

**Publication Command:**
```bash
cargo publish -p textquest-common
```

**Notes:**
- Only `textquest-common` is published as a library crate
- Binary crates (`textquest`, `textquest-soul`, etc.) are distributed via GitHub Releases
- The `edition = "2024"` field requires Rust 1.85+

## PyPI (Python)

### Status

The Python SDK package is **not yet published**. The following setup is in place:

### Package Configuration

**Location:** `pytextquest/` (to be created)

**Files Required:**

```
pytextquest/
├── pyproject.toml
├── src/
│   └── pytextquest/
│       ├── __init__.py
│       ├── client.py
│       ├── session.py
│       └── types.py
└── README.md
```

### `pyproject.toml` Template

```toml
[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "textquest"
version = "0.1.0"
description = "Python SDK for TextQuest EverQuest multibox controller"
readme = "README.md"
requires-python = ">=3.10"
license = { text = "MIT" }
authors = [
    { name = "TextQuest Team", email = "contact@example.com" }
]
classifiers = [
    "Development Status :: 3 - Alpha",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Operating System :: Microsoft :: Windows",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.10",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
]
dependencies = [
    "bincode>=1.3",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0",
    "mypy>=1.0",
]

[project.urls]
Homepage = "https://github.com/Maleick/TextQuest"
Documentation = "https://maleick.github.io/TextQuest/"
Repository = "https://github.com/Maleick/TextQuest"
Issues = "https://github.com/Maleick/TextQuest/issues"

[tool.setuptools.packages.find]
where = ["src"]

[tool.mypy]
python_version = "3.10"
warn_return_any = true
warn_unused_configs = true
```

### Publication Workflow

**Required Secrets:**

| Secret | Description |
|--------|-------------|
| `PYPI_API_TOKEN` | PyPI API token with publish permissions |

**Workflow Template (`.github/workflows/publish-python.yml`):**

```yaml
name: Publish Python SDK

on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'
      
      - name: Install build dependencies
        run: pip install build
      
      - name: Build package
        run: python -m build
      
      - name: Publish to PyPI
        uses: pypa/gh-action-pypi-publish@release/v1
        with:
          password: ${{ secrets.PYPI_API_TOKEN }}
```

## npm (TypeScript)

### Status

The TypeScript SDK package is **not yet published**. The following setup is in place:

### Package Configuration

**Location:** `tstextquest/` (to be created)

**Files Required:**

```
tstextquest/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts
│   ├── client.ts
│   ├── session.ts
│   └── types.ts
└── README.md
```

### `package.json` Template

```json
{
  "name": "@textquest/client",
  "version": "0.1.0",
  "description": "TypeScript SDK for TextQuest EverQuest multibox controller",
  "main": "dist/index.js",
  "module": "dist/index.mjs",
  "types": "dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.mjs",
      "require": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  },
  "files": [
    "dist"
  ],
  "scripts": {
    "build": "tsup",
    "dev": "tsup --watch",
    "lint": "eslint src",
    "typecheck": "tsc --noEmit",
    "test": "vitest"
  },
  "keywords": [
    "everquest",
    "eq",
    "multibox",
    "automation"
  ],
  "author": "TextQuest Team",
  "license": "MIT",
  "peerDependencies": {
    "typescript": ">=5.0.0"
  },
  "devDependencies": {
    "@typescript-eslint/eslint-plugin": "^7.0.0",
    "@typescript-eslint/parser": "^7.0.0",
    "eslint": "^8.57.0",
    "tsup": "^8.0.0",
    "typescript": "^5.4.0",
    "vitest": "^1.0.0"
  },
  "engines": {
    "node": ">=18.0.0"
  }
}
```

### `tsconfig.json` Template

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022"],
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "outDir": "./dist",
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
```

### Publication Workflow

**Required Secrets:**

| Secret | Description |
|--------|-------------|
| `NPM_TOKEN` | npm API token with publish permissions |

**Workflow Template (`.github/workflows/publish-npm.yml`):**

```yaml
name: Publish npm Package

on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'
          
      - name: Install dependencies
        run: npm ci
      
      - name: Build package
        run: npm run build
      
      - name: Publish to npm
        run: npm publish --access public
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

## Registry Secret Blockers

The following secrets are **required but not yet configured** for automated publication:

| Registry | Secret Name | Status | Instructions |
|----------|-------------|--------|--------------|
| crates.io | `CARGO_REGISTRY_TOKEN` | Configured | Already published |
| PyPI | `PYPI_API_TOKEN` | **Required** | Generate at pypi.org/manage/account |
| npm | `NPM_TOKEN` | **Required** | Generate at npmjs.com/settings/tokens |

### Setting Up Registry Tokens

1. **PyPI:**
   - Go to https://pypi.org/manage/account
   - Create an API token with upload permissions
   - Add as `PYPI_API_TOKEN` in GitHub repo settings

2. **npm:**
   - Go to https://npmjs.com/settings/tokens
   - Create a classic token with "Automation" scope
   - Add as `NPM_TOKEN` in GitHub repo settings

## Manual Publication

Until automated workflows are configured, packages can be published manually:

```bash
# Python
cd pytextquest
pip install build
python -m build
twine upload dist/*

# npm
cd tstextquest
npm login
npm publish --access public
```

## Version Management

SDK packages follow semantic versioning aligned with the core TextQuest releases:

- **Major**: Breaking changes to the IPC protocol
- **Minor**: New commands, responses, or features
- **Patch**: Bug fixes and documentation updates

See the main [CHANGELOG.md](../../CHANGELOG.md) for release history.
