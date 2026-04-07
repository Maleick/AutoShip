from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "validate_roadmap_unknowns.py"


def load_module():
    spec = importlib.util.spec_from_file_location("validate_roadmap_unknowns", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load validate_roadmap_unknowns module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class RoadmapValidatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_analyze_actual_roadmap(self) -> None:
        report = self.module.analyze_roadmap(REPO_ROOT / "docs" / "implementation-roadmap.md")
        self.assertEqual(report.missing_milestones, [])
        self.assertEqual(report.missing_domains, [])
        self.assertEqual(report.missing_requested_domains, [])
        self.assertEqual(report.missing_evidence_states, [])
        self.assertEqual(report.missing_evidence_rules, [])
        self.assertEqual(report.missing_sections, [])
        self.assertEqual(report.unresolved_critical_unknowns, 4)
        self.assertTrue(report.evidence_mechanics_ok)
        self.assertTrue(report.ready_for_autoresearch)
        self.assertEqual(
            report.milestone_ids,
            ["M5", "M6", "M7", "M8", "M9", "M10", "M11"],
        )

    def test_cli_json_output(self) -> None:
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--json", "--domains", "packet,zoning,anticheat"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn('"unresolved_critical_unknowns": 4', completed.stdout)
        self.assertIn('"evidence_mechanics_ok": true', completed.stdout)
        self.assertIn('"missing_requested_domains": []', completed.stdout)

    def test_strict_mode_fails_for_broken_evidence_rules(self) -> None:
        broken_doc = textwrap.dedent(
            """
            # TextQuest Implementation Roadmap

            ## Canonical Milestone Order

            ### `M5` Packet Engine

            ## Domain Tracks

            - Packet Engine
            - Zoning/Movement
            - Anti-Cheat
            - Orchestrator
            - Docs/Workflow

            ## Evidence Model

            - `Provisional`
            - `Research-backed`
            - `Needs Live Proof`
            - `Live-validated`
            - `Invalidated`

            Rules:

            - `Research-backed` items may enter active execution work.
            - `Provisional` items may exist in research ledgers and checkpoint intake.
            """
        ).strip()

        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "roadmap.md"
            path.write_text(broken_doc, encoding="utf-8")
            report = self.module.analyze_roadmap(path)
            self.assertFalse(report.evidence_mechanics_ok)
            self.assertIn(
                self.module.EXPECTED_EVIDENCE_RULES[1],
                report.missing_evidence_rules,
            )
            self.assertIn("Autoresearch Workflow", report.missing_sections)
            code = self.module.main(["--roadmap", str(path), "--strict"])
            self.assertEqual(code, 1)

    def test_analyze_custom_roadmap_detects_missing_sections(self) -> None:
        custom_doc = textwrap.dedent(
            """
            # TextQuest Implementation Roadmap

            ## Canonical Milestone Order

            ### `M5` Packet Engine

            ### `M6` Zoning/Movement

            ## Domain Tracks

            - Packet Engine

            ## Evidence Model

            - `Provisional`

            Rules:

            - `Research-backed` items may enter active execution work.
            """
        ).strip()

        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "roadmap.md"
            path.write_text(custom_doc, encoding="utf-8")
            report = self.module.analyze_roadmap(path)

        self.assertEqual(report.missing_milestones, ["M7", "M8", "M9", "M10", "M11"])
        self.assertEqual(
            report.missing_domains,
            ["Anti-Cheat", "Orchestrator", "Docs/Workflow"],
        )
        self.assertEqual(
            report.missing_evidence_states,
            ["Needs Live Proof", "Live-validated", "Invalidated"],
        )
        self.assertEqual(
            report.missing_evidence_rules,
            [
                self.module.EXPECTED_EVIDENCE_RULES[1],
                self.module.EXPECTED_EVIDENCE_RULES[2],
            ],
        )
        self.assertIn("Autoresearch Workflow", report.missing_sections)
        self.assertFalse(report.ready_for_autoresearch)

    def test_requested_domains_are_checked(self) -> None:
        report = self.module.analyze_roadmap(
            REPO_ROOT / "docs" / "implementation-roadmap.md",
            requested_domains=["Packet Engine", "Zoning/Movement", "Anti-Cheat"],
        )
        self.assertEqual(report.missing_requested_domains, [])

    def test_m9_section_defines_metrics_and_guardrails(self) -> None:
        roadmap = (REPO_ROOT / "docs" / "implementation-roadmap.md").read_text(
            encoding="utf-8"
        )
        m9_section = roadmap.split("### `M9` Learning/RL", maxsplit=1)[1].split(
            "### `M10` Economy", maxsplit=1
        )[0]

        self.assertIn("behavior optimization targets", m9_section)
        self.assertIn("measurable tuning loops", m9_section)
        self.assertIn("guardrails that prevent regressions from training-driven changes", m9_section)
        self.assertIn("named baseline, success metric, regression budget, and rollback path", m9_section)
        self.assertIn("replay, shadow, or canary mode", m9_section)
        self.assertIn("authenticated control boundary", m9_section)


if __name__ == "__main__":
    unittest.main()
