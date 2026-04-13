"""Unit tests for scripts/etw_parser.py."""

import json
import sys
import unittest
from io import StringIO
from pathlib import Path

# Ensure scripts/ is importable
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.etw_parser import (
    EtwEvent,
    build_summary,
    filter_event,
    format_text_report,
    iter_events,
    main,
    parse_event,
)


def _make_line(
    event_type="ProcessCreate",
    process_name="eqgame.exe",
    pid=1234,
    timestamp="2026-04-13T10:00:00Z",
    caller="ntdll!NtCreateUserProcess",
    **extra,
) -> str:
    data = {
        "EventType": event_type,
        "ProcessName": process_name,
        "PID": pid,
        "Timestamp": timestamp,
        "Caller": caller,
    }
    data.update(extra)
    return json.dumps(data)


class TestParseEvent(unittest.TestCase):
    def test_parses_valid_line(self):
        line = _make_line(event_type="ThreadCreate", pid=42)
        ev = parse_event(line)
        self.assertIsNotNone(ev)
        self.assertEqual(ev.event_type, "ThreadCreate")
        self.assertEqual(ev.pid, 42)

    def test_returns_none_for_blank(self):
        self.assertIsNone(parse_event(""))
        self.assertIsNone(parse_event("   "))

    def test_returns_none_for_comment(self):
        self.assertIsNone(parse_event("# this is a comment"))

    def test_returns_none_for_bad_json(self):
        self.assertIsNone(parse_event("{not valid json}"))

    def test_defaults_missing_fields(self):
        ev = parse_event('{"EventType":"Foo"}')
        self.assertIsNotNone(ev)
        self.assertEqual(ev.event_type, "Foo")
        self.assertEqual(ev.process_name, "")
        self.assertEqual(ev.pid, 0)
        self.assertEqual(ev.caller, "")

    def test_stores_raw(self):
        line = _make_line()
        ev = parse_event(line)
        self.assertIn("EventType", ev.raw)


class TestFilterEvent(unittest.TestCase):
    def _event(self, name="eqgame.exe", pid=1000):
        return EtwEvent("T", name, pid, "ts", "caller", {})

    def test_no_filter_passes_all(self):
        ev = self._event()
        self.assertTrue(filter_event(ev, process_name=None, pid=None))

    def test_process_name_match_case_insensitive(self):
        ev = self._event(name="EQGame.exe")
        self.assertTrue(filter_event(ev, process_name="eqgame", pid=None))

    def test_process_name_no_match(self):
        ev = self._event(name="notepad.exe")
        self.assertFalse(filter_event(ev, process_name="eqgame", pid=None))

    def test_pid_match(self):
        ev = self._event(pid=5000)
        self.assertTrue(filter_event(ev, process_name=None, pid=5000))

    def test_pid_no_match(self):
        ev = self._event(pid=5000)
        self.assertFalse(filter_event(ev, process_name=None, pid=9999))

    def test_both_filters_and_logic(self):
        ev = self._event(name="eqgame.exe", pid=1000)
        self.assertTrue(filter_event(ev, process_name="eqgame", pid=1000))
        self.assertFalse(filter_event(ev, process_name="eqgame", pid=9999))
        self.assertFalse(filter_event(ev, process_name="notepad", pid=1000))

    def test_substring_match(self):
        ev = self._event(name="eqgame.exe")
        self.assertTrue(filter_event(ev, process_name="eq", pid=None))
        self.assertTrue(filter_event(ev, process_name="game", pid=None))


class TestIterEvents(unittest.TestCase):
    def test_yields_valid_events(self):
        lines = [
            _make_line(event_type="A"),
            "",
            "# comment",
            _make_line(event_type="B"),
        ]
        events = list(iter_events(iter(lines)))
        self.assertEqual(len(events), 2)
        self.assertEqual(events[0].event_type, "A")
        self.assertEqual(events[1].event_type, "B")

    def test_empty_input(self):
        events = list(iter_events(iter([])))
        self.assertEqual(events, [])


class TestBuildSummary(unittest.TestCase):
    def _events(self):
        return [
            EtwEvent("ProcessCreate", "eqgame.exe", 100, "2026-01-01T00:00:00Z", "ntdll!foo", {}),
            EtwEvent("ThreadCreate", "eqgame.exe", 100, "2026-01-01T00:01:00Z", "ntdll!bar", {}),
            EtwEvent("ProcessCreate", "eqgame.exe", 200, "2026-01-01T00:02:00Z", "ntdll!foo", {}),
        ]

    def test_total_events(self):
        s = build_summary(self._events())
        self.assertEqual(s["total_events"], 3)

    def test_event_type_counts(self):
        s = build_summary(self._events())
        self.assertEqual(s["event_type_counts"]["ProcessCreate"], 2)
        self.assertEqual(s["event_type_counts"]["ThreadCreate"], 1)

    def test_top_callers(self):
        s = build_summary(self._events(), top_n=5)
        self.assertEqual(s["top_callers"][0][0], "ntdll!foo")
        self.assertEqual(s["top_callers"][0][1], 2)

    def test_timestamps(self):
        s = build_summary(self._events())
        self.assertEqual(s["earliest"], "2026-01-01T00:00:00Z")
        self.assertEqual(s["latest"], "2026-01-01T00:02:00Z")

    def test_pids_and_names(self):
        s = build_summary(self._events())
        self.assertIn(100, s["pids"])
        self.assertIn(200, s["pids"])
        self.assertIn("eqgame.exe", s["process_names"])

    def test_empty_events(self):
        s = build_summary([])
        self.assertEqual(s["total_events"], 0)
        self.assertIsNone(s["earliest"])
        self.assertIsNone(s["latest"])


