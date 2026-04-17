# Installation

## Rust SDK

The Rust SDK (`textquest-common`) is published to crates.io:

```toml
[dependencies]
textquest-common = "0.6"
```

For the latest development version from GitHub:

```toml
[dependencies]
textquest-common = { git = "https://github.com/Maleick/TextQuest", branch = "master", package = "textquest-common" }
```

## Python SDK

The Python SDK is available via PyPI:

```bash
pip install textquest
```

### Requirements

- Python 3.10 or later
- Windows (for live EQ integration)
- `bincode` for serialization

## TypeScript SDK

The TypeScript SDK is available via npm:

```bash
npm install @textquest/client
```

### Requirements

- Node.js 18 or later
- TypeScript 5.0+ (for type definitions)

## Development Installation

To work with the SDK source directly:

```bash
git clone https://github.com/Maleick/TextQuest.git
cd TextQuest
cargo build
```

## Verifying Installation

After installation, verify your setup:

```bash
# Rust
cargo add textquest-common

# Python
python -c "import textquest; print(textquest.__version__)"

# TypeScript
npx tsc --noEmit -p @textquest/client
```
