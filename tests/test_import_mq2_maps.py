from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "import_mq2_maps.py"


def load_module():
    spec = importlib.util.spec_from_file_location("import_mq2_maps", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load import_mq2_maps module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_VALID_L = "L 0.0, 0.0, 0.0, 10.0, 10.0, 0.0, 0, 255, 0\n"
_VALID_P = "P 5.0, 5.0, 0.0, 128, 64, 0, 2, MyLabel\n"
_VALID_CONTENT = _VALID_L + _VALID_P


def _write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


class ValidateMapFileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_valid_file_returns_counts(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(_VALID_CONTENT)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 1)
            self.assertEqual(points, 1)
            self.assertEqual(errors, [])
        finally:
            p.unlink()

    def test_blank_and_comment_lines_ignored(self) -> None:
        content = "\n# this is a comment\n" + _VALID_L
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 1)
            self.assertEqual(points, 0)
            self.assertEqual(errors, [])
        finally:
            p.unlink()

    def test_bad_l_record_reported(self) -> None:
        content = "L not_a_number, bad\n"
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 0)
            self.assertEqual(len(errors), 1)
            self.assertIn("bad L record", errors[0])
        finally:
            p.unlink()

    def test_bad_p_record_reported(self) -> None:
        content = "P not_a_number\n"
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(points, 0)
            self.assertEqual(len(errors), 1)
            self.assertIn("bad P record", errors[0])
        finally:
            p.unlink()

    def test_unknown_record_type_reported(self) -> None:
        content = "X 1.0, 2.0, 3.0\n"
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(len(errors), 1)
            self.assertIn("unknown record", errors[0])
        finally:
            p.unlink()

    def test_empty_file_returns_zeros(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 0)
            self.assertEqual(points, 0)
            self.assertEqual(errors, [])
        finally:
            p.unlink()

    def test_negative_coordinates_accepted(self) -> None:
        content = "L -100.5, -200.0, -10.0, -50.0, -100.0, -5.0, 0, 0, 255\n"
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 1)
            self.assertEqual(errors, [])
        finally:
            p.unlink()

    def test_multiple_valid_records(self) -> None:
        content = _VALID_L * 3 + _VALID_P * 2
        with tempfile.NamedTemporaryFile(suffix=".txt", mode="w", delete=False) as f:
            f.write(content)
            p = Path(f.name)
        try:
            lines, points, errors = self.module.validate_map_file(p)
            self.assertEqual(lines, 3)
            self.assertEqual(points, 2)
            self.assertEqual(errors, [])
        finally:
            p.unlink()


class ImportFileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_valid_file_copied(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            src = Path(src_dir) / "gfaydark.txt"
            _write(src, _VALID_CONTENT)
            ok = self.module.import_file(src, Path(dest_dir))
            self.assertTrue(ok)
            self.assertTrue((Path(dest_dir) / "gfaydark.txt").exists())

    def test_dry_run_does_not_copy(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            src = Path(src_dir) / "zone.txt"
            _write(src, _VALID_CONTENT)
            ok = self.module.import_file(src, Path(dest_dir), dry_run=True)
            self.assertTrue(ok)
            self.assertFalse((Path(dest_dir) / "zone.txt").exists())

    def test_file_with_only_errors_returns_false(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            src = Path(src_dir) / "bad.txt"
            _write(src, "X bad line\n")
            ok = self.module.import_file(src, Path(dest_dir))
            self.assertFalse(ok)
            self.assertFalse((Path(dest_dir) / "bad.txt").exists())

    def test_file_with_errors_but_valid_records_copied(self) -> None:
        """A file with parse errors but at least one valid record still imports."""
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            src = Path(src_dir) / "mixed.txt"
            _write(src, _VALID_L + "X bad\n")
            ok = self.module.import_file(src, Path(dest_dir))
            self.assertTrue(ok)
            self.assertTrue((Path(dest_dir) / "mixed.txt").exists())


class CmdListTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_cmd_list_prints_files(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            src = Path(d)
            (src / "zone1.txt").write_text(_VALID_CONTENT)
            (src / "zone2.txt").write_text(_VALID_CONTENT)
            # cmd_list prints to stdout and then calls sys.exit(0)
            # We can't easily capture it without mocking, but we can verify
            # it raises SystemExit(0) when there are files.
            import io
            from contextlib import redirect_stdout
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.module.cmd_list(src)
            output = buf.getvalue()
            self.assertIn("zone1.txt", output)
            self.assertIn("zone2.txt", output)

    def test_cmd_list_exits_when_no_files(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(SystemExit) as cm:
                self.module.cmd_list(Path(d))
            self.assertEqual(cm.exception.code, 1)


class CmdImportDirTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_imports_all_txt_files(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            for name in ("alpha.txt", "beta.txt"):
                (Path(src_dir) / name).write_text(_VALID_CONTENT)
            import io
            from contextlib import redirect_stdout
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.module.cmd_import_dir(Path(src_dir), Path(dest_dir), dry_run=True)
            output = buf.getvalue()
            # dry_run → files not physically copied but both processed with OK
            self.assertIn("alpha.txt", output)
            self.assertIn("beta.txt", output)

    def test_exits_when_source_has_no_txt(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            with self.assertRaises(SystemExit) as cm:
                self.module.cmd_import_dir(Path(src_dir), Path(dest_dir), False)
            self.assertEqual(cm.exception.code, 1)

    def test_skips_existing_files(self) -> None:
        with tempfile.TemporaryDirectory() as src_dir, \
             tempfile.TemporaryDirectory() as dest_dir:
            name = "zone.txt"
            (Path(src_dir) / name).write_text(_VALID_CONTENT)
            # Pre-create file in dest to trigger skip path
            (Path(dest_dir) / name).write_text(_VALID_CONTENT)
            import io
            from contextlib import redirect_stdout
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.module.cmd_import_dir(Path(src_dir), Path(dest_dir), False)
            self.assertIn("EXIST", buf.getvalue())


class ZoneListTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_zone_list_is_non_empty(self) -> None:
        self.assertGreater(len(self.module.ZONE_LIST), 0)

    def test_zone_list_entries_are_pairs(self) -> None:
        for entry in self.module.ZONE_LIST:
            self.assertEqual(len(entry), 2, f"Unexpected entry shape: {entry!r}")

    def test_zone_list_stems_are_lowercase(self) -> None:
        for _name, stem in self.module.ZONE_LIST:
            self.assertEqual(stem, stem.lower(), f"Stem not lowercase: {stem!r}")


if __name__ == "__main__":
    unittest.main()
