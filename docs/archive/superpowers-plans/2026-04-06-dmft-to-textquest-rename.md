# TextQuest to TextQuest Rename — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the entire project from TextQuest to TextQuest — crate directories, package names, Rust imports, CI workflows, documentation, and scripts.

**Architecture:** Four crates rename in lockstep: `textquest` -> `textquest`, `textquest-common` -> `textquest-common`, `textquest-dll` -> `textquest-dll`, `textquest-web` -> `textquest-web`. Directory renames via `git mv`, then bulk `sed` for imports and references. Build verification after each major phase.

**Tech Stack:** Rust (Cargo workspace), GitHub Actions, shell/Python scripts, Markdown docs

**Scope:** ~853 Rust import references across ~90 .rs files, 11 CI workflow files, 50+ docs, 20+ scripts. Does NOT include GitHub repo rename or Frostreaver runner label update (those are manual ops after merge).

---

## Task 1: Rename Crate Directories

**Files:**

- Rename: `dmft/` -> `textquest/`
- Rename: `textquest-common/` -> `textquest-common/`
- Rename: `textquest-dll/` -> `textquest-dll/`
- Rename: `textquest-web/` -> `textquest-web/`

- [ ] **Step 1: Rename all four crate directories with git mv**

```bash
git mv dmft textquest
git mv textquest-common textquest-common
git mv textquest-dll textquest-dll
git mv textquest-web textquest-web
```

- [ ] **Step 2: Verify git tracks the renames**

Run: `git status`
Expected: Four renames shown, no untracked files

---

## Task 2: Update Cargo.toml Files

**Files:**

- Modify: `Cargo.toml` (workspace root)
- Modify: `textquest/Cargo.toml`
- Modify: `textquest-common/Cargo.toml`
- Modify: `textquest-dll/Cargo.toml`
- Modify: `textquest-web/Cargo.toml`

- [ ] **Step 1: Update workspace root Cargo.toml**

Change:

```toml
members = ["dmft", "textquest-dll", "textquest-common", "textquest-web"]
```

To:

```toml
members = ["textquest", "textquest-dll", "textquest-common", "textquest-web"]
```

- [ ] **Step 2: Update textquest/Cargo.toml**

Change `name = "dmft"` to `name = "textquest"`.
Change `textquest-common = { path = "../textquest-common" }` to `textquest-common = { path = "../textquest-common" }`.
Update description from "External process memory reader and multibox controller for EverQuest (TLP 36-box)" to "TextQuest — EverQuest multibox controller and automation platform".

- [ ] **Step 3: Update textquest-common/Cargo.toml**

Change `name = "textquest-common"` to `name = "textquest-common"`.

- [ ] **Step 4: Update textquest-dll/Cargo.toml**

Change `name = "textquest-dll"` to `name = "textquest-dll"`.
Change `textquest-common = { path = "../textquest-common" }` to `textquest-common = { path = "../textquest-common" }`.

- [ ] **Step 5: Update textquest-web/Cargo.toml**

Change `name = "textquest-web"` to `name = "textquest-web"`.
Change `textquest-common = { path = "../textquest-common" }` to `textquest-common = { path = "../textquest-common" }`.

- [ ] **Step 6: Verify Cargo resolves the workspace**

Run: `cargo metadata --format-version=1 | python3 -c "import sys,json; pkgs=[p['name'] for p in json.load(sys.stdin)['packages'] if 'textquest' in p['name']]; print(pkgs); assert len(pkgs)==4"`
Expected: Four textquest-prefixed packages listed, no errors

---

## Task 3: Bulk Rename Rust Imports

**Files:** ~90 .rs files across all four crates

The Rust import rename is mechanical. Crate names use underscores in code:

- `textquest_common` -> `textquest_common`
- `textquest_dll` -> `textquest_dll`

These appear as `use textquest_common::`, `textquest_common::`, `extern crate textquest_common`, etc.

- [ ] **Step 1: Replace `textquest_common` with `textquest_common` in all .rs files**

```bash
find textquest textquest-common textquest-dll textquest-web -name '*.rs' -exec \
  sed -i '' 's/textquest_common/textquest_common/g' {} +
```

- [ ] **Step 2: Replace `textquest_dll` with `textquest_dll` in all .rs files**

```bash
find textquest textquest-common textquest-dll textquest-web -name '*.rs' -exec \
  sed -i '' 's/textquest_dll/textquest_dll/g' {} +
```

