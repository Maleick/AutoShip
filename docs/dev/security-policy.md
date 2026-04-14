# Security Scanning and Vulnerability Management

This document describes the security scanning infrastructure and vulnerability management procedures for TextQuest.

## Overview

TextQuest uses multiple layers of security scanning to identify and manage:
- **Dependency vulnerabilities** (advisories) via `cargo audit`
- **Unsafe code usage** via `cargo-geiger`
- **License compliance** via `cargo-deny`
- **Secrets and credentials** via TruffleHog OSS

All scanning jobs run in CI but are **advisory and non-blocking** by design, allowing PRs to proceed while maintainers investigate and address issues.

## Security Scanning Tools

### 1. Cargo Audit (Dependency Vulnerabilities)

**Purpose**: Track and report known security vulnerabilities in Rust dependencies.

**Tool**: [`cargo-audit`](https://github.com/rustsec/cargo-audit) — official advisory scanner from RustSec.

**When It Runs**:
- In CI on every push/PR to `master`
- Locally: `cargo audit`

**Configuration**:
- Database: RustSec Advisory Database
- Command: `cargo audit --deny warnings`
- Report level: `warn` for unmaintained crates, `warn` for notices, `deny` for vulnerabilities
- Ignored advisories: None configured (see [`deny.toml`](../../deny.toml) to suppress known false positives)

**Output**:
- ✓ PASS: No advisories found (all dependencies clean)
- ⚠ WARN: Unmaintained or noticed crates (informational)
- ✗ DENY: Known vulnerabilities in transitive dependencies (requires investigation)

**Response Procedure**:
1. If a vulnerability is found, check the advisory ID at [RustSec Advisory DB](https://rustsec.org/).
2. Determine if the vulnerability applies to our usage of the crate.
3. If applicable:
   - **Recommended**: Update the dependency to a patched version.
   - **If no patch available**: Document in `deny.toml` under `[advisories].ignore` with a comment explaining why it's safe to ignore.
4. If the crate is deprecated or unmaintained, consider alternative dependencies.

### 2. Cargo Geiger (Unsafe Code Report)

**Purpose**: Report unsafe Rust usage and quantify unsafety in the codebase.

**Tool**: [`cargo-geiger`](https://github.com/geiger-rs/cargo-geiger) — comprehensive unsafe code scanner.

**When It Runs**:
- In CI on every push/PR to `master`
- Locally: `cargo geiger --output Json`

**Why It's Important**:
- TextQuest uses significant unsafe code, particularly in `textquest-dll` for DLL injection, function hooking, and memory reads.
- Unsafe code is necessary for game memory access but introduces risk.
- Geiger helps identify and audit all unsafe blocks across the workspace.

**Output Format**:
- JSON report with unsafe counts by crate and region
- Unsafe block locations and context

**Response Procedure**:
1. Review any **new** unsafe code introduced in the PR.
2. Verify that every `unsafe` block has a `// SAFETY:` comment explaining:
   - Why the operation is safe
   - Preconditions that must hold
   - Invariants being maintained
3. Focus audits on:
   - `textquest-dll` — DLL injection, hooking, direct memory writes
   - `textquest-common` — IPC shared memory access, pointer arithmetic
4. Never use `unsafe` to suppress clippy warnings without documenting why.

**Example SAFETY Comment**:
```rust
// SAFETY: We validate the pointer from the process handle is within the game's
// memory range before dereferencing. The SpawnManager is guaranteed to be allocated
// in the main module by EverQuest's loader, and we lock access to prevent concurrent mutation.
unsafe {
    let spawn_count = *(ptr as *const u32);
    // ...
}
```

### 3. Cargo Deny (Advisories and Licenses)

**Purpose**: Enforce policy on dependency licenses and manage duplicate/conflicting versions.

**Tool**: [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/) — comprehensive dependency policy enforcer.

**Configuration**: [`deny.toml`](../../deny.toml)

**Policy**:
- **Allowed Licenses**: MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-DFS-2016, Zlib
- **Copyleft Threshold**: WARN (we allow copyleft but require review)
- **Prohibited Licenses**: GPL-2.0, GPL-3.0, AGPL-3.0 (incompatible with our license strategy)
- **Duplicate Crate Versions**: WARN (allowed but flagged for review)

**When It Runs**:
- Locally: `cargo deny check`
- CI: Integrated into dependency audits (non-blocking, advisory)

**Response Procedure**:
1. For license violations:
   - Check if the crate is optional. If so, verify it's not included in release builds.
   - If the license is unavoidable, add an exception in `deny.toml` with a comment.
2. For multiple versions of the same crate:
   - Investigate whether dependency version constraints can be aligned.
   - If alignment is impossible, document in `deny.toml` under `skip`.

### 4. TruffleHog OSS (Secret Scanning)

**Purpose**: Prevent accidental commits of credentials, API keys, and other secrets.

**Tool**: [`TruffleHog`](https://github.com/trufflesecurity/trufflehog) — extensible secret scanner with verified results.

**Configuration**:
- Mode: `--only-verified` (high confidence, low false positive rate)
- Scope: Full git history on every push/PR
- Allow-list: [`.gitleaksignore`](../../.gitleaksignore) for false positives

**When It Runs**:
- `secrets_scan` job in CI on every push/PR
- Locally: Install [gitleaks](https://github.com/gitleaks/gitleaks) and run `gitleaks detect --source github-pr`

**Response Procedure**:
1. If a secret is detected in a PR:
   - **DO NOT merge** the PR until the secret is removed.
   - Remove the secret from the commit (use `git rebase` or `git reset`).
   - If the secret was in code (e.g., hardcoded API key), rotate the credential immediately.
2. If it's a false positive (e.g., a test fixture or placeholder):
   - Add an entry to [`.gitleaksignore`](../../.gitleaksignore) with a comment explaining why.
   - File an issue to refactor the false positive out of the code.

## CI Jobs and Failure Modes

### Merge Gate Job: `merge_gate`

**Status**: REQUIRED (blocks PR merge)

**Includes**:
- Formatting check (`cargo fmt --check`)
- Linting (`cargo clippy`)
- Unit tests (`cargo test`)
- Python tests
- Wiki and offset validation

**Failure Handling**: Fix and push a new commit.

### Security Jobs (Non-Blocking)

#### `secrets_scan`
- **Status**: RECOMMENDED (visible, does not block)
- **Action on Failure**: Review and remove secrets; rotate credentials if necessary
- **Time**: ~5 minutes

#### `cargo_audit`
- **Status**: ADVISORY (visible, does not block)
- **Action on Failure**: Assess vulnerability, update dependency or ignore in `deny.toml`
- **Time**: ~5-10 minutes

#### `unsafe_code_report`
- **Status**: ADVISORY (visible, does not block)
- **Action on Failure**: Not really a "failure" — always completes successfully. Review new unsafe code in PR.
- **Time**: ~5-10 minutes

## Running Security Checks Locally

### Before Pushing

Run the full pre-flight script (mirrors CI):
```bash
python3 scripts/dev-preflight.py
```

This runs fmt, clippy, and tests but does not include security scanning.

### Optional Security Checks

Run individually:

```bash
# Dependency vulnerability audit
cargo audit

# Unsafe code report
cargo geiger

# License and policy compliance
cargo deny check

# Secret scanning (requires gitleaks installed)
gitleaks detect --source github-pro
```

### Install Security Tools Locally

```bash
cargo install cargo-audit      # ~10-20 MB, depends on build environment
cargo install cargo-geiger     # ~10-20 MB
cargo install cargo-deny       # ~30-50 MB
cargo install gitleaks         # ~50-100 MB or use pre-built binary
```

## Crate-Specific Security Notes

### textquest-dll (DLL Injection, Unsafe-Heavy)

- **Primary Unsafe Operations**:
  - Memory-mapped DLL injection into game processes
  - Function hooking via `retour` crate
  - Direct reads/writes to EverQuest memory (using `ReadProcessMemory` / `WriteProcessMemory`)
  - Shared memory access via `windows-sys` and raw pointers

- **Safety Model**:
  - All pointer arithmetic is validated before dereferencing
  - Memory regions are checked against module boundaries
  - Spawns are bounded with max-count limits to prevent infinite loops
  - Hooked functions maintain exact calling convention compatibility

- **Audit Checklist** (PR reviewers):
  - [ ] Every `unsafe` block has a `// SAFETY:` comment
  - [ ] Comments explain why pointers are valid and assumptions hold
  - [ ] No unbounded loops or recursive calls
  - [ ] Error paths are safe (no corrupted state on failure)

### textquest-common (IPC, Moderate Unsafe)

- **Unsafe Operations**:
  - Shared memory deserialization (converting raw bytes to `GameState`)
  - Named pipe round-trips with serialized state
  - Channel-to-slice conversions for buffer efficiency

- **Audit Checklist**:
  - [ ] All deserialized data is validated before use
  - [ ] Buffer sizes match expected message sizes
  - [ ] No uninitialized reads

### textquest (TUI, Low Unsafe)

- **Unsafe Operations**: None expected; uses high-level async Tokio and ratatui.

### textquest-web (Web Backend, Low Unsafe)

- **Unsafe Operations**: None expected; uses safe Axum HTTP stack.

## Handling Vulnerability Disclosures

If you discover a security vulnerability in TextQuest:

1. **Do not file a public GitHub issue.**
2. **Email security concerns** to the maintainer (check `CONTRIBUTING.md` for security contact).
3. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Proposed fix (if any)
4. **Timeline**:
   - We will acknowledge receipt within 48 hours
   - We will work on a fix and coordinate disclosure timing
   - Public disclosure happens only after a patch is available and users have time to upgrade

## Resources

- [RustSec Advisory Database](https://rustsec.org/)
- [OWASP Top 10 Rust Security Issues](https://owasp.org/www-community/attacks/)
- [Rust API Guidelines — Unsafe Code](https://rust-lang.github.io/api-guidelines/safety.html)
- [Cargo-Audit Docs](https://github.com/rustsec/cargo-audit)
- [Cargo-Geiger Docs](https://github.com/geiger-rs/cargo-geiger)
- [Cargo-Deny Docs](https://embarkstudios.github.io/cargo-deny/)
- [TruffleHog Docs](https://github.com/trufflesecurity/trufflehog)

## Summary

Security scanning in TextQuest follows a **defense-in-depth** model:

| Layer | Tool | Scope | Enforcement |
|-------|------|-------|-------------|
| Dependencies | `cargo audit` | Transitive advisories | Advisory, non-blocking |
| Unsafe Code | `cargo-geiger` | Unsafe block census | Advisory, non-blocking |
| Licenses | `cargo-deny` | Compliance with license policy | Advisory, non-blocking |
| Secrets | TruffleHog | Committed credentials | Visible, recommend fix |
| Code Quality | Clippy + fmt | Linting and formatting | REQUIRED (blocks merge) |

All security tools are **non-blocking** to unblock developer velocity while maintaining visibility. Maintainers review scan results asynchronously and address issues in follow-up commits or issues.
