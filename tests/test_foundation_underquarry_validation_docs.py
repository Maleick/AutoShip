from __future__ import annotations

from pathlib import Path
import unittest

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11 fallback
    import tomli as tomllib
REPO_ROOT = Path(__file__).resolve().parents[1]


class FoundationUnderquarryValidationDocsTests(unittest.TestCase):
    def test_validation_doc_and_template_exist_with_explicit_evidence_boundaries(self) -> None:
        doc = REPO_ROOT / "docs" / "wiki" / "Foundation-Underquarry-Scouts-Camp.md"
        template = (
            REPO_ROOT
            / "docs"
            / "wiki"
            / "assets"
            / "foundation-underquarry-validation-template.csv"
        )

        self.assertTrue(doc.exists(), "Foundation validation doc should exist")
        self.assertTrue(template.exists(), "Foundation validation CSV template should exist")

        text = doc.read_text(encoding="utf-8")
        self.assertIn("## Current Evidence State", text)
        self.assertIn("## Research Waypoint Lattice", text)
        self.assertIn("## Checked-in Sampling Template", text)
        self.assertIn("## Needs Live Proof Before This Issue Can Close", text)
        self.assertIn("foundation-underquarry-validation-template.csv", text)
        self.assertIn("6-box live validation is blocked in this session.", text)
        self.assertIn("there is no live EverQuest runtime in the workspace", text)
        self.assertIn("issue `#1396`", text)
        self.assertIn("`config/maps/*.txt`", text)
        self.assertIn("no other checked-in Underfoot operator note", text)

        header = template.read_text(encoding="utf-8").splitlines()[0]
        self.assertEqual(
            header,
            "sample_id,validated_at_utc,character,launch_staging_point,zone_path,required_keying,camp_name,waypoint_sequence,waypoint_traversal_result,pull_point,pull_target,pull_leash_result,rest_mana_pct,pull_mana_pct,mana_behavior_result,return_route,return_to_camp_result,measurement_window_minutes,placeholder_count,named_count,mean_respawn_minutes,operator_mode,notes,evidence_state",
        )

    def test_validation_doc_records_runtime_config_defaults_as_loader_safe_inputs(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Foundation-Underquarry-Scouts-Camp.md").read_text(
            encoding="utf-8"
        )
        camp_config = tomllib.loads(
            (REPO_ROOT / "config" / "camps" / "foundation_underquarry_scouts.toml").read_text(
                encoding="utf-8"
            )
        )

        self.assertIn(f"`pull_radius` | `{camp_config['pull_radius']}`", text)
        self.assertIn(f"`camp_radius` | `{camp_config['camp_radius']}`", text)
        self.assertIn(f"`leash_radius` | `{camp_config['leash_radius']}`", text)
        self.assertIn(f"`rest_mana_pct` | `{camp_config['rest_mana_pct']}`", text)
        self.assertIn(f"`pull_mana_pct` | `{camp_config['pull_mana_pct']}`", text)
        self.assertIn(
            f"`level_range` | `[{camp_config['level_range'][0]}, {camp_config['level_range'][1]}]`",
            text,
        )
        self.assertIn("The runtime config stays inside the existing center/pull/radius/mana schema.", text)
        self.assertIn("Issue `#1900` still tracks the camp-schema work", text)

    def test_validation_doc_records_waypoint_and_template_fields_needed_for_live_proof(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Foundation-Underquarry-Scouts-Camp.md").read_text(
            encoding="utf-8"
        )

        for waypoint in ("UF-01", "UF-05", "UF-10", "UF-24"):
            self.assertIn(waypoint, text)

        self.assertIn("waypoint traversal outcome", text)
        self.assertIn("pull leash outcome", text)
        self.assertIn("mana rest threshold behavior", text)
        self.assertIn("return-to-camp recovery outcome", text)
        self.assertIn("approximate respawn cadence", text)
        self.assertIn("Record all values as observed results only after a real Foundation run.", text)
        self.assertIn("`UF-05 -> UF-06 -> UF-08 -> UF-09 -> UF-10 -> UF-09 -> UF-07 -> UF-05`", text)


if __name__ == "__main__":
    unittest.main()
