from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "collect_patch_evidence.py"


def load_module():
    spec = importlib.util.spec_from_file_location("collect_patch_evidence", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load collect_patch_evidence module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class Sha256FileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_sha256_file_is_deterministic(self) -> None:
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f:
            f.write(b"hello world")
            path = Path(f.name)
        try:
            result1 = self.module.sha256_file(path)
            result2 = self.module.sha256_file(path)
            self.assertEqual(result1, result2)
        finally:
            path.unlink(missing_ok=True)

    def test_sha256_file_matches_stdlib(self) -> None:
        data = b"test content for hashing"
        expected = hashlib.sha256(data).hexdigest()
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f:
            f.write(data)
            path = Path(f.name)
        try:
            result = self.module.sha256_file(path)
            self.assertEqual(result, expected)
        finally:
            path.unlink(missing_ok=True)

    def test_sha256_file_differs_for_different_content(self) -> None:
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f1:
            f1.write(b"content A")
            path1 = Path(f1.name)
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f2:
            f2.write(b"content B")
            path2 = Path(f2.name)
        try:
            self.assertNotEqual(
                self.module.sha256_file(path1),
                self.module.sha256_file(path2),
            )
        finally:
            path1.unlink(missing_ok=True)
            path2.unlink(missing_ok=True)

    def test_sha256_file_empty_file(self) -> None:
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f:
            path = Path(f.name)
        try:
            result = self.module.sha256_file(path)
            expected = hashlib.sha256(b"").hexdigest()
            self.assertEqual(result, expected)
        finally:
            path.unlink(missing_ok=True)


class CountEntriesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def _write_json(self, data, suffix=".json") -> Path:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=suffix, delete=False, encoding="utf-8"
        ) as f:
            json.dump(data, f)
            return Path(f.name)

    def test_count_entries_list_json(self) -> None:
        path = self._write_json([1, 2, 3])
        path = path.rename(path.parent / "functions.json")
        try:
            count = self.module.count_entries(path)
            self.assertEqual(count, 3)
        finally:
            path.unlink(missing_ok=True)

    def test_count_entries_dict_json(self) -> None:
        path = self._write_json({"a": 1, "b": 2})
        path = path.rename(path.parent / "classes.json")
        try:
            count = self.module.count_entries(path)
            self.assertEqual(count, 2)
        finally:
            path.unlink(missing_ok=True)

    def test_count_entries_returns_none_for_metadata(self) -> None:
        # metadata.json is in SYMBOL_FILES but NOT in COUNTABLE_JSON_FILES
        path = self._write_json({"program_name": "eqgame"})
        path = path.rename(path.parent / "metadata.json")
        try:
            count = self.module.count_entries(path)
            self.assertIsNone(count)
        finally:
            path.unlink(missing_ok=True)

    def test_count_entries_empty_list(self) -> None:
        path = self._write_json([])
        path = path.rename(path.parent / "exports.json")
        try:
            count = self.module.count_entries(path)
            self.assertEqual(count, 0)
        finally:
            path.unlink(missing_ok=True)

    def test_count_entries_returns_none_for_unknown_filename(self) -> None:
        # A file not in COUNTABLE_JSON_FILES should return None
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False,
            prefix="unknown_", encoding="utf-8"
        ) as f:
            json.dump([1, 2, 3], f)
            path = Path(f.name)
        try:
            count = self.module.count_entries(path)
            self.assertIsNone(count)
        finally:
            path.unlink(missing_ok=True)


class CollectFileInfoTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_collect_file_info_missing_file(self) -> None:
        path = Path("/nonexistent/path/functions.json")
        info = self.module.collect_file_info(path)
        self.assertFalse(info["exists"])
        self.assertEqual(info["path"], str(path))
        self.assertNotIn("sha256", info)
        self.assertNotIn("size_bytes", info)

    def test_collect_file_info_existing_file(self) -> None:
        data = [{"name": "func1"}, {"name": "func2"}]
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "functions.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            info = self.module.collect_file_info(path)
        self.assertTrue(info["exists"])
        self.assertIn("sha256", info)
        self.assertIn("size_bytes", info)
        self.assertGreater(info["size_bytes"], 0)
        # functions.json is countable
        self.assertEqual(info.get("count"), 2)

    def test_collect_file_info_non_countable_existing_file(self) -> None:
        data = {"program_name": "eqgame", "ghidra_version": "10.4"}
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "metadata.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            info = self.module.collect_file_info(path)
        self.assertTrue(info["exists"])
        self.assertIn("sha256", info)
        # metadata.json is not in COUNTABLE_JSON_FILES
        self.assertNotIn("count", info)


class CompareModulesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def _make_summary(self, files: dict) -> dict:
        """Build a minimal module summary with the given file info."""
        return {
            "exists": True,
            "files": files,
            "metadata": {},
        }

    def test_compare_modules_identical_files(self) -> None:
        sha = "abc123"
        files = {"functions.json": {"exists": True, "sha256": sha}}
        live = self._make_summary(files)
        test = self._make_summary(files)
        result = self.module.compare_modules(live, test)
        self.assertEqual(result["changed_files"], [])
        self.assertEqual(result["only_in_live"], [])
        self.assertEqual(result["only_in_test"], [])

    def test_compare_modules_detects_changed_files(self) -> None:
        live = self._make_summary(
            {"functions.json": {"exists": True, "sha256": "aaa"}}
        )
        test = self._make_summary(
            {"functions.json": {"exists": True, "sha256": "bbb"}}
        )
        result = self.module.compare_modules(live, test)
        self.assertIn("functions.json", result["changed_files"])
        self.assertEqual(result["only_in_live"], [])
        self.assertEqual(result["only_in_test"], [])

    def test_compare_modules_detects_only_in_live(self) -> None:
        live = self._make_summary(
            {"functions.json": {"exists": True, "sha256": "aaa"}}
        )
        test = self._make_summary({})
        result = self.module.compare_modules(live, test)
        self.assertIn("functions.json", result["only_in_live"])
        self.assertEqual(result["only_in_test"], [])

    def test_compare_modules_detects_only_in_test(self) -> None:
        live = self._make_summary({})
        test = self._make_summary(
            {"exports.json": {"exists": True, "sha256": "xyz"}}
        )
        result = self.module.compare_modules(live, test)
        self.assertIn("exports.json", result["only_in_test"])
        self.assertEqual(result["only_in_live"], [])

    def test_compare_modules_metadata_same_flag(self) -> None:
        live = {
            "exists": True,
            "files": {},
            "metadata": {"function_count": 100, "ghidra_version": "10.4"},
        }
        test = {
            "exists": True,
            "files": {},
            "metadata": {"function_count": 100, "ghidra_version": "10.4"},
        }
        result = self.module.compare_modules(live, test)
        self.assertTrue(result["metadata"]["function_count"]["same"])
        self.assertTrue(result["metadata"]["ghidra_version"]["same"])

    def test_compare_modules_metadata_different_values(self) -> None:
        live = {
            "exists": True,
            "files": {},
            "metadata": {"function_count": 100},
        }
        test = {
            "exists": True,
            "files": {},
            "metadata": {"function_count": 200},
        }
        result = self.module.compare_modules(live, test)
        self.assertFalse(result["metadata"]["function_count"]["same"])
        self.assertEqual(result["metadata"]["function_count"]["live"], 100)
        self.assertEqual(result["metadata"]["function_count"]["test"], 200)

    def test_compare_modules_metadata_only_in_live(self) -> None:
        live = {
            "exists": True,
            "files": {},
            "metadata": {"function_count": 100},
        }
        test = {"exists": True, "files": {}, "metadata": {}}
        result = self.module.compare_modules(live, test)
        meta = result["metadata"]["function_count"]
        self.assertEqual(meta["live"], 100)
        self.assertIsNone(meta["test"])
        self.assertFalse(meta["same"])

    def test_compare_modules_empty_summaries(self) -> None:
        live = {"exists": True, "files": {}, "metadata": {}}
        test = {"exists": True, "files": {}, "metadata": {}}
        result = self.module.compare_modules(live, test)
        self.assertEqual(result["changed_files"], [])
        self.assertEqual(result["only_in_live"], [])
        self.assertEqual(result["only_in_test"], [])
        self.assertEqual(result["metadata"], {})


