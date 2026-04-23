# AutoShip OpenCode Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route AutoShip issue dispatches through OpenCode Go and OpenCode Zen free model pools instead of the old fixed `haiku` / `sonnet` / `opus` defaults.

**Architecture:** Keep the routing logic in the autoship dispatch hook and make the actual model names data-driven via a small JSON config file under `~/.config/opencode/.autoship/`. Simple issues will rotate through the Zen-free pool, medium issues will rotate through the Go pool, and complex issues will keep a stronger fallback pool. The dispatcher will still honor an explicit model override argument.

**Tech Stack:** Bash, `jq`, OpenCode CLI, existing AutoShip hook scripts.

---

## File Map

| Task | File(s) Modified |
| ---- | ---------------- |
| T1 | `~/.config/opencode/.autoship/hooks/init.sh`, `~/.config/opencode/.autoship/model-routing.json` |
| T2 | `~/.config/opencode/.autoship/hooks/dispatch.sh` |
| T3 | `docs/wiki/FAQ.md` |

---

## Task 1: Add Routing Config and Defaults

**Files:**

- Modify: `~/.config/opencode/.autoship/hooks/init.sh`
- Add: `~/.config/opencode/.autoship/model-routing.json`

**What changes:**
Create a default routing config with three pools:

- `zen_free`: `opencode/ling-2.6-flash-free`, `opencode/minimax-m2.5-free`, `opencode/nemotron-3-super-free`
- `go`: `opencode-go/glm-5.1`, `opencode-go/qwen3.6-plus`, `opencode-go/mimo-v2.5-pro`
- `premium`: `opencode/gpt-5.3-codex-spark`, `opencode/big-pickle`

The init hook should create the file only if it is missing so a user can edit the pools later without losing changes.

- [ ] **Step 1:** Add the JSON file with the three model pools and a `defaultFallback` value of `opencode/gpt-5.3-codex-spark`
- [ ] **Step 2:** Update `init.sh` so it seeds the routing file only when absent
- [ ] **Step 3:** Run `jq '.' ~/.config/opencode/.autoship/model-routing.json` to verify the JSON parses

## Task 2: Make Dispatch Select From Pools Automatically

**Files:**

- Modify: `~/.config/opencode/.autoship/hooks/dispatch.sh`

**What changes:**
Replace the fixed `simple -> haiku`, `medium -> sonnet`, `complex -> opus` mapping with a resolver that:

- uses an explicit override argument when provided
- otherwise maps `simple` issues to the `zen_free` pool
- maps `medium` issues to the `go` pool
- maps `complex` issues to the `premium` pool
- selects a deterministic model from the chosen pool using the issue number as the rotation index
- falls back to `defaultFallback` if the pool is missing or empty

This keeps the routing policy in code while letting the exact model names live in config.

- [ ] **Step 1:** Add a small Bash helper that reads a pool from `model-routing.json` with `jq`
- [ ] **Step 2:** Add a deterministic selection function based on `issue_number % pool_size`
- [ ] **Step 3:** Wire the existing `MODEL` variable to call the resolver before `update-state.sh`
- [ ] **Step 4:** Run `bash -n ~/.config/opencode/.autoship/hooks/dispatch.sh` and fix any syntax errors

## Task 3: Update Operator Docs

**Files:**

- Modify: `docs/wiki/FAQ.md`

**What changes:**
Update the AutoShip automation note to describe the new OpenCode-first routing, so the operator docs match the actual dispatch behavior.

- [ ] **Step 1:** Rewrite the AutoShip FAQ line to mention OpenCode Go and OpenCode Zen free pools
- [ ] **Step 2:** Keep the line short and factual so it stays readable in the FAQ list
- [ ] **Step 3:** Review the rendered diff for clarity and consistency

## Verification

- [ ] `bash -n ~/.config/opencode/.autoship/hooks/init.sh`
- [ ] `bash -n ~/.config/opencode/.autoship/hooks/dispatch.sh`
- [ ] `jq '.' ~/.config/opencode/.autoship/model-routing.json`
- [ ] `opencode models opencode-go | sed -n '1,20p'`
- [ ] `opencode models openrouter | grep ':free' | sed -n '1,20p'`
