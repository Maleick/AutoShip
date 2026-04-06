#!/usr/bin/env python3
"""Validate the canonical roadmap structure and count unresolved unknowns."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ROADMAP = REPO_ROOT / "docs" / "implementation-roadmap.md"

EXPECTED_MILESTONES = [
    ("M5", "Packet Engine"),
    ("M6", "Zoning/Movement"),
    ("M7", "Anti-Cheat"),
    ("M8", "Orchestrator"),
    ("M9", "Learning/RL"),
    ("M10", "Soul Engine + LLM"),
    ("M11", "Economy"),
]

EXPECTED_EVIDENCE_STATES = [
    "Provisional",
    "Research-backed",
    "Needs Live Proof",
    "Live-validated",
    "Invalidated",
]

EXPECTED_DOMAINS = [
    "Packet Engine",
    "Zoning/Movement",
    "Anti-Cheat",
    "Orchestrator",
    "Docs/Workflow",
]

EXPECTED_EVIDENCE_RULES = [
    "Research-backed items may enter active execution work.",
    "Live-validated is required to retire critical unknowns or satisfy milestone exit gates.",
    "Provisional items may exist in research ledgers and checkpoint intake, but they should not be used as proof of milestone completion.",
]


@dataclass
class RoadmapReport:
    roadmap: str
    milestone_ids: list[str]
    missing_milestones: list[str]
    missing_domains: list[str]
    requested_domains: list[str]
    missing_requested_domains: list[str]
    missing_evidence_states: list[str]
    missing_evidence_rules: list[str]
    missing_sections: list[str]
    unresolved_critical_unknowns: int
    provisional_unknowns: list[str]
    evidence_mechanics_ok: bool
    ready_for_autoresearch: bool


class ValidationError(RuntimeError):
    """User-facing validation failure."""


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--roadmap",
        "--plan",
        dest="roadmap",
        default=str(DEFAULT_ROADMAP),
        help="Path to docs/implementation-roadmap.md",
    )
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Return non-zero when the roadmap is not ready for autoresearch",
    )
    parser.add_argument(
        "--domains",
        help="Optional comma-separated domain aliases to require, for example packet,zoning,anticheat",
    )
    return parser.parse_args(argv)


def load_text(path: Path) -> str:
    if not path.is_file():
        raise ValidationError(f"Roadmap file does not exist: {path}")
    return path.read_text(encoding="utf-8")


def normalize_text(text: str) -> str:
    return text.replace("`", "")


def extract_bullets_under_heading(text: str, heading: str) -> list[str]:
    lines = text.splitlines()
    start = None
    heading_pattern = re.compile(rf"^###\s+{re.escape(heading)}\s*$")
    for index, line in enumerate(lines):
        if heading_pattern.match(line.strip()):
            start = index + 1
            break
    if start is None:
        return []

    bullets: list[str] = []
    for line in lines[start:]:
        if line.startswith("### "):
            break
        stripped = line.strip()
        if stripped.startswith("- "):
            bullets.append(stripped[2:].strip())
    return bullets


def parse_requested_domains(raw: str | None) -> list[str]:
    if not raw:
        return []

    alias_map = {
        "packet": "Packet Engine",
        "packet engine": "Packet Engine",
        "zoning": "Zoning/Movement",
        "movement": "Zoning/Movement",
        "zoning/movement": "Zoning/Movement",
        "anticheat": "Anti-Cheat",
        "anti-cheat": "Anti-Cheat",
        "orchestrator": "Orchestrator",
        "docs": "Docs/Workflow",
        "workflow": "Docs/Workflow",
        "docs/workflow": "Docs/Workflow",
    }
    requested: list[str] = []
    for chunk in raw.split(","):
        key = chunk.strip().lower()
        if not key:
            continue
        requested.append(alias_map.get(key, chunk.strip()))
    return requested


def collect_milestones(text: str) -> tuple[list[str], list[str]]:
    found = re.findall(r"^### `?(M\d+(?:\.\d+)?)`?\s+(.+)$", text, flags=re.MULTILINE)
    milestone_map = {milestone: title.strip() for milestone, title in found}
    milestone_ids = [milestone for milestone, _ in EXPECTED_MILESTONES if milestone in milestone_map]
    missing = [milestone for milestone, _ in EXPECTED_MILESTONES if milestone not in milestone_map]
    return milestone_ids, missing


def analyze_roadmap(path: Path, *, requested_domains: list[str] | None = None) -> RoadmapReport:
    text = load_text(path)
    normalized_text = normalize_text(text)

    if "# TextQuest Implementation Roadmap" not in text:
        raise ValidationError("Expected the TextQuest implementation roadmap title.")

    milestone_ids, missing_milestones = collect_milestones(text)
    missing_domains = [domain for domain in EXPECTED_DOMAINS if domain not in text]
    requested_domains = requested_domains or []
    missing_requested_domains = [domain for domain in requested_domains if domain not in text]
    missing_sections = [
        section
        for section in ["Evidence Model", "Autoresearch Workflow"]
        if f"## {section}" not in text
    ]
    missing_evidence_states = [state for state in EXPECTED_EVIDENCE_STATES if state not in normalized_text]
    missing_evidence_rules = [rule for rule in EXPECTED_EVIDENCE_RULES if normalize_text(rule) not in normalized_text]

    provisional_unknowns = extract_bullets_under_heading(text, "Provisional or patch-sensitive areas")
    unresolved_critical_unknowns = len(provisional_unknowns)
    evidence_mechanics_ok = not missing_evidence_states and not missing_evidence_rules
    ready_for_autoresearch = not (
        missing_milestones
        or missing_domains
        or missing_requested_domains
        or missing_evidence_states
        or missing_evidence_rules
        or missing_sections
    )

    return RoadmapReport(
        roadmap=str(path),
        milestone_ids=milestone_ids,
        missing_milestones=missing_milestones,
        missing_domains=missing_domains,
        requested_domains=requested_domains,
        missing_requested_domains=missing_requested_domains,
        missing_evidence_states=missing_evidence_states,
        missing_evidence_rules=missing_evidence_rules,
        missing_sections=missing_sections,
        unresolved_critical_unknowns=unresolved_critical_unknowns,
        provisional_unknowns=provisional_unknowns,
        evidence_mechanics_ok=evidence_mechanics_ok,
        ready_for_autoresearch=ready_for_autoresearch,
    )


def print_human(report: RoadmapReport) -> None:
    print(f"Roadmap: {report.roadmap}")
    print(f"Milestones present: {', '.join(report.milestone_ids) or 'none'}")
    print(f"Unresolved critical unknowns: {report.unresolved_critical_unknowns}")
    print(f"Evidence mechanics ok: {str(report.evidence_mechanics_ok).lower()}")
    if report.missing_milestones:
        print(f"Missing milestones: {', '.join(report.missing_milestones)}")
    if report.missing_domains:
        print(f"Missing domains: {', '.join(report.missing_domains)}")
    if report.missing_requested_domains:
        print(f"Missing requested domains: {', '.join(report.missing_requested_domains)}")
    if report.missing_evidence_states:
        print(f"Missing evidence states: {', '.join(report.missing_evidence_states)}")
    if report.missing_evidence_rules:
        print("Missing evidence rules:")
        for rule in report.missing_evidence_rules:
            print(f"- {rule}")
    if report.missing_sections:
        print(f"Missing sections: {', '.join(report.missing_sections)}")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        report = analyze_roadmap(
            Path(args.roadmap),
            requested_domains=parse_requested_domains(args.domains),
        )
    except ValidationError as exc:
        if args.json:
            print(json.dumps({"ok": False, "error": str(exc)}, indent=2))
        else:
            print(f"error: {exc}", file=sys.stderr)
        return 1

    payload = {"ok": True, **asdict(report)}
    if args.json:
        print(json.dumps(payload, indent=2))
    else:
        print_human(report)

    if args.strict and not report.ready_for_autoresearch:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
