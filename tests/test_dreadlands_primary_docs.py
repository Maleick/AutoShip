from __future__ import annotations

from pathlib import Path
import unittest

try:
    import tomllib
except ImportError:
    import tomli as tomllib


REPO_ROOT = Path(__file__).resolve().parents[1]


class DreadlandsPrimaryDocsTests(unittest.TestCase):
    def test_dreadlands_primary_doc_and_config_exist(self) -> None:
        doc = REPO_ROOT / "docs" / "wiki" / "Dreadlands-Primary-Camp.md"
        config = REPO_ROOT / "config" / "camps" / "dreadlands_primary.toml"

        self.assertTrue(doc.exists(), "Dreadlands primary camp doc should exist")
        self.assertTrue(config.exists(), "Dreadlands primary camp config should exist")

    def test_dreadlands_primary_doc_covers_route_controls_and_blockers(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Dreadlands-Primary-Camp.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("## Current Evidence State", text)
        self.assertIn("## Primary Camp Definition", text)
        self.assertIn("## Waypoint Route", text)
        self.assertIn("## Restriction Zones and Pull Controls", text)
        self.assertIn("## Travel, Faction, and Level Notes", text)
        self.assertIn("## Validation Status", text)
        self.assertIn("## Schema and Data Gaps", text)
        self.assertIn("`config/camps/dreadlands_primary.toml`", text)
        self.assertIn("6:40", text)
        self.assertIn("6-box live validation", text)
        self.assertIn("blocked", text.lower())

        waypoint_rows = [
            line
            for line in text.splitlines()
            if (line.lstrip().lstrip("|").lstrip().startswith("DL-"))
            and not (line.lstrip().lstrip("|").lstrip().startswith("DL-ID "))
        ]
        self.assertGreaterEqual(
            len(waypoint_rows),
            20,
            "Dreadlands route doc should define at least 20 waypoints",
        )

    def test_dreadlands_primary_config_and_docs_stay_aligned(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Dreadlands-Primary-Camp.md").read_text(
            encoding="utf-8"
        )
        camp_config = tomllib.loads(
            (REPO_ROOT / "config" / "camps" / "dreadlands_primary.toml").read_text(
                encoding="utf-8"
            )
        )

        self.assertIn(f"`pull_radius = {camp_config['pull_radius']}`", text)
        self.assertIn(f"`camp_radius = {camp_config['camp_radius']}`", text)
        self.assertIn(f"`leash_radius = {camp_config['leash_radius']}`", text)
        self.assertIn(f"`rest_mana_pct = {camp_config['rest_mana_pct']}`", text)
        self.assertIn(f"`pull_mana_pct = {camp_config['pull_mana_pct']}`", text)
        self.assertIn(
            f"`level_range = [{camp_config['level_range'][0]}, {camp_config['level_range'][1]}]`",
            text,
        )
        self.assertIn("Ancient Combine Outpost", text)
        self.assertIn("safe med", text.lower())
        self.assertIn("roamers", text.lower())
        self.assertIn("Gorenaire", text)
