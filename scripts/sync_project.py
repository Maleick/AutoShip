#!/usr/bin/env python3
"""Promote mature checkpoint items to GitHub Project / issues and record sync results.

Usage:
    python scripts/sync_project.py [options]

Typical nightly invocation (from automation.toml):
    python scripts/sync_project.py --promote --record

Options:
    --state FILE     Autoresearch state file (default: autoresearch-state.json)
    --roadmap FILE   Canonical roadmap file (default: docs/implementation-roadmap.md)
    --out FILE       Where to write sync results (default: autoresearch-project-sync.json)
    --promote        Actually call `gh` to add/update project items and create issues.
                     Without this flag the script runs in dry-run mode.
    --record         Write the sync results file even in dry-run mode.
    --project-url    GitHub Project URL (default: https://github.com/users/Maleick/projects/1)
    --json           Print JSON summary to stdout
    --verbose        Print each item action as it happens

Exit codes:
    0  All eligible items processed; results file written (if --record).
    1  Fatal error (bad state file, verifier failed, gh not found in --promote mode).
    2  Partial success: some items failed; results file still written.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent

DEFAULT_STATE_FILE  = REPO_ROOT / "autoresearch-state.json"
DEFAULT_ROADMAP     = REPO_ROOT / "docs" / "implementation-roadmap.md"
DEFAULT_OUT_FILE    = REPO_ROOT / "autoresearch-project-sync.json"
DEFAULT_PROJECT_URL = "https://github.com/users/Maleick/projects/1"

# Evidence states ordered from weakest to strongest.
EVIDENCE_ORDER = [
    "Provisional",
    "Research-backed",
    "Needs Live Proof",
    "Live-validated",
    "Invalidated",
]
MIN_PROMOTION_EVIDENCE = "Research-backed"


# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass
class CheckpointItem:
    """A single candidate extracted from the autoresearch state."""

    title: str
    milestone: str
    domain: str
    evidence_state: str
    source_doc: str
    citation: str
    repo_fit: str
    slice_task: str
    checkpoint_batch: str
    target_window: str
    effort: str
    priority: str
    item_type: str
    raw: dict[str, Any]


@dataclass
class SyncAction:
    title: str
    action: str          # "promoted_issue" | "updated_project" | "skipped" | "error"
    reason: str
    issue_url: str = ""
    error: str = ""


@dataclass
class SyncResult:
    run_at: str
    dry_run: bool
    project_url: str
    state_file: str
    roadmap_file: str
    items_seen: int
    items_promoted: int
    items_updated_project: int
    items_skipped: int
    items_errored: int
    actions: list[SyncAction] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def evidence_rank(state: str) -> int:
    try:
        return EVIDENCE_ORDER.index(state)
    except ValueError:
        return -1


def is_mature(item: CheckpointItem) -> tuple[bool, str]:
    """Return (True, '') or (False, reason) based on promotion criteria."""
    if evidence_rank(item.evidence_state) < evidence_rank(MIN_PROMOTION_EVIDENCE):
        return False, f"evidence state is {item.evidence_state!r}; minimum is {MIN_PROMOTION_EVIDENCE!r}"
    if not item.citation.strip():
        return False, "no citation"
    if not item.repo_fit.strip():
        return False, "no repo-fit rationale"
    if not item.slice_task.strip():
        return False, "no concrete slice/validation task"
    return True, ""


def run_verifier(roadmap: Path) -> tuple[bool, str]:
    """Run the roadmap verifier; return (passed, output)."""
    cmd = [
        sys.executable,
        str(REPO_ROOT / "scripts" / "validate_roadmap_unknowns.py"),
        "--plan", str(roadmap),
        "--domains", "packet,zoning,anticheat",
        "--strict",
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        output = result.stdout + result.stderr
        return result.returncode == 0, output.strip()
    except FileNotFoundError as exc:
        return False, f"verifier not found: {exc}"


def gh(*args: str, capture: bool = True) -> tuple[int, str]:
    """Run a `gh` CLI command; return (returncode, stdout+stderr)."""
    cmd = ["gh", *args]
    try:
        result = subprocess.run(cmd, capture_output=capture, text=True)
        output = (result.stdout or "") + (result.stderr or "")
        return result.returncode, output.strip()
    except FileNotFoundError:
        return 127, "gh CLI not found; install GitHub CLI or skip --promote"


# ---------------------------------------------------------------------------
# State file parsing
# ---------------------------------------------------------------------------


def load_state(path: Path) -> list[CheckpointItem]:
    """Parse autoresearch-state.json and extract checkpoint items.

    The state file is written by the Codex research agent and its schema is
    flexible.  We tolerate missing fields by using empty-string defaults so
    the rest of the script can apply the promotion criteria cleanly.
    """
    if not path.is_file():
        return []

    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Cannot parse state file {path}: {exc}") from exc

    raw_items: list[dict[str, Any]] = []

    # Support a few common shapes the research agent might produce.
    if isinstance(data, list):
        raw_items = data
    elif isinstance(data, dict):
        for key in ("checkpoint_items", "items", "candidates", "slice_candidates"):
            if isinstance(data.get(key), list):
                raw_items = data[key]
                break
        if not raw_items:
            # Flat dict representing a single item
            if "title" in data or "milestone" in data:
                raw_items = [data]

    items: list[CheckpointItem] = []
    for raw in raw_items:
        if not isinstance(raw, dict):
            continue
        items.append(CheckpointItem(
            title            = str(raw.get("title", "")).strip(),
            milestone        = str(raw.get("milestone", "")).strip(),
            domain           = str(raw.get("domain", "")).strip(),
            evidence_state   = str(raw.get("evidence_state", "Provisional")).strip(),
            source_doc       = str(raw.get("source_doc", "")).strip(),
            citation         = str(raw.get("citation", "")).strip(),
            repo_fit         = str(raw.get("repo_fit", "")).strip(),
            slice_task       = str(raw.get("slice_task", raw.get("task", ""))).strip(),
            checkpoint_batch = str(raw.get("checkpoint_batch", "")).strip(),
            target_window    = str(raw.get("target_window", "")).strip(),
            effort           = str(raw.get("effort", "")).strip(),
            priority         = str(raw.get("priority", "")).strip(),
            item_type        = str(raw.get("item_type", "task")).strip(),
            raw              = raw,
        ))

    return [i for i in items if i.title]


# ---------------------------------------------------------------------------
# GitHub Project / issue helpers
# ---------------------------------------------------------------------------


def create_github_issue(item: CheckpointItem, dry_run: bool, verbose: bool) -> tuple[str, str]:
    """Create a GitHub issue for a mature checkpoint item.

    Returns (issue_url, error_message).  On dry-run, returns a placeholder URL.
    """
    body_lines = [
        f"**Milestone**: {item.milestone}",
        f"**Domain**: {item.domain}",
        f"**Evidence State**: {item.evidence_state}",
        f"**Checkpoint Batch**: {item.checkpoint_batch}",
        f"**Source Doc**: {item.source_doc}",
        "",
        "## Slice / Validation Task",
        "",
        item.slice_task,
        "",
        "## Repo-Fit Rationale",
        "",
        item.repo_fit,
        "",
        "## Citation",
        "",
        item.citation,
        "",
        "---",
        "_Promoted from checkpoint draft by dmft-night-research automation._",
    ]
    body = "\n".join(body_lines)

    label = f"milestone:{item.milestone.lower()}" if item.milestone else "roadmap"

    if dry_run:
        if verbose:
            print(f"  [dry-run] would create issue: {item.title!r}")
        return "[dry-run]", ""

    code, out = gh(
        "issue", "create",
        "--title", item.title,
        "--body", body,
        "--label", label,
    )
    if code != 0:
        return "", out
    # gh prints the issue URL on success
    url = out.strip().splitlines()[-1] if out.strip() else ""
    return url, ""


def add_project_item(item: CheckpointItem, project_url: str, dry_run: bool, verbose: bool) -> str:
    """Add or update a draft item in the GitHub Project.

    Returns an error string or empty string on success.
    """
    if dry_run:
        if verbose:
            print(f"  [dry-run] would add project draft: {item.title!r}")
        return ""

    # Add a draft item (no linked issue yet).
    code, out = gh(
        "project", "item-add", _project_number(project_url),
        "--owner", _project_owner(project_url),
        "--title", item.title,
    )
    if code != 0:
        return out
    return ""


def _project_number(url: str) -> str:
    m = re.search(r"/projects/(\d+)", url)
    return m.group(1) if m else "1"


def _project_owner(url: str) -> str:
    m = re.search(r"github\.com/(?:users|orgs)/([^/]+)/projects", url)
    return m.group(1) if m else "@me"


# ---------------------------------------------------------------------------
# Core sync logic
# ---------------------------------------------------------------------------


def sync_items(
    items: list[CheckpointItem],
    project_url: str,
    dry_run: bool,
    verbose: bool,
) -> list[SyncAction]:
    actions: list[SyncAction] = []

    for item in items:
        mature, reason = is_mature(item)

        if not mature:
            if verbose:
                print(f"  skip  {item.title!r}: {reason}")
            actions.append(SyncAction(
                title  = item.title,
                action = "skipped",
                reason = reason,
            ))
            continue

        # Mature item → create a GitHub issue and link it in the project.
        if verbose:
            print(f"  promote  {item.title!r}")

        url, err = create_github_issue(item, dry_run=dry_run, verbose=verbose)
        if err:
            actions.append(SyncAction(
                title  = item.title,
                action = "error",
                reason = "gh issue create failed",
                error  = err,
            ))
            continue

        # Add to GitHub Project (best-effort; continue on failure).
        proj_err = add_project_item(item, project_url, dry_run=dry_run, verbose=verbose)
        if proj_err and verbose:
            print(f"    warning: project item add failed: {proj_err}")

        actions.append(SyncAction(
            title     = item.title,
            action    = "promoted_issue",
            reason    = "mature item: citation, repo-fit, and slice task present",
            issue_url = url,
        ))

    return actions


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--state",       default=str(DEFAULT_STATE_FILE),  metavar="FILE")
    parser.add_argument("--roadmap",     default=str(DEFAULT_ROADMAP),     metavar="FILE")
    parser.add_argument("--out",         default=str(DEFAULT_OUT_FILE),    metavar="FILE")
    parser.add_argument("--project-url", default=DEFAULT_PROJECT_URL)
    parser.add_argument("--promote",     action="store_true",
                        help="Actually call gh CLI (default: dry-run)")
    parser.add_argument("--record",      action="store_true",
                        help="Write the results file even in dry-run mode")
    parser.add_argument("--json",        action="store_true",
                        help="Print JSON summary to stdout")
    parser.add_argument("--verbose",     action="store_true")
    return parser.parse_args(argv)


def build_result(
    actions: list[SyncAction],
    *,
    dry_run: bool,
    project_url: str,
    state_file: Path,
    roadmap_file: Path,
    items_seen: int,
) -> SyncResult:
    return SyncResult(
        run_at                = datetime.now(tz=timezone.utc).isoformat(),
        dry_run               = dry_run,
        project_url           = project_url,
        state_file            = str(state_file),
        roadmap_file          = str(roadmap_file),
        items_seen            = items_seen,
        items_promoted        = sum(1 for a in actions if a.action == "promoted_issue"),
        items_updated_project = sum(1 for a in actions if a.action == "updated_project"),
        items_skipped         = sum(1 for a in actions if a.action == "skipped"),
        items_errored         = sum(1 for a in actions if a.action == "error"),
        actions               = actions,
    )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    state_file   = Path(args.state)
    roadmap_file = Path(args.roadmap)
    out_file     = Path(args.out)
    dry_run      = not args.promote

    # ------------------------------------------------------------------
    # Step 1: run the roadmap verifier before touching the project.
    # ------------------------------------------------------------------
    if args.verbose:
        print("Running roadmap verifier …")
    passed, verifier_out = run_verifier(roadmap_file)
    if not passed:
        print(f"error: roadmap verifier failed — aborting project sync\n{verifier_out}",
              file=sys.stderr)
        return 1
    if args.verbose:
        print(f"Verifier passed.\n{verifier_out}")

    # ------------------------------------------------------------------
    # Step 2: load checkpoint items from autoresearch state.
    # ------------------------------------------------------------------
    try:
        items = load_state(state_file)
    except RuntimeError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.verbose:
        print(f"Loaded {len(items)} checkpoint item(s) from {state_file}")

    # ------------------------------------------------------------------
    # Step 3: sync items (promote mature, skip provisional).
    # ------------------------------------------------------------------
    actions = sync_items(items, args.project_url, dry_run=dry_run, verbose=args.verbose)

    # ------------------------------------------------------------------
    # Step 4: record results.
    # ------------------------------------------------------------------
    result = build_result(
        actions,
        dry_run      = dry_run,
        project_url  = args.project_url,
        state_file   = state_file,
        roadmap_file = roadmap_file,
        items_seen   = len(items),
    )

    payload = asdict(result)

    if args.json or not args.verbose:
        print(json.dumps(payload, indent=2))

    if args.promote or args.record:
        out_file.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        if args.verbose:
            print(f"Sync results written to {out_file}")

    if result.items_errored > 0:
        return 2

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
