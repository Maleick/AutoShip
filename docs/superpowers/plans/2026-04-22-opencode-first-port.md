# AutoShip OpenCode-First Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make AutoShip install and run as an OpenCode-only plugin in the same repository.

**Architecture:** The repo remains the single source of truth. OpenCode becomes the only install/runtime path via a repo-local plugin entrypoint, repo-root path resolution, and an OpenCode bootstrap script that installs the plugin into `~/.config/opencode`. Public docs and GitHub Pages content must say OpenCode is the only worker runtime.

**Tech Stack:** Bash hooks, OpenCode config, TypeScript plugin modules, Markdown docs, HTML site content, GitHub CLI.

---

## File Map

- Create: `plugins/autoship.ts`
- Create: `hooks/opencode/install.sh`
- Modify: `hooks/opencode/init.sh`
- Modify: `hooks/opencode/classify-issue.sh`
- Modify: `hooks/opencode/monitor-agents.sh`
- Modify: `hooks/opencode/cleanup-worktree.sh`
- Modify: `commands/start.md`
- Modify: `commands/plan.md`
- Modify: `commands/status.md`
- Modify: `commands/stop.md`
- Modify: `commands/setup.md`
- Modify: `skills/orchestrate/SKILL.md`
- Modify: `skills/dispatch/SKILL.md`
- Modify: `skills/verify/SKILL.md`
- Modify: `skills/status/SKILL.md`
- Modify: `skills/poll/SKILL.md`
- Modify: `skills/setup/SKILL.md`
- Modify: `README.md`
- Modify: `docs/index.html`
- Modify: `docs/OPENCODE_INSTALL.md`
- Modify: `docs/OPENCODE_PORT_SPEC.md`
- Modify: `wiki/Home.md`
- Modify: `wiki/Architecture.md`
- Modify: `wiki/Configuration.md`
- Modify: `wiki/Troubleshooting.md`
- Modify: `AGENTS.md`
- Modify: `AUTOSHIP.md`
- Modify: `AUTOSHIP_ARCHITECTURE.md`
- Modify: `CHANGELOG.md`

---

### Task 1: Add the OpenCode plugin entrypoint and installer

**Files:**
- Create: `plugins/autoship.ts`
- Create: `hooks/opencode/install.sh`
- Modify: `hooks/opencode/init.sh`
- Modify: `README.md`
- Modify: `docs/OPENCODE_INSTALL.md`

- [ ] **Step 1: Add the repo-local OpenCode plugin module**

Create `plugins/autoship.ts` as the canonical OpenCode plugin entrypoint. It should resolve the repository root from the module location, expose the AutoShip version, and export the OpenCode-facing metadata/hooks needed for install-time registration.

```ts
const repoRoot = new URL("../", import.meta.url).pathname

export const autoshipPlugin = {
  name: "autoship",
  version: "1.5.0-opencode",
  repoRoot,
}
```

- [ ] **Step 2: Add an OpenCode installer script**

Create `hooks/opencode/install.sh` to:
- validate `jq`, `gh`, and a writable `~/.config/opencode`
- symlink or copy `plugins/autoship.ts` into `~/.config/opencode/plugins/`
- add `file:///Users/maleick/.config/opencode/plugins/autoship.ts` to the `plugin` array in `~/.config/opencode/opencode.json` if missing
- run `hooks/opencode/init.sh` after install

Use the repo-local plugin file as the source of truth; do not fetch from external plugin cache paths.

- [ ] **Step 3: Make the bootstrap path OpenCode-native**

Update `hooks/opencode/init.sh` so it writes state for OpenCode and records the repo-root hooks directory. Keep the existing `.autoship/` layout, but remove any dependency on external cache paths for locating AutoShip scripts.

- [ ] **Step 4: Document install and bootstrap**

Update `README.md` and `docs/OPENCODE_INSTALL.md` so the first install path is OpenCode-native. The docs should instruct users to run the AutoShip OpenCode installer/bootstrap path from a repo checkout in `~/Projects/<repo>`.

- [ ] **Step 5: Verify the install path**

Run:
```bash
bash hooks/opencode/install.sh
bash hooks/opencode/init.sh
```

Expected:
- `.autoship/state.json` exists
- `.autoship/hooks_dir` points at the repo hooks directory
- `~/.config/opencode/opencode.json` references the AutoShip plugin file

---

### Task 2: Remove external cache assumptions from OpenCode runtime paths

**Files:**
- Modify: `commands/start.md`
- Modify: `commands/plan.md`
- Modify: `commands/status.md`
- Modify: `commands/stop.md`
- Modify: `commands/setup.md`
- Modify: `skills/orchestrate/SKILL.md`
- Modify: `skills/dispatch/SKILL.md`
- Modify: `skills/verify/SKILL.md`
- Modify: `skills/status/SKILL.md`
- Modify: `skills/poll/SKILL.md`
- Modify: `skills/setup/SKILL.md`
- Modify: `hooks/opencode/classify-issue.sh`
- Modify: `hooks/opencode/monitor-agents.sh`
- Modify: `hooks/opencode/cleanup-worktree.sh`
- Modify: `AUTOSHIP.md`

