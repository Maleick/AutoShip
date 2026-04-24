# AutoShip Slogan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved slogan `Turn backlog into reviewed PRs.` consistently across AutoShip public messaging.

**Architecture:** This is a documentation and static asset update. Keep the message layer consistent across README, GitHub Pages, banner SVGs, Wiki, and repository metadata while preserving the existing OpenCode-only technical details.

**Tech Stack:** Markdown, static HTML/CSS, SVG, GitHub CLI, Bash verification hooks.

---

## File Structure

- Modify: `README.md` for the hero slogan, opening description, and alt text.
- Modify: `assets/autoship-banner.svg` for the primary graphic slogan and accessible title/description.
- Modify: `docs/assets/autoship-banner.svg` by syncing it to match `assets/autoship-banner.svg`.
- Modify: `docs/index.html` for title/meta/Open Graph copy, hero copy, stale worker count, and merge-overpromise copy.
- Modify: `wiki/Home.md` for the Wiki landing slogan and supporting line.
- Update externally: GitHub repository About description with `gh repo edit`.
- Verify: run repository policy/smoke tests and targeted stale-copy scans.

Do not commit during implementation unless the user explicitly asks for a commit.

## Task 1: README Messaging

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update hero alt text and centered slogan**

Change the banner `alt` text to include the new slogan:

```html
<img src="assets/autoship-banner.svg" width="900" alt="AutoShip — Turn backlog into reviewed PRs" />
```

Replace the centered slogan paragraph with:

```html
<p align="center"><strong>Turn backlog into reviewed PRs.</strong></p>
```

- [ ] **Step 2: Update the opening description**

Replace the current one-line product description with:

```markdown
AutoShip is the OpenCode plugin for solo maintainers who want their GitHub issue queue planned, routed, verified, and packaged into pull requests without babysitting every worker.
```

- [ ] **Step 3: Verify README contains the exact slogan once in hero copy**

Run: `rg "Turn backlog into reviewed PRs|babysitting every worker" README.md`

Expected: output includes the hero slogan and supporting line.

## Task 2: Banner Graphics

**Files:**
- Modify: `assets/autoship-banner.svg`
- Modify: `docs/assets/autoship-banner.svg`

- [ ] **Step 1: Update SVG accessibility text**

In `assets/autoship-banner.svg`, set:

```xml
<title id="title">AutoShip turns backlog into reviewed PRs</title>
<desc id="desc">AutoShip is an OpenCode plugin that plans GitHub issues, dispatches workers, verifies results, and packages reviewed pull requests.</desc>
```

- [ ] **Step 2: Update visible banner copy**

Replace the secondary and tertiary text lines with:

```xml
<text x="74" y="166" font-family="Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif" font-size="32" font-weight="800" fill="#cbd5e1">Turn backlog into reviewed PRs.</text>
<text x="76" y="220" font-family="Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif" font-size="24" fill="#fbbf24">OpenCode plans • 15 workers ship • GPT-5.5 verifies</text>
```

- [ ] **Step 3: Sync Pages banner**

Run: `cp assets/autoship-banner.svg docs/assets/autoship-banner.svg`

Expected: no output.

- [ ] **Step 4: Verify banners match**

Run: `cmp -s assets/autoship-banner.svg docs/assets/autoship-banner.svg && printf 'banners match\n'`

Expected: `banners match`.

## Task 3: GitHub Pages Copy

**Files:**
- Modify: `docs/index.html`

- [ ] **Step 1: Update page title and metadata**

Set these head values:

```html
<title>AutoShip — turn backlog into reviewed PRs</title>
<meta name="description" content="AutoShip is an OpenCode plugin for solo maintainers that plans GitHub issues, routes OpenCode workers, verifies results, and packages reviewed pull requests." />
<meta property="og:title" content="AutoShip — turn backlog into reviewed PRs" />
<meta property="og:description" content="Turn backlog into reviewed PRs with OpenCode issue planning, worker routing, and verification." />
```

- [ ] **Step 2: Update hero image alt and hero paragraph**

Set hero image alt text to:

```html
alt="AutoShip — turn backlog into reviewed PRs"
```

Replace the hero paragraph with:

