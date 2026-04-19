# Repo Audit — 2026-04-13

This audit summarizes a local-repo consistency pass against roadmap docs, RedGuides/MacroQuest comparison docs, issue index hygiene, PR-process evidence in-repo, and unit-test coverage signals.

## Scope and constraints

- Audited from the local checkout only (`/workspace/TextQuest`).
- Direct GitHub PR comment/thread inspection was **not possible** in this environment (`gh` CLI unavailable and outbound GitHub API blocked by proxy tunnel `403`).
- Because of that, this report flags where GitHub-side verification is still required.

## 1) Roadmap vs RedGuides/MacroQuest docs

### Findings

1. The roadmap page says active milestone execution resumes at **M5–M11**, with explicit live-validation gaps still open. This indicates ongoing/incomplete validation work in core areas.
2. The MQ2 comparison page states "M1–M5 complete" in its generated header language. That claim is stronger than the roadmap's current "needs live proof" framing and can be read as inconsistent status signaling.
3. RedGuides research content is useful and detailed, but should be treated as external research/reference evidence rather than milestone completion proof.

### Recommendation

- Normalize milestone-status language across research/comparison docs so they cannot be interpreted as milestone completion claims unless backed by live validation criteria from the canonical roadmap.

## 2) Redundant issue references in local issue index docs

### Findings

- `docs/ISSUE_INDEX.md` currently has one duplicate issue ID with two different task titles:
  - `#1105` appears as both "Drag system testing and documentation" and "Implement DirectX overlay hook".
- This is a local-doc hygiene conflict that should be reconciled with canonical GitHub issue metadata.

### Recommendation

- Resolve the duplicate by checking canonical issue history on GitHub and updating the incorrect row.
- Add an automated doc lint check for duplicate issue IDs in roadmap/index tables.

## 3) Closed PR comments / unresolved review thread coverage

### Findings

- Local docs and scripts encode policy that PRs should not merge with unresolved threads.
- Repo includes `scripts/resolve_pr_threads.py`, which implies the team already has a workflow for thread resolution.
- However, this audit could not directly verify all closed PRs and their comment resolution status due to network/tooling constraints.

### Recommendation

- Run a GitHub-side sweep from a network-enabled host:
  1. list recently closed/merged PRs,
  2. enumerate unresolved review threads,
  3. cross-check comments requesting follow-up work against linked issues.
- Store that sweep output in `docs/wiki/` as evidence for future audits.

## 4) Unit-test coverage signals (repo-local)

### Findings

- Most Rust source files include inline test markers, but a small set of implementation files have no obvious inline unit tests.
- Candidate Rust files without inline tests (excluding module glue files):
  - `textquest/src/credentials/prompt.rs`
  - `textquest/src/bin/import_ghidra.rs`
  - `textquest/src/eq/cheater.rs`
  - `textquest/src/tui/ui/explorer.rs`
  - `textquest/src/tui/ui/navigation.rs`
  - `textquest/src/tui/ui/eq_internals.rs`
  - `textquest/src/tui/ui/dashboard.rs`
  - `textquest-dll/src/hooks/movement.rs`
  - `textquest-common/src/lib.rs`
- Python script coverage is present for several automation scripts, but direct tests appear missing for:
  - `scripts/import_mq2_maps.py`
  - `scripts/sync_wiki.py`
  - `scripts/import_mq_offsets.py`
  - `scripts/resolve_pr_threads.py`
  - `scripts/winrm_exec.py`
  - `scripts/collect_patch_evidence.py`

### Recommendation

- Prioritize unit tests for logic-bearing files listed above.
- For scripts that are difficult to fully integration test, add focused parser/arg/validation unit tests first.

## 5) Action checklist

- [ ] Reconcile duplicate `#1105` entry in `docs/ISSUE_INDEX.md` against canonical GitHub issues.
- [ ] Add doc-lint test for duplicate issue IDs.
- [ ] Run GitHub closed-PR comment/thread sweep from network-enabled environment; capture artifact in `docs/wiki/`.
- [ ] Add unit tests for untested Rust/UI/scripting surfaces listed above.
- [ ] Reconcile milestone-completion wording between roadmap and MQ2 comparison docs.
