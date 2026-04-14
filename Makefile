.PHONY: help build test fmt clippy coverage clean doc check audit install-hooks

help:
	@echo "TextQuest Development Commands"
	@echo "=============================="
	@echo "make build          - Build debug binary"
	@echo "make release        - Build release binary"
	@echo "make test           - Run all tests"
	@echo "make fmt            - Format code"
	@echo "make fmt-check      - Check formatting without changes"
	@echo "make clippy         - Run clippy linter"
	@echo "make coverage       - Generate coverage report"
	@echo "make doc            - Build documentation"
	@echo "make check          - Run format + clippy + test (CI equivalent)"
	@echo "make audit          - Check dependencies for security issues"
	@echo "make clean          - Clean build artifacts"
	@echo "make install-hooks  - Install git pre-commit hooks"

# Build targets
build:
	@echo "Building debug..."
	cargo build --all-features

release:
	@echo "Building release..."
	cargo build --release --all-features

# Testing
test:
	@echo "Running tests..."
	cargo test --all-features --lib
	cargo test --all-features --test '*' --doc

test-quiet:
	@cargo test --all-features --lib -- --nocapture

# Code quality
fmt:
	@echo "Formatting code..."
	cargo fmt --all

fmt-check:
	@echo "Checking format..."
	cargo fmt --all -- --check

clippy:
	@echo "Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

coverage:
	@echo "Generating coverage report..."
	cargo tarpaulin --out Html --timeout 600
	@echo "Coverage report generated: tarpaulin-report.html"

doc:
	@echo "Building documentation..."
	cargo doc --no-deps --all-features

# Comprehensive checks (like CI)
check: fmt-check clippy test
	@echo "✅ All checks passed!"

# Security and dependencies
audit:
	@echo "Running security audit..."
	cargo audit
	cargo outdated --exit-code 1

# Git hooks
install-hooks:
	@echo "Installing pre-commit hooks..."
	mkdir -p .git/hooks
	echo "#!/bin/bash" > .git/hooks/pre-commit
	echo "set -e" >> .git/hooks/pre-commit
	echo "cargo fmt -- --check" >> .git/hooks/pre-commit
	echo "cargo clippy --all-targets --all-features -- -D warnings" >> .git/hooks/pre-commit
	echo "cargo test --lib" >> .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit
	@echo "✅ Pre-commit hooks installed"

# Cleanup
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf tarpaulin-report.html
	@echo "✅ Clean complete"
