from __future__ import annotations

import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class M10EconomyDocsTests(unittest.TestCase):
    def test_readme_points_to_canonical_roadmap_and_keeps_economy_before_soul(self) -> None:
        text = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("README stays focused on building, running, and operating TextQuest.", text)
        self.assertIn("economy work at `M10`", text)
        self.assertIn("Soul Engine + LLM work at `M11`", text)
        self.assertLess(text.index("economy work at `M10`"), text.index("Soul Engine + LLM work at `M11`"))
        self.assertIn("tracked under `M11` in the canonical roadmap", text)

    def test_roadmap_defines_m10_execution_slices_and_gates(self) -> None:
        text = (REPO_ROOT / "docs" / "implementation-roadmap.md").read_text(encoding="utf-8")
        self.assertIn("### `M10` Economy", text)
        self.assertIn("Loot intake and distribution workflow", text)
        self.assertIn("Vendor cycle controller", text)
        self.assertIn("Banking cycle controller", text)
        self.assertIn("Economy-facing TUI summaries and overrides", text)
        self.assertIn("Entry gate:", text)
        self.assertIn("Exit gate:", text)

    def test_supporting_docs_keep_economy_and_soul_milestones_aligned(self) -> None:
        expectations = {
            REPO_ROOT / "docs" / "wiki" / "Soul-Engine.md": [
                "under `M11` in the canonical roadmap",
            ],
            REPO_ROOT / "docs" / "wiki" / "Architecture-Overview.md": [
                "belongs to `M11` in the canonical roadmap",
            ],
            REPO_ROOT / "docs" / "wiki" / "Research-Packet-Engine.md": [
                "candidates for `M11` Soul Engine chat",
                "economy work is `M10`",
                "chat ingest (`M11`)",
            ],
            REPO_ROOT / "docs" / "wiki" / "Roadmap-and-Known-Gaps.md": [
                "## Current `M10` economy guidance",
                "`M10`: Economy",
                "`M11`: Soul Engine + LLM (local AI only)",
            ],
        }

        for path, snippets in expectations.items():
            text = path.read_text(encoding="utf-8")
            with self.subTest(path=path):
                for snippet in snippets:
                    self.assertIn(snippet, text)

    def test_feature_list_tracks_m10_economy_scope(self) -> None:
        payload = json.loads((REPO_ROOT / "feature-list.json").read_text(encoding="utf-8"))
        feature = next(item for item in payload["features"] if item["id"] == "m10-economy-execution-scope")
        self.assertEqual(feature["status"], "in_progress")
        self.assertIn(
            "Roadmap-facing docs route provider-backed Soul/LLM work to M11 and economy execution loops to M10",
            feature["acceptance_tests"],
        )


if __name__ == "__main__":
    unittest.main()
