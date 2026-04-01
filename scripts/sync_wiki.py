#!/usr/bin/env python3
"""Validate and publish repo-tracked GitHub wiki pages for DMFT."""

from __future__ import annotations

import argparse
import base64
import os
import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE_DIR = REPO_ROOT / "docs" / "wiki"
PUBLISH_BRANCH = "master"
COMMIT_MESSAGE = "docs: sync wiki from docs/wiki"

REQUIRED_FILES = [
    "Home.md",
    "Quick-Start.md",
    "Installation-and-Build.md",
    "Operating-the-TUI.md",
    "Command-Reference.md",
    "Architecture-Overview.md",
    "DLL-Injection-and-IPC-Pipeline.md",
    "Combat-and-Camp-Loop.md",
    "Navigation-and-Maps.md",
    "Login-Automation.md",
    "Soul-Engine.md",
    "Configuration.md",
    "Development-Workflow.md",
    "Offsets-EQ-Internals-and-MacroQuest-References.md",
    "Troubleshooting.md",
    "Security-and-Anti-Detection-Notes.md",
    "Roadmap-and-Known-Gaps.md",
    "Maintaining-the-Wiki.md",
    "_Sidebar.md",
]

BANNED_LITERALS = {
    "mq2-reference": "stale pre-submodule reference",
    "Source: third_party/macroquest/src/eqlib": "vendor eqlib path used as a primary source citation",
    "source: third_party/macroquest/src/eqlib": "vendor eqlib path used as a primary source citation",
}


class WikiSyncError(RuntimeError):
    """User-facing wiki sync error."""


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=str(cwd) if cwd else None,
        check=check,
        text=True,
        capture_output=True,
    )


def fail(message: str) -> None:
    raise WikiSyncError(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="Validate docs/wiki only")
    mode.add_argument("--dry-run", action="store_true", help="Materialize a synced wiki checkout and print changes")
    mode.add_argument("--push", action="store_true", help="Sync, commit, and push the wiki")
    parser.add_argument("--wiki-dir", type=Path, help="Use an existing local wiki checkout")
    return parser.parse_args()


def discover_source_files() -> dict[str, Path]:
    if not SOURCE_DIR.is_dir():
        fail(f"Missing canonical wiki source directory: {SOURCE_DIR}")

    nested_markdown = [
        path.relative_to(SOURCE_DIR)
        for path in SOURCE_DIR.rglob("*.md")
        if path.parent != SOURCE_DIR
    ]
    if nested_markdown:
        joined = ", ".join(str(path) for path in sorted(nested_markdown))
        fail(f"docs/wiki must stay flat; nested markdown files found: {joined}")

    files = {path.name: path for path in SOURCE_DIR.glob("*.md")}
    missing = [name for name in REQUIRED_FILES if name not in files]
    if missing:
        fail(f"Missing required wiki pages: {', '.join(missing)}")
    return files


def validate_sources() -> dict[str, Path]:
    files = discover_source_files()
    for name, path in sorted(files.items()):
        text = path.read_text(encoding="utf-8")
        if not text.strip():
            fail(f"Wiki page is empty: {name}")
        for literal, reason in BANNED_LITERALS.items():
            if literal in text:
                fail(f"Banned reference in {name}: {reason} ({literal})")
    return files


def git_remote_repo_full_name() -> str:
    try:
        origin = run(["git", "remote", "get-url", "origin"], cwd=REPO_ROOT).stdout.strip()
    except subprocess.CalledProcessError as exc:
        fail(f"Unable to determine origin remote: {exc.stderr.strip()}")

    normalized = origin.rstrip("/").removesuffix(".git")
    for prefix in ("https://github.com/", "git@github.com:", "ssh://git@github.com/"):
        if normalized.startswith(prefix):
            return normalized[len(prefix) :]
    fail(f"Unsupported GitHub origin URL format: {origin}")
    raise AssertionError("unreachable")


def github_token() -> str:
    token = os.environ.get("GH_TOKEN")
    if token:
        return token
    try:
        token = run(["gh", "auth", "token"], cwd=REPO_ROOT).stdout.strip()
    except subprocess.CalledProcessError as exc:
        fail(
            "Unable to obtain GitHub token. Set GH_TOKEN or run `gh auth login`.\n"
            + exc.stderr.strip()
        )
    if not token:
        fail("GitHub token lookup returned an empty value.")
    return token


def wiki_display_url(repo_full_name: str) -> str:
    return f"https://github.com/{repo_full_name}.wiki.git"


def git_auth_config(token: str) -> str:
    basic = base64.b64encode(f"x-access-token:{token}".encode("utf-8")).decode("ascii")
    return f"http.extraheader=AUTHORIZATION: basic {basic}"


