from __future__ import annotations

import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
LEDGER = REPO_ROOT / "docs" / "external-research" / "packet-zoning-send-path-and-state-ledger.md"
EXTERNAL_RESEARCH_README = REPO_ROOT / "docs" / "external-research" / "README.md"
WIKI_ROADMAP = REPO_ROOT / "docs" / "wiki" / "Roadmap-and-Known-Gaps.md"
WIKI_ARCHITECTURE = REPO_ROOT / "docs" / "wiki" / "Architecture-Overview.md"
FEATURE_LIST = REPO_ROOT / "feature-list.json"


class PacketControlPathInventoryTests(unittest.TestCase):
    def test_ledger_has_curated_scope_and_sources(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("# Packet and Zoning Send-Path and State Ledger", text)
        self.assertIn("This ledger is the curated packet/zoning intake surface", text)
        self.assertIn("## Source Set", text)
        self.assertIn("docs/implementation-roadmap.md", text)
        self.assertIn("EQ_Network_Architecture.md", text)
        self.assertIn("EQ_Ability_Packet_Structures.md", text)
        self.assertIn("EQ_Zoning_System.md", text)

    def test_status_labels_are_defined(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Status Labels", text)
        self.assertIn("| `In-process` |", text)
        self.assertIn("| `Packet candidate` |", text)
        self.assertIn("| `Blocked` |", text)

    def test_combat_matrix_covers_live_and_missing_paths(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Combat Control Matrix", text)
        self.assertIn("Combat engage and sustained melee loop", text)
        self.assertIn("Direct target and cast IPC pair from the combat coordinator", text)
        self.assertIn("Direct `Attack`, `StopAttack`, `ClearTarget`, `CombatForceAbility`, and `CombatEmergencyHeal` IPC variants", text)
        self.assertIn("Ability packets from imported research", text)

    def test_utility_matrix_covers_navigation_login_and_zoning(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Utility and Movement Matrix", text)
        self.assertIn("Waypoint navigation and camp movement", text)
        self.assertIn("Direct `MoveTo`, `StopMovement`, and `SetHookState` IPC variants", text)
        self.assertIn("Login automation", text)
        self.assertIn("Zone request and transition control", text)

    def test_chat_matrix_covers_modeled_dispatch_gap(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Chat and Social Matrix", text)
        self.assertIn("Generic chat and operator-authored commands", text)
        self.assertIn("`Say`, `Emote`, and `SoulAction` protocol variants", text)
        self.assertIn("Imported emote/say packet path", text)
        self.assertIn("The manager can generate these commands, but the DLL dispatch path does not currently execute them.", text)

    def test_follow_on_task_mapping_covers_issue_family(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Follow-On Task Mapping", text)
        self.assertIn("`#49` Inventory in-process vs packet control paths", text)
        self.assertIn("`#50` Map zone transition states and failure codes", text)
        self.assertIn("`#60` Ability packet coverage and targetability validation matrix", text)

    def test_external_research_index_lists_the_ledger(self) -> None:
        text = EXTERNAL_RESEARCH_README.read_text(encoding="utf-8")
        self.assertIn("Use these files in order:", text)
        self.assertIn("1. `automation-source-ledger.md`", text)
        self.assertIn("2. `packet-zoning-send-path-and-state-ledger.md`", text)

    def test_wiki_pages_reference_the_current_packet_boundary(self) -> None:
        roadmap = WIKI_ROADMAP.read_text(encoding="utf-8")
        architecture = WIKI_ARCHITECTURE.read_text(encoding="utf-8")
        self.assertIn("docs/external-research/packet-zoning-send-path-and-state-ledger.md", roadmap)
        self.assertIn("the current packet inventory keeps combat, utility, and chat packet seams separate", roadmap)
        self.assertIn("authenticated IPC into in-process DLL execution", architecture)
        self.assertIn("packet-zoning-send-path-and-state-ledger.md", architecture)

    def test_feature_list_marks_the_m5_slice_complete(self) -> None:
        payload = json.loads(FEATURE_LIST.read_text(encoding="utf-8"))
        features = [item for item in payload["features"] if item["id"] == "m5-packet-control-path-matrix"]
        self.assertEqual(len(features), 1, "Feature 'm5-packet-control-path-matrix' not found in feature-list.json")
        feature = features[0]
        self.assertEqual(feature["status"], "complete")
        self.assertIn(
            "python3 -m unittest discover -s tests -p 'test_packet_control_path_inventory.py' -v",
            feature["acceptance_tests"],
        )
        self.assertIn(
            "python3 scripts/sync_wiki.py --check",
            feature["acceptance_tests"],
        )

    def test_packet_send_path_notes_keep_transport_claims_bounded(self) -> None:
        text = LEDGER.read_text(encoding="utf-8")
        self.assertIn("## Packet Send-Path Notes From Imports", text)
        self.assertIn("`NetworkSend`", text)
        self.assertIn("`UdpConnection::SendMessage`", text)
        self.assertIn("the repo does not yet implement a general packet sender", text)
        self.assertIn("the imports call out anti-cheat counters", text)


if __name__ == "__main__":
    unittest.main()