```html
<p class="hero-desc">
  <strong>Turn backlog into reviewed PRs.</strong> AutoShip is the OpenCode plugin for solo maintainers
  who want their GitHub issue queue planned, routed, verified, and packaged into pull requests without
  babysitting every worker.
</p>
```

- [ ] **Step 3: Fix stale stats and command copy**

Change the active worker stat from `10` to `15`.

Change the example command from `/autoship:start` to `/autoship`.

- [ ] **Step 4: Remove hero-level merge overpromise**

Replace the With AutoShip paragraph with:

```html
AutoShip reads your issues, classifies each one, picks a configured OpenCode model,
creates an isolated worktree, dispatches with a focused prompt,
verifies the result, and opens a reviewed PR for the work that passed.
```

Replace `You come back to merged PRs.` with:

```html
You come back to reviewed PRs ready for final decision.
```

- [ ] **Step 5: Update CTA copy to match the approved promise**

Replace the CTA heading and subcopy with:

```html
<h2 style="font-size:2rem;">Ready to <span style="color:var(--amber);">turn backlog into reviewed PRs?</span></h2>
<p style="color:var(--muted);max-width:520px;margin:16px auto 36px;font-size:1.05rem;">One install. One command. AutoShip plans, routes, verifies, and packages the PRs.</p>
```

## Task 4: Wiki Messaging

**Files:**
- Modify: `wiki/Home.md`

- [ ] **Step 1: Update image alt text and introduction**

Replace the top image alt text and intro with:

```markdown
![AutoShip — Turn backlog into reviewed PRs](../assets/autoship-banner.svg)

**Turn backlog into reviewed PRs.**

AutoShip is the OpenCode plugin for solo maintainers who want their GitHub issue queue planned, routed, verified, and packaged into pull requests without babysitting every worker.
```

- [ ] **Step 2: Verify Wiki home has the exact slogan**

Run: `rg "Turn backlog into reviewed PRs|babysitting every worker" wiki/Home.md`

Expected: output includes both phrases.

## Task 5: Repository About Metadata

**Files:**
- External GitHub repository metadata

- [ ] **Step 1: Update GitHub About description**

Run:

```bash
gh repo edit Maleick/AutoShip --description "Turn backlog into reviewed PRs with an OpenCode issue-to-PR orchestration plugin."
```

Expected: no output on success.

- [ ] **Step 2: Verify GitHub About description**

Run:

```bash
gh repo view Maleick/AutoShip --json description,homepageUrl --jq '.description + "\n" + .homepageUrl'
```

Expected output includes:

```text
Turn backlog into reviewed PRs with an OpenCode issue-to-PR orchestration plugin.
https://autoship.teamoperator.red
```

## Task 6: Final Verification

**Files:**
- Verify all changed public-facing files

- [ ] **Step 1: Run policy and smoke verification**

Run:

```bash
bash hooks/opencode/test-policy.sh
bash -n hooks/opencode/*.sh hooks/*.sh
bash hooks/opencode/smoke-test.sh
```

Expected output includes:

```text
OpenCode policy tests passed
OpenCode install smoke test passed
```

- [ ] **Step 2: Run targeted slogan and stale-copy scan**

Run:

```bash
rg "Turn backlog into reviewed PRs|10 default active workers|/autoship:start|You come back to merged PRs|merges them" README.md docs/index.html assets/autoship-banner.svg docs/assets/autoship-banner.svg wiki/Home.md
```

Expected: slogan matches appear. Stale strings `10 default active workers`, `/autoship:start`, `You come back to merged PRs`, and broad hero/meta `merges them` do not appear.

- [ ] **Step 3: Review git diff**

Run: `git diff -- README.md assets/autoship-banner.svg docs/assets/autoship-banner.svg docs/index.html wiki/Home.md docs/superpowers/specs/2026-04-24-autoship-slogan-design.md docs/superpowers/plans/2026-04-24-autoship-slogan.md`

Expected: diff only contains the approved slogan, supporting line, factual copy fixes, spec, and this plan.

## Self-Review

- Spec coverage: all requested surfaces are covered by Tasks 1-5, with final verification in Task 6.
- Placeholder scan: no `TBD`, `TODO`, or unspecified implementation steps remain.
- Consistency: the exact slogan and supporting line are reused across README, Pages, SVG, Wiki, and repository metadata.
