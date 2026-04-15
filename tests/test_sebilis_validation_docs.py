from __future__ import annotations

from pathlib import Path
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
            "sample_id,validated_at_utc,character,zone_path,camp_name,camp_area,target_metric,measurement_window_minutes,attempts,successes,result_per_hour,notes,evidence_state",
        )

    def test_farming_guide_links_to_validation_doc_and_marks_sebilis_as_unproven(self) -> None:
        text = (REPO_ROOT / "docs" / "wiki" / "Frostreaver-Farming-Guide.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Sebilis-Farming-Validation.md", text)
        self.assertIn("research-backed and still needs live proof", text)


if __name__ == "__main__":
    unittest.main()
