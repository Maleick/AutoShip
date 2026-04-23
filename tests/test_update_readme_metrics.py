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

    def test_test_count_falls_back_to_source_scan_when_cargo_fails(self) -> None:
        # script falls back to source scan on non-zero cargo exit (no SystemExit)
        completed = subprocess.CompletedProcess(
            args=["cargo", "test", "--workspace", "--", "--list"],
            returncode=7,
            stdout="running 1 tests\n",
            stderr="boom\n",
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)
            stub = tmpdir / "stub.rs"
            stub.write_text("#[test]\nfn t() {}\n", encoding="utf-8")

            stderr = io.StringIO()
            with mock.patch.object(self.module.subprocess, "run", return_value=completed):
                with mock.patch.object(self.module, "tracked_rust_files", return_value=[stub]):
                    with mock.patch.object(self.module.sys, "stderr", stderr):
                        count, exact = self.module.test_count()

        self.assertFalse(exact)
        self.assertGreaterEqual(count, 0)
        self.assertIn("cargo test failed", stderr.getvalue())

    def test_replace_line_raises_when_prefix_is_missing(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "Could not find README line"):
            self.module.replace_line("hello\nworld\n", "[![Tests]", "replacement")

    def test_workspace_crate_count_reads_workspace_members(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            manifest = Path(tmpdir) / "Cargo.toml"
            manifest.write_text(
                '[workspace]\nmembers = ["textquest", "textquest-common", "tools/eqdiff"]\n',
                encoding="utf-8",
            )

            self.assertEqual(self.module.workspace_crate_count(manifest), 3)

    def test_latest_release_tag_ignores_non_release_tags(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["git", "for-each-ref"],
            returncode=0,
            stdout="nightly\nbeacon-pre-simplify-issue-841\nv0.7.0-alpha\nv0.7.0\n",
            stderr="",
        )

        with mock.patch.object(self.module.subprocess, "run", return_value=completed):
            self.assertEqual(self.module.latest_release_tag(), "v0.7.0-alpha")

    def test_last_commit_summary_formats_date_and_short_sha(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["git", "log"],
            returncode=0,
            stdout="56e51b3712345678\x0056e51b37\x002026-04-23\n",
            stderr="",
        )

        with mock.patch.object(self.module.subprocess, "run", return_value=completed):
            label, sha = self.module.last_commit_summary()

        self.assertEqual(label, "2026-04-23 56e51b37")
        self.assertEqual(sha, "56e51b3712345678")

    def test_static_badge_url_preserves_hyphenated_release_message(self) -> None:
        url = self.module.static_badge_url("release", "v0.7.0-alpha", "success")

        self.assertIn("label=release", url)
        self.assertIn("message=v0.7.0-alpha", url)
        self.assertIn("color=success", url)

    def test_main_rewrites_readme_and_metrics_page_badges(self) -> None:
        readme = """\
<p align="center">
  <a href="https://github.com/Maleick/TextQuest/actions/workflows/release.yml"><img src="https://github.com/Maleick/TextQuest/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="https://github.com/Maleick/TextQuest/commits/master"><img src="https://img.shields.io/github/last-commit/Maleick/TextQuest?style=flat" alt="Last Commit"></a>
</p>

[![Rust LOC](https://img.shields.io/badge/Rust%20LOC-1-blue?style=flat-square)](#testing)
[![Tests](https://img.shields.io/badge/Tests-1%20exact-brightgreen?style=flat-square)](#testing)
![Crates](https://img.shields.io/badge/Crates-6-purple?style=flat-square)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey?style=flat-square)
![Accounts](https://img.shields.io/badge/Accounts-26%2F36-cyan?style=flat-square)

Current workspace totals: 1 Rust lines and 1 exact tests.
"""
        metrics_page = "Current workspace totals: 1 Rust lines and 1 exact tests.\n"

        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)
            readme_path = tmpdir / "README.md"
            metrics_path = tmpdir / "Project-Metrics.md"
            readme_path.write_text(readme, encoding="utf-8")
            metrics_path.write_text(metrics_page, encoding="utf-8")

            with mock.patch.object(self.module, "README_PATH", readme_path):
                with mock.patch.object(self.module, "METRICS_PAGE_PATH", metrics_path):
                    with mock.patch.object(self.module, "rust_loc", return_value=42):
                        with mock.patch.object(self.module, "test_count", return_value=(7, True)):
                            with mock.patch.object(self.module, "workspace_crate_count", return_value=8):
                                with mock.patch.object(self.module, "latest_release_tag", return_value="v0.7.0-alpha"):
                                    with mock.patch.object(
                                        self.module,
                                        "last_commit_summary",
                                        return_value=("2026-04-23 56e51b37", "56e51b37abc"),
                                    ):
                                        self.assertEqual(self.module.main(), 0)

            updated_readme = readme_path.read_text(encoding="utf-8")
            updated_metrics = metrics_path.read_text(encoding="utf-8")

        self.assertNotIn("Accounts", updated_readme)
        self.assertIn("releases/tag/v0.7.0-alpha", updated_readme)
        self.assertIn("message=v0.7.0-alpha", updated_readme)
        self.assertIn("commit/56e51b37abc", updated_readme)
        self.assertIn("message=2026-04-23+56e51b37", updated_readme)
        self.assertIn("Workspace%20Crates-8", updated_readme)
        self.assertIn("Platform&message=Windows+%7C+macOS+%7C+Linux", updated_readme)
        self.assertIn(
            "Current workspace totals: 42 Rust lines, 7 exact tests, and 8 workspace crates. Latest release: v0.7.0-alpha.",
            updated_readme,
        )
        self.assertIn("42 Rust lines, 7 exact tests, and 8 workspace crates", updated_metrics)


if __name__ == "__main__":
    unittest.main()
