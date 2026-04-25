from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "validate-maps.py"


def load_module():
    spec = importlib.util.spec_from_file_location("validate_maps", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load validate-maps module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ValidateMapsModuleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Validator script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    # --- validate_l_line ---

    def test_l_valid(self) -> None:
        err = self.module.validate_l_line("L 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0, 255, 128")
        self.assertIsNone(err)

    def test_l_wrong_field_count(self) -> None:
        err = self.module.validate_l_line("L 1.0, 2.0, 3.0")
        self.assertIsNotNone(err)
        self.assertIn("9 fields", err)

    def test_l_color_out_of_range(self) -> None:
        err = self.module.validate_l_line("L 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 300, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("out of range", err)

    def test_l_negative_color(self) -> None:
        err = self.module.validate_l_line("L 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, -1, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("out of range", err)

    def test_l_non_numeric_coord(self) -> None:
        err = self.module.validate_l_line("L abc, 2.0, 3.0, 4.0, 5.0, 6.0, 0, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("not a valid float", err)

    def test_l_nan_coord_rejected(self) -> None:
        err = self.module.validate_l_line("L NaN, 0, 0, 0, 0, 0, 0, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("must be finite", err)

    def test_l_inf_coord_rejected(self) -> None:
        err = self.module.validate_l_line("L Inf, 0, 0, 0, 0, 0, 0, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("must be finite", err)

    # --- validate_p_line ---

    def test_p_valid(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 255, 128, 0, 3, Some_Label")
        self.assertIsNone(err)

    def test_p_label_with_commas(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 255, 128, 0, 3, Label, with, commas")
        self.assertIsNone(err)

    def test_p_too_few_fields(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 255, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("8 fields", err)

    def test_p_color_out_of_range(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 256, 0, 0, 3, Label")
        self.assertIsNotNone(err)
        self.assertIn("out of range", err)

    def test_p_size_must_be_finite_float(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 255, 0, 0, NaN, Label")
        self.assertIsNotNone(err)
        self.assertIn("must be finite", err)

    def test_p_non_numeric_coord(self) -> None:
        err = self.module.validate_p_line("P bad, 2.0, 3.0, 255, 0, 0, 3, Label")
        self.assertIsNotNone(err)
        self.assertIn("not a valid float", err)

    # --- validate_map_file ---

    def test_valid_l_file(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("L -100.0, -200.0, -10.0, 100.0, 200.0, 10.0, 0, 128, 255\n")
            f.write("L 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 255, 0, 0\n")
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(errors, [])
        finally:
            fname.unlink(missing_ok=True)

    def test_valid_p_file(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("P -4500.0, -3780.0, 400.0, 255, 255, 0, 3, Great_Span\n")
            f.write("P -1785.0, 6660.0, 200.0, 255, 0, 0, 3, To_Great_Divide\n")
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(errors, [])
        finally:
            fname.unlink(missing_ok=True)

    def test_invalid_file_catches_errors(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("L 1.0, 2.0, 3.0\n")  # wrong field count
            f.write("P 1.0, 2.0, 3.0, 256, 0, 0, 3, Label\n")  # bad color
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(len(errors), 2)
            self.assertIn("line 1", errors[0])
            self.assertIn("line 2", errors[1])
        finally:
            fname.unlink(missing_ok=True)

    def test_invalid_file_catches_each_requested_error_type(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("L 1.0, 2.0, 3.0\n")  # wrong field count
            f.write("L abc, 2.0, 3.0, 4.0, 5.0, 6.0, 0, 0, 0\n")  # invalid float
            f.write("L NaN, 2.0, 3.0, 4.0, 5.0, 6.0, 0, 0, 0\n")  # non-finite float
            f.write("P 1.0, 2.0, 3.0, 256, -1, 0, 3, Label\n")  # color out of range
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(len(errors), 4)
            self.assertTrue(any("expected 9 fields" in error for error in errors))
            self.assertTrue(any("not a valid float" in error for error in errors))
            self.assertTrue(any("must be finite" in error for error in errors))
            self.assertTrue(any("out of range 0-255" in error for error in errors))
        finally:
            fname.unlink(missing_ok=True)

    def test_comments_and_blank_lines_ignored(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("# This is a comment\n")
            f.write("\n")
            f.write("L 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0, 0, 0\n")
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(errors, [])
        finally:
            fname.unlink(missing_ok=True)

    def test_unknown_line_type_reported(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("X something unknown\n")
            fname = Path(f.name)
        try:
            errors = self.module.validate_map_file(fname)
            self.assertEqual(len(errors), 1)
            self.assertIn("unknown line type", errors[0])
        finally:
            fname.unlink(missing_ok=True)

    def test_result_reports_line_count_and_bounds(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("# comment\n")
            f.write("L -10, -20, 0, 30, 40, 0, 1, 2, 3\n")
            f.write("P 5, 60, 0, 4, 5, 6, 3, Label\n")
            fname = Path(f.name)
        try:
            result = self.module.validate_map_file_result(fname)
            self.assertTrue(result.is_valid)
            self.assertEqual(result.line_count, 3)
            self.assertEqual(result.record_count, 2)
            self.assertEqual(result.bounds.display(), "x=-10..30, y=-20..60")
        finally:
            fname.unlink(missing_ok=True)


class ValidateMapsIntegrationTests(unittest.TestCase):
    """Integration tests: run the script against the actual config/maps/ directory."""

    def test_existing_maps_pass(self) -> None:
        """All maps in config/maps/ must pass validation."""
        map_dir = REPO_ROOT / "config" / "maps"
        if not map_dir.exists():
            self.skipTest("config/maps/ directory not found")
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH)],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=f"Map validation failed:\n{completed.stdout}\n{completed.stderr}",
        )

    def test_invalid_map_exits_nonzero(self) -> None:
        """Script exits with nonzero when an invalid map is present."""
        with tempfile.TemporaryDirectory() as tmpdir:
            map_dir = Path(tmpdir) / "config" / "maps"
            map_dir.mkdir(parents=True)
            bad_map = map_dir / "bad_zone.txt"
            bad_map.write_text("L 1.0, 2.0\n")  # too few fields
            completed = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(map_dir)],
                cwd=REPO_ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("✗ Error: bad_zone.txt, line 1", completed.stdout)
            self.assertIn("invalid_lines=1", completed.stdout)

    def test_cli_reports_valid_file_summary(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            map_dir = Path(tmpdir) / "maps"
            map_dir.mkdir()
            good_map = map_dir / "good_zone.txt"
            good_map.write_text("L -1, -2, 0, 3, 4, 0, 10, 20, 30\n")
            completed = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(map_dir)],
                cwd=REPO_ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0)
            self.assertIn("✓ Valid: good_zone.txt, line_count=1, bounds=(x=-1..3, y=-2..4)", completed.stdout)
            self.assertIn("valid_files=1", completed.stdout)


if __name__ == "__main__":
    unittest.main()
