# TextQuest Documentation Index

<p align="center">
  <img src="../Art/TextQuest.jpeg" alt="TextQuest Logo" width="400">
</p>

<p align="center">
  <i>"What happens in Neriak, stays in Neriak."</i>
</p>

This directory stores project documentation and is split into distinct buckets.

## Canonical split

- `README.md`
  - Usage-first build/run/operator entry point from the repo root.
  - Keep it focused on how to use TextQuest rather than roadmap, project, or governance detail.
- `docs/wiki/`
  - Canonical markdown authoring tree for the public docs site and the repo wiki mirror.
  - Managed by `scripts/sync_wiki.py` and published through the GitHub Pages build.
  - Keep this directory flat (no nested markdown folders), matching the wiki sync contract.
- `mkdocs.yml`
  - Public docs site navigation, theme, and draft filtering for the GitHub Pages build.
- `docs/implementation-roadmap.md`
  - Canonical milestone order, evidence-state rules, and GitHub tracking guidance.
- `docs/research-*`, `docs/external-research/`, `docs/research-imports/`
  - TextQuest-owned runbooks, references, investigation notes, and historical summaries.
  - Immutable Ghidra evidence stays canonical in the sibling `TextQuest-Ghidra` repo under `snapshots/` and `baseline-selection/`; keep only lightweight references here.
  - Examples: `docs/research-imports/`, `docs/external-research/`, and similarly named markdown files at the repo root.
- `docs/knowledge/` (optional)
  - Internal notes and working memory that should remain repo-local and not part of the wiki export.
  - If this folder is not present yet, create it only when needed.

## Other top-level docs

- Design, audits, and architecture notes: `docs/implementation-roadmap.md`, `docs/orchestration-design.md`, `docs/roadmap-review.md`, etc.
- Ops/process references and runbooks: `docs/frostreaver-farming-guide.md`, `docs/remote-control-setup.md`, etc.
- Repo-external eqlib or MacroQuest trees are optional research inputs, not repo-managed setup.
- Root markdown files that do not belong to the wiki can be linked directly from this index as needed.

## Suggested historical snapshot convention

For periodic full documentation snapshots, use a date-stamped archive folder:

- `docs/archive/YYYY-MM-DD/`

Keep this optional, and only publish snapshots intentionally.

## Tooling references

- Wiki source sync: `scripts/sync_wiki.py`
- Public docs site workflow: `.github/workflows/docs-pages.yml`
- Legacy wiki mirror sync: `.github/workflows/wiki-nightly.yml`
