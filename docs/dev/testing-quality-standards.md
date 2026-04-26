# Testing, Quality, and Polish Standards

This document defines comprehensive testing, validation, and polish standards to ensure production-quality implementation across all TextQuest features and sub-issues. Adherence to these standards guarantees consistent test coverage, maintainability, and developer experience.

---

## Testing Strategy by Issue Type

### Research Issues

**Goal**: Validated design with proof-of-concept implementation, not just documentation.

```rust
// Example: Debuff Detection Research
// Should produce:
// 1. Debuff database (CSV or SQLite)
// 2. Detection algorithm (pseudo-code or flowchart)
// 3. Proof of concept in Rust (< 200 lines)
// 4. Test vectors (10+ example debuffs with expected parse results)
```

**Acceptance Criteria**:
- [ ] Design document complete with diagrams
- [ ] PoC compiles and runs
- [ ] All test vectors pass PoC
- [ ] Database populated with 100+ entries
- [ ] Comparison document with reference approach

### Implementation Issues

**Goal**: Production-ready code with 80%+ test coverage.

```rust
// Example structure for a feature implementation

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_discovery() {
        // Test discovering feature components
    }

    #[test]
    fn test_feature_load_success() {
        // Test successful feature loading
    }

    #[test]
    fn test_feature_load_missing_symbols() {
        // Test graceful failure on missing symbols
    }

    #[test]
    fn test_feature_lifecycle_hooks() {
        // Test init/shutdown hooks called
    }

    #[test]
    fn test_feature_error_isolation() {
        // Test feature crash doesn't crash core
    }

    #[test]
    fn test_feature_unload() {
        // Test feature unload without crashes
    }

    #[test]
    fn test_feature_reload() {
        // Test reload functionality
    }

    #[test]
    fn test_multiple_features() {
        // Test 3+ features loading simultaneously
    }

    #[test]
    fn test_feature_dependency_order() {
        // Test features load in correct order
    }

    #[test]
    fn test_invalid_feature_ignored() {
        // Test invalid features don't block others
    }
}
```

**Unit Test Standards**:
- [ ] 1 test per function (minimum)
- [ ] Happy path + 2-3 error paths per function
- [ ] 80%+ line coverage (measured with `cargo tarpaulin`)
- [ ] All public APIs have tests
- [ ] Tests compile and run in CI

**Integration Tests**:
- [ ] Test across module boundaries
- [ ] Test with real filesystem operations (where applicable)
- [ ] Test with real external systems (Lua interpreter, network, etc.)
- [ ] Test error handling and recovery

### Validation Issues

**Goal**: Proof with real in-game testing and validation.

```rust
// Example: Community Plugin Validation
// Requirements:
// 1. Plugin compiles against public API
// 2. Plugin loads without crashes
// 3. Feature works in live environment
// 4. Data persists across sessions
// 5. UI displays stats correctly
// 6. Plugin can be unloaded cleanly
```

**Validation Checklist**:
- [ ] Builds without warnings
- [ ] Loads in live process
- [ ] Runs for 30+ minutes without crash
- [ ] Data accuracy verified (manual spot check)
- [ ] Performance impact < 0.5ms per frame
- [ ] Memory leak tested (valgrind/windbg)
- [ ] Error scenarios handled gracefully

### Documentation Issues

**Goal**: Comprehensive, tested examples with verified links.

- [ ] All code examples compile
- [ ] Examples follow project style guide
- [ ] Examples have comments explaining key concepts
- [ ] Examples tested (separate test suite if needed)
- [ ] Screenshots/diagrams up-to-date
- [ ] Links verified (no 404s)

---

## Unit Test Template for Issues

Use this template for all implementation sub-issues:

```rust
// src/feature/mod.rs

pub struct FeatureName {
    // fields
}

impl FeatureName {
    pub fn new() -> Self {
        // implementation
    }

    pub fn method_name(&self, arg: Type) -> Result<Output, Error> {
        // implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> FeatureName {
        // Common test setup
        FeatureName::new()
    }

    #[test]
    fn test_new_creates_instance() {
        let feature = FeatureName::new();
        assert_eq!(feature.some_field, expected_value);
    }

    #[test]
    fn test_method_name_success_case() {
        let feature = setup();
        let result = feature.method_name(test_arg);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_output);
    }

    #[test]
    fn test_method_name_error_case() {
        let feature = setup();
        let result = feature.method_name(invalid_arg);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorType::SpecificError);
    }

    #[test]
    fn test_method_name_boundary_case() {
        let feature = setup();
        let result = feature.method_name(boundary_arg);
        assert!(result.is_ok());
    }

    // Edge cases, concurrency, performance tests follow same pattern
}
```

