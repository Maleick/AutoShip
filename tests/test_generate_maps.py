from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "generate_maps.py"


def load_module():
    spec = importlib.util.spec_from_file_location("generate_maps", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load generate_maps module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class CreateZoneMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    # --- boundary lines ---

    def test_boundary_lines_present(self) -> None:
        """create_zone_map produces four outer boundary L-lines."""
        result = self.module.create_zone_map((-100, 100, -200, 200), [], [])
        lines = result.splitlines()
        l_lines = [l for l in lines if l.startswith("L ")]
        self.assertEqual(len(l_lines), 4)

    def test_boundary_uses_correct_coords(self) -> None:
        """Boundary lines encode x_min, x_max, y_min, y_max correctly."""
        result = self.module.create_zone_map((-10, 10, -20, 20), [], [])
        self.assertIn("L -10, -20, 0.0, 10, -20, 0.0, 0, 0, 0", result)
        self.assertIn("L 10, -20, 0.0, 10, 20, 0.0, 0, 0, 0", result)
        self.assertIn("L 10, 20, 0.0, -10, 20, 0.0, 0, 0, 0", result)
        self.assertIn("L -10, 20, 0.0, -10, -20, 0.0, 0, 0, 0", result)

    # --- rooms ---

    def test_room_produces_four_l_lines(self) -> None:
        """Each room adds exactly four L-lines with the room colour."""
        rooms = [(10, 20, 30, 40, -5)]
        result = self.module.create_zone_map((0, 100, 0, 100), rooms, [])
        # 4 boundary + 4 room
        l_lines = [l for l in result.splitlines() if l.startswith("L ")]
        self.assertEqual(len(l_lines), 8)

    def test_room_uses_room_colour(self) -> None:
        """Room L-lines use the fixed room colour 150, 120, 80."""
        rooms = [(0, 0, 50, 50, 0)]
        result = self.module.create_zone_map((-100, 100, -100, 100), rooms, [])
        room_lines = [l for l in result.splitlines() if "150, 120, 80" in l]
        self.assertEqual(len(room_lines), 4)

    def test_room_z_coordinate_propagated(self) -> None:
        """Room L-line z values match the room's z parameter."""
        rooms = [(0, 0, 10, 10, -99)]
        result = self.module.create_zone_map((-200, 200, -200, 200), rooms, [])
        self.assertIn("-99", result)

    def test_multiple_rooms(self) -> None:
        """Two rooms produce 4 boundary + 8 room lines."""
        rooms = [(0, 0, 10, 10, 0), (20, 20, 30, 30, -5)]
        result = self.module.create_zone_map((-100, 100, -100, 100), rooms, [])
        l_lines = [l for l in result.splitlines() if l.startswith("L ")]
        self.assertEqual(len(l_lines), 12)

    # --- POIs ---

    def test_poi_produces_p_line(self) -> None:
        """Each POI adds one P-line."""
        pois = [(5, 10, 0, 255, 0, 0, 3, "Entrance")]
        result = self.module.create_zone_map((0, 100, 0, 100), [], pois)
        p_lines = [l for l in result.splitlines() if l.startswith("P ")]
        self.assertEqual(len(p_lines), 1)

    def test_poi_fields_encoded(self) -> None:
        """POI fields appear in the correct positions in the P-line."""
        pois = [(1, 2, 3, 10, 20, 30, 4, "MyLabel")]
        result = self.module.create_zone_map((0, 100, 0, 100), [], pois)
        self.assertIn("P 1, 2, 3, 10, 20, 30, 4, MyLabel", result)

    def test_multiple_pois(self) -> None:
        """Multiple POIs all appear as P-lines."""
        pois = [
            (0, 0, 0, 255, 0, 0, 3, "A"),
            (1, 1, 0, 0, 255, 0, 2, "B"),
        ]
        result = self.module.create_zone_map((0, 100, 0, 100), [], pois)
        p_lines = [l for l in result.splitlines() if l.startswith("P ")]
        self.assertEqual(len(p_lines), 2)

    # --- empty inputs ---

    def test_no_rooms_no_pois(self) -> None:
        """Map with no rooms or POIs contains only the four boundary lines."""
        result = self.module.create_zone_map((-500, 500, -500, 500), [], [])
        lines = [l for l in result.splitlines() if l.strip()]
        self.assertEqual(len(lines), 4)

    # --- ZONES constant ---

    def test_zones_constant_is_non_empty(self) -> None:
        """The ZONES dict has at least one entry."""
        self.assertGreater(len(self.module.ZONES), 0)

    def test_zones_keys_are_strings(self) -> None:
        """Every key in ZONES is a non-empty string."""
        for key in self.module.ZONES:
            self.assertIsInstance(key, str)
            self.assertTrue(key, f"Empty zone key found")

    def test_zones_values_have_three_elements(self) -> None:
        """Each ZONES entry is a 3-tuple of (bounds, rooms, pois)."""
        for name, value in self.module.ZONES.items():
            self.assertEqual(len(value), 3, f"Zone {name!r} does not have 3 elements")

    def test_zones_can_be_rendered(self) -> None:
        """create_zone_map can render every entry in ZONES without error."""
        for zone_name, (bounds, rooms, pois) in self.module.ZONES.items():
            with self.subTest(zone=zone_name):
                result = self.module.create_zone_map(bounds, rooms, pois)
                self.assertIsInstance(result, str)
                self.assertTrue(result, f"Empty output for zone {zone_name!r}")


if __name__ == "__main__":
    unittest.main()
