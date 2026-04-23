#!/usr/bin/env python3

from __future__ import annotations

import os
import pathlib
import re
import subprocess
import sys
import tomllib
from urllib.parse import quote, urlencode


REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
README_PATH = REPO_ROOT / "README.md"
METRICS_PAGE_PATH = REPO_ROOT / "docs" / "wiki" / "Project-Metrics.md"
TEST_LIST_SUMMARY_RE = re.compile(r"^(\d+) tests?, \d+ benchmarks$", re.MULTILINE)
TEST_ANNOTATION_RE = re.compile(r"^\s*#\[\s*(?:tokio::)?test(?:\s*\([^]]*\))?\s*\]")
SEMVER_TAG_RE = re.compile(r"^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")
ACCOUNTS_BADGE_LINE_RE = re.compile(r"^!\[Accounts\]\([^)]+\)\n", re.MULTILINE)
CRATES_BADGE_LINE_RE = re.compile(r"^(?:!\[Crates\]|\[!\[Workspace Crates\])")
PLATFORM_BADGE_LINE_RE = re.compile(r"^!\[Platform\]\([^)]+\)$")
RELEASE_BADGE_LINE_RE = re.compile(
    r'^\s*<a href="https://github\.com/Maleick/TextQuest/'
    r'(?:actions/workflows/release\.yml|releases[^"]*)">'
    r'<img src="[^"]+" alt="Release"></a>$'
)
LAST_COMMIT_BADGE_LINE_RE = re.compile(
    r'^\s*<a href="https://github\.com/Maleick/TextQuest/'
    r'(?:commits/master|commit/[^"]+)">'
    r'<img src="[^"]+" alt="Last Commit"></a>$'
)