class TestFormatTextReport(unittest.TestCase):
    def _summary(self):
        return {
            "total_events": 5,
            "event_type_counts": {"ProcessCreate": 3, "ThreadCreate": 2},
            "top_callers": [("ntdll!foo", 3), ("ntdll!bar", 2)],
            "process_names": ["eqgame.exe"],
            "pids": [1000],
            "earliest": "2026-04-13T00:00:00Z",
            "latest": "2026-04-13T01:00:00Z",
        }

    def test_contains_header(self):
        report = format_text_report(self._summary())
        self.assertIn("ETW-TI Event Summary", report)

    def test_contains_total(self):
        report = format_text_report(self._summary())
        self.assertIn("Total events:   5", report)

    def test_contains_process_name(self):
        report = format_text_report(self._summary())
        self.assertIn("eqgame.exe", report)

    def test_contains_event_types(self):
        report = format_text_report(self._summary())
        self.assertIn("ProcessCreate", report)
        self.assertIn("ThreadCreate", report)

    def test_contains_callers(self):
        report = format_text_report(self._summary())
        self.assertIn("ntdll!foo", report)
        self.assertIn("ntdll!bar", report)


class TestMainCLI(unittest.TestCase):
    def _make_jsonl(self, events: list[dict]) -> str:
        return "\n".join(json.dumps(e) for e in events)

    def _run_main(self, argv, stdin_text=None):
        """Run main() capturing stdout, returning (exit_code, stdout_text)."""
        old_stdout = sys.stdout
        old_stdin = sys.stdin
        sys.stdout = StringIO()
        if stdin_text is not None:
            sys.stdin = StringIO(stdin_text)
        try:
            code = main(argv)
            output = sys.stdout.getvalue()
        finally:
            sys.stdout = old_stdout
            sys.stdin = old_stdin
        return code, output

    def test_text_output_no_filter(self):
        import tempfile
        import os

        events = [
            {"EventType": "ProcessCreate", "ProcessName": "eqgame.exe", "PID": 100, "Timestamp": "T", "Caller": "ntdll!foo"},
            {"EventType": "ThreadCreate", "ProcessName": "notepad.exe", "PID": 200, "Timestamp": "T", "Caller": "ntdll!bar"},
        ]
        with tempfile.NamedTemporaryFile(mode="w", suffix=".jsonl", delete=False) as f:
            f.write(self._make_jsonl(events))
            fname = f.name
        try:
            code, out = self._run_main([fname])
            self.assertEqual(code, 0)
            self.assertIn("Total events:   2", out)
        finally:
            os.unlink(fname)

    def test_filter_by_process_name(self):
        import tempfile
        import os

        events = [
            {"EventType": "ProcessCreate", "ProcessName": "eqgame.exe", "PID": 100, "Timestamp": "T", "Caller": "ntdll!foo"},
            {"EventType": "ProcessCreate", "ProcessName": "notepad.exe", "PID": 200, "Timestamp": "T", "Caller": "ntdll!bar"},
        ]
        with tempfile.NamedTemporaryFile(mode="w", suffix=".jsonl", delete=False) as f:
            f.write(self._make_jsonl(events))
            fname = f.name
        try:
            code, out = self._run_main(["--process-name", "eqgame", fname])
            self.assertEqual(code, 0)
            self.assertIn("Total events:   1", out)
            self.assertIn("eqgame.exe", out)
        finally:
            os.unlink(fname)

    def test_filter_by_pid(self):
        import tempfile
        import os

        events = [
            {"EventType": "ProcessCreate", "ProcessName": "eqgame.exe", "PID": 100, "Timestamp": "T", "Caller": "ntdll!foo"},
            {"EventType": "ProcessCreate", "ProcessName": "eqgame.exe", "PID": 200, "Timestamp": "T", "Caller": "ntdll!bar"},
        ]
        with tempfile.NamedTemporaryFile(mode="w", suffix=".jsonl", delete=False) as f:
            f.write(self._make_jsonl(events))
            fname = f.name
        try:
            code, out = self._run_main(["--pid", "100", fname])
            self.assertEqual(code, 0)
            self.assertIn("Total events:   1", out)
        finally:
            os.unlink(fname)

    def test_json_output_format(self):
        import tempfile
        import os

        events = [
            {"EventType": "ProcessCreate", "ProcessName": "eqgame.exe", "PID": 100, "Timestamp": "T", "Caller": "ntdll!foo"},
        ]
        with tempfile.NamedTemporaryFile(mode="w", suffix=".jsonl", delete=False) as f:
            f.write(self._make_jsonl(events))
            fname = f.name
        try:
            code, out = self._run_main(["--output-format", "json", fname])
            self.assertEqual(code, 0)
            data = json.loads(out)
            self.assertEqual(data["total_events"], 1)
            self.assertIn("top_callers", data)
        finally:
            os.unlink(fname)

    def test_missing_file_returns_error(self):
        code, _ = self._run_main(["/nonexistent/path/file.jsonl"])
        self.assertNotEqual(code, 0)


if __name__ == "__main__":
    unittest.main()