def run_git_authenticated(
    args: list[str],
    *,
    token: str,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    # Use an in-memory auth header for this process invocation so persistent wiki
    # clones can keep a plain https remote URL without writing tokens to .git/config.
    return run(["git", "-c", git_auth_config(token), *args], cwd=cwd, check=check)


def wiki_remote_exists(display_url: str, token: str) -> bool:
    result = run_git_authenticated(
        ["ls-remote", display_url, "HEAD"],
        token=token,
        cwd=REPO_ROOT,
        check=False,
    )
    return result.returncode == 0


def ensure_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def ensure_origin_remote(target_dir: Path, display_url: str) -> None:
    remotes = run(["git", "remote"], cwd=target_dir).stdout.split()
    if "origin" in remotes:
        run(["git", "remote", "set-url", "origin", display_url], cwd=target_dir)
    else:
        run(["git", "remote", "add", "origin", display_url], cwd=target_dir)


def prepare_wiki_checkout(
    target_dir: Path,
    display_url: str,
    remote_exists: bool,
    token: str,
) -> None:
    ensure_dir(target_dir.parent)

    if (target_dir / ".git").is_dir():
        ensure_origin_remote(target_dir, display_url)
        if remote_exists:
            run_git_authenticated(["fetch", "origin"], token=token, cwd=target_dir)
            run(["git", "checkout", PUBLISH_BRANCH], cwd=target_dir)
            run(["git", "reset", "--hard", f"origin/{PUBLISH_BRANCH}"], cwd=target_dir)
        return

    if target_dir.exists() and any(target_dir.iterdir()):
        fail(f"Target wiki directory is not empty and is not a git checkout: {target_dir}")

    if remote_exists:
        target_dir.rmdir()
        run_git_authenticated(["clone", display_url, str(target_dir)], token=token, cwd=REPO_ROOT)
        run(["git", "checkout", PUBLISH_BRANCH], cwd=target_dir)
        return

    run(["git", "init", str(target_dir)], cwd=REPO_ROOT)
    run(["git", "checkout", "-B", PUBLISH_BRANCH], cwd=target_dir)
    ensure_origin_remote(target_dir, display_url)


def sync_files(source_files: dict[str, Path], wiki_dir: Path) -> list[str]:
    actions: list[str] = []
    existing_markdown = {path.name: path for path in wiki_dir.glob("*.md")}

    for name, source in sorted(source_files.items()):
        target = wiki_dir / name
        source_text = source.read_text(encoding="utf-8")
        if not target.exists():
            target.write_text(source_text, encoding="utf-8")
            actions.append(f"ADD    {name}")
            continue
        target_text = target.read_text(encoding="utf-8")
        if target_text != source_text:
            target.write_text(source_text, encoding="utf-8")
            actions.append(f"UPDATE {name}")

    for name, target in sorted(existing_markdown.items()):
        if name not in source_files:
            target.unlink()
            actions.append(f"DELETE {name}")

    return actions


def has_git_changes(wiki_dir: Path) -> bool:
    status = run(["git", "status", "--short"], cwd=wiki_dir)
    return bool(status.stdout.strip())


def commit_changes(wiki_dir: Path) -> None:
    run(["git", "add", "-A"], cwd=wiki_dir)
    if not has_git_changes(wiki_dir):
        return
    run(["git", "commit", "-m", COMMIT_MESSAGE], cwd=wiki_dir)


def push_changes(wiki_dir: Path, display_url: str, token: str) -> None:
    result = run_git_authenticated(
        ["push", "-u", "origin", PUBLISH_BRANCH],
        token=token,
        cwd=wiki_dir,
        check=False,
    )
    if result.returncode == 0:
        return
    stderr = result.stderr.strip()
    if "Repository not found" in stderr:
        fail(
            "GitHub has wiki support enabled for the repository, but the wiki git remote still does not exist: "
            f"{display_url}\n\n"
            "One-time bootstrap required:\n"
            "1. Open the repository's Wiki tab in the GitHub UI.\n"
            "2. Create a first page (Home is fine) and save it.\n"
            "3. Re-run `python3 scripts/sync_wiki.py --push`.\n"
        )
    fail(f"Failed to push wiki changes:\n{stderr}")


def print_actions(actions: list[str], *, title: str) -> None:
    print(title)
    if not actions:
        print("  No content changes.")
        return
    for action in actions:
        print(f"  {action}")


def main() -> int:
    args = parse_args()
    source_files = validate_sources()

    if args.check:
        print(f"Validated {len(source_files)} wiki pages in {SOURCE_DIR}")
        return 0

    repo_full_name = git_remote_repo_full_name()
    token = github_token()
    display_url = wiki_display_url(repo_full_name)
    remote_exists = wiki_remote_exists(display_url, token)

    temp_dir: tempfile.TemporaryDirectory[str] | None = None
    if args.wiki_dir:
        wiki_dir = args.wiki_dir.resolve()
        ensure_dir(wiki_dir)
    else:
        temp_dir = tempfile.TemporaryDirectory(prefix="dmft-wiki-")
        wiki_dir = Path(temp_dir.name) / "wiki"
        ensure_dir(wiki_dir)

    prepare_wiki_checkout(wiki_dir, display_url, remote_exists, token)
    actions = sync_files(source_files, wiki_dir)

    if args.dry_run:
        title = f"Dry run for {display_url} (remote {'exists' if remote_exists else 'missing'})"
        print_actions(actions, title=title)
        print(f"Materialized wiki checkout: {wiki_dir}")
        return 0

    print_actions(actions, title=f"Publishing wiki to {display_url}")
    commit_changes(wiki_dir)
    if has_git_changes(wiki_dir):
        fail("Wiki checkout still has uncommitted changes after commit attempt.")
    push_changes(wiki_dir, display_url, token)
    print(f"Wiki publish complete: {display_url}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except WikiSyncError as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
