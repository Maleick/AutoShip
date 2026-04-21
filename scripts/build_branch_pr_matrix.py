#!/usr/bin/env python3
"""Generate a branch/PR audit matrix with resilient GitHub JSON handling."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
BASE_BRANCH = "origin/master"
OUTPUT_PATH = REPO_ROOT / "BRANCH_PR_MATRIX.md"
REPO_OWNER = "Maleick"


def run(cmd: list[str], *, check: bool = True) -> str:
    result = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"command failed: {' '.join(cmd)}")
    return result.stdout.strip()


def parse_local_branches() -> dict[str, str | None]:
    output = run(["git", "branch", "--format=%(refname:short)\t%(upstream:short)"])
    branches: dict[str, str | None] = {}
    for line in output.splitlines():
        name, upstream = (line.split("\t") + [""])[:2]
        branches[name] = upstream or None
    return branches


def parse_remote_branches() -> set[str]:
    output = run(
        ["git", "branch", "-r", "--format=%(refname:short)"],
        check=False,
    )
    remote_branches = set()
    for line in output.splitlines():
        if line.startswith("origin/"):
            remote_branches.add(line)
    return remote_branches


def upstream_ahead_behind(branch_ref: str) -> tuple[int, int]:
    try:
        counts = run(
            ["git", "rev-list", "--left-right", "--count", f"{branch_ref}...{BASE_BRANCH}"],
            check=False,
        )
    except RuntimeError:
        return 0, 0

    if not counts or "\t" not in counts:
        return 0, 0
    ahead, behind = [int(part.strip()) for part in counts.split("\t")]
    return ahead, behind


def is_clean(branch: str) -> bool:
    # Conservative check: a non-current branch may not be checkoutable in this worktree.
    current = run(["git", "rev-parse", "--abbrev-ref", "HEAD"])
    if current != branch:
        return True
    return run(["git", "status", "--short"], check=False) == ""


def open_prs() -> dict[str, dict]:
    output = run(
        [
            "gh",
            "pr",
            "list",
            "--state",
            "open",
            "--json",
            "number,title,url,headRefName,headRepositoryOwner,baseRefName,mergeable,mergeStateStatus,updatedAt",
            "--limit",
            "200",
        ],
        check=False,
    )
    if not output:
        return {}
    try:
        data = json.loads(output)
    except json.JSONDecodeError:
        return {}
    by_head: dict[str, dict] = {}
    for item in data:
        head_ref = item.get("headRefName")
        owner = (item.get("headRepositoryOwner") or {}).get("login")
        if not head_ref or owner != REPO_OWNER:
            continue
        by_head[head_ref] = item
    return by_head


def render_matrix_rows() -> list[tuple[str, str | None, str | None, str, str]]:
    local_branches = parse_local_branches()
    remote_branches = parse_remote_branches()
    prs = open_prs()
    rows: list[tuple[str, str | None, str | None, str, str]] = []
    used_local = set()

    for branch, upstream in sorted(local_branches.items()):
        used_local.add(branch)
        origin_ref = f"origin/{branch}"
        remote_ref = upstream or (origin_ref if origin_ref in remote_branches else None)
        pr = prs.get(branch)
        pr_label = f"[#{pr['number']}]({pr['url']})" if pr else "none"

        if upstream is None and pr is None:
            status = "orphan"
        elif not is_clean(branch):
            status = "blocked"
        else:
            ahead, behind = (0, 0)
            if upstream:
                ahead, behind = upstream_ahead_behind(upstream)
            elif remote_ref and remote_ref in remote_branches:
                ahead, behind = upstream_ahead_behind(remote_ref)

            if ahead and behind:
                status = "stacked"
            elif ahead:
                status = "ahead"
            elif behind:
                status = "behind"
            else:
                status = "clean"

            if pr is None and ahead == 0 and behind == 0:
                status = "no-PR"

        rows.append((branch, remote_ref, pr_label, status, "local"))

    for branch_name, pr in sorted(prs.items()):
        if branch_name in used_local:
            continue
        remote_ref = f"origin/{branch_name}"
        if remote_ref not in remote_branches:
            status = "remote-missing"
        else:
            ahead, behind = upstream_ahead_behind(remote_ref)
            if ahead and behind:
                status = "stacked"
            elif ahead:
                status = "ahead"
            elif behind:
                status = "behind"
            else:
                status = "clean"
        rows.append((branch_name, remote_ref, f"[#{pr['number']}]({pr['url']})", status, "remote"))

    return rows


def main() -> None:
    rows = render_matrix_rows()
    lines = [
        "# Branch / PR Matrix",
        "",
        "| Branch | Remote | PR | Status | Context |",
        "| --- | --- | --- | --- | --- |",
    ]
    for branch, remote, pr_display, status, context in rows:
        remote_display = remote or "none"
        lines.append(f"| {branch} | {remote_display} | {pr_display} | {status} | {context} |")
    OUTPUT_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
