#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
import textwrap
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
VALIDATOR = REPO_ROOT / "scripts" / "validate-map-bounds.py"


CAMPS = {
    "crescent_reach_newbie": {"zone": "crescent", "camp_center": [-1200.0, -250.0, 3.0], "pull_point": [-1150.0, -200.0, 3.0], "pull_radius": 150},
    "crescent_reach_undead": {"zone": "crescent", "camp_center": [-900.0, -100.0, 1.0], "pull_point": [-930.0, -130.0, 1.0], "pull_radius": 150},
    "crushbone_entrance": {"zone": "crushbone", "camp_center": [300.0, 600.0, 2.0], "pull_point": [350.0, 620.0, 2.0], "pull_radius": 180},
    "crushbone_throne": {"zone": "crushbone", "camp_center": [340.0, 620.0, 2.0], "pull_point": [330.0, 650.0, 2.0], "pull_radius": 180},
    "lguk_live_side": {"zone": "gukbottom", "camp_center": [55.0, -40.0, 0.0], "pull_point": [70.0, -20.0, 0.0], "pull_radius": 180},
    "lguk_dead_side": {"zone": "gukbottom", "camp_center": [65.0, -35.0, 0.0], "pull_point": [80.0, -15.0, 0.0], "pull_radius": 180},
    "mistmoore_entrance": {"zone": "mistmoore", "camp_center": [120.0, 140.0, 4.0], "pull_point": [110.0, 130.0, 4.0], "pull_radius": 220},
    "mistmoore_castle": {"zone": "mistmoore", "camp_center": [140.0, 160.0, 4.0], "pull_point": [130.0, 145.0, 4.0], "pull_radius": 220},
    "sebilis_disco": {"zone": "sebilis", "camp_center": [200.0, 180.0, 4.0], "pull_point": [180.0, 140.0, 4.0], "pull_radius": 150},
    "unrest_basement": {"zone": "unrest", "camp_center": [100.0, -100.0, 6.0], "pull_point": [90.0, -90.0, 6.0], "pull_radius": 250},
    "unrest_yard": {"zone": "unrest", "camp_center": [120.0, -120.0, 6.0], "pull_point": [105.0, -90.0, 6.0], "pull_radius": 250},
}


def write_toml_file(path: Path, values: dict[str, object]) -> None:
    path.write_text(
        textwrap.dedent(
            f"""\
            name = "{path.stem}"
            zone = "{values['zone']}"
            camp_center = {values['camp_center']}
            pull_point = {values['pull_point']}
            pull_radius = {values['pull_radius']}
            """
        ).strip()
        + "\n"
    )


def write_map_file(path: Path, x_min: float, x_max: float, y_min: float, y_max: float) -> None:
    path.write_text(
        "\n".join(
            [
                f"L {x_min}, {y_min}, 0, {x_max}, {y_min}, 0, 255, 255, 255",
                f"L {x_max}, {y_min}, 0, {x_max}, {y_max}, 0, 255, 255, 255",
                f"L {x_max}, {y_max}, 0, {x_min}, {y_max}, 0, 255, 255, 255",
                f"L {x_min}, {y_max}, 0, {x_min}, {y_min}, 0, 255, 255, 255",
                "",
            ]
        )
    )


def run_validator(camp_dir: Path, map_dir: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(VALIDATOR), str(camp_dir), str(map_dir)],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )


class ValidateMapBoundsTests(unittest.TestCase):
    def test_all_target_camps_pass_within_bounds(self) -> None:
        with tempfile.TemporaryDirectory(prefix="camps_") as camp_root, tempfile.TemporaryDirectory(prefix="maps_") as map_root:
            camp_dir = Path(camp_root)
            map_dir = Path(map_root)

            for name, values in CAMPS.items():
                write_toml_file(camp_dir / f"{name}.toml", values)
                map_name = f"{values['zone']}.txt"
                if not (map_dir / map_name).exists():
                    write_map_file(map_dir / map_name, -2000, 2000, -2000, 2000)

            result = run_validator(camp_dir, map_dir)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("PASS: all 11 target camp locations validated.", result.stdout)
            self.assertIn("Diagram:", result.stdout)

    def test_out_of_bounds_camp_reports_error(self) -> None:
        with tempfile.TemporaryDirectory(prefix="camps_") as camp_root, tempfile.TemporaryDirectory(prefix="maps_") as map_root:
            camp_dir = Path(camp_root)
            map_dir = Path(map_root)

            for name, values in CAMPS.items():
                if name == "sebilis_disco":
                    values = dict(values)
                    values["camp_center"] = [5000.0, 5000.0, 4.0]
                write_toml_file(camp_dir / f"{name}.toml", values)
                map_name = f"{values['zone']}.txt"
                if not (map_dir / map_name).exists():
                    write_map_file(map_dir / map_name, -200, 200, -200, 200)

            result = run_validator(camp_dir, map_dir)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Camp: sebilis_disco", result.stdout)
            self.assertIn("Camp within bounds? ✗", result.stdout)
