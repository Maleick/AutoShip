# Result: #1114 — Create map file format validator tool

Status: DONE

Changes Made:
- Rebuilt `scripts/validate-maps.py` as a directory-aware Brewall map validator with finite coordinate checks, u8 color validation, P/L field-count validation, per-file line counts, bounds, and actionable line-numbered errors.
- Wired `scripts/dev-preflight.py` to call the validator as the canonical map validation step.
- Added a dedicated GitHub Actions `validate_maps` job that publishes validator output to the PR step summary.
- Expanded `tests/test_validate_maps.py` to cover existing maps, known invalid map cases, NaN/Inf, invalid floats, color range errors, wrong field counts, bounds, and CLI output.

Tests:
- `python3 scripts/validate-maps.py config/maps` — pass, 36 valid files.
- `python3 -m unittest tests/test_validate_maps.py -v` — pass, 23 tests.
- `python3 scripts/dev-preflight.py --env-only` — pass, 11 checks.
- `cargo check` — pass.
- `git diff --check` — pass.
- `/verify` — not available as an executable in this shell.

Notes:
- Full `cargo test` was not run per issue instruction to use `cargo check` only and skip cargo test unless a single file.

COMPLETE
