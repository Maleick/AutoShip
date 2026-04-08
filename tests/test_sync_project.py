from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import sys
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "sync_project.py"


def load_module():
    spec = importlib.util.spec_from_file_location("sync_project", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load sync_project module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class SyncProjectTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def make_item(self, **overrides):
        base = {
            "title": "Audit surface",
            "milestone": "M5",
            "domain": "Packet",
            "evidence_state": "Research-backed",
            "source_doc": "docs/wiki/Roadmap-and-Known-Gaps.md",
            "citation": "docs/wiki/Roadmap-and-Known-Gaps.md#packet",
            "repo_fit": "Fits the repo's existing packet ledger and verifier model.",
            "slice_task": "Add a focused regression test.",
            "checkpoint_batch": "batch-1",
            "target_window": "nightly",
            "effort": "small",
            "priority": "P2",
            "item_type": "task",
        }
        base.update(overrides)
        return self.module.CheckpointItem(raw={}, **base)

    def test_load_state_accepts_list_and_flat_dict_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)
            list_path = tmpdir / "list.json"
            flat_path = tmpdir / "flat.json"

            list_path.write_text(
                json.dumps([
                    {
                        "title": "List item",
                        "milestone": "M5",
                        "domain": "Packet",
                    }
                ]),
                encoding="utf-8",
            )
            flat_path.write_text(
                json.dumps(
                    {
                        "title": "Flat item",
                        "milestone": "M6",
                        "domain": "Zoning",
                    }
                ),
                encoding="utf-8",
            )

            list_items = self.module.load_state(list_path)
            flat_items = self.module.load_state(flat_path)

        self.assertEqual([item.title for item in list_items], ["List item"])
        self.assertEqual([item.milestone for item in flat_items], ["M6"])

    def test_is_mature_requires_citation_repo_fit_and_slice(self) -> None:
        item = self.make_item()
        mature, reason = self.module.is_mature(item)
        self.assertTrue(mature)
        self.assertEqual(reason, "")

        cases = [
            ("citation", "", "no citation"),
            ("repo_fit", "", "no repo-fit rationale"),
            ("slice_task", "", "no concrete slice/validation task"),
        ]
        for field, value, expected_reason in cases:
            with self.subTest(field=field):
                tweaked = self.make_item(**{field: value})
                mature, reason = self.module.is_mature(tweaked)
                self.assertFalse(mature)
                self.assertEqual(reason, expected_reason)

        weak = self.make_item(evidence_state="Provisional")
        mature, reason = self.module.is_mature(weak)
        self.assertFalse(mature)
        self.assertIn("minimum is 'Research-backed'", reason)

    def test_main_aborts_when_verifier_fails(self) -> None:
        with mock.patch.object(self.module, "run_verifier", return_value=(False, "verifier failed")) as run_verifier:
            with mock.patch.object(self.module, "load_state") as load_state:
                with mock.patch.object(self.module.sys, "stderr", new=__import__("io").StringIO()):
                    code = self.module.main(["--state", "state.json", "--roadmap", "roadmap.md"])

        self.assertEqual(code, 1)
        run_verifier.assert_called_once()
        load_state.assert_not_called()

    def test_main_returns_partial_failure_exit_code(self) -> None:
        item = self.make_item()
        actions = [
            self.module.SyncAction(
                title=item.title,
                action="error",
                reason="gh issue create failed",
                error="boom",
            )
        ]

        with mock.patch.object(self.module, "run_verifier", return_value=(True, "ok")):
            with mock.patch.object(self.module, "load_state", return_value=[item]):
                with mock.patch.object(self.module, "sync_items", return_value=actions):
                    with mock.patch.object(self.module.sys, "stdout", new=__import__("io").StringIO()):
                        code = self.module.main(["--state", "state.json", "--roadmap", "roadmap.md"])

        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
