# Install AutoShip

AutoShip is a multi-runtime plugin that turns GitHub issues into verified pull requests. It plans eligible issues, dispatches workers (OpenCode or Hermes), verifies results, and creates pull requests with local runtime state in `.autoship/`.

## Supported Runtimes

| Runtime | Type | Max Workers | Label | Setup |
|---------|------|-------------|-------|-------|
| **OpenCode** | Interactive CLI | 20 | `agent:ready` | `opencode-autoship install` |
| **Hermes** | Cron-based | 20 | `autoship:ready-simple` | `bash hooks/hermes/setup.sh` |

## OpenCode Handoff

Paste this into OpenCode if you want the agent to install and verify AutoShip for you:

```text
Fetch and follow instructions from https://raw.githubusercontent.com/Maleick/AutoShip/refs/tags/v2.2.1/INSTALL.md
```

## Prerequisites

- OpenCode installed and available in your shell (for OpenCode runtime).
- Hermes Agent installed and configured (for Hermes runtime).
- Node.js 18 or newer with npm, or Bun for one-time package execution.
- Git installed and available in your shell.
- GitHub CLI (`gh`) authenticated with access to the target repository.
- `jq` installed and available on `PATH` (Unix-like systems).
- PowerShell 5.1 or later (Windows).
- A GitHub repository with issues labeled `agent:ready` (OpenCode) or `autoship:ready-simple` (Hermes).

## npm Global Install

For normal OpenCode usage, install the CLI globally and register the bundled plugin assets:

```bash
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor
```

Restart OpenCode after installation. Then open the target repository and run:

```text
/autoship-setup
/autoship
```

## One-Time Bun Path

If your environment supports one-time package execution through Bun:

```bash
bunx opencode-autoship install
bunx opencode-autoship doctor
```

Use the npm global install for long-term usage.

## OpenCode Plugin Assets

`opencode-autoship install` updates your OpenCode config and copies bundled assets into the OpenCode config directory:

```text
~/.config/opencode/opencode.json
~/.config/opencode/.autoship/
```

The installer adds `opencode-autoship` to the top-level `plugin` array without removing unrelated settings.

## Project Setup

Run setup inside the repository you want AutoShip to operate on:

```text
/autoship-setup
```

Setup discovers live models from `opencode models`, writes `.autoship/config.json`, and writes `.autoship/model-routing.json`. Do not commit `.autoship/`; it is local runtime state.

## Windows Setup

On Windows, the `opencode` CLI is a PowerShell cmdlet and is not accessible from bash/WSL/MSYS2 shells. Use the PowerShell setup script instead:

```powershell
# From the target repository root:
.\hooks\opencode\setup.ps1

# Non-interactive with defaults:
.\hooks\opencode\setup.ps1 -NoTui

# Custom configuration:
.\hooks\opencode\setup.ps1 -NoTui -MaxAgents 10 -Labels "agent:ready,needs-work"
```

The PowerShell script does not require `jq` and uses `opencode models` directly from PowerShell. All other setup steps remain the same.

## Verification

These checks do not require publishing credentials:

```bash
npm view opencode-autoship version
npm view opencode-autoship dist-tags
opencode-autoship doctor
```

For a local clone:

```bash
npm install
npm run build
npm run typecheck
bash hooks/opencode/verify-package.sh
bash hooks/opencode/check.sh
bash -n hooks/opencode/*.sh hooks/*.sh
```

To verify command availability, restart OpenCode in a GitHub-backed repository and run:

```text
/autoship-status
```

## Updating

For npm global installs:

```bash
npm install -g opencode-autoship@latest
opencode-autoship install
opencode-autoship doctor
```

For reproducible installs, pin a published version:

```bash
npm install -g opencode-autoship@2.2.1
opencode-autoship install
```

## Troubleshooting

### Plugin Not Loading

- Confirm `opencode.json` is valid JSON.
- Confirm `opencode-autoship` appears in the top-level `plugin` array.
- Restart OpenCode after changing the config.
- Rerun `opencode-autoship install` if commands are missing.

### Doctor Reports Missing Project Config

- Open the target GitHub repository in OpenCode.
- Run `/autoship-setup` from that repository.
- Confirm `.autoship/config.json` and `.autoship/model-routing.json` exist locally.

### Setup Fails on Windows with "opencode is required"

If `bash hooks/opencode/setup.sh` fails with "opencode is required" on Windows, use the PowerShell script instead:

```powershell
.\\hooks\\opencode\\setup.ps1
```

The bash setup script cannot access the PowerShell-based `opencode` CLI. The PowerShell script handles Windows model discovery and config generation natively.

### No Issues Are Planned

- Confirm `gh auth status` succeeds.
- Confirm the repository has open GitHub issues labeled `agent:ready` (OpenCode) or `autoship:ready-simple` (Hermes).
- Check `/autoship-plan` (OpenCode) or `bash hooks/hermes/plan-issues.sh` (Hermes) before starting.

### Hermes Runtime Issues

- Confirm Hermes CLI is installed: `hermes --version`
- Confirm `~/.hermes/config.yaml` has a valid provider and model configured.
- Run `bash hooks/hermes/setup.sh` to verify Hermes detection.
- Check `bash hooks/hermes/status.sh` for workspace state.

## Hermes Quick Start

```bash
# 1. Navigate to AutoShip repo
cd ~/projects/AutoShip

# 2. Setup Hermes runtime
bash hooks/hermes/setup.sh

# 3. Plan issues for your target repo
HERMES_TARGET_REPO=your-org/your-repo bash hooks/hermes/plan-issues.sh

# 4. Dispatch an issue to Hermes
bash hooks/hermes/dispatch.sh <issue-number>

# 5. The Hermes cron will pick up queued issues and implement them
```

### Runtime State

- Do not commit `.autoship/`; it is project-local runtime state.
- Use `/autoship-stop` before manually cleaning running workspaces.
- Run `opencode-autoship doctor` after upgrading.

## Links

- OpenCode install details: https://github.com/Maleick/AutoShip/blob/main/docs/OPENCODE_INSTALL.md
- OpenCode agent install entrypoint: https://github.com/Maleick/AutoShip/blob/main/.opencode/INSTALL.md
- Releases: https://github.com/Maleick/AutoShip/releases
- Issues: https://github.com/Maleick/AutoShip/issues
