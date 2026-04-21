#!/usr/bin/env python3
"""Validate and publish repo-tracked GitHub wiki pages for TextQuest."""

from __future__ import annotations

import argparse
import base64
import os
import shutil
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
    "Project-Metrics.md",
    "_Sidebar.md",
]

STATIC_DIRS = ["assets"]

BANNED_LITERALS = {
    "mq2-reference": "stale pre-submodule reference",
    "Source: third_party/macroquest/src/eqlib": "vendor eqlib path used as a primary source citation",
    "source: third_party/macroquest/src/eqlib": "vendor eqlib path used as a primary source citation",
    "third_party/": "deleted local vendor path",
}


class WikiSyncError(RuntimeError):
    """User-facing wiki sync error."""


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            args,
            cwd=str(cwd) if cwd else None,
            check=False,
            text=True,
            capture_output=True,
            env=env,
        )
    except OSError as exc:
        location = f" (cwd: {cwd})" if cwd else ""
        raise WikiSyncError(
            f"Command failed{location}: {' '.join(args)}\n\n"
            f"stderr:\n{exc}"
        ) from exc
    if check and result.returncode != 0:
        location = f" (cwd: {cwd})" if cwd else ""
        details: list[str] = [f"Command failed{location}: {' '.join(args)}"]
        if result.stdout.strip():
            details.append(f"stdout:\n{result.stdout.strip()}")
        if result.stderr.strip():
            details.append(f"stderr:\n{result.stderr.strip()}")
        raise WikiSyncError("\n\n".join(details))
    return result


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
    origin = run(["git", "remote", "get-url", "origin"], cwd=REPO_ROOT).stdout.strip()
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
    if shutil.which("gh") is None:
        fail(
            "GitHub CLI (`gh`) is not installed or not on PATH, and GH_TOKEN is unset.\n"
            "Install `gh` and run `gh auth login`, or provide GH_TOKEN."
        )
    try:
        token = run(["gh", "auth", "token"], cwd=REPO_ROOT).stdout.strip()
    except WikiSyncError as exc:
        fail(
            "Unable to obtain GitHub auth from the local GitHub CLI session.\n"
            "Run `gh auth status` to verify the runner login, then `gh auth login` if needed.\n"
            f"\n{exc}"
        )
    if not token:
        fail(
            "GitHub CLI auth did not return a token. Run `gh auth status` and refresh the local login."
        )
    return token


def wiki_display_url(repo_full_name: str) -> str:
    return f"https://github.com/{repo_full_name}.wiki.git"


def git_auth_config(token: str) -> str:
    basic = base64.b64encode(f"x-access-token:{token}".encode("utf-8")).decode("ascii")
    return f"AUTHORIZATION: basic {basic}"


def git_auth_env(token: str) -> dict[str, str]:
    env = os.environ.copy()
    env["GIT_CONFIG_COUNT"] = "1"
    env["GIT_CONFIG_KEY_0"] = "http.extraheader"
    env["GIT_CONFIG_VALUE_0"] = git_auth_config(token)
    return env


