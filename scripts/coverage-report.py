#!/usr/bin/env python3
"""
Coverage report script for TextQuest.

Generates test coverage metrics using cargo-tarpaulin and reports summary statistics.
Requires: cargo-tarpaulin (install with: cargo install cargo-tarpaulin)

Usage:
  python3 scripts/coverage-report.py [--html] [--threshold <percent>]

  --html         Generate HTML report (output to target/tarpaulin-report.html)
  --threshold N  Exit with code 1 if coverage falls below N% (default: 60)

Examples:
  python3 scripts/coverage-report.py                    # Text report
  python3 scripts/coverage-report.py --html             # Text + HTML report
  python3 scripts/coverage-report.py --threshold 80     # Warn if below 80%
"""

import subprocess
import sys
import json
import re
from pathlib import Path
from typing import Optional, Tuple

COMMAND_TIMEOUT_SECONDS = 1800

def run_command(cmd: list, capture_output: bool = True) -> Tuple[int, str, str]:
    """Run a command and return (exit_code, stdout, stderr)."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=capture_output,
            text=True,
            timeout=COMMAND_TIMEOUT_SECONDS
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        timeout_minutes = COMMAND_TIMEOUT_SECONDS // 60
        return 1, "", f"Coverage command timed out ({timeout_minutes} minutes)"
    except FileNotFoundError:
        return 127, "", f"Command not found: {cmd[0]}"

def check_tarpaulin_installed() -> bool:
    """Check if cargo-tarpaulin is installed."""
    exit_code, _, _ = run_command(["cargo", "tarpaulin", "--version"])
    return exit_code == 0

def generate_coverage_report(html: bool = False) -> Tuple[int, Optional[float]]:
    """
    Generate coverage report using cargo-tarpaulin.

    Returns: (exit_code, coverage_percentage)
    """
    if not check_tarpaulin_installed():
        print("ERROR: cargo-tarpaulin is not installed.")
        print("Install it with: cargo install cargo-tarpaulin")
        return 1, None

    cmd = [
        "cargo",
        "tarpaulin",
        "--workspace",
        "--all-features",
        "--tests",
        "--timeout", "300",
        "--out", "Stdout"
    ]

    if html:
        cmd.extend(["--out", "Html"])

    print(f"Running: {' '.join(cmd)}")
    print("-" * 80)

    exit_code, stdout, stderr = run_command(cmd)

    if exit_code != 0:
        print(f"ERROR: Coverage command failed (exit code {exit_code})")
        if stdout:
            print("STDOUT:")
            print(stdout)
        if stderr:
            print("STDERR:")
            print(stderr)
        return exit_code, None

    # Parse coverage percentage from tarpaulin output
    # Pattern: "X.XX% coverage"
    match = re.search(r'(\d+\.\d+)%\s+coverage', stdout)
    coverage_percent = None

    print(stdout)

    if match:
        coverage_percent = float(match.group(1))
    else:
        print("\nWARNING: Could not parse coverage percentage from output")

    if html:
        print("\nHTML report generated to: target/tarpaulin-report.html")

    return 0, coverage_percent

def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Generate test coverage report for TextQuest"
    )
    parser.add_argument(
        "--html",
        action="store_true",
        help="Generate HTML report (output to target/tarpaulin-report.html)"
    )
    parser.add_argument(
        "--threshold",
        type=int,
        default=60,
        help="Exit with code 1 if coverage falls below this percentage (default: 60)"
    )

    args = parser.parse_args()
    threshold_explicit = any(arg.startswith("--threshold") for arg in sys.argv[1:])

    exit_code, coverage_percent = generate_coverage_report(html=args.html)

    if exit_code != 0:
        return exit_code

    if coverage_percent is not None:
        print("-" * 80)
        print(f"Coverage Summary: {coverage_percent:.2f}%")
        print(f"Target Threshold: {args.threshold}%")

        if coverage_percent < args.threshold:
            print(f"COVERAGE BELOW THRESHOLD ({coverage_percent:.2f}% < {args.threshold}%)")
            return 1
        print(f"Coverage meets threshold ({coverage_percent:.2f}% >= {args.threshold}%)")
        return 0

    if threshold_explicit:
        print(
            "ERROR: Coverage threshold enforcement requested but coverage "
            "percentage could not be parsed from tarpaulin output.",
            file=sys.stderr,
        )
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
