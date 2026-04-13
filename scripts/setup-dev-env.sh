#!/usr/bin/env bash
# setup-dev-env.sh — TextQuest local developer environment setup
# Supports --profile=dev (minimal), --profile=test (includes python deps),
# --profile=full (everything). Idempotent — safe to run multiple times.
# Compatible with macOS and Linux.

set -euo pipefail

# ── colour helpers ────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
    GREEN="\033[0;32m"
    RED="\033[0;31m"
    YELLOW="\033[0;33m"
    CYAN="\033[0;36m"
    BOLD="\033[1m"
    RESET="\033[0m"
else
    GREEN="" RED="" YELLOW="" CYAN="" BOLD="" RESET=""
fi

PASS="${GREEN}✓${RESET}"
FAIL="${RED}✗${RESET}"
WARN="${YELLOW}!${RESET}"
INFO="${CYAN}→${RESET}"

pass() { printf "  %b  %s\n" "$PASS" "$1"; }
fail() { printf "  %b  %s\n" "$FAIL" "$1"; ERRORS=$((ERRORS + 1)); }
warn() { printf "  %b  %s\n" "$WARN" "$1"; }
info() { printf "  %b  %s\n" "$INFO" "$1"; }
header() { printf "\n%b%s%b\n" "$BOLD" "$1" "$RESET"; }

ERRORS=0
PROFILE="dev"
DRY_RUN=0

# ── argument parsing ──────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Set up the TextQuest local development environment.

Options:
  --profile=PROFILE   dev (default), test, or full
  --dry-run           Check what would be done without making changes
  -h, --help          Show this help message

Profiles:
  dev   Install Rust stable + nightly, clippy, rustfmt. Check for cargo, git.
  test  Everything in dev, plus check for python3 and install Python test deps.
  full  Everything in test, plus check for gh CLI (GitHub CLI).

Examples:
  $(basename "$0")                     # Minimal dev setup
  $(basename "$0") --profile=test      # Dev + Python test dependencies
  $(basename "$0") --profile=full      # Complete environment including gh CLI
  $(basename "$0") --dry-run           # Preview without making changes
EOF
}

for arg in "$@"; do
    case "$arg" in
        --profile=*) PROFILE="${arg#--profile=}" ;;
        --dry-run)   DRY_RUN=1 ;;
        -h|--help)   usage; exit 0 ;;
        *)
            printf "Unknown argument: %s\n" "$arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

case "$PROFILE" in
    dev|test|full) ;;
    *)
        printf "Invalid profile '%s'. Must be one of: dev, test, full\n" "$PROFILE" >&2
        exit 1
        ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── banner ────────────────────────────────────────────────────────────────────
printf "\n%bTextQuest dev environment setup%b  (profile: %s)\n" "$BOLD" "$RESET" "$PROFILE"
printf "Repo: %s\n" "$REPO_ROOT"
[[ $DRY_RUN -eq 1 ]] && printf "%b[dry-run mode — no changes will be made]%b\n" "$YELLOW" "$RESET"

# ── helper: run or preview ────────────────────────────────────────────────────
maybe_run() {
    if [[ $DRY_RUN -eq 1 ]]; then
        info "would run: $*"
    else
        "$@"
    fi
}

# ── 1. rustup ─────────────────────────────────────────────────────────────────
header "1. rustup"
if command -v rustup &>/dev/null; then
    RUSTUP_VER=$(rustup --version 2>&1 | head -1)
    pass "rustup is installed ($RUSTUP_VER)"
    info "Updating rustup self..."
    maybe_run rustup self update 2>/dev/null || true
else
    warn "rustup is not installed — installing now"
    if [[ $DRY_RUN -eq 1 ]]; then
        info "would run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    else
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
        # Source cargo env so subsequent commands can find rustup/cargo
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env" 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
    fi
    if command -v rustup &>/dev/null; then
        pass "rustup installed successfully"
    else
        fail "rustup installation failed — install manually from https://rustup.rs"
    fi
fi

# ── 2. Rust stable toolchain ──────────────────────────────────────────────────
header "2. Rust stable toolchain"
if command -v rustup &>/dev/null; then
    if rustup toolchain list 2>/dev/null | grep -q "^stable"; then
        STABLE_VER=$(rustup run stable rustc --version 2>/dev/null || echo "")
        pass "stable toolchain installed ($STABLE_VER)"
    else
        info "Installing stable toolchain..."
        maybe_run rustup toolchain install stable
        pass "stable toolchain installed"
    fi
    info "Setting stable as default..."
    maybe_run rustup default stable
    pass "stable set as default toolchain"
else
    fail "rustup not available — cannot install stable toolchain"
fi

# ── 3. Rust nightly toolchain ─────────────────────────────────────────────────
header "3. Rust nightly toolchain"
if command -v rustup &>/dev/null; then
    if rustup toolchain list 2>/dev/null | grep -q "^nightly"; then
        NIGHTLY_VER=$(rustup run nightly rustc --version 2>/dev/null || echo "")
        pass "nightly toolchain installed ($NIGHTLY_VER)"
        info "Updating nightly..."
        maybe_run rustup update nightly
    else
        info "Installing nightly toolchain..."
        maybe_run rustup toolchain install nightly
        pass "nightly toolchain installed"
    fi
