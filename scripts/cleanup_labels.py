#!/usr/bin/env python3
"""Consolidate redundant GitHub labels via rename-first, delete-after.

Current label surface has three competing priority schemes, two epic schemes,
several inconsistent milestone formats, and ~20 single-use or stale labels.
This script:

1. Renames redundant labels into their keeper (issue assignments transfer
   automatically — GitHub preserves them through PATCH on the label itself).
   Where two source labels must converge onto one target, the second is
   deleted after removal from any lingering issues.
2. Deletes labels that are stale and have zero open-issue usage.
3. Leaves anything with lingering usage untouched and reports it.

Usage:
    export GITHUB_TOKEN=ghp_...        # needs repo scope

    # Dry-run (default):
    python3 scripts/cleanup_labels.py

    # Apply changes:
    python3 scripts/cleanup_labels.py --apply
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

OWNER = "Maleick"
REPO = "TextQuest"
API = "https://api.github.com"

# (source_label, target_label) — PATCH the source label's name to target.
# If target already exists, we instead reassign the source's issues to target and delete source.
RENAMES: list[tuple[str, str]] = [
    ("p1-high",          "slice-p1"),
    ("priority:high",    "slice-p1"),
    ("p2-medium",        "slice-p2"),
    ("epic-tracking",    "epic"),
    ("M7",               "milestone-m7"),
    ("M7.logout",        "milestone-m7"),
    ("M7.harness",       "milestone-m7"),
    ("M7.config",        "milestone-m7"),
]

# Labels to delete outright — only if they have zero open-issue usage.
# Anything still in use gets flagged for manual review.
DELETE_IF_UNUSED: list[str] = [
    "vendor",
    "writing",
    "week-2/3",
    "unit-tests",
    "scenario",
    "release",
    "personality",
    "operations",
    "optimization",
    "monitoring",
    "local-only",
    "loot",
    "gap-analysis",
    "domain:nav",
    "banking",
    "audio",
    "sdk",
    "movement",
    "combat",
    "ipc",
    "web-dashboard",
]


def api_request(token: str, method: str, path: str, body: dict | None = None) -> dict | list | None:
    url = f"{API}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"bearer {token}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req) as resp:
            text = resp.read().decode()
            return json.loads(text) if text else None
    except urllib.error.HTTPError as exc:
        body_text = exc.read().decode(errors="replace")
        raise RuntimeError(f"{method} {path} -> HTTP {exc.code}: {body_text}") from None


def label_exists(token: str, name: str) -> bool:
    try:
        api_request(token, "GET", f"/repos/{OWNER}/{REPO}/labels/{urllib.parse.quote(name)}")
        return True
    except RuntimeError as exc:
        if "HTTP 404" in str(exc):
            return False
        raise


def rename_label(token: str, old: str, new: str) -> None:
    api_request(
        token,
        "PATCH",
        f"/repos/{OWNER}/{REPO}/labels/{urllib.parse.quote(old)}",
        {"new_name": new},
    )


def list_open_issues_with_label(token: str, label: str) -> list[dict]:
    out: list[dict] = []
    page = 1
    while True:
        qs = urllib.parse.urlencode(
            {"state": "open", "labels": label, "per_page": "100", "page": str(page)}
        )
        data = api_request(token, "GET", f"/repos/{OWNER}/{REPO}/issues?{qs}")
        assert isinstance(data, list)
        out.extend(d for d in data if "pull_request" not in d)
        if len(data) < 100:
            break
        page += 1
    return out


def add_label_to_issue(token: str, issue_number: int, label: str) -> None:
    api_request(
        token,
        "POST",
        f"/repos/{OWNER}/{REPO}/issues/{issue_number}/labels",
        {"labels": [label]},
    )


def remove_label_from_issue(token: str, issue_number: int, label: str) -> None:
    api_request(
        token,
        "DELETE",
        f"/repos/{OWNER}/{REPO}/issues/{issue_number}/labels/{urllib.parse.quote(label)}",
    )


def delete_label(token: str, name: str) -> None:
    api_request(token, "DELETE", f"/repos/{OWNER}/{REPO}/labels/{urllib.parse.quote(name)}")


def migrate_or_rename(token: str, apply: bool, old: str, new: str) -> None:
    if not label_exists(token, old):
        print(f"  skip (missing): {old}")
        return

    if not label_exists(token, new):
        print(f"  rename: {old} -> {new}")
        if apply:
            rename_label(token, old, new)
        return

    # Target exists; reassign any remaining issues, then delete source.
    issues = list_open_issues_with_label(token, old)
    print(f"  merge: {old} -> {new} ({len(issues)} issue(s) to migrate)")
    if apply:
        for issue in issues:
            add_label_to_issue(token, issue["number"], new)
            remove_label_from_issue(token, issue["number"], old)
        delete_label(token, old)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--apply", action="store_true", help="Actually mutate labels (default is dry-run)")
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("GITHUB_TOKEN not set", file=sys.stderr)
        return 1

    print("=== Step 1: renames / merges ===")
    for old, new in RENAMES:
        migrate_or_rename(token, args.apply, old, new)

    print("\n=== Step 2: delete-if-unused ===")
    for name in DELETE_IF_UNUSED:
        if not label_exists(token, name):
            print(f"  skip (missing): {name}")
            continue
        issues = list_open_issues_with_label(token, name)
        if issues:
            print(f"  KEEP (in use): {name} — {len(issues)} open issue(s): {[i['number'] for i in issues[:8]]}")
        else:
            print(f"  delete (unused): {name}")
            if args.apply:
                delete_label(token, name)

    if not args.apply:
        print("\n(dry-run — re-run with --apply)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
