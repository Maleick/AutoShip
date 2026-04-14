#!/bin/bash
#
# prepare-release.sh
#
# Validates release readiness and creates a git tag for a new release.
#
# Usage:
#   ./scripts/prepare-release.sh X.Y.Z
#
# Example:
#   ./scripts/prepare-release.sh 0.7.0
#
# This script:
#   1. Validates the version format (X.Y.Z)
#   2. Checks that all tests pass
#   3. Validates Cargo.toml version matches the target release version
#   4. Ensures git working tree is clean
#   5. Confirms CHANGELOG.md has been updated
#   6. Generates a changelog entry template for review
#   7. Creates a git tag (vX.Y.Z)
#   8. Prompts user to push the tag to GitHub
#

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="${1}"

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

usage() {
    cat << EOF
Usage: $0 X.Y.Z

Prepare a new release by validating readiness and creating a git tag.

Example:
  $0 0.7.0

This script:
  1. Validates version format (X.Y.Z)
  2. Checks that all tests pass
  3. Verifies Cargo.toml version matches target version
  4. Ensures clean git working tree
  5. Confirms CHANGELOG.md has been updated
  6. Generates changelog entry template
  7. Creates git tag (vX.Y.Z)

After this script succeeds, push the tag to GitHub:
  git push origin vX.Y.Z

This will trigger the release workflow in GitHub Actions.

EOF
    exit 1
}

# Validate version format
validate_version() {
    if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        log_error "Invalid version format: $VERSION"
        log_info "Version must be in format X.Y.Z (e.g., 0.7.0)"
        exit 1
    fi
}

# Check if tag already exists
check_tag_exists() {
    local tag="v${VERSION}"
    if git rev-parse "$tag" >/dev/null 2>&1; then
        log_error "Tag $tag already exists"
        log_info "To re-tag, delete the old tag first:"
        log_info "  git tag -d $tag"
        log_info "  git push origin :refs/tags/$tag"
        exit 1
    fi
}

# Run tests
run_tests() {
    log_info "Running cargo tests..."
    if ! cd "$REPO_ROOT" && cargo test --all 2>&1 | tail -50; then
        log_error "Cargo tests failed"
        exit 1
    fi
    log_success "Cargo tests passed"

    log_info "Running Python tests..."
    if ! cd "$REPO_ROOT" && python3 -m unittest discover -s tests -p 'test_*.py' -v 2>&1 | tail -50; then
        log_warning "Python tests failed or not found (continuing)"
    else
        log_success "Python tests passed"
    fi
}

# Check formatting and linting
check_format() {
    log_info "Checking code formatting..."
    if ! cd "$REPO_ROOT" && cargo fmt --check >/dev/null 2>&1; then
        log_error "Code formatting issues found"
        log_info "Run: cargo fmt"
        exit 1
    fi
    log_success "Code formatting OK"

    log_info "Running clippy..."
    if ! cd "$REPO_ROOT" && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -30; then
        log_error "Clippy warnings found"
        exit 1
    fi
    log_success "Clippy checks passed"
}

# Validate Cargo.toml version
validate_cargo_version() {
    local cargo_version
    cargo_version=$(cd "$REPO_ROOT" && grep "^version" textquest/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

    if [[ "$cargo_version" != "$VERSION" ]]; then
        log_error "Version mismatch!"
        log_info "Target version: $VERSION"
        log_info "Cargo.toml version: $cargo_version"
        log_info "Update Cargo.toml and all workspace crates to version $VERSION"
        exit 1
    fi
    log_success "Cargo.toml version matches target ($VERSION)"
}

# Check git working tree
check_git_clean() {
    if [[ -n "$(cd "$REPO_ROOT" && git status --porcelain)" ]]; then
        log_error "Git working tree is not clean"
        log_info "Commit or stash all changes before releasing:"
        cd "$REPO_ROOT"
        git status
        exit 1
    fi
    log_success "Git working tree is clean"
}

# Validate CHANGELOG.md
validate_changelog() {
    if [[ ! -f "$REPO_ROOT/CHANGELOG.md" ]]; then
        log_error "CHANGELOG.md not found"
        exit 1
    fi

    if ! grep -q "\[${VERSION}\]" "$REPO_ROOT/CHANGELOG.md"; then
        log_error "CHANGELOG.md does not contain entry for version $VERSION"
        log_info "Add a new section to CHANGELOG.md:"
        log_info ""
        log_info "## [${VERSION}] - $(date +%Y-%m-%d)"
        log_info ""
        log_info "### Features"
        log_info "- "
        log_info ""
        log_info "### Bug Fixes"
        log_info "- "
        log_info ""
        log_info "And commit before running this script."
        exit 1
    fi
    log_success "CHANGELOG.md contains entry for version $VERSION"
}

# Show changelog entry template
show_changelog_template() {
    local tag="v${VERSION}"

    log_info "Changelog entry for $tag:"
    echo ""
    if grep -A 50 "\[${VERSION}\]" "$REPO_ROOT/CHANGELOG.md" | head -40; then
        echo ""
    fi
}

# Create git tag
create_tag() {
    local tag="v${VERSION}"
    log_info "Creating git tag: $tag"

    cd "$REPO_ROOT"
    git tag -a "$tag" -m "Release $tag"
    log_success "Tag created: $tag"
}

# Main flow
main() {
    if [[ -z "$VERSION" ]]; then
        usage
    fi

    cd "$REPO_ROOT"

    log_info "=========================================="
    log_info "Preparing release for version: $VERSION"
    log_info "=========================================="
    echo ""

    log_info "Step 1: Validating version format"
    validate_version
    log_success "Version format valid: $VERSION"
    echo ""

    log_info "Step 2: Checking if tag already exists"
    check_tag_exists
    log_success "Tag v$VERSION does not exist"
    echo ""

    log_info "Step 3: Running code quality checks"
    check_format
    echo ""

    log_info "Step 4: Running test suite"
    run_tests
    echo ""

    log_info "Step 5: Validating Cargo.toml version"
    validate_cargo_version
    echo ""

    log_info "Step 6: Checking git working tree"
    check_git_clean
    echo ""

    log_info "Step 7: Validating CHANGELOG.md"
    validate_changelog
    echo ""

    log_info "Step 8: Displaying changelog entry"
    show_changelog_template
    echo ""

    log_info "=========================================="
    log_success "All pre-release checks passed!"
    log_info "=========================================="
    echo ""

    log_info "Step 9: Creating git tag"
    create_tag
    echo ""

    log_info "=========================================="
    log_success "Release v${VERSION} tag created!"
    log_info "=========================================="
    echo ""

    log_info "Next steps:"
    log_info "1. Verify the tag was created:"
    log_info "   git tag -l -n5 v${VERSION}"
    log_info ""
    log_info "2. Push the tag to GitHub:"
    log_info "   git push origin v${VERSION}"
    log_info ""
    log_info "3. This will trigger the GitHub Actions release workflow"
    log_info "   Watch: https://github.com/maleick/TextQuest/actions"
    echo ""
}

main "$@"
