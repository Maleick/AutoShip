from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "export_ghidra_patterns.py"


def load_module():
    spec = importlib.util.spec_from_file_location("export_ghidra_patterns", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load export_ghidra_patterns module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ExportGhidraPatternsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_ida_pattern_formats_bytes_and_wildcards(self) -> None:
        pattern = self.module.ida_pattern(
            [0x48, 0x8B, 0x05, 0xAA, 0xBB, 0xCC, 0xDD, 0x48],
            wildcard_ranges=[(3, 4)],
        )
        self.assertEqual(pattern, "48 8B 05 ?? ?? ?? ?? 48")

    def test_build_scan_entry_uses_rip_relative_for_global_symbols(self) -> None:
        symbol = {
            "name": "pinstLocalPlayer",
            "address": "0x140123456",
            "bytes": "48 8B 05 11 22 33 44 48 85 C0",
            "category": "Global",
            "resolve": {"RipRelative": {"disp_offset": 3}},
            "wildcards": [[3, 4]],
        }

        entry = self.module.build_scan_entry(symbol, module="EqGame")

        self.assertEqual(entry["name"], "pinstLocalPlayer")
        self.assertEqual(entry["module"], "EqGame")
        self.assertEqual(entry["category"], "Global")
        self.assertEqual(entry["pattern"], "48 8B 05 ?? ?? ?? ?? 48 85 C0")
        self.assertEqual(entry["resolve"], {"RipRelative": {"disp_offset": 3}})
        self.assertEqual(entry["expected_preferred"], 0x140123456)

    def test_export_patterns_filters_symbols_without_bytes(self) -> None:
        symbols = [
            {
                "name": "ProcessGameEvents",
                "address": 0x14028E0F0,
                "bytes": [0x48, 0x89, 0x5C, 0x24, 0x08],
                "category": "Function",
                "resolve": "Direct",
            },
            {
                "name": "NoBytesYet",
                "address": 0x140000000,
                "category": "Function",
                "resolve": "Direct",
            },
        ]

        exported = self.module.export_patterns(symbols, module="EqGame")

        self.assertEqual(len(exported), 1)
        self.assertEqual(exported[0]["name"], "ProcessGameEvents")
        self.assertEqual(exported[0]["pattern"], "48 89 5C 24 08")

    def test_parse_symbols_accepts_list_and_object_wrapper(self) -> None:
        direct = self.module.parse_symbols(json.dumps([{"name": "a"}]))
        wrapped = self.module.parse_symbols(json.dumps({"symbols": [{"name": "b"}]}))

        self.assertEqual(direct, [{"name": "a"}])
        self.assertEqual(wrapped, [{"name": "b"}])


if __name__ == "__main__":
    unittest.main()
