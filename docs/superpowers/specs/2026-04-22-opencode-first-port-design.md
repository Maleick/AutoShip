# AutoShip OpenCode-First Port Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make AutoShip install and run as an OpenCode-first orchestration package while keeping the same repository as the single source of truth.

**Architecture:** AutoShip remains one repo, and OpenCode becomes the only supported worker runtime. The OpenCode path should resolve from the repo itself and `~/.config/opencode`. Documentation and GitHub Pages content must present OpenCode-only operation everywhere users would look.

**Tech Stack:** Bash hooks, OpenCode config, GitHub Pages content, Markdown docs, existing AutoShip skills/commands, GitHub CLI.

---

## Scope

This port changes packaging, bootstrap, documentation, and site content.

It does not change AutoShip's core orchestration model:
- GitHub issues still drive work
- workers still run through the same dispatch/verify pipeline
- `.autoship/` remains the runtime state boundary

## Target State

- OpenCode is the default install and runtime path.
- AutoShip starts from the current repo without relying on external plugin cache paths.
- The repo includes a clear OpenCode install/bootstrap path.
- GitHub Pages and docs describe OpenCode first.
- OpenCode model discovery is the only worker path.

## Design

### 1. OpenCode Packaging Surface

Add a repo-local OpenCode install path that bootstraps the package into `~/.config/opencode` and wires the existing commands, skills, and hooks to the OpenCode runtime.

This path should:
- initialize `.autoship/`
- register OpenCode hooks
- resolve scripts from the repo or OpenCode config
- keep `VERSION` as the release source of truth

### 2. Runtime Resolution

Replace all external cache path lookups in OpenCode-facing entrypoints with repo-root or OpenCode-config resolution.

The important rule is:
- repo execution should work from `~/Projects/<repo>`
- runtime state should stay in `.autoship/`
- shared OpenCode config should stay in `~/.config/opencode`

### 3. Documentation and Website

Update the primary docs so they clearly say:
- OpenCode is the default
- the install path lives in this repo
- OpenCode is the only supported worker runtime

Update GitHub Pages content to match the same message, using the repo's existing published content tree (`docs/` and `wiki/`) so the website and repo docs do not diverge.

## Files and Responsibilities

- `hooks/opencode/init.sh`: bootstrap OpenCode runtime state and config
- `hooks/opencode/classify-issue.sh`: classify issue complexity for OpenCode routing
- `hooks/opencode/monitor-agents.sh`: poll OpenCode agent status markers
- `hooks/opencode/cleanup-worktree.sh`: clean completed worktrees
- `commands/*.md`: OpenCode command entrypoints and help text
- `skills/*/SKILL.md`: OpenCode orchestration protocols
- `README.md`: first-stop install and usage docs
- `AGENTS.md`: install guidance and repo conventions
- `docs/OPENCODE_INSTALL.md`: primary OpenCode install guide
- `docs/OPENCODE_PORT_SPEC.md`: architecture and port reference
- `wiki/*` and `docs/*`: public-facing OpenCode-first messaging for the GitHub Pages site

## Acceptance Criteria

- OpenCode install and startup instructions are the first documented path.
- No OpenCode entrypoint references external plugin cache paths.
- The repo can be installed and run from `~/Projects/<repo>` using OpenCode.
- GitHub Pages content matches the repo docs and says OpenCode is the default.
- OpenCode model discovery and setup are the only supported worker path.

## Verification

- Run the OpenCode init/bootstrap path from a clean checkout.
- Verify the startup path creates `.autoship/state.json` and `hooks_dir` correctly.
- Verify the main OpenCode commands load from the repo root.
- Search the repo for external cache path assumptions and remove them from OpenCode paths.
- Verify the website/docs render the OpenCode-first installation story consistently.

## Non-Goals

- No repo split.
- No rewrite of the core issue-dispatch model.
- No support for non-OpenCode worker runtimes.