- [ ] **Step 3: Verify no stale `textquest_common` or `textquest_dll` references remain**

Run: `grep -rn 'textquest_common\|textquest_dll' --include='*.rs' textquest textquest-common textquest-dll textquest-web | wc -l`
Expected: 0

- [ ] **Step 4: Build the workspace to verify all imports resolve**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 5: Run the full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: All ~1250 tests pass

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: rename crates from dmft to textquest

Renames all four workspace crates:
- dmft -> textquest
- textquest-common -> textquest-common
- textquest-dll -> textquest-dll
- textquest-web -> textquest-web

Updates all Cargo.toml files and ~850 Rust import statements."
```

---

## Task 4: Update CI/CD Workflows

**Files:**

- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/nightly-release.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/wiki-nightly.yml`
- Modify: `.github/workflows/readme-metrics.yml`
- Modify: `.github/workflows/claude-agent.yml` (if it references TextQuest)
- Modify: `.github/workflows/copilot-ci-dispatch.yml` (check for references)
- Modify: `.github/workflows/post-merge-sync.yml` (check for references)

Two categories of changes:

**Binary names** (build output):

- `textquest.exe` -> `textquest.exe`
- `textquest_dll.dll` -> `textquest_dll.dll`

**Runner labels:**

- `[self-hosted, Windows, X64, dmft]` -> `[self-hosted, Windows, X64, textquest]`

**Artifact names:**

- `dmft-nightly-${{ github.run_number }}` -> `textquest-nightly-${{ github.run_number }}`

- [ ] **Step 1: Bulk replace dmft binary/artifact references in workflow files**

```bash
find .github/workflows -name '*.yml' -exec sed -i '' \
  -e 's/textquest_dll\.dll/textquest_dll.dll/g' \
  -e 's/dmft\.exe/textquest.exe/g' \
  -e 's/dmft-nightly/textquest-nightly/g' \
  -e 's/\[self-hosted, Windows, X64, dmft\]/[self-hosted, Windows, X64, textquest]/g' \
  {} +
```

- [ ] **Step 2: Check for remaining `textquest` references in workflows (case-insensitive)**

Run: `grep -rni 'dmft' .github/workflows/`
Expected: Zero results, or only historical/comment references that don't affect behavior. If any remain in system prompts or comments mentioning "TextQuest" as a project name, update those to "TextQuest".

- [ ] **Step 3: Manually review release.yml and nightly-release.yml**

Read both files to verify binary paths, artifact names, and copy commands are consistent after the sed pass.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/
git commit -m "ci: update workflow references from dmft to textquest

Binary names, runner labels, and artifact names updated across all
workflow files to match the textquest crate rename."
```

---

## Task 5: Update Scripts

**Files:** ~20 scripts in `scripts/` directory

- [ ] **Step 1: Bulk replace dmft references in scripts**

```bash
find scripts -type f \( -name '*.sh' -o -name '*.ps1' -o -name '*.bat' -o -name '*.py' \) -exec sed -i '' \
  -e 's/textquest_dll/textquest_dll/g' \
  -e 's/textquest-dll/textquest-dll/g' \
  -e 's/textquest-common/textquest-common/g' \
  -e 's/textquest_common/textquest_common/g' \
  -e 's/dmft\.exe/textquest.exe/g' \
  -e 's/textquest-web/textquest-web/g' \
  -e 's/textquest_web/textquest_web/g' \
  -e 's/TextQuest/TextQuest/g' \
  -e 's/dmft/textquest/g' \
  {} +
```

Note: The order matters. Specific patterns (textquest_dll, textquest-common) must come before the generic `textquest` replacement to avoid double-replacing.

- [ ] **Step 2: Verify no stale dmft references in scripts**

Run: `grep -rni 'dmft' scripts/ --include='*.sh' --include='*.ps1' --include='*.bat' --include='*.py' | grep -v '__pycache__'`
Expected: Zero results

- [ ] **Step 3: Commit**

```bash
git add scripts/
git commit -m "chore: update script references from dmft to textquest"
```

---

## Task 6: Update Documentation

**Files:** 50+ tracked markdown files in `docs/`, `README.md`, `CLAUDE.md`, `AGENTS.md`, `HANDOFF.md`, `REVIEW.md` (local-only tooling files excluded)

- [ ] **Step 1: Bulk replace in all markdown files (project root + docs/)**

```bash
# Specific patterns first, then generic
find . -maxdepth 1 -name '*.md' -exec sed -i '' \
  -e 's/textquest_dll/textquest_dll/g' \
  -e 's/textquest-dll/textquest-dll/g' \
  -e 's/textquest-common/textquest-common/g' \
  -e 's/textquest_common/textquest_common/g' \
  -e 's/textquest-web/textquest-web/g' \
  -e 's/textquest_web/textquest_web/g' \
  -e 's/TextQuest/TextQuest/g' \
  -e 's/`textquest`/`textquest`/g' \
  -e 's|textquest/src|textquest/src|g' \
  -e 's|textquest/tests|textquest/tests|g' \
  {} +