def run_git_authenticated(
    args: list[str],
    *,
    token: str,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    # Use an in-memory Git config override so persistent wiki clones can keep a
    # plain https remote URL without writing tokens to .git/config or putting
    # secrets in the git process argv.
    return run(["git", *args], cwd=cwd, check=check, env=git_auth_env(token))


def wiki_remote_exists(display_url: str, token: str) -> bool:
    result = run_git_authenticated(
        ["ls-remote", display_url, "HEAD"],
        token=token,
        cwd=REPO_ROOT,
        check=False,
    )
    if result.returncode == 0:
        return True

    stderr = (result.stderr or "").strip().lower()
    auth_indicators = (
        "authentication failed",
        "fatal: authentication failed",
        "permission denied",
        "permission to ",
        "http basic: access denied",
        "could not read from remote repository",
        "could not read username",
        "403",
        "401",
        "requires sso",
        "saml sso",
    )
    if any(indicator in stderr for indicator in auth_indicators):
        fail(
            "Failed to authenticate to the GitHub wiki remote when checking whether it exists.\n\n"
            "Please verify `GH_TOKEN` or `gh auth` configuration.\n\n"
            f"Git error output:\n{result.stderr.strip()}"
        )

    return False


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


def regular_files_under(root: Path, *, label: str) -> dict[Path, Path]:
    files: dict[Path, Path] = {}
    root = root.resolve()

    for path in root.rglob("*"):
        if path.is_symlink():
            fail(f"Refusing to follow symlink in {label}: {path}")
        if path.is_file():
            files[path.relative_to(root)] = path

    return files


def sync_static_dirs(wiki_dir: Path) -> list[str]:
    actions: list[str] = []
    for directory_name in STATIC_DIRS:
        source_dir = SOURCE_DIR / directory_name
        if not source_dir.is_dir():
            continue

        target_dir = wiki_dir / directory_name
        source_files = regular_files_under(source_dir, label=f"{directory_name} source")
        target_files = (
            regular_files_under(target_dir, label=f"{directory_name} target")
            if target_dir.exists()
            else {}
        )

        for relative_path, source in sorted(source_files.items()):
            target = target_dir / relative_path
            ensure_dir(target.parent)
            source_bytes = source.read_bytes()
            if not target.exists():
                target.write_bytes(source_bytes)
                actions.append(f"ADD    {directory_name}/{relative_path.as_posix()}")
                continue
            if target.read_bytes() != source_bytes:
                target.write_bytes(source_bytes)
                actions.append(f"UPDATE {directory_name}/{relative_path.as_posix()}")

        for relative_path, target in sorted(target_files.items()):
            if relative_path not in source_files:
                target.unlink()
                actions.append(f"DELETE {directory_name}/{relative_path.as_posix()}")

    return actions


def has_git_changes(wiki_dir: Path) -> bool:
    status = run(["git", "status", "--short"], cwd=wiki_dir)
    return bool(status.stdout.strip())


def commit_changes(wiki_dir: Path) -> None:
    run(["git", "config", "user.name", "github-actions[bot]"], cwd=wiki_dir)
    run(["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"], cwd=wiki_dir)
    run(["git", "add", "-A"], cwd=wiki_dir)
    if not has_git_changes(wiki_dir):
        return
    result = run(["git", "commit", "-m", COMMIT_MESSAGE], cwd=wiki_dir, check=False)
    if result.returncode == 0:
        return

    details: list[str] = []
    stdout = result.stdout.strip()
    if stdout:
        details.append(f"stdout:\n{stdout}")
    stderr = result.stderr.strip()
    if stderr:
        details.append(f"stderr:\n{stderr}")
    fail(
        f"Failed to commit wiki changes (exit code {result.returncode}).\n"
        + ("\n\n".join(details) if details else "git commit exited without any output.")
    )


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
    normalized_stderr = stderr.lower()
    if "repository not found" in normalized_stderr:
        fail(
            "GitHub wiki remote is not initialized yet: "
            f"{display_url}\n\n"
            "One-time bootstrap required:\n"
            "1. Open the repository's Wiki tab in the GitHub UI.\n"
            "2. Create a first page (Home is fine) and save it.\n"
            "3. Re-run `python scripts/sync_wiki.py --push`.\n"
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
        temp_dir = tempfile.TemporaryDirectory(prefix="textquest-wiki-")
        wiki_dir = Path(temp_dir.name) / "wiki"
        ensure_dir(wiki_dir)

    prepare_wiki_checkout(wiki_dir, display_url, remote_exists, token)
    actions = sync_files(source_files, wiki_dir)
    actions.extend(sync_static_dirs(wiki_dir))

    if args.dry_run:
        title = f"Dry run for {display_url} (remote {'exists' if remote_exists else 'missing'})"
        print_actions(actions, title=title)
        if args.wiki_dir:
            print(f"Materialized wiki checkout: {wiki_dir}")
        else:
            print(f"Materialized wiki checkout (ephemeral; deleted after this run): {wiki_dir}")
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
