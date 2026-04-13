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
        self.assertIn("not numeric", err)

    def test_l_coord_out_of_bounds(self) -> None:
        err = self.module.validate_l_line("L 2000000.0, 0, 0, 0, 0, 0, 0, 0, 0")
        self.assertIsNotNone(err)
        self.assertIn("out of bounds", err)

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

    def test_p_size_out_of_range(self) -> None:
        err = self.module.validate_p_line("P 1.0, 2.0, 3.0, 255, 0, 0, 0, Label")
        self.assertIsNotNone(err)
        self.assertIn("out of range", err)

    def test_p_non_numeric_coord(self) -> None:
        err = self.module.validate_p_line("P bad, 2.0, 3.0, 255, 0, 0, 3, Label")
        self.assertIsNotNone(err)
        self.assertIn("not numeric", err)

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
            self.assertIn("Line 1", errors[0])
            self.assertIn("Line 2", errors[1])
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
            self.assertIn("Unknown line type", errors[0])
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
                [sys.executable, str(SCRIPT_PATH)],
                cwd=tmpdir,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("FAILED", completed.stdout)


if __name__ == "__main__":
    unittest.main()
