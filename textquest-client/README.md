# TextQuest Client SDK

Multi-language SDK for programmatic access to TextQuest-managed EverQuest clients.

## Documentation

The SDK documentation is built with [mdBook](https://rust-lang.github.io/mdBook/) and published to GitHub Pages.

### Building Locally

```bash
# Install mdbook
cargo install mdbook

# Build
mdbook build

# Serve locally
mdbook serve
```

### Published Docs

https://maleick.github.io/TextQuest/textquest-client/

## Packages

| Language | Package | Registry | Status |
|----------|---------|----------|--------|
| Rust | `textquest-common` | [crates.io](https://crates.io/crates/textquest-common) | Published |
| Python | `textquest` | PyPI | In progress |
| TypeScript | `@textquest/client` | npm | Planned |

## Repository Structure

```
textquest-client/
├── docs/           # mdBook documentation
│   ├── book.toml   # mdBook configuration
│   └── src/        # Documentation source
├── pytextquest/    # Python SDK (to be implemented)
└── tstextquest/    # TypeScript SDK (to be implemented)
```

## Quick Start

### Rust

```toml
[dependencies]
textquest-common = "0.6"
```

### Python

```bash
pip install textquest
```

### TypeScript

```bash
npm install @textquest/client
```

## SDK Architecture

The SDK provides language bindings for the TextQuest IPC protocol:

- **Command Channel**: Named pipes for request/response
- **State Channel**: Shared memory for read-only snapshots

See [docs/src/](docs/src/) for language-specific quickstarts.

## Publication

SDK packages are published via GitHub Actions workflows:

- `.github/workflows/publish-python.yml`
- `.github/workflows/publish-npm.yml`

**Required secrets (not yet configured):**

- `PYPI_API_TOKEN` — PyPI API token
- `NPM_TOKEN` — npm API token

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

For SDK-specific questions, open an issue at https://github.com/Maleick/TextQuest/issues.
