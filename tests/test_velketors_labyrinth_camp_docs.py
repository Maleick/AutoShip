from __future__ import annotations

from pathlib import Path
import re
import tomllib
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class VelketorsLabyrinthCampDocsTests(unittest.TestCase):
    def test_companion_doc_exists_with_waypoints_restrictions_route_and_schema_gap(self) -> None:
        doc = REPO_ROOT / "docs" / "wiki" / "Velketors-Labyrinth-Frenzy-Camp.md"
        self.assertTrue(doc.exists(), "Velketor companion doc should exist")

        text = doc.read_text(encoding="utf-8")
        self.assertIn("# Velketor's Labyrinth Frenzy Camp", text)
        self.assertIn("## Runtime Camp Config", text)
        self.assertIn("## Travel, Level, And Faction Constraints", text)
        self.assertIn("## Research Waypoint Lattice", text)
        self.assertIn("## Restriction Zones", text)
        self.assertIn("## Multibox Route Notes", text)
        self.assertIn("## Spawn Pattern Notes", text)
        self.assertIn("## Schema Gap", text)
        self.assertIn("## Validation Status", text)
        self.assertIn("Great Divide", text)
        self.assertIn("Velketor (-0)", text)
        self.assertIn("return_no_aggro", text)
        self.assertIn("32:50", text)

        waypoint_rows = re.findall(r"^\|\s*[0-9]{1,2}\s*\|", text, re.MULTILINE)
        self.assertGreaterEqual(
            len(waypoint_rows),
            20,
            "Velketor companion doc should define at least 20 waypoint rows",
        )

    def test_configuration_doc_points_to_velketor_companion_doc(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Configuration.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Velketors-Labyrinth-Frenzy-Camp", text)
        self.assertIn("document richer waypoint and restriction notes", text)
        self.assertIn("consumes the baseline TOML fields", text)

    def test_farming_guide_links_velketor_docs_and_marks_route_as_unproven(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Frostreaver-Farming-Guide.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Velketors-Labyrinth-Frenzy-Camp.md", text)
        self.assertIn("Velketors-Labyrinth-Validation.md", text)
        self.assertIn("research-backed", text)
        self.assertIn("still needs live proof", text)

    def test_sidebar_links_velketor_companion_doc(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "_Sidebar.md").read_text(encoding="utf-8")

        self.assertIn("[Velketor's Labyrinth Frenzy Camp](Velketors-Labyrinth-Frenzy-Camp)", text)
        self.assertIn("[Velketor's Labyrinth Validation](Velketors-Labyrinth-Validation)", text)

    def test_companion_doc_reflects_checked_in_runtime_defaults(self) -> None:
        text = (
            REPO_ROOT / "docs" / "wiki" / "Velketors-Labyrinth-Frenzy-Camp.md"
        ).read_text(encoding="utf-8")
        camp_config = tomllib.loads(
            (
                REPO_ROOT / "config" / "camps" / "velketors_labyrinth_frenzy.toml"
            ).read_text(encoding="utf-8")
        )

        self.assertIn(f"`camp_center` | `{camp_config['camp_center']}`", text)
        self.assertIn(f"`pull_point` | `{camp_config['pull_point']}`", text)
        self.assertIn(f"`pull_radius` | `{camp_config['pull_radius']}`", text)
        self.assertIn(f"`camp_radius` | `{camp_config['camp_radius']}`", text)
        self.assertIn(f"`leash_radius` | `{camp_config['leash_radius']}`", text)
        self.assertIn(f"`rest_mana_pct` | `{camp_config['rest_mana_pct']}`", text)
        self.assertIn(f"`pull_mana_pct` | `{camp_config['pull_mana_pct']}`", text)
        self.assertIn(
            f"`level_range` | `[{camp_config['level_range'][0]}, {camp_config['level_range'][1]}]`",
            text,
        )
        self.assertIn(
            "The runtime loader does not yet model named waypoints, restriction polygons, or",
            text,
        )
        self.assertIn(
            "ordered route segments. Those stay documented here so",
            text,
        )

    def test_companion_doc_records_current_pull_controls_and_named_filters(self) -> None:
        text = (
            REPO_ROOT / "docs" / "wiki" / "Velketors-Labyrinth-Frenzy-Camp.md"
        ).read_text(encoding="utf-8")
        camp_config = tomllib.loads(
            (
                REPO_ROOT / "config" / "camps" / "velketors_labyrinth_frenzy.toml"
            ).read_text(encoding="utf-8")
        )

        self.assertTrue(camp_config["return_no_aggro"])
        self.assertEqual(camp_config["prev_camp"], "sebilis_disco")
        for mob_name in camp_config["burn_mob_names"]:
            self.assertIn(f"`{mob_name}`", text)
        self.assertIn("Lord Bob", text)
        self.assertIn("Bled/Bledrek", text)
        self.assertIn("upper dogs", text)

    def test_validation_ledger_and_template_exist(self) -> None:
        validation_doc = (
            REPO_ROOT / "docs" / "wiki" / "Velketors-Labyrinth-Validation.md"
        )
        template = (
            REPO_ROOT / "docs" / "wiki" / "assets" / "velketors-validation-template.csv"
        )

        self.assertTrue(validation_doc.exists(), "Velketor validation ledger should exist")
        self.assertTrue(template.exists(), "Velketor validation template should exist")

        validation_text = validation_doc.read_text(encoding="utf-8")
        self.assertIn("# Velketor's Labyrinth Validation", validation_text)
        self.assertIn("## Current Evidence State", validation_text)
        self.assertIn("## Checked-in Sampling Template", validation_text)
        self.assertIn("velketors-validation-template.csv", validation_text)
        self.assertIn("6-box", validation_text)

        companion_text = (
            REPO_ROOT / "docs" / "wiki" / "Velketors-Labyrinth-Frenzy-Camp.md"
        ).read_text(encoding="utf-8")
        self.assertIn("Velketor's Labyrinth Validation", companion_text)
        self.assertIn("velketors-validation-template.csv", companion_text)


if __name__ == "__main__":
    unittest.main()
