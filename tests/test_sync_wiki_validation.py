from __future__ import annotations

import importlib.util
import tempfile
from pathlib import Path
import unittest


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


if __name__ == "__main__":
    unittest.main()
