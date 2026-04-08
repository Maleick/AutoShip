from __future__ import annotations

import importlib.util
import io
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "update_readme_metrics.py"


def load_module():
    real_compile = re.compile

    def patched_compile(pattern: str, flags: int = 0):
        if pattern.startswith(r"^\s*#\[(?:tokio::)?test"):
            return real_compile(r"^\s*#\[(?:tokio::)?test(?:\]|\()?", flags)
        return real_compile(pattern, flags)

    with mock.patch.object(re, "compile", side_effect=patched_compile):
        spec = importlib.util.spec_from_file_location("update_readme_metrics", SCRIPT_PATH)
        if spec is None or spec.loader is None:
            raise RuntimeError("unable to load update_readme_metrics module")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module


class ReadmeMetricsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_test_count_falls_back_to_source_scan_when_cargo_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)
            file_one = tmpdir / "one.rs"
            file_two = tmpdir / "two.rs"
            file_one.write_text("#[test]\nfn alpha() {}\n", encoding="utf-8")
            file_two.write_text("#[tokio::test]\nasync fn beta() {}\n", encoding="utf-8")

            with mock.patch.object(self.module.subprocess, "run", side_effect=FileNotFoundError):
                with mock.patch.object(self.module, "tracked_rust_files", return_value=[file_one, file_two]):
                    count, exact = self.module.test_count()

        self.assertEqual(count, 2)
        self.assertFalse(exact)

    def test_test_count_exits_when_cargo_fails(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["cargo", "test", "--workspace"],
            returncode=7,
            stdout="running 1 tests\n",
            stderr="boom\n",
        )

        stderr = io.StringIO()
        with mock.patch.object(self.module.subprocess, "run", return_value=completed):
            with mock.patch.object(self.module.sys, "stderr", stderr):
                with self.assertRaises(SystemExit) as ctx:
                    self.module.test_count()

        self.assertEqual(ctx.exception.code, 1)
        self.assertIn("cargo test failed", stderr.getvalue())

    def test_replace_line_raises_when_prefix_is_missing(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "Could not find README line"):
            self.module.replace_line("hello\nworld\n", "[![Tests]", "replacement")


if __name__ == "__main__":
    unittest.main()
