from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "validate_offsets_sync.py"


def load_module():
    spec = importlib.util.spec_from_file_location("validate_offsets_sync", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load validate_offsets_sync module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class OffsetSyncTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_repo_offsets_match_compiled_constants(self) -> None:
        json_offsets = self.module.load_json_offsets(REPO_ROOT / "config" / "offsets.json")
        compiled_offsets = self.module.load_compiled_offsets(
            REPO_ROOT / "textquest-common" / "src" / "offsets.rs"
        )

        self.assertEqual(self.module.compare_offsets(json_offsets, compiled_offsets), [])

    def test_compare_offsets_reports_mismatch(self) -> None:
        diffs = self.module.compare_offsets(
            {"globals": {"pinstLocalPlayer": 1}},
            {"globals": {"pinstLocalPlayer": 2}},
        )

        self.assertEqual(diffs, ["globals.pinstLocalPlayer: JSON=0x1 compiled=0x2"])

    def test_compiled_offsets_include_module_specific_maps(self) -> None:
        compiled_offsets = self.module.load_compiled_offsets(
            REPO_ROOT / "textquest-common" / "src" / "offsets.rs"
        )

        self.assertEqual(
            compiled_offsets["eqmain_globals"]["cxwndManager"], 0x0001_8038_24B8
        )
        self.assertEqual(
            compiled_offsets["eqmain_functions"]["joinServer"], 0x0001_8001_8050
        )
        self.assertEqual(
            compiled_offsets["eqgraphics_functions"]["realRenderWorld"],
            0x0001_40A0_0100,
        )


if __name__ == "__main__":
    unittest.main()
