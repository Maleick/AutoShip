from __future__ import annotations

import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


def read(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


class RoadmapM10EconomyTests(unittest.TestCase):
    def test_canonical_roadmap_keeps_economy_before_soul_engine(self) -> None:
        roadmap = read("docs/implementation-roadmap.md")
        self.assertIn("### `M10` Economy", roadmap)
        self.assertIn("### `M11` Soul Engine + LLM", roadmap)
        self.assertLess(
            roadmap.index("### `M10` Economy"),
            roadmap.index("### `M11` Soul Engine + LLM"),
        )

    def test_readme_and_wiki_mirror_m10_m11_order(self) -> None:
        readme = read("README.md")
        wiki = read("docs/wiki/Roadmap-and-Known-Gaps.md")
        architecture = read("docs/wiki/Architecture-Overview.md")

        self.assertIn("- [ ] **M10** — Economy", readme)
        self.assertIn("- [ ] **M11** — Soul Engine + LLM", readme)
        self.assertIn("tracked under `M11` in the canonical roadmap", readme)
        self.assertIn("- `M10`: Economy", wiki)
        self.assertIn("- `M11`: Soul Engine + LLM (local AI only)", wiki)
        self.assertIn("belongs to `M11` in the canonical roadmap", architecture)

    def test_feature_list_tracks_m10_economy_execution_surfaces(self) -> None:
        payload = json.loads(read("feature-list.json"))
        feature = next(
            (item for item in payload["features"] if item["id"] == "m10-economy-execution-surfaces"),
            None,
        )

        self.assertIsNotNone(feature)
        self.assertEqual(feature["status"], "in_progress")
        self.assertIn("loot/distribution", feature["notes"])
        self.assertIn("vendor", feature["notes"])
        self.assertIn("banking", feature["notes"])
        self.assertIn("operator-visible", feature["notes"])


if __name__ == "__main__":
    unittest.main()
