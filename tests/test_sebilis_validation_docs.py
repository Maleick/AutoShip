from __future__ import annotations

from pathlib import Path
import re
import tomllib
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class SebilisValidationDocsTests(unittest.TestCase):
    def test_validation_doc_and_template_exist_with_explicit_evidence_boundaries(self) -> None:
        doc = REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md"
        template = REPO_ROOT / "docs" / "wiki" / "assets" / "sebilis-validation-template.csv"

        self.assertTrue(doc.exists(), "Sebilis validation doc should exist")
        self.assertTrue(template.exists(), "Sebilis validation CSV template should exist")

        text = doc.read_text(encoding="utf-8")
        self.assertIn("## Current Evidence State", text)
        self.assertIn("Research-backed inputs already in repo", text)
        self.assertIn("Needs live proof before this issue can close", text)
        self.assertIn("Nodding Blue Lily", text)
        self.assertIn("textquest/src/camp/forage.rs", text)
        self.assertIn("config/camps/sebilis_disco.toml", text)
        self.assertIn("config/named_mobs/sebilis.toml", text)
        self.assertIn("sebilis-validation-template.csv", text)

        header = template.read_text(encoding="utf-8").splitlines()[0]
        self.assertEqual(
            header,
            "sample_id,validated_at_utc,character,zone_path,camp_name,camp_area,target_metric,target_name,measurement_window_minutes,attempts,successes,observed_item,observed_item_count,result_per_hour,notes,evidence_state",
        )

    def test_farming_guide_links_to_validation_doc_and_marks_sebilis_as_unproven(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Frostreaver-Farming-Guide.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Sebilis-Farming-Validation.md", text)
        self.assertIn("research-backed and still needs live proof", text)

    def test_validation_doc_records_current_sebilis_config_defaults_as_provisional_inputs(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        camp_config = tomllib.loads(
            (REPO_ROOT / "config" / "camps" / "sebilis_disco.toml").read_text(
                encoding="utf-8"
            )
        )

        self.assertIn(f"`pull_radius = {camp_config['pull_radius']}`", text)
        self.assertIn(f"`camp_radius = {camp_config['camp_radius']}`", text)
        self.assertIn(
            f"`level_range = [{camp_config['level_range'][0]}, {camp_config['level_range'][1]}]`",
            text,
        )
        self.assertIn(f"`prev_camp = \"{camp_config['prev_camp']}\"`", text)
        self.assertIn("These defaults are planning inputs only, not live-validated route or spawn proof.", text)

    def test_validation_doc_records_named_timer_baselines_as_config_not_live_spawn_data(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        named_config = tomllib.loads(
            (REPO_ROOT / "config" / "named_mobs" / "sebilis.toml").read_text(
                encoding="utf-8"
            )
        )

        for named in named_config["named"]:
            self.assertIn(named["name"], text)
            self.assertIn(
                f"`{named['respawn_min_minutes']}-{named['respawn_max_minutes']} minutes`",
                text,
            )
        self.assertIn(
            "These timer windows come from the checked-in named config and remain unvalidated until a live sample confirms them.",
            text,
        )

    def test_validation_doc_records_repo_routing_assumptions_as_unvalidated_pathing_inputs(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        prev_camp_config = tomllib.loads(
            (REPO_ROOT / "config" / "camps" / "lguk_dead_side.toml").read_text(
                encoding="utf-8"
            )
        )
        generated_maps = (
            REPO_ROOT / "scripts" / "generate_maps.py"
        ).read_text(encoding="utf-8")

        self.assertEqual(prev_camp_config["next_camp"], "sebilis_disco")
        self.assertIn("\"to_Field_of_Bone\"", generated_maps)
        self.assertIn("\"to_Trakanons_Teeth\"", generated_maps)
        self.assertIn(f"`next_camp = \"{prev_camp_config['next_camp']}\"`", text)
        self.assertIn("`to_Field_of_Bone`", text)
        self.assertIn("`to_Trakanons_Teeth`", text)
        self.assertIn(
            "These routing references show current repo assumptions, not a live-confirmed Scars launch path into Sebilis.",
            text,
        )

    def test_validation_doc_records_forage_defaults_as_runtime_inputs_not_safety_proof(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        forage_source = (REPO_ROOT / "textquest" / "src" / "camp" / "forage.rs").read_text(
            encoding="utf-8"
        )

        interval_match = re.search(
            r"fn default_interval_ms\(\) -> u64 \{\s*([0-9_]+)\s*\}",
            forage_source,
            re.MULTILINE,
        )
        history_match = re.search(
            r"fn default_max_results_history\(\) -> usize \{\s*([0-9_]+)\s*\}",
            forage_source,
            re.MULTILINE,
        )

        self.assertIsNotNone(interval_match)
        self.assertIsNotNone(history_match)
        interval_ms = int(interval_match.group(1).replace("_", ""))
        max_results_history = int(history_match.group(1).replace("_", ""))

        self.assertIn("enabled: false", forage_source)
        self.assertIn("`enabled = false`", text)
        self.assertIn(f"`interval_ms = {interval_ms}`", text)
        self.assertIn(f"`max_results_history = {max_results_history}`", text)
        self.assertIn(
            "These forage defaults describe the current command loop only; they do not prove a safe unattended cadence or a live Nodding Blue Lily rate.",
            text,
        )

    def test_validation_doc_records_current_pull_controls_as_planning_inputs(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        camp_config = tomllib.loads(
            (REPO_ROOT / "config" / "camps" / "sebilis_disco.toml").read_text(
                encoding="utf-8"
            )
        )

        self.assertIn(f"`leash_radius = {camp_config['leash_radius']}`", text)
        self.assertIn(f"`rest_mana_pct = {camp_config['rest_mana_pct']}`", text)
        self.assertIn(f"`pull_mana_pct = {camp_config['pull_mana_pct']}`", text)
        for mob_name in camp_config["pull_mob_names"]:
            self.assertIn(f"`{mob_name}`", text)
        for mob_name in camp_config["ignore_mob_names"]:
            self.assertIn(f"`{mob_name}`", text)
        for mob_name in camp_config["burn_mob_names"]:
            self.assertIn(f"`{mob_name}`", text)
        self.assertIn(
            "These pull-control defaults describe current camp intent only; they do not prove live camp-rotation efficiency or safe overlap.",
            text,
        )

    def test_validation_doc_records_repo_antidetection_confidence_as_non_guarantee(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        security_text = (
            REPO_ROOT / "docs" / "wiki" / "Security-and-Anti-Detection-Notes.md"
        ).read_text(encoding="utf-8")

        self.assertIn("| Timing variation", security_text)
        self.assertIn("| Operator environment", security_text)
        self.assertIn("`Timing variation` as `Medium` confidence", text)
        self.assertIn("`Operator environment` as `High` confidence", text)
        self.assertIn(
            "These repo-grounded exposure labels do not make unattended Sebilis macroing safe.",
            text,
        )

    def test_validation_doc_and_template_record_theorized_items_without_promoting_them_to_validated_outputs(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        template_header = (
            REPO_ROOT / "docs" / "wiki" / "assets" / "sebilis-validation-template.csv"
        ).read_text(encoding="utf-8").splitlines()[0]

        for item_name in (
            "Nodding Blue Lily",
            "Runebranded Girdle",
            "Fungi Tunic",
            "Froglok Blood",
        ):
            self.assertIn(f"`{item_name}`", text)

        self.assertIn("target_name", template_header)
        self.assertIn("observed_item", template_header)
        self.assertIn("observed_item_count", template_header)
        self.assertIn(
            "Record theorized Sebilis outputs as hypotheses only until a live sample observes them.",
            text,
        )

    def test_validation_doc_distinguishes_research_backed_sebilis_loot_from_unsourced_theory(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        frostreaver_text = (
            REPO_ROOT / "docs" / "wiki" / "Frostreaver-Farming-Guide.md"
        ).read_text(encoding="utf-8")
        p99_text = (REPO_ROOT / "docs" / "wiki" / "P99-Zone-Guide.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Runebranded Girdle", frostreaver_text)
        self.assertIn("Runebranded Girdle", p99_text)
        self.assertIn("`Runebranded Girdle`", text)
        self.assertIn("Research-backed loot theory", text)
        self.assertIn("Issue-theory only", text)
        self.assertIn(
            "`Nodding Blue Lily` remains an issue-theory hypothesis until a repo-local source or live sample anchors it.",
            text,
        )
        self.assertIn(
            "`Fungi Tunic` currently appears only in a generic item-command example, not a Sebilis evidence source.",
            text,
        )
        self.assertIn(
            "`Froglok Blood` currently has no repo-local Sebilis evidence source beyond the issue theory.",
            text,
        )


if __name__ == "__main__":
    unittest.main()