def tracked_rust_files() -> list[pathlib.Path]:
    result = subprocess.run(
        ["git", "ls-files", "--", "*.rs"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [REPO_ROOT / path for path in result.stdout.splitlines() if path]


def rust_loc() -> int:
    total = 0
    for path in tracked_rust_files():
        with path.open("r", encoding="utf-8") as handle:
            total += sum(1 for _ in handle)
    return total


def test_count_from_source() -> int:
    count = 0
    for path in tracked_rust_files():
        with path.open("r", encoding="utf-8") as handle:
            for line in handle:
                if TEST_ANNOTATION_RE.search(line):
                    count += 1
    return count


def test_count() -> tuple[int, bool]:
    """Return ``(count, is_exact)``.

    *is_exact* is ``True`` when the count comes from a successful
    ``cargo test --workspace -- --list`` run (runner-reported total).  It is ``False``
    when cargo is unavailable and the count comes from the source-annotation
    fallback scan, which is approximate (counts ``#[test]``/``#[tokio::test]``
    annotations, not compiled test binaries).
    """
    try:
        result = subprocess.run(
            ["cargo", "test", "--workspace", "--", "--list"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
            env={**os.environ, "CARGO_TERM_COLOR": "never"},
        )
    except FileNotFoundError:
        print(
            "warning: cargo is unavailable; falling back to source-scan test counting",
            file=sys.stderr,
        )
        return test_count_from_source(), False
    output = f"{result.stdout}\n{result.stderr}"
    if result.returncode != 0:
        print(
            f"warning: cargo test failed (exit {result.returncode}); falling back to source-scan test counting",
            file=sys.stderr,
        )
        if output.strip():
            print(output, file=sys.stderr)
        return test_count_from_source(), False
    listed = sum(int(match.group(1)) for match in TEST_LIST_SUMMARY_RE.finditer(output))
    if listed > 0:
        return listed, True
    print(
        "warning: `cargo test --workspace -- --list` did not report any summary lines; "
        "falling back to source scan",
        file=sys.stderr,
    )
    return test_count_from_source(), False


def workspace_crate_count(cargo_toml_path: pathlib.Path = REPO_ROOT / "Cargo.toml") -> int:
    with cargo_toml_path.open("rb") as handle:
        manifest = tomllib.load(handle)
    members = manifest.get("workspace", {}).get("members", [])
    return len(members)


def latest_release_tag() -> str | None:
    result = subprocess.run(
        [
            "git",
            "for-each-ref",
            "refs/tags",
            "--sort=-creatordate",
            "--format=%(refname:short)",
        ],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    for tag in result.stdout.splitlines():
        tag = tag.strip()
        if SEMVER_TAG_RE.fullmatch(tag):
            return tag
    return None


def last_commit_summary() -> tuple[str, str | None]:
    result = subprocess.run(
        ["git", "log", "-1", "--date=short", "--format=%H%x00%h%x00%cd"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return "unknown", None

    parts = result.stdout.strip().split("\x00")
    if len(parts) != 3 or not all(parts):
        return "unknown", None

    full_sha, short_sha, commit_date = parts
    return f"{commit_date} {short_sha}", full_sha


def static_badge_url(label: str, value: str, color: str, style: str = "flat") -> str:
    return "https://img.shields.io/static/v1?" + urlencode(
        {
            "label": label,
            "message": value,
            "color": color,
            "style": style,
        }
    )


def plain_badge(label: str, value: str, color: str) -> str:
    return f"![{label}]({static_badge_url(label, value, color, style='flat-square')})"


def html_badge_line(href: str, src: str, alt: str) -> str:
    return f'  <a href="{href}"><img src="{src}" alt="{alt}"></a>'


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


def replace_line_re(text: str, pattern: re.Pattern[str], replacement: str) -> str:
    lines = text.splitlines()
    replaced = False
    for index, line in enumerate(lines):
        if pattern.search(line):
            lines[index] = replacement
            replaced = True
            break
    if not replaced:
        raise RuntimeError(f"Could not find README line matching {pattern.pattern!r}")
    return "\n".join(lines) + "\n"


def update_summary_line(path: pathlib.Path, summary_line: str) -> None:
    text = path.read_text(encoding="utf-8")
    updated = replace_line(text, "Current workspace totals:", summary_line)
    if updated != text:
        path.write_text(updated, encoding="utf-8")


def main() -> int:
    readme = README_PATH.read_text(encoding="utf-8")

    required_markers = (
        "[![Rust LOC]",
        "[![Tests]",
        "Current workspace totals:",
    )
    present = [m for m in required_markers if m in readme]
    if not present:
        return 0
    missing = [m for m in required_markers if m not in readme]
    if missing:
        print(
            f"ERROR: README is partially instrumented. "
            f"Missing markers: {missing}",
            file=sys.stderr,
        )
        return 1

    loc = rust_loc()
    tests, tests_exact = test_count()
    test_label = f"{tests:,} exact" if tests_exact else f"~{tests:,}"
    crates = workspace_crate_count()
    release_tag = latest_release_tag()
    release_label = release_tag or "unreleased"
    release_href = (
        f"https://github.com/Maleick/TextQuest/releases/tag/{quote(release_tag, safe='')}"
        if release_tag
        else "https://github.com/Maleick/TextQuest/releases"
    )
    last_commit_label, last_commit_sha = last_commit_summary()
    last_commit_href = (
        f"https://github.com/Maleick/TextQuest/commit/{last_commit_sha}"
        if last_commit_sha
        else "https://github.com/Maleick/TextQuest/commits/master"
    )

    updated = replace_line(
        readme,
        "[![Rust LOC]",
        badge("Rust LOC", f"{loc:,}", "blue"),
    )
    updated = replace_line(
        updated,
        "[![Tests]",
        badge("Tests", test_label, "brightgreen"),
    )
    updated = replace_line_re(
        updated,
        CRATES_BADGE_LINE_RE,
        badge("Workspace Crates", f"{crates}", "purple"),
    )
    updated = replace_line_re(
        updated,
        PLATFORM_BADGE_LINE_RE,
        plain_badge("Platform", "Windows | macOS | Linux", "lightgrey"),
    )
    updated = replace_line_re(
        updated,
        RELEASE_BADGE_LINE_RE,
        html_badge_line(
            release_href,
            static_badge_url("release", release_label, "success"),
            "Release",
        ),
    )
    updated = replace_line_re(
        updated,
        LAST_COMMIT_BADGE_LINE_RE,
        html_badge_line(
            last_commit_href,
            static_badge_url("last commit", last_commit_label, "informational"),
            "Last Commit",
        ),
    )
    updated = ACCOUNTS_BADGE_LINE_RE.sub("", updated)
    updated = replace_line(
        updated,
        "Current workspace totals:",
        (
            f"Current workspace totals: {loc:,} Rust lines, {test_label} tests, "
            f"and {crates} workspace crates. Latest release: {release_label}. "
            "This line and the badges above are auto-refreshed by "
            "`scripts/update_readme_metrics.py`."
        ),
    )

    if updated == readme:
        summary_line = (
            f"Current workspace totals: {loc:,} Rust lines, {test_label} tests, "
            f"and {crates} workspace crates. Latest release: {release_label}. "
            "This page and the README badges are auto-refreshed by "
            "`scripts/update_readme_metrics.py`."
        )
        update_summary_line(METRICS_PAGE_PATH, summary_line)
        return 0

    README_PATH.write_text(updated, encoding="utf-8")
    summary_line = (
        f"Current workspace totals: {loc:,} Rust lines, {test_label} tests, "
        f"and {crates} workspace crates. Latest release: {release_label}. "
        "This page and the README badges are auto-refreshed by "
        "`scripts/update_readme_metrics.py`."
    )
    update_summary_line(METRICS_PAGE_PATH, summary_line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
