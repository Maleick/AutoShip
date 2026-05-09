# Discord Webhook Setup Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe Discord webhook persistence to the OpenCode setup wizard without storing secrets in repo state.

**Architecture:** `hooks/opencode/setup.sh` remains the single setup entrypoint. It validates Discord webhook URLs, writes them only to `~/.config/autoship/env` with restricted permissions, and prints masked status. Policy tests verify both persistence and non-leakage.

**Tech Stack:** Bash, jq, existing OpenCode policy test harness, Markdown docs.

---

### Task 1: Policy Test

**Files:**
- Modify: `hooks/opencode/test-policy.sh`

- [x] **Step 1: Add assertions to the existing setup fixture**

Add setup runs that pass `AUTOSHIP_DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/123456789/test_token` and assert:

```bash
test -f "$XDG_CONFIG_HOME/autoship/env" || fail "setup writes user env file for Discord webhook"
grep -F 'AUTOSHIP_DISCORD_WEBHOOK_URL=' "$XDG_CONFIG_HOME/autoship/env" >/dev/null || fail "setup persists Discord webhook env var"
test "$(stat -f %Lp "$XDG_CONFIG_HOME/autoship/env" 2>/dev/null || stat -c %a "$XDG_CONFIG_HOME/autoship/env")" = "600" || fail "Discord env file is private"
! grep -R "$setup_discord_url" .autoship >/dev/null 2>&1 || fail "setup must not write Discord webhook to project state"
```

- [x] **Step 2: Run policy to verify failure before implementation**

Run: `bash hooks/opencode/check.sh --policy`
Expected: FAIL because setup does not yet write the user env file.

### Task 2: Setup Implementation

**Files:**
- Modify: `hooks/opencode/setup.sh`
- Modify: `hooks/opencode/notify-discord.sh`

- [x] **Step 1: Add options and helpers**

Add `AUTOSHIP_DISCORD_WEBHOOK_URL`, URL validation, env-file path resolution, and private file writing. Do not add a CLI flag that accepts the webhook as an argument.

- [x] **Step 2: Add interactive prompt**

When interactive, prompt `Discord webhook URL [leave blank to skip]:` with `read -rs` so the pasted webhook is not echoed.

- [x] **Step 3: Wire persistence**

Persist valid webhook URLs to `${XDG_CONFIG_HOME:-$HOME/.config}/autoship/env`; never write them to `.autoship/config.json` or curl temp files.

- [x] **Step 4: Run policy to verify pass**

Run: `bash hooks/opencode/check.sh --policy`
Expected: PASS.

### Task 3: Docs

**Files:**
- Modify: `README.md`
- Modify: `docs/OPENCODE_INSTALL.md`
- Modify: `wiki/Configuration.md`

- [x] **Step 1: Document wizard setup**

Explain that `/autoship-setup` can persist the webhook to `~/.config/autoship/env`, the prompt is silent, and the notifier reads the persisted file automatically.

- [x] **Step 2: Run verification**

Run:

```bash
npm run typecheck
npm run build
npm run verify:pack
bash hooks/opencode/check.sh
bash -n hooks/opencode/*.sh hooks/*.sh hooks/hermes/*.sh
```

Expected: all commands pass.

### Self-Review

- Spec coverage: setup persistence, validation, non-leakage, docs, and verification are covered.
- Placeholder scan: no placeholders remain.
- Type consistency: Bash variable names use `AUTOSHIP_DISCORD_WEBHOOK_URL` consistently with the notifier.
