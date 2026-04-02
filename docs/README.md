# DMFT Documentation Index

This directory stores project documentation and is split into distinct buckets.

## Canonical split

- `docs/wiki/`
  - Canonical published operator/developer documentation.
  - Managed by `scripts/sync_wiki.py` and published by the `wiki-nightly` workflow.
  - Keep this directory flat (no nested markdown folders), matching the wiki sync contract.
- `docs/research-*`, `docs/external-research/`, `docs/research-imports/`
  - Evidence, references, investigation notes, and historical snapshots.
  - Examples: `docs/research-imports/`, `docs/external-research/`, and similarly named markdown files at the repo root.
- `docs/knowledge/` (optional)
  - Internal notes and working memory that should remain repo-local and not part of the wiki export.
  - If this folder is not present yet, create it only when needed.

## Other top-level docs

- Design, audits, and architecture notes: `docs/implementation-roadmap.md`, `docs/orchestration-design.md`, `docs/roadmap-review.md`, etc.
- Ops/process references and runbooks: `docs/frostreaver-farming-guide.md`, `docs/remote-control-setup.md`, etc.
- Root markdown files that do not belong to the wiki can be linked directly from this index as needed.

## Suggested historical snapshot convention

For periodic full documentation snapshots, use a date-stamped archive folder:

- `docs/archive/YYYY-MM-DD/`

Keep this optional, and only publish snapshots intentionally.

## Tooling references

- Wiki source sync: `scripts/sync_wiki.py`
- CI + release workflow for publishing nightly docs: `.github/workflows/wiki-nightly.yml`