else
    fail "rustup not available — cannot install nightly toolchain"
fi

# ── 4. rustfmt ────────────────────────────────────────────────────────────────
header "4. rustfmt"
if command -v rustup &>/dev/null; then
    if rustup component list --installed 2>/dev/null | grep -q "rustfmt"; then
        RUSTFMT_VER=$(rustfmt --version 2>/dev/null || cargo fmt --version 2>/dev/null || echo "")
        pass "rustfmt is installed ($RUSTFMT_VER)"
    else
        info "Adding rustfmt component..."
        maybe_run rustup component add rustfmt
        pass "rustfmt installed"
    fi
else
    fail "rustup not available — cannot install rustfmt"
fi

# ── 5. clippy ─────────────────────────────────────────────────────────────────
header "5. clippy"
if command -v rustup &>/dev/null; then
    if rustup component list --installed 2>/dev/null | grep -q "clippy"; then
        CLIPPY_VER=$(cargo clippy --version 2>/dev/null || echo "")
        pass "clippy is installed ($CLIPPY_VER)"
    else
        info "Adding clippy component..."
        maybe_run rustup component add clippy
        pass "clippy installed"
    fi
else
    fail "rustup not available — cannot install clippy"
fi

# ── 6. Required tools check ───────────────────────────────────────────────────
header "6. Required tools"

# cargo
if command -v cargo &>/dev/null; then
    CARGO_VER=$(cargo --version 2>&1 | head -1)
    pass "cargo: $CARGO_VER"
else
    fail "cargo is not on PATH — install Rust from https://rustup.rs"
fi

# git
if command -v git &>/dev/null; then
    GIT_VER=$(git --version 2>&1 | head -1)
    pass "git: $GIT_VER"
else
    fail "git is not on PATH — install git (https://git-scm.com)"
fi

# ── 7. python3 (test + full profiles) ────────────────────────────────────────
if [[ "$PROFILE" == "test" || "$PROFILE" == "full" ]]; then
    header "7. python3"
    if command -v python3 &>/dev/null; then
        PY_VER=$(python3 --version 2>&1)
        PY_PATH=$(command -v python3)
        pass "python3: $PY_VER ($PY_PATH)"

        # Install Python test dependencies if requirements file exists
        REQ_FILE="$REPO_ROOT/requirements-dev.txt"
        if [[ -f "$REQ_FILE" ]]; then
            info "Installing Python dev dependencies from requirements-dev.txt..."
            maybe_run python3 -m pip install -r "$REQ_FILE" --quiet
            pass "Python dev dependencies installed"
        else
            # No requirements file — check unittest (stdlib, always present)
            if python3 -c "import unittest" 2>/dev/null; then
                pass "Python unittest available (stdlib)"
            else
                warn "Python unittest not available"
            fi
        fi
    else
        fail "python3 is not on PATH — install Python 3 (https://python.org)"
    fi
fi

# ── 8. gh CLI (full profile only) ────────────────────────────────────────────
if [[ "$PROFILE" == "full" ]]; then
    header "8. GitHub CLI (gh)"
    if command -v gh &>/dev/null; then
        GH_VER=$(gh --version 2>&1 | head -1)
        pass "gh: $GH_VER"
    else
        PLATFORM="$(uname -s)"
        if [[ "$PLATFORM" == "Darwin" ]] && command -v brew &>/dev/null; then
            info "Installing gh via Homebrew..."
            maybe_run brew install gh
            if command -v gh &>/dev/null; then
                GH_VER=$(gh --version 2>&1 | head -1)
                pass "gh installed: $GH_VER"
            else
                fail "gh installation failed — install manually: https://cli.github.com"
            fi
        elif [[ "$PLATFORM" == "Linux" ]]; then
            warn "gh CLI is not installed"
            info "Install instructions: https://cli.github.com/manual/installation"
            info "  Debian/Ubuntu: sudo apt install gh"
            info "  Fedora/RHEL:   sudo dnf install gh"
            info "  Arch:          sudo pacman -S github-cli"
            ERRORS=$((ERRORS + 1))
        else
            fail "gh CLI is not installed — install from https://cli.github.com"
        fi
    fi
fi

# ── summary ───────────────────────────────────────────────────────────────────
printf "\n%b──────────────────────────────────────────%b\n" "$BOLD" "$RESET"
if [[ $ERRORS -eq 0 ]]; then
    printf "%b%b All checks passed.%b\n" "$BOLD" "$GREEN" "$RESET"
    printf "\nNext steps:\n"
    printf "  cargo build          # Debug build\n"
    printf "  cargo test           # Full test suite\n"
    printf "  python3 scripts/dev-preflight.py  # CI preflight check\n"
    exit 0
else
    printf "%b%b %d check(s) failed. Fix the issues above and rerun this script.%b\n" \
        "$BOLD" "$RED" "$ERRORS" "$RESET"
    exit 1
fi
