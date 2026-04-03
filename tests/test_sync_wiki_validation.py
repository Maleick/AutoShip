from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "sync_wiki.py"


def load_module():
    spec = importlib.util.spec_from_file_location("sync_wiki", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load sync_wiki module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WikiValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_discover_source_files_rejects_nested_pages(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            wiki_root = Path(tmpdir) / "wiki"
            nested = wiki_root / "nested"
            nested.mkdir(parents=True)
            (wiki_root / "Home.md").write_text("# Home\n", encoding="utf-8")
            (wiki_root / "Quick-Start.md").write_text("# Quick Start\n", encoding="utf-8")
            for required in self.module.REQUIRED_FILES:
                if required not in {"Home.md", "Quick-Start.md"}:
                    (wiki_root / required).write_text(f"# {required}\n", encoding="utf-8")
            (nested / "Bad.md").write_text("# Bad\n", encoding="utf-8")

            original_source_dir = self.module.SOURCE_DIR
            self.module.SOURCE_DIR = wiki_root
            try:
                with self.assertRaisesRegex(self.module.WikiSyncError, "docs/wiki must stay flat"):
                    self.module.discover_source_files()
            finally:
                self.module.SOURCE_DIR = original_source_dir

    def test_validate_sources_rejects_banned_literals(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            wiki_root = Path(tmpdir) / "wiki"
            wiki_root.mkdir()
            for required in self.module.REQUIRED_FILES:
                text = "# page\n"
                if required == "Home.md":
                    text = "mq2-reference\n"
                (wiki_root / required).write_text(text, encoding="utf-8")

            original_source_dir = self.module.SOURCE_DIR
            self.module.SOURCE_DIR = wiki_root
            try:
                with self.assertRaisesRegex(self.module.WikiSyncError, "stale pre-submodule reference"):
                    self.module.validate_sources()
            finally:
                self.module.SOURCE_DIR = original_source_dir

    def test_run_includes_stdout_and_stderr_on_failure(self) -> None:
        command = [
            os.environ.get("PYTHON", sys.executable),
            "-c",
            "import sys; print('hello from stdout'); print('hello from stderr', file=sys.stderr); raise SystemExit(7)",
        ]

        with self.assertRaisesRegex(self.module.WikiSyncError, "hello from stderr"):
            self.module.run(command, cwd=REPO_ROOT)

        with self.assertRaisesRegex(self.module.WikiSyncError, "hello from stdout"):
            self.module.run(command, cwd=REPO_ROOT)

    def test_github_token_requires_gh_when_env_missing(self) -> None:
        with mock.patch.dict(self.module.os.environ, {}, clear=True):
            with mock.patch.object(self.module.shutil, "which", return_value=None):
                with self.assertRaisesRegex(self.module.WikiSyncError, "GitHub CLI \\(`gh`\\) is not installed"):
                    self.module.github_token()

    def test_github_token_reports_missing_local_auth_clearly(self) -> None:
        error = self.module.WikiSyncError("Command failed (cwd: /repo): gh auth token\n\nstderr:\nmissing auth")
        with mock.patch.dict(self.module.os.environ, {}, clear=True):
            with mock.patch.object(self.module.shutil, "which", return_value="/usr/bin/gh"):
                with mock.patch.object(self.module, "run", side_effect=error):
                    with self.assertRaisesRegex(self.module.WikiSyncError, "Unable to obtain GitHub auth from the local GitHub CLI session"):
                        self.module.github_token()

    def test_github_token_reports_empty_cli_token_clearly(self) -> None:
        result = subprocess.CompletedProcess(
            args=["gh", "auth", "token"],
            returncode=0,
            stdout="   \n",
            stderr="",
        )
        with mock.patch.dict(self.module.os.environ, {}, clear=True):
            with mock.patch.object(self.module.shutil, "which", return_value="/usr/bin/gh"):
                with mock.patch.object(self.module, "run", return_value=result):
                    with self.assertRaisesRegex(self.module.WikiSyncError, "did not return a token"):
                        self.module.github_token()


if __name__ == "__main__":
    unittest.main()
