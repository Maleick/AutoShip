# AutoShip Result — Issue #919

## Status: COMPLETE

## What was implemented

**Python script:** `scripts/etw_parser.py`

- Parses EtwTiViewer JSONL output line-by-line
- Filters events by EQ process name (case-insensitive substring match) and/or PID
- Produces summary report: total events, event type counts, top callers, process names, PIDs, earliest/latest timestamps
- Supports `--output-format text` (default) or `json`
- Configurable `--top-callers N` (default 10)
- Reads from file argument or stdin

**Test file:** `tests/test_etw_parser.py`

- 31 unit tests across 6 test classes
- Covers: parse_event, filter_event, iter_events, build_summary, format_text_report, main CLI
- All tests pass: `python3 -m pytest tests/test_etw_parser.py` — 31 passed
- Doctests also pass: `python3 -m doctest scripts/etw_parser.py`

## Commit

`8e9f58c2f` on branch `autoship/issue-919`
