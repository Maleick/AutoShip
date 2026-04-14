# Documentation & Polish Standards

This document defines the quality and documentation standards for all work in the TextQuest repository. Following these standards ensures maintainability, clarity, and a high-quality developer and user experience.

## Documentation Requirements

### For Every Task Issue

Every task should be clearly defined before work begins. Use the following structure in issue descriptions:

**Task Description**:
- Clear, concise summary of the goal.
- Links to parent epic.
- Links to dependencies.
- Links to related tasks.

**Scope Section**:
- What gets built (with code examples if helpful).
- What does **NOT** get built (boundaries).
- File paths affected.
- Module structure (if introducing new modules).

**Design Section**:
- Architecture diagrams (if complex).
- Data structures (with field descriptions).
- API contracts (if public).
- Configuration format (if applicable).
- Examples of usage.

**Implementation Notes**:
- Key algorithms or patterns.
- Performance considerations.
- Platform-specific handling (e.g., `#[cfg(windows)]`).
- Anti-patterns to avoid.

**Testing**:
- Unit test expectations.
- Integration test scenarios.
- Acceptance criteria.
- Performance benchmarks.

**Acceptance Criteria**:
- [ ] Feature implemented.
- [ ] Tests pass (target 70%+ coverage on new logic).
- [ ] No clippy warnings.
- [ ] Formatted with `rustfmt`.
- [ ] Documentation added (in-code and/or `docs/`).
- [ ] Performance is acceptable.

### For Every PR

**Commit Message Format**:
Follow Conventional Commits. Provide enough detail to understand the "why".
```
feat(help-system): implement search functionality

- Added fuzzy matching with levenshtein distance
- Created inverted index for fast lookups
- Performance: <10ms for typical queries

Closes #1103
Tests: Added 8 unit tests, 2 integration tests
Coverage: 85% in search module
```

**Code Comments**:
- Explain **"why"**, not "what" (the code says "what").
- Link to GitHub issues for context if the logic is non-obvious.
- Mark `unsafe` blocks with a `// SAFETY:` rationale.
- Note `TODO`/`FIXME` with associated issue numbers.

**Commit Hygiene**:
- One logical feature per commit.
- Bisect-friendly history (every commit should build and pass tests).
- No WIP or debug commits in final history (squash before merge).
- Descriptive, high-signal messages.

### Rust Code Documentation

**Public APIs**:
Use `///` for doc comments. Include Arguments, Returns, Performance notes, and Examples.
```rust
/// Searches help content for matching topics.
///
/// # Arguments
/// * `query` - Search string (supports fuzzy matching)
/// * `limit` - Maximum results to return
///
/// # Returns
/// Vector of search results ranked by relevance
///
/// # Performance
/// Typical query: <10ms
/// See #1103 for implementation details
///
/// # Examples
/// ```ignore
/// let db = HelpDatabase::load_from_file("config/help.toml")?;
/// let results = db.search("help", 10);
/// assert!(!results.is_empty());
/// ```
pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
    // implementation
}
```

**Internal Documentation**:
Use `//` for internal implementation details.
```rust
// We clone the query string here because the fuzzy matcher
// needs owned strings for performance reasons (fewer allocations
// in the hot path). See benchmark results in tests.
let query_owned = query.to_string();
```

**Error Types**:
Document error variants and their causes.
```rust
/// Failed to load help database from TOML file.
#[derive(Debug, thiserror::Error)]
pub enum HelpLoadError {
    /// File not found at path
    #[error("Help file not found: {0}")]
    NotFound(PathBuf),
    /// TOML parse error with line/column
    #[error("Parse error at {line}:{column}: {message}")]
    ParseError { line: usize, column: usize, message: String },
    /// Missing required field in help entry
    #[error("Missing required field '{field}' in entry '{entry}'")]
    MissingField { entry: String, field: String },
}
```

## Code Formatting Polish

### Rust Style
- Always use `cargo fmt`.
- Prefer multi-line function signatures for 3+ arguments.
- Organize imports logically (standard lib, then external crates, then local modules).

### TypeScript/Web Style
- Use clear interfaces and type safety.
- Prefer functional components and hooks in React.
- Maintain clear component boundaries.

## Test Documentation

Organize tests into logical modules and document the intent.
```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Unit tests for the search function.
    /// See issue #1103 for acceptance criteria.
    mod search {
        use super::*;

        #[test]
        fn test_search_substring() {
            // Test details
        }

        #[test]
        fn test_search_fuzzy_tolerance() {
            // Test details
        }
    }

    /// Performance benchmarks.
    /// Target: <10ms per query on typical datasets.
    mod benchmarks {
        use super::*;

        #[test]
        fn bench_search_performance() {
            // Benchmark details
        }
    }
}
```

## Polish Checklist

### Code Quality
- [ ] No `unwrap()` or `panic!()` without explicit justification (prefer `expect("reason")` or `Result`).
- [ ] No dead code or unused imports.
- [ ] Error types have helpful, descriptive messages.
- [ ] Performance is acceptable for the intended use case.
- [ ] No unnecessary clones or allocations in hot paths.
- [ ] Memory safety is guaranteed (no `unsafe` without rigorous care and documentation).

### Testing
- [ ] Unit tests cover core logic.
- [ ] Edge cases (empty input, null, max/min values) are covered.
- [ ] Error paths are explicitly tested.
- [ ] Integration tests verify cross-module/cross-crate behavior.
- [ ] Coverage target >70% for new logic.

### Polish
- [ ] Code formatted with `rustfmt`.
- [ ] No `clippy` warnings (fix them, don't just suppress).
- [ ] Variable and function names are clear and descriptive.
- [ ] Functions follow the single-responsibility principle.
- [ ] No magic numbers; use named constants.
- [ ] Consistent style with the rest of the codebase.

### User Experience
- [ ] Error messages are helpful and actionable for the end user.
- [ ] TUI/CLI output is clear and properly aligned.
- [ ] No silent failures; use logging or error returns.
- [ ] Performance feels "snappy" in interactive components.

## Automated Polish

The following tools are used to maintain standards:
- **Format**: `cargo fmt`
- **Lint**: `cargo clippy --all-targets --all-features -- -D warnings`
- **Test**: `cargo test`
- **Preflight**: `python3 scripts/dev-preflight.py` (runs all of the above)
