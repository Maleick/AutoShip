# Result: #1239 — CI/CD: Set up code coverage tracking
Status: DONE

- Added tarpaulin coverage enforcement to CI at 80% (`.github/workflows/ci.yml`).
- Updated `scripts/coverage-report.py` to support `--html` and `--xml` report outputs and set default threshold to 80%.
- Added HTML/XML artifact staging and upload in CI for accessible reports.
- Added optional Codecov upload step (`optional` via non-blocking failure behavior).
- Added a coverage badge to `README.md`.
- Ran `cargo check` successfully.
