from __future__ import annotations

from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class AwarenessCoordinationParityDocsTests(unittest.TestCase):
    def test_parity_doc_exists_and_classifies_every_issue_1691_plugin(self) -> None:
        doc = (
            REPO_ROOT
            / "docs"
            / "wiki"
            / "RedGuides-Awareness-and-Coordination-Parity.md"
        )

        self.assertTrue(doc.exists(), "Awareness parity doc should exist")
        text = doc.read_text(encoding="utf-8")

        self.assertIn("# RedGuides Awareness and Coordination Parity", text)
        self.assertIn(
            "| Plugin | TextQuest mapping | Operator surface | Classification | Owner |",
            text,
        )

        for plugin in (
            "MQ2Status",
            "MQ2Targets",
            "MQ2Spawns",
            "MQ2SpawnSort",
            "MQ2Tracking",
            "MQ2ToolTip",
            "MQ2OTD",
            "MQ2Posse",
            "MQ2WorstHurt",
            "MQ2GroupInfo",
            "MQ2XAssist",
            "MQ2Paranoid",
            "MQ2Say",
        ):
            self.assertIn(f"`{plugin}`", text)

        self.assertIn("Native", text)
        self.assertIn("Adapted", text)
        self.assertIn("Deferred", text)

    def test_parity_doc_records_related_macroquest_core_surfaces_and_owners(self) -> None:
        text = (
            REPO_ROOT
            / "docs"
            / "wiki"
            / "RedGuides-Awareness-and-Coordination-Parity.md"
        ).read_text(encoding="utf-8")

        self.assertIn("`targetinfo`", text)
        self.assertIn("`xtarinfo`", text)
        self.assertIn("map-label", text)
        self.assertIn("`#1691`", text)
        self.assertIn("`#836`", text)
        self.assertIn("`#838`", text)
        self.assertIn("`#851`", text)
        self.assertIn("`#859`", text)

    def test_operator_console_doc_links_to_awareness_parity_reference(self) -> None:
        operator_doc = (
            REPO_ROOT / "docs" / "wiki" / "Web-Dashboard-Operator-Console.md"
        ).read_text(encoding="utf-8")

        self.assertIn("RedGuides-Awareness-and-Coordination-Parity.md", operator_doc)


if __name__ == "__main__":
    unittest.main()
