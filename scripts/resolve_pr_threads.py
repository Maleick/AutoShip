#!/usr/bin/env python3
"""Bulk-resolve all review threads on a GitHub PR.

Usage:
    # Set your GitHub token (needs repo scope):
    export GITHUB_TOKEN=ghp_...

    # Resolve all threads on PR #761:
    python3 scripts/resolve_pr_threads.py 761

    # Dry-run (show threads without resolving):
    python3 scripts/resolve_pr_threads.py --dry-run 761
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request

OWNER = "maleick"
REPO = "textquest"
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
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read())


def get_unresolved_threads(token: str, pr_number: int) -> list[dict]:
    """Fetch all review threads, return unresolved ones with their node IDs."""
    threads = []
    cursor = None

    while True:
        after = f', after: "{cursor}"' if cursor else ""
        query = f"""
        {{
          repository(owner: "{OWNER}", name: "{REPO}") {{
            pullRequest(number: {pr_number}) {{
              reviewThreads(first: 100{after}) {{
                pageInfo {{ hasNextPage endCursor }}
                nodes {{
                  id
                  isResolved
                  isOutdated
                  comments(first: 1) {{
                    nodes {{
                      body
                      path
                      author {{ login }}
                    }}
                  }}
                }}
              }}
            }}
          }}
        }}
        """
        result = graphql(token, query)
        if "errors" in result:
            print(f"GraphQL errors: {result['errors']}", file=sys.stderr)
            sys.exit(1)

        data = result["data"]["repository"]["pullRequest"]["reviewThreads"]
        for node in data["nodes"]:
            if not node["isResolved"]:
                threads.append(node)

        if data["pageInfo"]["hasNextPage"]:
            cursor = data["pageInfo"]["endCursor"]
        else:
            break

    return threads


def resolve_thread(token: str, thread_id: str) -> bool:
    """Resolve a single review thread by its GraphQL node ID."""
    query = """
    mutation($threadId: ID!) {
      resolveReviewThread(input: {threadId: $threadId}) {
        thread { id isResolved }
      }
    }
    """
    result = graphql(token, query, {"threadId": thread_id})
    if "errors" in result:
        print(f"  Error: {result['errors']}", file=sys.stderr)
        return False
    return True


def main() -> None:
    parser = argparse.ArgumentParser(description="Bulk-resolve PR review threads")
    parser.add_argument("pr_number", type=int, help="Pull request number")
    parser.add_argument(
        "--dry-run", action="store_true", help="Show threads without resolving"
    )
    parser.add_argument(
        "--token",
        default=None,
        help="GitHub token (default: GITHUB_TOKEN env var)",
    )
    args = parser.parse_args()

    import os

    token = args.token or os.environ.get("GITHUB_TOKEN")
    if not token:
        print("Error: Set GITHUB_TOKEN or pass --token", file=sys.stderr)
        sys.exit(1)

    print(f"Fetching unresolved threads for PR #{args.pr_number}...")
    threads = get_unresolved_threads(token, args.pr_number)
    print(f"Found {len(threads)} unresolved thread(s).")

    if not threads:
        print("Nothing to resolve.")
        return

    for i, thread in enumerate(threads, 1):
        comments = thread["comments"]["nodes"]
        first = comments[0] if comments else {}
        path = first.get("path", "?")
        author = first.get("author", {}).get("login", "?")
        body_preview = (first.get("body", "")[:80] + "...") if first.get("body") else ""
        print(f"  [{i}/{len(threads)}] {path} by {author}: {body_preview}")

        if not args.dry_run:
            if resolve_thread(token, thread["id"]):
                print(f"    Resolved.")
            else:
                print(f"    Failed to resolve.", file=sys.stderr)

    if args.dry_run:
        print(f"\nDry run complete. Use without --dry-run to resolve all threads.")
    else:
        print(f"\nDone. Resolved {len(threads)} thread(s).")


if __name__ == "__main__":
    main()
