# Installing AutoShip for OpenCode

## Prerequisites

- [OpenCode](https://opencode.ai) installed
- Node.js 18 or newer
- `gh` authenticated with access to the target GitHub repository
- `jq` available on `PATH`

## Installation

For long-term use, install the AutoShip CLI globally:

```bash
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor
```

For a one-time install without keeping a global CLI:

```bash
bunx opencode-autoship install
bunx opencode-autoship doctor
```

Restart OpenCode after installation so the plugin and commands are loaded.

## Project Setup

Open the target GitHub repository in OpenCode and run:

```text
/autoship-setup
```

The setup wizard discovers live models from `opencode models` and writes project-local runtime state under `.autoship/`. Do not commit `.autoship/`.

Start orchestration with:

```text
/autoship
```

## Updating

Update the npm package and reinstall the OpenCode assets:

```bash
npm install -g opencode-autoship@latest
opencode-autoship install
opencode-autoship doctor
```

To pin a version for reproducible installs:

```bash
npm install -g opencode-autoship@2.2.1
opencode-autoship install
```

## Troubleshooting

If commands are missing, rerun `opencode-autoship install` and restart OpenCode.

If doctor reports missing project config, run `/autoship-setup` inside the target repository.

Detailed install guide: https://github.com/Maleick/AutoShip/blob/main/INSTALL.md
OpenCode-specific docs: https://github.com/Maleick/AutoShip/blob/main/docs/OPENCODE_INSTALL.md