---

## Code Quality Standards

### Style & Formatting

- [ ] `cargo fmt` passes without warnings
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] No unsafe code without `// SAFETY: ...` comments
- [ ] Function length < 50 lines (prefer 20-30)
- [ ] Cyclomatic complexity < 10 per function

### Documentation

- [ ] All public items have doc comments
- [ ] Doc comments include examples (where applicable)
- [ ] Complex algorithms have algorithm description
- [ ] Panic conditions documented
- [ ] Thread safety documented

```rust
/// Performs an action with all prerequisites checked.
///
/// # Arguments
/// * `id` - The ID of the resource
/// * `target` - Target for the action
///
/// # Returns
/// * `Ok(())` if action started
/// * `Err(ActionError)` if preconditions failed
///
/// # Example
/// ```
/// let result = manager.perform(12345, target_id);
/// assert!(result.is_ok());
/// ```
///
/// # Panics
/// Panics if id is 0 (invalid ID).
pub fn perform(&self, id: u32, target: u32) -> Result<(), ActionError> {
    // ...
}
```

### Error Handling

- [ ] All errors are documented with context
- [ ] Error types are specific (not generic String)
- [ ] Errors include recovery suggestions where applicable
- [ ] No panics in error paths (except documented panic conditions)

```rust
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("Resource {id} not found in database")]
    ResourceNotFound { id: u32 },

    #[error("Insufficient resources: need {required} but have {current}")]
    InsufficientResources { required: u32, current: u32 },

    #[error("Action on cooldown for {remaining_ms}ms")]
    OnCooldown { remaining_ms: u64 },

    #[error("Cannot perform while performing {id}")]
    AlreadyPerforming { id: u32 },
}
```

### Performance

- [ ] Performance-critical code has benchmarks
- [ ] Benchmarks tracked in CI
- [ ] No unnecessary allocations in hot paths
- [ ] Caching used where appropriate

---

## Test Coverage Requirements by Issue Type

### Research Issues

- **Minimum**: Proof-of-concept compiles and runs
- **Target**: PoC passes 10+ test vectors
- **Coverage**: N/A (PoC, not production code)

### Implementation Issues

- **Minimum**: 60% line coverage
- **Target**: 80%+ line coverage
- **Coverage**: Measured with `cargo tarpaulin --out Html`
- **Report**: Generated and linked in PR

### Integration Tests

- **Minimum**: 1 integration test per module
- **Target**: Happy path + 2-3 error paths per feature
- **Location**: `tests/` directory with `#[test]` or `#[tokio::test]`

### System Tests (Validation Issues)

- **Minimum**: 30-minute live run without crash
- **Target**: 8-hour run, 100+ operations, zero crashes
- **Metrics**: CPU, memory, frame timing captured

---

## Polish & Code Review Checklist

### Before PR Submission

- [ ] All tests pass (`cargo test`)
- [ ] All clippy warnings resolved
- [ ] Code formatted (`cargo fmt`)
- [ ] Coverage >= 80% (report attached)
- [ ] Documentation complete (no missing doc comments)
- [ ] No compiler warnings
- [ ] Performance benchmarks included (if applicable)
- [ ] Error handling comprehensive
- [ ] Edge cases tested
- [ ] Thread safety verified (if concurrent)

### Code Review Criteria

- [ ] Tests verify behavior, not just code
- [ ] Error paths tested and handled
- [ ] Performance acceptable (no regressions)
- [ ] API design clear and intuitive
- [ ] Documentation complete and accurate
- [ ] No security vulnerabilities
- [ ] Follows project conventions
- [ ] No hardcoded values or magic numbers

### Post-Review Before Merge

