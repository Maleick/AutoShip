# PR #2423 AutoShip Result

## Diagnosis
- Wiki-gate actually PASSED in run 24868974527: "Wiki update detected alongside source changes."
- PR gate failure was a flaky Rust tarpaulin infra issue (zerocopy dep-info parse error), unrelated to source or docs changes.

## Actions
Addressed actionable Copilot + Codex inline review comments on `scripts/export_ghidra_patterns.py`:
- Docstring documents `address` as optional (emits `expected_preferred: null` when absent).
- `_parse_addr` accepts bare hex strings like `14028E0F0` (falls back to base-16 when base-0 parse fails).
- Added `_normalize_module` (EqGame/EqMain/EqGraphics) and `_normalize_category` (Function/Global) with case-insensitive matching and clear errors.
- argparse `--module` uses `choices=VALID_MODULES`.
- Wiki page updated to note optional `address`, bare-hex support, and module/category normalization.

## Verification
- `pytest tests/test_export_ghidra_patterns.py`: 4 passed
- Commit pushed to `autoship/issue-763` (HEAD of PR #2423)
