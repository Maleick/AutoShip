# TextQuest Test Coverage Standards (Issue #1202)

## Overview

This document establishes and enforces unit test coverage standards for the TextQuest project.

**Requirement:** Minimum 80% line coverage per module across all workspace crates.

## Coverage Requirements

### Minimum Coverage Thresholds
- **Global minimum:** 80% line coverage across all crates
- **Per-crate enforcement:** Each TextQuest workspace crate must maintain 80%+ coverage:
  - `textquest` (orchestrator)
  - `textquest-common` (shared types and protocol)
  - `textquest-dll` (Windows DLL injection)
  - `textquest-client` (per-client automation logic)
  - `textquest-soul` (LLM/AI layer)
  - `textquest-web` (Axum web backend)

### Test Coverage Requirements

Coverage must include:
1. **Happy path tests** - Normal operation and primary code flows
2. **Edge case tests** - Boundary conditions, empty inputs, maximum values
3. **Error case tests** - Error handling, panic recovery, invalid inputs

## CI Enforcement

### Automated Checks
- Coverage is measured on every push and pull request to `master`
- CI blocks PRs if coverage falls below 80%
- Coverage reports are generated and archived as CI artifacts

### Running Coverage Locally

Generate a coverage report on your machine before pushing:

```bash
# Install cargo-tarpaulin if not already installed
cargo install cargo-tarpaulin

# Generate text coverage report with 80% threshold
python3 scripts/coverage-report.py --threshold 80

# Generate both text and HTML reports
python3 scripts/coverage-report.py --html --threshold 80
```

The HTML report is saved to `target/tarpaulin-report.html` and can be opened in a browser for detailed per-file coverage visualization.

## Test Patterns and Best Practices

### Module Structure
Use `#[cfg(test)]` modules at the end of source files:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // Normal operation
    }

    #[test]
    fn test_edge_case_empty_input() {
        // Boundary condition
    }

    #[test]
    fn test_error_invalid_state() {
        // Error handling
    }
}
```

### Test Organization
- **One module per source file** - Keep tests colocated with code
- **Descriptive names** - Use `test_<function>_<scenario>` naming
- **Isolated tests** - Each test should be independent
- **Shared fixtures** - Use `test_support.rs` for common test utilities (see CLAUDE.md)

### Code Coverage Strategy

To achieve 80% coverage efficiently:

1. **Prioritize core logic** - Coverage must include critical paths first
2. **Test error paths** - Include tests for `Err()`, `None`, and panic cases
3. **Avoid trivial coverage** - Don't test getters/setters unless they contain logic
4. **Use property-based testing** - `proptest` for algorithmic correctness
5. **Integration tests** - Add `tests/` directory integration tests for workflows

### Windows-Specific Testing
Gate platform-specific tests with `#[cfg(windows)]`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn test_windows_only_behavior() {
        // Windows-specific test
    }
}
```

## Configuration Files

### `.github/tarpaulin.toml`
Central coverage configuration:
- Sets 80% threshold per crate
- Configures test timeout (300 seconds)
- Specifies output formats (Stdout, Html)
- Defines excluded paths

### `scripts/coverage-report.py`
Python script for CI execution:
- Checks tarpaulin installation
- Runs coverage with configurable threshold
- Generates detailed per-file reports
- Exits with code 1 if below threshold

### `.github/workflows/ci.yml`
CI workflow step:
```yaml
- name: Run coverage (threshold 80%)
  if: steps.scope.outputs.docs_only != 'true'
  run: python3 scripts/coverage-report.py --threshold 80
```

## Measuring Coverage

### Per-Module Coverage
Use tarpaulin's output to identify modules below 80%:

```bash
cargo tarpaulin --workspace --all-features --tests
```

Look for files marked as below threshold and add tests accordingly.

### Improving Coverage
1. Find uncovered lines: `cargo tarpaulin --out Html` → `target/tarpaulin-report.html`
2. Red lines = uncovered code → add test cases
3. Re-run coverage to verify improvement
4. Commit test additions

## Rollout Timeline

- **Current (April 2026):** 80% threshold enforcement active in CI
- **Target:** All crates at 80%+ coverage by M9
- **Tracking:** GitHub issues tagged `coverage` and `testing-standards`

## FAQ

**Q: Can I exclude files from coverage?**
A: Yes, via `.github/tarpaulin.toml` `exclude-files`. Use sparingly and document the reason.

**Q: Why 80% and not 100%?**
A: 80% is a practical threshold that encourages comprehensive testing while allowing for:
- Platform-specific code (`#[cfg(windows)]` branches)
- Error paths difficult to trigger in unit tests
- Performance-critical inline code

**Q: How do I test async code?**
A: Use `tokio::test` attribute:
```rust
#[tokio::test]
async fn test_async_operation() { }
```

**Q: What about integration tests?**
A: Integration tests in `tests/` directory count toward coverage. They're essential for complex workflows.

## References

- Issue: [#1202](https://github.com/maleick/TextQuest/issues/1202)
- Tarpaulin: https://github.com/xd009642/tarpaulin
- TextQuest CLAUDE.md: See `## Testing` section for test patterns
