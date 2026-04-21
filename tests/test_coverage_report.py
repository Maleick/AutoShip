from __future__ import annotations

import importlib.util
import io
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "coverage-report.py"


def load_module():
    spec = importlib.util.spec_from_file_location("coverage_report", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load coverage-report module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class RunCommandTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_run_command_captures_stdout_and_stderr(self) -> None:
        fake_result = subprocess.CompletedProcess(
            args=["echo", "hello"],
            returncode=0,
            stdout="hello\n",
            stderr="",
        )
        with mock.patch.object(self.module.subprocess, "run", return_value=fake_result):
            code, out, err = self.module.run_command(["echo", "hello"])
        self.assertEqual(code, 0)
        self.assertEqual(out, "hello\n")
        self.assertEqual(err, "")

    def test_run_command_returns_nonzero_exit_code(self) -> None:
        fake_result = subprocess.CompletedProcess(
            args=["false"],
            returncode=1,
            stdout="",
            stderr="error message",
        )
        with mock.patch.object(self.module.subprocess, "run", return_value=fake_result):
            code, out, err = self.module.run_command(["false"])
        self.assertEqual(code, 1)
        self.assertEqual(err, "error message")

    def test_run_command_handles_timeout(self) -> None:
        with mock.patch.object(
            self.module.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(cmd=["cargo", "tarpaulin"], timeout=600),
        ):
            code, out, err = self.module.run_command(["cargo", "tarpaulin"])
        self.assertEqual(code, 1)
        self.assertIn("timed out", err.lower())

    def test_run_command_handles_missing_executable(self) -> None:
        with mock.patch.object(
            self.module.subprocess,
            "run",
            side_effect=FileNotFoundError(),
        ):
            code, out, err = self.module.run_command(["nonexistent_tool"])
        self.assertEqual(code, 127)
        self.assertIn("not found", err.lower())


class CheckTarpaulinInstalledTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_returns_true_when_tarpaulin_exits_zero(self) -> None:
        with mock.patch.object(self.module, "run_command", return_value=(0, "cargo-tarpaulin 0.27.0\n", "")):
            self.assertTrue(self.module.check_tarpaulin_installed())

    def test_returns_false_when_tarpaulin_not_found(self) -> None:
        with mock.patch.object(self.module, "run_command", return_value=(127, "", "command not found")):
            self.assertFalse(self.module.check_tarpaulin_installed())

    def test_returns_false_when_tarpaulin_exits_nonzero(self) -> None:
        with mock.patch.object(self.module, "run_command", return_value=(1, "", "error")):
            self.assertFalse(self.module.check_tarpaulin_installed())


class GenerateCoverageReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_returns_coverage_percentage_when_parseable(self) -> None:
        tarpaulin_output = (
            "test result: ok. 50 passed; 0 failed;\n"
            "73.45% coverage, 150/200 lines covered\n"
        )
        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(self.module, "run_command", return_value=(0, tarpaulin_output, "")):
                exit_code, pct = self.module.generate_coverage_report(html=False)
        self.assertEqual(exit_code, 0)
        self.assertAlmostEqual(pct, 73.45)

    def test_parses_coverage_percentage_from_stderr_when_stdout_empty(self) -> None:
        tarpaulin_stderr = (
            "tarpaulin log line\n"
            "73.45% coverage, 150/200 lines covered\n"
        )
        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(
                self.module,
                "run_command",
                return_value=(0, "", tarpaulin_stderr),
            ):
                exit_code, pct = self.module.generate_coverage_report(html=False)
        self.assertEqual(exit_code, 0)
        self.assertAlmostEqual(pct, 73.45)

    def test_returns_none_when_percentage_unparseable(self) -> None:
        tarpaulin_output = "something happened but no percentage\n"
        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(self.module, "run_command", return_value=(0, tarpaulin_output, "")):
                exit_code, pct = self.module.generate_coverage_report(html=False)
        self.assertEqual(exit_code, 0)
        self.assertIsNone(pct)

    def test_returns_error_when_tarpaulin_not_installed(self) -> None:
        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=False):
            exit_code, pct = self.module.generate_coverage_report(html=False)
        self.assertNotEqual(exit_code, 0)
        self.assertIsNone(pct)

    def test_returns_error_when_tarpaulin_command_fails(self) -> None:
        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(
                self.module,
                "run_command",
                return_value=(1, "partial stdout", "build failed"),
            ):
                with mock.patch("sys.stdout", new_callable=io.StringIO) as captured_stdout:
                    exit_code, pct = self.module.generate_coverage_report(html=False)
        self.assertNotEqual(exit_code, 0)
        self.assertIsNone(pct)
        output = captured_stdout.getvalue()
        self.assertIn("STDOUT:", output)
        self.assertIn("partial stdout", output)
        self.assertIn("STDERR:", output)
        self.assertIn("build failed", output)

    def test_passes_html_flag_to_tarpaulin_command(self) -> None:
        tarpaulin_output = "80.00% coverage, 80/100 lines covered\n"
        captured_cmd = {}

        def fake_run_command(cmd, capture_output=True):
            captured_cmd["cmd"] = cmd
            return (0, tarpaulin_output, "")

        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(self.module, "run_command", side_effect=fake_run_command):
                self.module.generate_coverage_report(html=True)

        self.assertIn("Html", captured_cmd["cmd"])
        self.assertIn("--out", captured_cmd["cmd"])
        self.assertIn("--stderr", captured_cmd["cmd"])
        self.assertTrue(captured_cmd["cmd"][-2:] == ["--", "--nocapture"])

    def test_passes_workspace_and_tests_flags_to_tarpaulin_command(self) -> None:
        tarpaulin_output = "80.00% coverage, 80/100 lines covered\n"
        captured_cmd = {}

        def fake_run_command(cmd, capture_output=True):
            captured_cmd["cmd"] = cmd
            return (0, tarpaulin_output, "")

        with mock.patch.object(self.module, "check_tarpaulin_installed", return_value=True):
            with mock.patch.object(self.module, "run_command", side_effect=fake_run_command):
                self.module.generate_coverage_report(html=False)

        self.assertIn("--workspace", captured_cmd["cmd"])
        self.assertIn("--tests", captured_cmd["cmd"])
        self.assertNotIn("--all", captured_cmd["cmd"])


class MainThresholdTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_main_exits_nonzero_when_below_threshold(self) -> None:
        with mock.patch("sys.argv", ["coverage-report.py", "--threshold", "80"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, 65.0)
            ):
                result = self.module.main()
        self.assertEqual(result, 1)

    def test_main_exits_zero_when_meets_threshold(self) -> None:
        with mock.patch("sys.argv", ["coverage-report.py", "--threshold", "60"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, 75.0)
            ):
                result = self.module.main()
        self.assertEqual(result, 0)

    def test_main_exits_zero_when_exactly_at_threshold(self) -> None:
        with mock.patch("sys.argv", ["coverage-report.py", "--threshold", "70"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, 70.0)
            ):
                result = self.module.main()
        self.assertEqual(result, 0)

    def test_main_returns_error_when_coverage_command_fails(self) -> None:
        with mock.patch("sys.argv", ["coverage-report.py"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(1, None)
            ):
                result = self.module.main()
        self.assertEqual(result, 1)

    def test_main_returns_zero_when_percentage_is_none_but_command_succeeded(self) -> None:
        # If tarpaulin ran OK but the output had no parseable percentage, and
        # no --threshold was passed, treat it as a non-blocking warning.
        with mock.patch("sys.argv", ["coverage-report.py"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, None)
            ):
                result = self.module.main()
        self.assertEqual(result, 0)

    def test_main_returns_error_when_threshold_explicit_but_percentage_unparseable(self) -> None:
        # When --threshold is passed explicitly, we must not silently pass
        # just because tarpaulin's stdout format drifted.
        with mock.patch("sys.argv", ["coverage-report.py", "--threshold", "75"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, None)
            ):
                result = self.module.main()
        self.assertEqual(result, 1)

    def test_default_threshold_is_80(self) -> None:
        # Default threshold is 80: 79.9% should fail, 80.0% should pass.
        with mock.patch("sys.argv", ["coverage-report.py"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, 79.9)
            ):
                result = self.module.main()
        self.assertEqual(result, 1)

        with mock.patch("sys.argv", ["coverage-report.py"]):
            with mock.patch.object(
                self.module, "generate_coverage_report", return_value=(0, 80.0)
            ):
                result = self.module.main()
        self.assertEqual(result, 0)


if __name__ == "__main__":
    unittest.main()
