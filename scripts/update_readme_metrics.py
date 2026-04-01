#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import os
import subprocess
import sys
from urllib.parse import quote


REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
README_PATH = REPO_ROOT / "README.md"


def rust_loc() -> int:
    total = 0
    for path in REPO_ROOT.rglob("*.rs"):
        relative_parts = path.relative_to(REPO_ROOT).parts
        if "target" in relative_parts or ".git" in relative_parts:
            continue
        with path.open("r", encoding="utf-8") as handle:
            total += sum(1 for _ in handle)
    return total


def test_count() -> int:
    result = subprocess.run(
        ["cargo", "test", "--workspace", "--", "--list"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
        env={**os.environ, "CARGO_TERM_COLOR": "never"},
    )
    return sum(1 for line in result.stdout.splitlines() if line.endswith(": test"))


def badge(label: str, value: str, color: str) -> str:
    return (
        f"[![{label}](https://img.shields.io/badge/"
        f"{quote(label)}-{quote(value)}-{color}?style=flat-square)](#testing)"
    )


def replace_line(text: str, prefix: str, replacement: str) -> str:
    lines = text.splitlines()
    replaced = False
    for index, line in enumerate(lines):
        if line.startswith(prefix):
            lines[index] = replacement
            replaced = True
            break
    if not replaced:
        raise RuntimeError(f"Could not find README line starting with {prefix!r}")
    return "\n".join(lines) + "\n"


def main() -> int:
    readme = README_PATH.read_text(encoding="utf-8")

    loc = rust_loc()
    tests = test_count()

    updated = replace_line(
        readme,
        "[![Rust LOC]",
        badge("Rust LOC", f"{loc:,}", "blue"),
    )
    updated = replace_line(
        updated,
        "[![Tests]",
        badge("Tests", f"{tests:,} exact", "brightgreen"),
    )
    updated = replace_line(
        updated,
        "Current workspace totals:",
        (
            f"Current workspace totals: {loc:,} Rust lines and {tests:,} exact tests. "
            "This line and the badges above are auto-refreshed by "
            "`scripts/update_readme_metrics.py`. CI runs on every push to master:"
        ),
    )

    if updated == readme:
        return 0

    README_PATH.write_text(updated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
