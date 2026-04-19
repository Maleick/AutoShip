#!/usr/bin/env python3
"""Delete GitHub Projects v2 attached to the TextQuest repo/owner.

Projects v2 is no longer used for tracking; native milestones and issue labels
replace it. This script enumerates every project owned by the repo owner and
every project linked to the repository, then deletes them on confirmation.

Usage:
    # Set your GitHub token (needs project scope, classic PAT or fine-grained
    # with Projects R/W):
    export GITHUB_TOKEN=ghp_...

    # Dry-run (default) — lists projects, does not delete:
    python3 scripts/delete_projects.py

    # Actually delete:
    python3 scripts/delete_projects.py --apply

    # Skip the interactive confirmation prompt:
    python3 scripts/delete_projects.py --apply --yes
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

OWNER = "Maleick"
REPO = "TextQuest"
GRAPHQL_URL = "https://api.github.com/graphql"


def graphql(token: str, query: str, variables: dict | None = None) -> dict:
    payload = json.dumps({"query": query, "variables": variables or {}}).encode()
    req = urllib.request.Request(
        GRAPHQL_URL,
        data=payload,
        headers={
            "Authorization": f"bearer {token}",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as exc:
        body = exc.read().decode(errors="replace")
        print(f"HTTP {exc.code}: {body}", file=sys.stderr)
        sys.exit(1)


def list_owner_projects(token: str, login: str) -> list[dict]:
    query = """
    query($login: String!, $cursor: String) {
      user(login: $login) {
        projectsV2(first: 50, after: $cursor) {
          pageInfo { hasNextPage endCursor }
          nodes {
            id
            number
            title
            closed
            updatedAt
            items(first: 1) { totalCount }
          }
        }
      }
    }
    """
    out: list[dict] = []
    cursor = None
    while True:
        result = graphql(token, query, {"login": login, "cursor": cursor})
        if result.get("errors"):
            # Fall back to org query if user query fails.
            break
        user = result["data"]["user"]
        if not user:
            break
        page = user["projectsV2"]
        out.extend(page["nodes"])
        if not page["pageInfo"]["hasNextPage"]:
            break
        cursor = page["pageInfo"]["endCursor"]
    return out


def list_repo_projects(token: str, owner: str, name: str) -> list[dict]:
    query = """
    query($owner: String!, $name: String!, $cursor: String) {
      repository(owner: $owner, name: $name) {
        projectsV2(first: 50, after: $cursor) {
          pageInfo { hasNextPage endCursor }
          nodes {
            id
            number
            title
            closed
            updatedAt
            items(first: 1) { totalCount }
          }
        }
      }
    }
    """
    out: list[dict] = []
    cursor = None
    while True:
        result = graphql(token, query, {"owner": owner, "name": name, "cursor": cursor})
        if result.get("errors"):
            print(f"GraphQL errors: {result['errors']}", file=sys.stderr)
            break
        page = result["data"]["repository"]["projectsV2"]
        out.extend(page["nodes"])
        if not page["pageInfo"]["hasNextPage"]:
            break
        cursor = page["pageInfo"]["endCursor"]
    return out


def delete_project(token: str, project_id: str) -> None:
    mutation = """
    mutation($id: ID!) {
      deleteProjectV2(input: {projectId: $id}) { projectV2 { id } }
    }
    """
    result = graphql(token, mutation, {"id": project_id})
    if result.get("errors"):
        print(f"  FAILED: {result['errors']}", file=sys.stderr)
    else:
        print("  deleted")


def dedupe(projects: list[dict]) -> list[dict]:
    seen = set()
    out = []
    for p in projects:
        if p["id"] in seen:
            continue
        seen.add(p["id"])
        out.append(p)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--apply", action="store_true", help="Actually delete (default is dry-run)")
    parser.add_argument("--yes", action="store_true", help="Skip interactive confirmation")
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("GITHUB_TOKEN not set", file=sys.stderr)
        return 1

    owner_projects = list_owner_projects(token, OWNER)
    repo_projects = list_repo_projects(token, OWNER, REPO)
    projects = dedupe(owner_projects + repo_projects)

    if not projects:
        print("No projects found.")
        return 0

    print(f"Found {len(projects)} project(s):")
    for p in projects:
        print(f"  #{p['number']} {p['title']!r} — {p['items']['totalCount']} items, updated {p['updatedAt']}, closed={p['closed']}")
        print(f"    id={p['id']}")

    if not args.apply:
        print("\n(dry-run — re-run with --apply to delete)")
        return 0

    if not args.yes:
        try:
            answer = input(f"\nDelete all {len(projects)} project(s)? [y/N] ").strip().lower()
        except EOFError:
            answer = ""
        if answer != "y":
            print("Aborted.")
            return 1

    for p in projects:
        print(f"Deleting #{p['number']} {p['title']!r}")
        delete_project(token, p["id"])

    return 0


if __name__ == "__main__":
    sys.exit(main())