- [ ] **Step 1: Replace cache-path lookups with repo-root resolution**

Search and replace all OpenCode-facing external cache references with either:
- the repo root returned by `git rev-parse --show-toplevel`, or
- the absolute hooks directory stored in `.autoship/hooks_dir`

Use this rule consistently in the command docs, skills, and hooks.

- [ ] **Step 2: Keep runtime state in `.autoship/` only**

Make sure the OpenCode runtime still writes `state.json`, `event-queue.json`, `quota.json`, and `token-ledger.json` inside the repo-local `.autoship/` directory. Do not move state into `~/.config/opencode`.

- [ ] **Step 3: Align all start/stop/plan/status commands**

Update the command files so they describe the OpenCode workflow and use the repo-local paths produced by the installer. The commands should no longer mention plugin cache lookups as the source of truth.

- [ ] **Step 4: Re-check the hooks**

Ensure `classify-issue.sh`, `monitor-agents.sh`, and `cleanup-worktree.sh` all derive paths from the repo checkout or `.autoship/hooks_dir` and do not assume external installation directories.

- [ ] **Step 5: Verify with a cache-path search**

Run:
```bash
grep -R "plugin cache" README.md AUTOSHIP.md AGENTS.md commands skills hooks docs wiki
```

Expected:
- no matches in OpenCode-facing docs or scripts

---

### Task 3: Update docs and GitHub Pages for OpenCode-first messaging

**Files:**
- Modify: `README.md`
- Modify: `docs/index.html`
- Modify: `docs/OPENCODE_INSTALL.md`
- Modify: `docs/OPENCODE_PORT_SPEC.md`
- Modify: `wiki/Home.md`
- Modify: `wiki/Architecture.md`
- Modify: `wiki/Configuration.md`
- Modify: `wiki/Troubleshooting.md`
- Modify: `AGENTS.md`
- Modify: `AUTOSHIP_ARCHITECTURE.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Rewrite the README install story**

Make `README.md` open with OpenCode-only positioning. Update the install snippets and platform bullets to match the new default.

- [ ] **Step 2: Update the public site content**

Change `docs/index.html` so the hero text, badges, install command, and meta description describe OpenCode-only AutoShip. Keep the page layout, but replace the install CTA and copy to point at the OpenCode install path.

- [ ] **Step 3: Refresh the port and wiki docs**

Update `docs/OPENCODE_INSTALL.md`, `docs/OPENCODE_PORT_SPEC.md`, and the `wiki/*.md` pages so they explain the same OpenCode-first install flow, state layout, and troubleshooting steps.

- [ ] **Step 4: Update operator docs**

Keep `AGENTS.md` in sync with the new OpenCode install path.

- [ ] **Step 5: Verify the public-facing text**

Search for legacy runtime marketing language and update it where it conflicts with the OpenCode-only story.

Run:
```bash
grep -R "legacy worker runtime" README.md docs wiki AGENTS.md AUTOSHIP_ARCHITECTURE.md CHANGELOG.md
```

Expected:
- no public docs advertise non-OpenCode worker runtimes

---

### Task 4: Verify the OpenCode-first install flow end to end

**Files:**
- No new files; verify the updated repo state

- [ ] **Step 1: Run the installer and bootstrap scripts**

Run:
```bash
bash hooks/opencode/install.sh
bash hooks/opencode/init.sh
```

Expected:
- install succeeds from `~/Projects/<repo>`
- OpenCode config includes the AutoShip plugin entry
- `.autoship/state.json` and `.autoship/hooks_dir` are present

- [ ] **Step 2: Run the OpenCode-facing smoke checks**

Run:
```bash
bash hooks/opencode/classify-issue.sh 1
bash hooks/opencode/monitor-agents.sh
```

Expected:
- each script exits cleanly
- each script resolves paths from the repo checkout or `.autoship/hooks_dir`

- [ ] **Step 3: Confirm the docs/site are aligned**

Check that `README.md`, `docs/index.html`, and `docs/OPENCODE_INSTALL.md` all say OpenCode is the default and point to the same install story.

- [ ] **Step 4: Re-run the two repository searches**

Run:
```bash
grep -R "plugin cache" README.md AUTOSHIP.md AGENTS.md commands skills hooks docs wiki
grep -R "legacy worker runtime" README.md docs wiki AGENTS.md AUTOSHIP_ARCHITECTURE.md CHANGELOG.md
```

Expected:
- no OpenCode path still depends on external cache locations
- public docs do not advertise non-OpenCode worker runtimes

- [ ] **Step 5: Commit-ready check**

Run:
```bash
git status --short
git diff --stat
```

Expected:
- only the intended OpenCode-first files are modified
- no `.autoship/` runtime residue is staged or left dirty
