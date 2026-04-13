#!/usr/bin/env python3
"""ETW-TI JSONL log parser for EQ-process-filtered events.

Parses EtwTiViewer JSONL output, filters by EQ process name or PID,
and produces a summary report: event type counts, top callers, timestamps.

Usage:
    python3 scripts/etw_parser.py [OPTIONS] <input.jsonl>

Options:
    --process-name NAME   Filter by process name (case-insensitive substring match)
    --pid PID             Filter by process ID (integer)
    --top-callers N       Number of top callers to show (default: 10)
    --output-format FMT   Output format: text or json (default: text)

Examples:
    python3 scripts/etw_parser.py --process-name eqgame events.jsonl
    python3 scripts/etw_parser.py --pid 1234 events.jsonl
    python3 scripts/etw_parser.py --process-name eq --output-format json events.jsonl

Doctest examples:
    >>> from scripts.etw_parser import parse_event, filter_event, build_summary
    >>> ev = parse_event('{"EventType":"ProcessCreate","ProcessName":"eqgame.exe","PID":1000,"Timestamp":"2026-04-13T10:00:00Z","Caller":"ntdll!NtCreateUserProcess"}')
    >>> ev.process_name
    'eqgame.exe'
    >>> filter_event(ev, process_name='eqgame', pid=None)
    True
    >>> filter_event(ev, process_name=None, pid=1000)
    True
    >>> filter_event(ev, process_name='notepad', pid=None)
    False
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from typing import Iterator, Optional


@dataclass
class EtwEvent:
    """Represents a single parsed ETW-TI event."""

    event_type: str
    process_name: str
    pid: int
    timestamp: str
    caller: str
    raw: dict = field(repr=False)


def parse_event(line: str) -> Optional[EtwEvent]:
    """Parse a single JSONL line into an EtwEvent.

    Returns None if the line is blank, a comment, or unparseable.

    >>> ev = parse_event('{"EventType":"ThreadCreate","ProcessName":"eqgame.exe","PID":42,"Timestamp":"2026-04-13T00:00:00Z","Caller":"ntdll!NtCreateThreadEx"}')
    >>> ev.event_type
    'ThreadCreate'
    >>> ev.pid
    42
    >>> parse_event('') is None
    True
    >>> parse_event('# comment') is None
    True
    >>> parse_event('{bad json}') is None
    True
    """
    line = line.strip()
    if not line or line.startswith("#"):
        return None
    try:
        data = json.loads(line)
    except json.JSONDecodeError:
        return None

    return EtwEvent(
        event_type=data.get("EventType", "Unknown"),
        process_name=data.get("ProcessName", ""),
        pid=int(data.get("PID", 0)),
        timestamp=data.get("Timestamp", ""),
        caller=data.get("Caller", ""),
        raw=data,
    )


def filter_event(
    event: EtwEvent,
    process_name: Optional[str],
    pid: Optional[int],
) -> bool:
    """Return True if the event matches the given filter criteria.

    At least one of process_name or pid must match (AND if both provided).
    If neither filter is set, all events pass.

    >>> from scripts.etw_parser import parse_event, filter_event
    >>> ev = parse_event('{"EventType":"ProcessOpen","ProcessName":"eqgame.exe","PID":5000,"Timestamp":"T","Caller":"kernel32!OpenProcess"}')
    >>> filter_event(ev, process_name='eqgame', pid=None)
    True
    >>> filter_event(ev, process_name='EQGAME', pid=None)
    True
    >>> filter_event(ev, process_name=None, pid=5000)
    True
    >>> filter_event(ev, process_name=None, pid=9999)
    False
    >>> filter_event(ev, process_name='eqgame', pid=5000)
    True
    >>> filter_event(ev, process_name='eqgame', pid=9999)
    False
    >>> filter_event(ev, process_name=None, pid=None)
    True
    """
    if process_name is not None and pid is not None:
        return (
            process_name.lower() in event.process_name.lower()
            and event.pid == pid
        )
    if process_name is not None:
        return process_name.lower() in event.process_name.lower()
    if pid is not None:
        return event.pid == pid
    return True


def iter_events(lines: Iterator[str]) -> Iterator[EtwEvent]:
    """Yield parsed EtwEvent objects from an iterable of JSONL lines."""
    for line in lines:
        ev = parse_event(line)
        if ev is not None:
            yield ev


def build_summary(events: list[EtwEvent], top_n: int = 10) -> dict:
    """Build a summary dict from a list of filtered events.

    Returns:
        {
            "total_events": int,
            "event_type_counts": {type: count, ...},
            "top_callers": [(caller, count), ...],
            "process_names": [name, ...],
            "pids": [pid, ...],
            "earliest": str or None,
            "latest": str or None,
        }

    >>> evs = [
    ...     EtwEvent("A", "eq.exe", 1, "2026-01-01T00:00:00Z", "ntdll!foo", {}),
    ...     EtwEvent("B", "eq.exe", 1, "2026-01-01T00:01:00Z", "ntdll!bar", {}),
    ...     EtwEvent("A", "eq.exe", 2, "2026-01-01T00:02:00Z", "ntdll!foo", {}),
    ... ]
    >>> s = build_summary(evs, top_n=5)
    >>> s["total_events"]
    3
    >>> s["event_type_counts"]["A"]
    2
    >>> s["top_callers"][0][0]
    'ntdll!foo'
    >>> s["earliest"]
    '2026-01-01T00:00:00Z'
    >>> s["latest"]
    '2026-01-01T00:02:00Z'
    """
    if not events:
        return {
            "total_events": 0,
            "event_type_counts": {},
            "top_callers": [],
            "process_names": [],
            "pids": [],
            "earliest": None,
            "latest": None,
        }

    type_counts: Counter = Counter(ev.event_type for ev in events)
    caller_counts: Counter = Counter(ev.caller for ev in events if ev.caller)
    process_names = sorted({ev.process_name for ev in events})
    pids = sorted({ev.pid for ev in events})
    timestamps = [ev.timestamp for ev in events if ev.timestamp]

    return {
        "total_events": len(events),
        "event_type_counts": dict(type_counts.most_common()),
        "top_callers": caller_counts.most_common(top_n),
        "process_names": process_names,
        "pids": pids,
        "earliest": min(timestamps) if timestamps else None,
        "latest": max(timestamps) if timestamps else None,
    }


def format_text_report(summary: dict) -> str:
    """Format a summary dict as a human-readable text report.

    >>> summary = {
    ...     "total_events": 2,
    ...     "event_type_counts": {"ProcessCreate": 2},
    ...     "top_callers": [("ntdll!foo", 2)],
    ...     "process_names": ["eqgame.exe"],
    ...     "pids": [1000],
    ...     "earliest": "2026-04-13T00:00:00Z",
    ...     "latest": "2026-04-13T01:00:00Z",
    ... }
    >>> "ETW-TI Event Summary" in format_text_report(summary)
    True
    >>> "Total events:" in format_text_report(summary)
    True
    """
    lines = [
        "=" * 60,
        "ETW-TI Event Summary",
        "=" * 60,
        f"Total events:   {summary['total_events']}",
        f"Earliest:       {summary['earliest'] or 'N/A'}",
        f"Latest:         {summary['latest'] or 'N/A'}",
        "",
        "Processes:",
    ]
    for name in summary["process_names"]:
        lines.append(f"  {name}")
    lines.append("")
    lines.append("PIDs:")
    for pid in summary["pids"]:
        lines.append(f"  {pid}")
    lines.append("")
    lines.append("Event Type Counts:")
    for etype, count in summary["event_type_counts"].items():
        lines.append(f"  {etype:<40} {count:>6}")
    lines.append("")
    lines.append("Top Callers:")
    for caller, count in summary["top_callers"]:
        lines.append(f"  {caller:<50} {count:>6}")
    lines.append("=" * 60)
    return "\n".join(lines)


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Parse EtwTiViewer JSONL output and filter by EQ process."
    )
    parser.add_argument("input", nargs="?", help="JSONL input file (default: stdin)")
    parser.add_argument(
        "--process-name",
        default=None,
        help="Filter by process name (case-insensitive substring match)",
    )
    parser.add_argument(
        "--pid",
        type=int,
        default=None,
        help="Filter by process ID",
    )
    parser.add_argument(
        "--top-callers",
        type=int,
        default=10,
        metavar="N",
        help="Number of top callers to show (default: 10)",
    )
    parser.add_argument(
        "--output-format",
        choices=["text", "json"],
        default="text",
        help="Output format: text or json (default: text)",
    )
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    """Main entry point. Returns exit code."""
    args = parse_args(argv)

    # Open input
    if args.input and args.input != "-":
        try:
            fh = open(args.input, encoding="utf-8")
        except OSError as exc:
            print(f"Error opening {args.input}: {exc}", file=sys.stderr)
            return 1
    else:
        fh = sys.stdin

    try:
        all_events = list(iter_events(fh))
    finally:
        if fh is not sys.stdin:
            fh.close()

    filtered = [
        ev
        for ev in all_events
        if filter_event(ev, process_name=args.process_name, pid=args.pid)
    ]

    summary = build_summary(filtered, top_n=args.top_callers)

    if args.output_format == "json":
        # Convert top_callers list-of-tuples to list-of-dicts for JSON
        summary["top_callers"] = [
            {"caller": c, "count": n} for c, n in summary["top_callers"]
        ]
        print(json.dumps(summary, indent=2))
    else:
        print(format_text_report(summary))

    return 0


if __name__ == "__main__":
    sys.exit(main())
