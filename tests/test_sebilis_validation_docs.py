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
            "sample_id,validated_at_utc,character,launch_staging_point,zone_path,required_keying,camp_name,camp_area,target_metric,target_name,measurement_window_minutes,travel_time_minutes,placeholder_count,named_count,mean_respawn_minutes,wait_time_minutes,attempts,successes,observed_item,observed_item_count,result_per_hour,operator_mode,notes,evidence_state",
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
            for drop in named["drops"]:
                self.assertIn(f"`{drop}`", text)
        self.assertIn(
            "These timer windows come from the checked-in named config and remain unvalidated until a live sample confirms them.",
            text,
        )
        self.assertIn(
            "Those checked-in named drops do not currently anchor the issue's primary",
            text,
        )
        self.assertIn(
            "target outputs of `Nodding Blue Lily`, `Runebranded Girdle`, `Fungi Tunic`,",
            text,
        )
        self.assertIn("or `Froglok Blood`.", text)

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
        orchestration_design = (
            REPO_ROOT / "docs" / "orchestration-design.md"
        ).read_text(encoding="utf-8")

        self.assertEqual(prev_camp_config["next_camp"], "sebilis_disco")
        self.assertIn("\"to_Field_of_Bone\"", generated_maps)
        self.assertIn("\"to_Trakanons_Teeth\"", generated_maps)
        self.assertIn("Camp database for launch zones", orchestration_design)
        self.assertIn(f"`next_camp = \"{prev_camp_config['next_camp']}\"`", text)
        self.assertIn("`to_Field_of_Bone`", text)
        self.assertIn("`to_Trakanons_Teeth`", text)
        self.assertIn("`Camp database for launch zones`", text)
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

    def test_validation_doc_records_overnight_safety_docs_as_gap_tracking_not_safe_automation(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        coverage_text = (
            REPO_ROOT / "docs" / "MQ2_COVERAGE_GAP_ANALYSIS.md"
        ).read_text(encoding="utf-8")
        overnight_summary_text = (
            REPO_ROOT / "docs" / "OVERNIGHT-ISSUE-SUMMARY.md"
        ).read_text(encoding="utf-8")

        self.assertIn("MQ2AutoCamp", coverage_text)
        self.assertIn("MQ2GMCheck", coverage_text)
        self.assertIn("MQ2KillTracker", coverage_text)
        self.assertIn("MQ2PlatTracker", coverage_text)
        self.assertIn("MQ2Log", coverage_text)
        self.assertIn("Protects accounts from bans, detection, and suspension during overnight testing.", overnight_summary_text)

        self.assertIn("Research-backed blocker", text)
        self.assertIn("`docs/MQ2_COVERAGE_GAP_ANALYSIS.md`", text)
        self.assertIn("`docs/OVERNIGHT-ISSUE-SUMMARY.md`", text)
        self.assertIn(
            "GM alerts, auto-camp-on-death, kill or plat tracking, and session logs are documented as required or gap-tracked overnight tooling, not validated Sebilis-safe automation.",
            text,
        )
        self.assertIn(
            "Those overnight safety docs describe required or proposed operator tooling;",
            text,
        )
        self.assertIn(
            "they do not prove that unattended Sebilis macroing is currently safe or fully",
            text,
        )
        self.assertIn("instrumented in TextQuest.", text)

    def test_validation_doc_records_operator_controls_and_observability_as_attended_run_aids(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        event_text = (REPO_ROOT / "textquest" / "src" / "tui" / "event.rs").read_text(
            encoding="utf-8"
        )
        state_text = (REPO_ROOT / "textquest" / "src" / "tui" / "state.rs").read_text(
            encoding="utf-8"
        )
        observability_text = (
            REPO_ROOT / "docs" / "dev" / "observability.md"
        ).read_text(encoding="utf-8")

        self.assertIn("KeyCode::Home", event_text)
        self.assertIn("Automation PAUSED", event_text)
        self.assertIn("KeyCode::End", event_text)
        self.assertIn("Automation RESUMED", event_text)
        self.assertIn('KeyCode::Char(\'p\' | \'P\')', event_text)
        self.assertIn('KeyCode::Char(\'r\' | \'R\')', event_text)
        self.assertIn('KeyCode::Char(\'a\' | \'A\')', event_text)
        self.assertIn("pub automation_paused: bool,", state_text)
        self.assertIn("automation_paused: false,", state_text)
        self.assertIn('zone = "sebilis"', observability_text)
        self.assertIn('`HOME` pauses automation, `END` resumes automation', text)
        self.assertIn('binds `P` to pause, `R` to resume, and `A` to abort', text)
        self.assertIn("`automation_paused` as explicit operator", text)
        self.assertIn('structured `zone = "sebilis"`', text)
        self.assertIn(
            "These operator controls and observability examples are useful for attended",
            text,
        )
        self.assertIn(
            "Sebilis sampling, but they do not prove that unattended handling is safe or",
            text,
        )
        self.assertIn(
            "that Sebilis-specific spawn or forage metrics are already captured.",
            text,
        )

    def test_validation_doc_records_spawn_observability_as_measurement_aid_not_respawn_proof(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        spawn_text = (REPO_ROOT / "textquest" / "src" / "eq" / "spawn.rs").read_text(
            encoding="utf-8"
        )
        tui_run_text = (REPO_ROOT / "textquest" / "src" / "tui" / "run.rs").read_text(
            encoding="utf-8"
        )
        observability_text = (
            REPO_ROOT / "docs" / "dev" / "observability.md"
        ).read_text(encoding="utf-8")

        self.assertIn("spawn_count = spawns.len()", spawn_text)
        self.assertIn("\"TUI spawn snapshot refreshed\"", tui_run_text)
        self.assertIn("spawn_count = client.spawns.len()", tui_run_text)
        self.assertIn("spawn_revision", tui_run_text)
        self.assertIn("elapsed_ms", tui_run_text)
        self.assertIn('name: "spawn_count".to_string()', observability_text)
        self.assertIn('("zone".to_string(), "sebilis".to_string())', observability_text)
        self.assertIn("TextQuest already emits spawn-refresh observability", text)
        self.assertIn("`spawn_count`", text)
        self.assertIn("`TUI spawn snapshot refreshed`", text)
        self.assertIn("`spawn_revision`", text)
        self.assertIn("`elapsed_ms`", text)
        self.assertIn(
            "These spawn observability hooks help instrument attended sampling sessions,",
            text,
        )
        self.assertIn(
            "but they still do not capture camp-by-camp Sebilis respawn timing, named",
            text,
        )
        self.assertIn("overlap, or validated wait-time baselines.", text)

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
        self.assertIn("launch_staging_point", template_header)
        self.assertIn("required_keying", template_header)
        self.assertIn("travel_time_minutes", template_header)
        self.assertIn("placeholder_count", template_header)
        self.assertIn("named_count", template_header)
        self.assertIn("mean_respawn_minutes", template_header)
        self.assertIn("wait_time_minutes", template_header)
        self.assertIn("observed_item", template_header)
        self.assertIn("observed_item_count", template_header)
        self.assertIn("operator_mode", template_header)
        self.assertIn(
            "Record theorized Sebilis outputs as hypotheses only until a live sample observes them.",
            text,
        )
        self.assertIn(
            "The checked-in template now includes explicit columns for",
            text,
        )
        self.assertIn("`launch_staging_point`", text)
        self.assertIn("`required_keying`", text)
        self.assertIn("`travel_time_minutes`", text)
        self.assertIn("`placeholder_count`", text)
        self.assertIn("`named_count`", text)
        self.assertIn("`mean_respawn_minutes`", text)
        self.assertIn("`wait_time_minutes`", text)
        self.assertIn("`operator_mode`", text)

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
        self.assertIn(
            "The current `config/named_mobs/sebilis.toml` drop list instead tracks",
            text,
        )
        self.assertIn("`Singing Short Sword`", text)
        self.assertIn("`Trakanon's Tooth`", text)
        self.assertIn("`Elder Spiritist's Helm`", text)
        self.assertIn("`Crypt Caretaker's Shield`", text)
        self.assertIn("`Sebilite Scale Leggings`", text)

    def test_validation_doc_distinguishes_rotation_and_output_priors_from_issue_only_theory(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Sebilis-Farming-Validation.md").read_text(
            encoding="utf-8"
        )
        frostreaver_text = (
            REPO_ROOT / "docs" / "wiki" / "Frostreaver-Farming-Guide.md"
        ).read_text(encoding="utf-8")
        p99_text = (REPO_ROOT / "docs" / "wiki" / "P99-Zone-Guide.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Right wing (Disco 1+2)", frostreaver_text)
        self.assertIn("juggs/myconids", frostreaver_text)
        self.assertIn("5-6 groups", p99_text)
        self.assertIn("Requires key", p99_text)
        self.assertIn("~400pp/hr", p99_text)
        self.assertIn("500-1000pp", frostreaver_text)

        self.assertIn("### Current routing, rotation, and output theory status", text)
        self.assertIn("Research-backed rotation theory", text)
        self.assertIn("Research-backed route requirement", text)
        self.assertIn("Research-backed economy theory", text)
        self.assertIn(
            "Current guides describe `4-6 groups` or `5-6 groups` across these camp areas, but the repo still lacks live wait-time and overlap measurements.",
            text,
        )
        self.assertIn(
            "The guide says `Requires key`, but this repo still has no live Scars-launch route or corpse-recovery sample proving the requirement in practice.",
            text,
        )
        self.assertIn(
            "These are planning priors from guides, not live TextQuest output data.",
            text,
        )
        self.assertIn(
            "Overnight Sebilis output should land around `1000-2000pp per Shaman per night`.",
            text,
        )
        self.assertIn(
            "This output target is still unanchored by repo-local evidence or live samples.",
            text,
        )


if __name__ == "__main__":
    unittest.main()
