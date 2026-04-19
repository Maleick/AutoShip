#!/usr/bin/env python3
"""Create native GitHub milestones and migrate issues from ``milestone-mN`` labels.

Open issues currently carry milestone-as-label (``milestone-m7`` etc.) but no
native GitHub milestone is set, so the Milestones view is empty. This script:

1. Creates native milestones M4, M7, M8, M9, M10, M11 (idempotent — skips
   existing ones matched by title).
2. For each open issue carrying a ``milestone-mN`` label, assigns the matching
   native milestone.
3. Leaves the ``milestone-mN`` labels in place for one release cycle (they're
   cleaned up separately via scripts/cleanup_labels.py after verification).

Usage:
    export GITHUB_TOKEN=ghp_...        # needs repo scope

    # Dry-run (default):
    python3 scripts/setup_milestones.py

    # Create milestones + assign issues:
    python3 scripts/setup_milestones.py --apply
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

# (label, title, description)
MILESTONES: list[tuple[str, str, str]] = [
    ("milestone-m4",  "M4 — Combat",              "Combat automation: ClassStrategy trait, 17 classes, HolyShit system, puller FSM."),
    ("milestone-m7",  "M7 — Zoning & Movement",   "Zone transitions, teleport/zone-in handling, movement state machines, navmesh integration."),
    ("milestone-m8",  "M8 — Orchestrator",        "Multi-client orchestration, formation assignment, group coordination, leader follows."),
    ("milestone-m9",  "M9 — Learning / RL",       "Reinforcement-learning loops, behavior cloning, experience logging, safety guardrails."),
    ("milestone-m10", "M10 — Economy",            "Loot disposition, vendor/bank flows, item intent wishlist, trade-price DB, ledger schema."),
    ("milestone-m11", "M11 — Soul Engine + LLM",  "LLM-backed personalities, persistent memory, social dynamics, chat orchestration."),
]


def api_request(token: str, method: str, path: str, body: dict | None = None) -> dict | list:
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
            return json.loads(text) if text else {}
    except urllib.error.HTTPError as exc:
        body_text = exc.read().decode(errors="replace")
        raise RuntimeError(f"{method} {path} -> HTTP {exc.code}: {body_text}") from None


def list_existing_milestones(token: str) -> dict[str, int]:
    out: dict[str, int] = {}
    page = 1
    while True:
        qs = urllib.parse.urlencode({"state": "all", "per_page": "100", "page": str(page)})
        data = api_request(token, "GET", f"/repos/{OWNER}/{REPO}/milestones?{qs}")
        assert isinstance(data, list)
        for m in data:
            out[m["title"]] = m["number"]
        if len(data) < 100:
            break
        page += 1
    return out


def create_milestone(token: str, title: str, description: str) -> int:
    resp = api_request(
        token,
        "POST",
        f"/repos/{OWNER}/{REPO}/milestones",
        {"title": title, "description": description, "state": "open"},
    )
    assert isinstance(resp, dict)
    return resp["number"]


def list_open_issues_with_label(token: str, label: str) -> list[dict]:
    out: list[dict] = []
    page = 1
    while True:
        qs = urllib.parse.urlencode(
            {"state": "open", "labels": label, "per_page": "100", "page": str(page)}
        )
        data = api_request(token, "GET", f"/repos/{OWNER}/{REPO}/issues?{qs}")
        assert isinstance(data, list)
        # Filter out PRs (the issues endpoint returns both).
        items = [d for d in data if "pull_request" not in d]
        out.extend(items)
        if len(data) < 100:
            break
        page += 1
    return out


def assign_milestone(token: str, issue_number: int, milestone_number: int) -> None:
    api_request(
        token,
        "PATCH",
        f"/repos/{OWNER}/{REPO}/issues/{issue_number}",
        {"milestone": milestone_number},
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--apply", action="store_true", help="Actually create milestones and assign (default is dry-run)")
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("GITHUB_TOKEN not set", file=sys.stderr)
        return 1

    existing = list_existing_milestones(token)
    print(f"Existing milestones: {len(existing)}")

    planned_creates: list[tuple[str, str, str]] = []
    title_to_number: dict[str, int] = {}
    for label, title, desc in MILESTONES:
        if title in existing:
            title_to_number[title] = existing[title]
            print(f"  skip (exists): {title} -> #{existing[title]}")
        else:
            planned_creates.append((label, title, desc))
            print(f"  create: {title}")

    if args.apply:
        for label, title, desc in planned_creates:
            n = create_milestone(token, title, desc)
            title_to_number[title] = n
            print(f"  created #{n}: {title}")

    print("\nAssignment plan:")
    total_assign = 0
    for label, title, _desc in MILESTONES:
        issues = list_open_issues_with_label(token, label)
        print(f"  {label} -> {title}: {len(issues)} issue(s)")
        for issue in issues:
            current = issue.get("milestone")
            if current and current["title"] == title:
                continue
            total_assign += 1
            if args.apply:
                target = title_to_number.get(title)
                if target is None:
                    print(f"    SKIP #{issue['number']} — no target milestone number")
                    continue
                assign_milestone(token, issue["number"], target)
                print(f"    assigned #{issue['number']} -> {title}")
            else:
                print(f"    would assign #{issue['number']} -> {title}")

    print(f"\n{'Assigned' if args.apply else 'Would assign'} {total_assign} issue(s).")
    if not args.apply:
        print("(dry-run — re-run with --apply)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