- [ ] All review comments addressed
- [ ] Changes re-tested
- [ ] Performance re-benchmarked if changed
- [ ] Documentation updated if needed
- [ ] Tests added for any new issues found

---

## Continuous Integration Requirements

### GitHub Actions Workflow

The CI pipeline must verify all quality standards before merge:

```yaml
name: Test & Quality

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      # Formatting
      - run: cargo fmt -- --check
      
      # Linting
      - run: cargo clippy --all-targets --all-features -- -D warnings
      
      # Tests
      - run: cargo test --all-features
      
      # Coverage
      - run: cargo tarpaulin --out Xml --timeout 600
      - uses: codecov/codecov-action@v2
      
      # Benchmarks
      - run: cargo bench --no-run
```

### Coverage Requirements

- [ ] Each sub-issue must have >= 80% coverage
- [ ] Coverage tracked per milestone
- [ ] Coverage regression blocks merge
- [ ] Coverage reports uploaded to Codecov

---

## Test Documentation Template

Include in every PR:

```markdown
## Test Summary

### Unit Tests
- [ ] X tests added (X% of functions covered)
- [ ] Y tests modified
- [ ] Coverage report: [attached/linked]

### Integration Tests
- [ ] Happy path verified
- [ ] Error cases tested: [list specific errors]
- [ ] Edge cases covered: [list edge cases]

### Performance Tests
- [ ] Benchmarks [improved/unchanged/degraded]
- [ ] Frame time impact: < X ms (out of 33ms budget)
- [ ] Memory overhead: Y MB

### Live Testing
- [ ] Tested in-game for X hours
- [ ] Tested with Y configurations
- [ ] Zero crashes observed
- [ ] Data accuracy verified

## Issues Found & Fixed
[List any issues discovered during testing and resolution]
```

---

## Quality Gates by Milestone

### Phase 1: Foundation & Core
- [ ] All unit tests passing
- [ ] >= 80% code coverage
- [ ] All doc comments present
- [ ] Code review approved (1+ approval)
- [ ] No compiler/clippy warnings

### Phase 2: Integration & Validation
- [ ] All integration tests passing
- [ ] 30-minute live run without crash
- [ ] Performance within budget
- [ ] Error handling validated
- [ ] Code review approved (2+ approvals)

### Phase 3: Production & Polish
- [ ] 8-hour live run with zero crashes
- [ ] Data accuracy 100% verified
- [ ] Performance benchmarks stable
- [ ] All documentation complete
- [ ] Ready for release

---

## Developer Onboarding: Test-First Workflow

For developers implementing features:

1. **Understand the feature** (2-4 hours)
   - Read research issue and documentation
   - Review reference implementations
   - Understand acceptance criteria

2. **Write tests first** (4-8 hours)
   - Create test file with comprehensive tests
   - Tests fail (red)
   - Commit test file

3. **Implement feature** (8-20 hours)
   - Implement feature code to pass tests
   - Run tests (should go green)
   - Add benchmarks and performance tests
   - Ensure formatting and clippy pass

4. **Code review** (2-4 hours)
   - Peer review of implementation
   - Verify tests comprehensive
   - Check performance acceptable
   - Approve or request changes

5. **Live validation** (4-12 hours)
   - Test feature in actual environment
   - Verify behavior matches design
   - Check for edge cases and crashes
   - Document findings

6. **Documentation & polish** (2-4 hours)
   - Add doc comments
   - Create examples
   - Update guides
   - Prepare release notes

---

## Measuring Success

### By Milestone

- [ ] Code coverage >= 80% for all code
- [ ] All tests passing
- [ ] Zero compiler/clippy warnings
- [ ] All clippy suggestions resolved
- [ ] Performance benchmarks stable or improved
- [ ] Live testing verification complete
- [ ] User documentation complete
- [ ] Zero critical/high bugs in production

### Overall

- [ ] 35+ feature issues implemented
- [ ] 80+ sub-issues completed
- [ ] 500+ unit tests written
- [ ] 50+ integration tests
- [ ] < 5% test failure rate in CI
- [ ] > 80% code coverage across project
- [ ] Zero production incidents in first month
- [ ] Community plugins building successfully

---

## Tags

`testing` `quality` `standards` `unit-tests` `integration-tests` `documentation` `polish`
