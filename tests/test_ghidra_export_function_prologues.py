from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "ghidra_export_function_prologues.py"


def load_module():
    spec = importlib.util.spec_from_file_location(
        "ghidra_export_function_prologues", SCRIPT_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load ghidra_export_function_prologues module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class FakeAddress:
    def __init__(self, offset: int) -> None:
        self.offset = offset

    def getOffset(self) -> int:
        return self.offset


class FakeJavaIterator:
    """Minimal Java-style iterator used by Ghidra APIs (hasNext/next)."""

    def __init__(self, items) -> None:
        self._items = list(items)
        self._index = 0

    def hasNext(self) -> bool:
        return self._index < len(self._items)

    def next(self):
        value = self._items[self._index]
        self._index += 1
        return value


class FakeAddressSet:
    def __init__(self, addresses: list[FakeAddress]) -> None:
        self.addresses = addresses

    def getAddresses(self, _forward: bool):
        return FakeJavaIterator(self.addresses)


class FakeFunction:
    def __init__(self, name: str, offset: int, size: int) -> None:
        self.name = name
        self.entry = FakeAddress(offset)
        # Model Ghidra's AddressSetView: function body exposes per-address iteration.
        self.body = FakeAddressSet([FakeAddress(offset + i) for i in range(size)])
        self.size = size

    def getName(self) -> str:
        return self.name

    def getEntryPoint(self) -> FakeAddress:
        return self.entry

    def getBody(self):
        return self.body


class FakeMemory:
    def __init__(self, values: list[int]) -> None:
        self.values = values

    def getBytes(self, _address, buffer) -> int:
        for index, value in enumerate(self.values[: len(buffer)]):
            buffer[index] = value
        return min(len(self.values), len(buffer))


class FakeBookmarkManager:
    def __init__(self, has_bookmark: bool) -> None:
        self.has_bookmark = has_bookmark

    def getBookmarks(self, _address):
        return [object()] if self.has_bookmark else []


class ExportFunctionPrologueTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_format_byte_string_uses_uppercase_two_digit_hex(self) -> None:
        self.assertEqual(
            self.module.format_byte_string([0, 10, 255, -1, -128]),
            "00 0A FF FF 80",
        )

    def test_default_function_names_are_not_treated_as_labels(self) -> None:
        self.assertTrue(self.module.is_default_function_name("FUN_140001000"))
        self.assertTrue(self.module.is_default_function_name("SUB_140001000"))
        self.assertFalse(self.module.is_default_function_name("CastSpell"))

    def test_export_record_clamps_byte_count_to_function_size(self) -> None:
        function = FakeFunction("CastSpell", 0x140D9F20, 3)
        memory = FakeMemory([0x48, 0x89, 0x5C, 0x24])

        record = self.module.build_export_record(function, memory, 64)

        self.assertEqual(
            record,
            {
                "name": "CastSpell",
                "address": "0x140D9F20",
                "size": 3,
                "bytes": "48 89 5C",
            },
        )

    def test_should_export_labeled_or_bookmarked_functions_only(self) -> None:
        labeled = FakeFunction("CastSpell", 0x140D9F20, 64)
        default = FakeFunction("FUN_140001000", 0x140001000, 64)

        self.assertTrue(
            self.module.should_export_function(labeled, FakeBookmarkManager(False))
        )
        self.assertTrue(
            self.module.should_export_function(default, FakeBookmarkManager(True))
        )
        self.assertFalse(
            self.module.should_export_function(default, FakeBookmarkManager(False))
        )

    def test_parse_script_args_accepts_output_and_clamps_byte_count(self) -> None:
        output_path, byte_count = self.module.parse_script_args(["out.json", "128"])

        self.assertEqual(output_path, "out.json")
        self.assertEqual(byte_count, 64)


if __name__ == "__main__":
    unittest.main()
