# Test Coverage Policy

## Overview

TextQuest maintains an active test coverage monitoring program to ensure code quality and reduce defects. All pull requests are encouraged to include test coverage for new code and changes to existing logic.

## Coverage Targets

- **New code**: Minimum 80% line coverage required for new files and major new functions
- **Modified code**: Aim for 70%+ coverage on changed logic
- **Overall repository**: Advisory target of 60%+ across all crates (not blocking)
- **Windows-specific code**: Platform-gated tests exempt from strict requirements (run on Windows CI only)

## How Coverage is Measured

Coverage is measured using [cargo-tarpaulin](https://github.com/xd009642/tarpaulin), which instruments compiled binaries and tracks execution paths.

### Run Coverage Locally

```bash
# Install tarpaulin (one-time)
cargo install cargo-tarpaulin

# Generate text report
python3 scripts/coverage-report.py

# Generate HTML report
python3 scripts/coverage-report.py --html

# Check against the advisory overall target (exit 1 if below 60%)
python3 scripts/coverage-report.py --threshold 60
```

### Coverage in CI

Pull requests trigger a **coverage job** that:

1. Runs `cargo tarpaulin` on the full workspace
2. Emits a coverage summary in the CI job logs (advisory)
3. Does NOT block merge (informational only)

The coverage job runs inside the main Linux CI gate with the same environment as the rest of the PR checks.

## Writing Testable Code

### Do

- **Unit test business logic**: Functions that calculate, filter, or transform data should have corresponding tests.
- **Mock external interfaces**: For code that reads Windows APIs or process memory, use `#[cfg(test)]` mocks.
- **Test error paths**: Include tests for failure cases and edge conditions.
- **Document test intent**: Use clear test names and comments explaining what is being validated.

### Don't

- **Don't use `unwrap()` in tests without asserting**: If a test needs to fail, use `assert!`, `expect!`, or `.unwrap_or_else()`.
- **Don't skip platform-specific tests**: Windows-only tests use `#[cfg(windows)]`; macOS/Linux stubs are acceptable for development.
- **Don't aim for 100% coverage**: Focus on meaningful coverage (happy path + error cases). Defensive panics and unreachable branches can be excluded.

## Coverage Exclusions

The following are explicitly exempt from strict coverage requirements:

1. **Platform-specific code** (`#[cfg(windows)]`): Tested on Windows CI only; macOS stubs are development aids.
2. **Panic branches**: Lines that panic due to impossible conditions or invariant violations.
3. **Debug logging**: Conditional debug/trace logging paths.
4. **Test utilities**: Helper modules used only in tests.

## PRs and Coverage Reports

When opening a pull request:

1. **New files or major changes**: Ensure new code has 80%+ coverage.
2. **Modified files**: Run `python3 scripts/coverage-report.py` locally to see impact.
3. **Coverage checklist**: Check the PR template coverage item before marking ready.

GitHub will post a coverage summary comment on PRs showing:

- Overall coverage percentage
- Change in coverage from base branch
- Uncovered lines (if HTML report is generated)

## Continuous Improvement

- **Monthly reviews**: Coverage metrics are reviewed during planning sprints.
- **Coverage trends**: Track coverage over time in `docs/dev/coverage-trends.md`.
- **Defect correlation**: Analyze defects in low-coverage areas and prioritize testing.

## Tools & References

- **Cargo Tarpaulin**: [GitHub](https://github.com/xd009642/tarpaulin) | [Docs](https://docs.rs/tarpaulin/)
- **Rust Testing Guide**: [The Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- **TextQuest CI**: See `.github/workflows/ci.yml` for coverage job configuration

## FAQ

**Q: Why is coverage not blocking merge?**
A: Coverage is a trend indicator, not a guarantee of correctness. High coverage without good tests can be misleading. We monitor it as advisory but merge decision is human-driven.

**Q: How do I test Windows-specific code on macOS?**
A: Use stubs (`.rs` files with `#[cfg(not(windows))]`) that return dummy data. For integration testing, run on Windows CI or a Windows machine.

**Q: Can I exclude a line from coverage?**
A: Yes. Use `#[cfg(not(coverage))]` for lines that genuinely cannot be tested, or add a comment explaining why the line is not covered.

**Q: Does coverage include `textquest-dll`?**
A: `textquest-dll` uses a cdylib target which has limitations with instrumentation. DLL tests are limited to unit tests of helper functions; integration tests run via the orchestrator.