find docs -name '*.md' -exec sed -i '' \
  -e 's/textquest_dll/textquest_dll/g' \
  -e 's/textquest-dll/textquest-dll/g' \
  -e 's/textquest-common/textquest-common/g' \
  -e 's/textquest_common/textquest_common/g' \
  -e 's/textquest-web/textquest-web/g' \
  -e 's/textquest_web/textquest_web/g' \
  -e 's/TextQuest/TextQuest/g' \
  -e 's/`textquest`/`textquest`/g' \
  -e 's|textquest/src|textquest/src|g' \
  -e 's|textquest/tests|textquest/tests|g' \
  {} +
```

- [ ] **Step 2: Update GitHub issue template**

```bash
sed -i '' 's/TextQuest/TextQuest/g' .github/ISSUE_TEMPLATE/agent-task.yml
```

- [ ] **Step 3: Update feature-list.json**

```bash
sed -i '' -e 's/TextQuest/TextQuest/g' -e 's/dmft/textquest/g' feature-list.json
```

- [ ] **Step 4: Spot-check key files for correctness**

Read `CLAUDE.md`, `README.md`, and `AGENTS.md` to verify the replacements didn't produce nonsensical text (e.g., "Dave Mike Fun Times" should become "TextQuest" in the description, not be left behind).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: rename all TextQuest references to TextQuest

Updates README, CLAUDE.md, AGENTS.md, docs/, issue templates,
and feature-list.json."
```

---

## Task 7: Update config/frostreaver.toml

**Files:**

- Modify: `config/frostreaver.toml` (if it contains dmft references)

- [ ] **Step 1: Check for dmft references**

Run: `grep -ni 'dmft' config/frostreaver.toml`

If references exist, replace them. If none, skip this task.

- [ ] **Step 2: Commit if changed**

```bash
git add config/
git commit -m "config: update frostreaver.toml references to textquest"
```

---

## Task 8: Final Verification

- [ ] **Step 1: Full grep for any remaining dmft references**

```bash
grep -rni 'dmft' --include='*.rs' --include='*.toml' --include='*.yml' --include='*.md' --include='*.py' --include='*.sh' --include='*.ps1' --include='*.bat' --include='*.json' \
  --exclude-dir='.git' --exclude-dir='target' --exclude-dir='third_party' --exclude-dir='__pycache__' --exclude-dir='.claude' \
  . | grep -v 'docs/superpowers/plans/2026-04-06' | head -50
```

Expected: Zero results outside of the plan file itself, third_party submodules, and .claude memory files. Any remaining references should be evaluated and fixed.

- [ ] **Step 2: Full build**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: All ~1250 tests pass

- [ ] **Step 4: Clippy**

Run: `cargo clippy 2>&1 | tail -10`
Expected: No warnings related to the rename

- [ ] **Step 5: Format check**

Run: `cargo fmt --check`
Expected: Clean

---

## Post-Merge Manual Steps (NOT automated)

These require human action after the code PR merges:

1. **GitHub repo rename:** Settings > General > Repository name: `TextQuest` -> `TextQuest`
2. **Frostreaver runner labels:** Update the self-hosted runner labels from `textquest` to `textquest` in GitHub Actions runner config on Frostreaver
3. **Local directory rename:** `mv ~/Projects/TextQuest ~/Projects/TextQuest`
4. **Claude Code memory:** The path-based memory at `~/.claude/projects/-Users-maleick-Projects-TextQuest/` will need to be migrated to the new path. Easiest approach: start a fresh session after the directory rename and let it rebuild, or `cp -r` the memory directory to match the new path.
5. **Wiki repo:** If the wiki is a separate repo, it may need its own rename pass
6. **Discord bot config:** Update any hardcoded paths referencing TextQuest
