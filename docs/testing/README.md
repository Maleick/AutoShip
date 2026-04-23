# TextQuest Testing Infrastructure

This document complements the wiki documentation for testing conventions,
tooling, and CI pipelines in the TextQuest workspace. Use it as a practical
summary of the current workspace setup and implementation details, and refer
to the main wiki entry points for broader guidance:

- [Test Guide](../wiki/Test-Guide.md)
- [Testing](../wiki/Testing.md)
---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Workspace Layout](#workspace-layout)
3. [Cargo Test Layout](#cargo-test-layout)
4. [Python Tests (pytest / unittest)](#python-tests)
5. [Integration Tests](#integration-tests)
6. [Coverage](#coverage)
7. [CI Pipelines](#ci-pipelines)
8. [Pre-commit and Local Preflight](#pre-commit-and-local-preflight)
9. [Platform Discipline](#platform-discipline)
10. [Related Documents](#related-documents)

---

## Quick Start

Run the same check set that CI runs before pushing any change:

```bash
# Option A — make target (fmt + clippy + test)
make check

# Option B — Python preflight script (runs the local preflight checks)
python3 scripts/dev-preflight.py

# Run only Rust tests
cargo test --all-features --lib

# Run only Python tests
python3 -m unittest discover -s tests -p 'test_*.py' -v

# Generate a coverage report with the workspace baseline threshold
python3 scripts/coverage-report.py --threshold 68
```

---

## Workspace Layout

The Cargo workspace contains six crates, each with their own test surface:

| Crate               | Description                   | Primary runner    |
| ------------------- | ----------------------------- | ----------------- |
| `textquest`         | Orchestrator binary           | Linux + Windows   |
| `textquest-common`  | Shared types and IPC protocol | Linux + Windows   |
| `textquest-dll`     | Windows DLL injection layer   | Windows (nightly) |
| `textquest-soul`    | LLM/AI automation logic       | Linux + Windows   |
| `textquest-web`     | Axum web backend              | Linux + Windows   |
| `textquest-web-sdk` | TypeScript/Python SDK         | Linux + Windows   |

In addition, a `tests/` directory at the repo root holds Python behavioral and
contract tests that exercise scripts, offline tooling, and documentation
contracts.

---

## Cargo Test Layout

### Unit Tests

Unit tests live **inline** inside each source file in an `#[cfg(test)] mod
tests { … }` block. No separate `tests/` subdirectory exists inside crates for
unit tests.

```
textquest/src/
    combat.rs          # inline #[cfg(test)] mod tests { … }
    ipc.rs             # inline #[cfg(test)] mod tests { … }
    nav/               # subdirectory modules also use inline tests
textquest-common/src/
    …
```

Run all unit tests across the workspace:

```bash
cargo test --all-features --lib
```

Run unit tests for a single crate:

```bash
cargo test -p textquest-web --lib
cargo test -p textquest-dll --lib --all-features   # Windows only; nightly Rust
```

### Test Naming Convention

Every test name must follow the three-segment `<unit>_<action>_<expected_result>` format in snake_case. See [docs/testing/best-practices.md](best-practices.md) for the full naming guide and anti-pattern list.

### Windows-Only Tests

Tests that require Windows APIs or the DLL are gated with `#[cfg(windows)]`.
They compile and run only on the Frostreaver self-hosted Windows runner; they
are **not** failures when skipped on macOS or Linux.

```rust
#[cfg(windows)]
#[test]
fn dll_module_name_requires_dll_extension() { … }
```

### Async Tests

Use `#[tokio::test]` for any `async fn` test in crates that already depend on
`tokio`. Do not use `#[tokio::main]`. If a crate needs async tests and does not
already have a `tokio` dev-dependency, add an appropriate one (typically with
`features = ["full", "test-util"]`) in that crate before using
`#[tokio::test]`.

```rust
#[tokio::test]
async fn bot_state_lockout_tracking() { … }
```

---

## Python Tests

The `tests/` directory at the repo root contains Python tests executed with the
standard `unittest` runner (no pytest dependency required).

### Running Python Tests

```bash
# Discover and run all tests verbosely
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

In CI the step is marked `continue-on-error: true` — Python test failures are
advisory and do not block the merge gate (though they are expected to pass).

### What the Python Tests Cover

| File                              | What it validates                                            |
| --------------------------------- | ------------------------------------------------------------ |
| `test_coverage_report.py`         | `scripts/coverage-report.py` logic and threshold enforcement |
| `test_dev_preflight.py`           | `scripts/dev-preflight.py` checks                            |
| `test_workflow_contract.py`       | CI workflow YAML structure contracts                         |
| `test_rustfmt_contract.py`        | `rustfmt.toml` settings contract                             |
| `test_build_branch_pr_matrix.py`  | `scripts/build_branch_pr_matrix.py`                          |
| `test_sync_project.py`            | Wiki sync validation                                         |
| `test_sync_wiki_validation.py`    | Wiki source consistency                                      |
| `test_import_mq_offsets.py`       | Offset import script                                         |
| `test_import_mq2_maps.py`         | Map import script                                            |
| `test_validate_maps.py`           | Map file format validation                                   |
| `test_validate_offsets_sync.py`   | Offset/source sync check                                     |
| `test_collect_patch_evidence.py`  | ETW/patch evidence collection script                         |
| `test_etw_parser.py`              | ETW log parser                                               |
| `test_generate_maps.py`           | Map generation script                                        |
| `test_sebilis_validation_docs.py` | Documentation contract (Sebilis)                             |
| `test_issue_index_consistency.py` | Issue index consistency                                      |
| Various `*_docs.py` tests         | Documentation presence and format contracts                  |

### Writing New Python Tests

- Place new files in `tests/` and prefix the filename with `test_`.
- Subclass `unittest.TestCase`; do not use pytest-specific APIs.
- Keep tests independent — no shared mutable state.
- Load scripts under test via `importlib.util.spec_from_file_location` (see
  `tests/test_coverage_report.py` for the pattern).

---

## Integration Tests

Integration tests require multiple modules or real file I/O and live in
`textquest/tests/` as separate `*.rs` files (Cargo's built-in integration test
harness).

```
textquest/tests/
    integration.rs          # Login → Enter World → Navigate pipeline (Windows)
    integration_cli.rs      # CLI argument and launch-profile integration
    alert_system.rs         # Alert trigger integration
    box_chat.rs             # Chat/macro integration
    item_score.rs           # Item scoring integration
    window_title_runtime.rs # Window-title polling integration
    scenarios/
        mod.rs              # Shared scenario helpers
```

### Running Integration Tests

```bash
# All integration tests in the textquest crate
cargo test -p textquest --test integration
cargo test -p textquest --test integration_cli
cargo test -p textquest --test alert_system

# All doc-tests and integration tests together
cargo test --all-features --tests --doc
```

### Key Integration Test Patterns

- Tests use **pure FSM APIs** directly — no live EQ process, no mocks, no trait
  abstractions.
- The Login → Enter World → Navigate pipeline is covered end-to-end by
  `integration.rs`.
- `integration_cli.rs` covers multi-client launch coordination, coordinator
  pause/resume, and fatal login error handling.
- Scenarios that require a live game connection are out of scope for automated
  CI and are reserved for manual testing on Frostreaver.

---

## Coverage

### Threshold

The repo-wide CI gate enforces the current workspace baseline, currently
**68 % line coverage**. New files and major new functions should still meet the
80 % target, and modified logic should stay at or above 70 % where practical.

### Measuring Coverage Locally

```bash
# Install cargo-tarpaulin once
cargo install cargo-tarpaulin --locked --version ^0.31

# Text report (exits 1 if below the workspace baseline)
python3 scripts/coverage-report.py --threshold 68

# Text + HTML report
python3 scripts/coverage-report.py --html --threshold 68

# Aspirational full-workspace target for coverage cleanup work
python3 scripts/coverage-report.py --threshold 80

# Raw tarpaulin invocation
cargo tarpaulin --workspace --all-features --tests
```

The HTML report is written to `target/tarpaulin-report.html`. Open it in a
browser to see per-file line-level coverage.

### Layer-by-Layer Targets

| Layer                                                 | Target                        | Rationale                                                 |
| ----------------------------------------------------- | ----------------------------- | --------------------------------------------------------- |
| Pure logic (parsers, state machines, data structures) | 90 %+ branch                  | Highest-value tests; portable                             |
| Platform-independent orchestration                    | 80 %+                         | Test decision logic; OS calls are `#[cfg(windows)]`       |
| Windows-only paths                                    | Best-effort                   | Run on Frostreaver; every error branch should have a test |
| TUI rendering                                         | Smoke-only                    | Verify no panic on common inputs                          |
| IPC wire protocol                                     | All encode/decode round-trips | Serialization bugs are silent and expensive               |

---

## CI Pipelines

All CI runs on **self-hosted runners only** (DigitalOcean Linux `textquest`
label + Frostreaver Windows `textquest` label). GitHub-hosted runners are never
used.

### `ci.yml` — Merge Gate

Triggers on every PR to `master` and every push to `master`.

**Smart skip:** If every changed file is under `docs/`, `site/`, `*.md`, or
the other configured documentation-only paths, the Rust build, test, and coverage steps are skipped. Only wiki
validation and offset sync always run.

| Step                                     | What it does                                               |
| ---------------------------------------- | ---------------------------------------------------------- |
| Detect docs-only change set              | Skips Rust work when only docs changed                     |
| Install native dependencies              | `cmake clang libclang-dev llvm-dev …` on Linux             |
| Check Rust formatting                    | `cargo fmt --all -- --check`                               |
| Validate wiki sources                    | `python3 scripts/sync_wiki.py --check`                     |
| Enforce wiki update for behavior changes | Fails if source files change without a `docs/wiki/` update |
| Validate offset sync                     | `python3 scripts/validate_offsets_sync.py`                 |
| Run Python tests (advisory)              | `python3 -m unittest discover -s tests -p 'test_*.py' -v`  |
| Run clippy                               | `cargo clippy --all-targets --all-features -- -D warnings` |
| Run coverage (threshold 68 %)            | `python3 scripts/coverage-report.py --threshold 68`        |

### `ci.yml` — `test-matrix` Job

Runs unit tests in parallel on three configurations:

| Config          | Runner       | Rust    | Command                                            |
| --------------- | ------------ | ------- | -------------------------------------------------- |
| Windows stable  | Frostreaver  | stable  | `cargo test -p textquest-web-sdk --lib`            |
| Windows nightly | Frostreaver  | nightly | `cargo test -p textquest-dll --lib --all-features` |
| Linux stable    | DigitalOcean | stable  | `cargo test --lib --all-features`                  |

The nightly Windows job has `continue-on-error: true` (unsafe hooks testing may
be unstable).

### `ci.yml` — `secrets_scan` Job

Runs on every PR/push. Uses TruffleHog OSS (`--only-verified`) to scan for
leaked credentials.

### `ci.yml` — `advisory_checks` Job

Runs only on `workflow_dispatch`. Executes `cargo audit` and `cargo deny` as
advisory checks (`continue-on-error: true`).

### `automation.yml` — Issue and PR Automation

Manages `agent:ready` / `agent:close` / `merge:auto` labels and post-merge
wiki sync. Not a test pipeline but relevant for understanding PR lifecycle.

### `benchmarks.yml` — Performance Benchmarks

`workflow_dispatch` only. Runs `cargo bench --benches` on Linux and uploads
results to `target/criterion/` as a CI artifact (retention 30 days).

```bash
# Run benchmarks locally
cargo bench --benches -- --verbose
```

### `nightly-release.yml` — Weekly Windows Validation

Runs every Monday at 08:00 UTC on Frostreaver. Builds release binaries and
publishes a rolling `nightly` pre-release tag with `textquest.exe` and
`textquest_dll.dll` artifacts.

### `release.yml` — Tagged Release

Triggered by `v*` tags. Runs `cargo test --lib --all` on Frostreaver before
building and uploading release artifacts.

---

## Pre-commit and Local Preflight

### `make check` (Makefile)

Runs the same three steps as the CI merge gate in sequence:

```bash
make check   # cargo fmt --check + cargo clippy + cargo test
```

### `scripts/dev-preflight.py`

A comprehensive Python script that mirrors the full CI gate locally. It checks:

- Rust toolchain and formatter
- Clippy warnings
- Unit tests across the workspace
- Map file validation (`config/maps/*.txt`)
- Python test discovery

```bash
python3 scripts/dev-preflight.py
```

### Git Pre-commit Hook

Install a lightweight pre-commit hook that runs fmt-check, clippy, and unit
tests before every commit:

```bash
make install-hooks
```

---

## Platform Discipline

- For Windows-only **tests**, prefer `#[cfg(windows)]`.
- Apply `#[cfg(windows)]` to **both** the `use` import and the `#[test]`
  function when the test requires Windows-only symbols.
- `#[cfg(target_os = "windows")]` may still be appropriate in non-test code
  when explicit OS gating better matches the implementation and existing
  project practice.
- Tests gated `#[cfg(windows)]` are not skipped failures — they simply do not
  compile on macOS. This is intentional.
- All Windows-targeted tests run on Frostreaver (self-hosted Windows runner,
  x86-64, MSVC toolchain, both stable and nightly Rust).

---

## Related Documents

| Document                                    | Location                                                    |
| ------------------------------------------- | ----------------------------------------------------------- |
| Test naming conventions and assertion style | [docs/testing/best-practices.md](best-practices.md)         |
| Unit test templates with live examples      | [docs/testing/unit-test-template.md](unit-test-template.md) |
| Coverage standards and tarpaulin config     | [docs/COVERAGE_STANDARDS.md](../COVERAGE_STANDARDS.md)      |
| CI runner infrastructure                    | `docs/wiki/` (CI-Runners page)                              |
| Patch-day test runbook                      | [docs/patch-day-runbook.md](../patch-day-runbook.md)        |