class CollectModuleSummaryTests(unittest.TestCase):
    """Integration-style tests for collect_module_summary using a fake directory tree."""

    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_missing_export_dir_returns_not_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            ghidra_root = Path(tmpdir)
            result = self.module.collect_module_summary(ghidra_root, "live", "eqgame")
        self.assertFalse(result["exists"])

    def test_existing_export_dir_lists_json_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            ghidra_root = Path(tmpdir)
            export_dir = ghidra_root / "live" / "eqgame" / "ghidra-export"
            export_dir.mkdir(parents=True)
            (export_dir / "functions.json").write_text("[]", encoding="utf-8")
            (export_dir / "classes.json").write_text("[]", encoding="utf-8")

            result = self.module.collect_module_summary(ghidra_root, "live", "eqgame")

        self.assertTrue(result["exists"])
        self.assertIn("functions.json", result["json_files"])
        self.assertIn("classes.json", result["json_files"])
        self.assertEqual(result["json_file_count"], 2)

    def test_metadata_json_is_parsed(self) -> None:
        metadata = {
            "program_name": "eqgame.exe",
            "image_base": "0x140000000",
            "function_count": 42,
            "ghidra_version": "10.4",
        }
        with tempfile.TemporaryDirectory() as tmpdir:
            ghidra_root = Path(tmpdir)
            export_dir = ghidra_root / "live" / "eqgame" / "ghidra-export"
            export_dir.mkdir(parents=True)
            (export_dir / "metadata.json").write_text(
                json.dumps(metadata), encoding="utf-8"
            )

            result = self.module.collect_module_summary(ghidra_root, "live", "eqgame")

        self.assertEqual(result["metadata"]["program_name"], "eqgame.exe")
        self.assertEqual(result["metadata"]["function_count"], 42)


class CollectFunctionSignatureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_collect_function_signature_reads_valid_decompiled_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            export_dir = Path(tmpdir)
            decompiled = export_dir / "decompiled"
            decompiled.mkdir(parents=True)
            source = decompiled / "foo.c"
            source.write_text("line1\nline2\n", encoding="utf-8")

            signature = self.module._collect_function_signature(
                {"name": "Foo", "address": "0x1000", "file": "foo.c"},
                export_dir,
            )

        payload = signature["fingerprint"]
        self.assertEqual(signature["decompiled_file"], str(source.resolve()))
        self.assertEqual(signature["decompiled_line_count"], 2)
        self.assertIsInstance(payload, str)

    def test_collect_function_signature_rejects_absolute_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            export_dir = Path(tmpdir)
            signature = self.module._collect_function_signature(
                {"name": "Foo", "address": "0x1000", "file": "/etc/hosts"},
                export_dir,
            )

        self.assertEqual(signature["decompiled_file"], "/etc/hosts")
        self.assertNotIn("decompiled_line_count", signature)

    def test_collect_function_signature_rejects_parent_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            export_dir = Path(tmpdir)
            signature = self.module._collect_function_signature(
                {"name": "Foo", "address": "0x1000", "file": "../outside.c"},
                export_dir,
            )

        self.assertEqual(signature["decompiled_file"], "../outside.c")
        self.assertNotIn("decompiled_line_count", signature)


if __name__ == "__main__":
    unittest.main()
