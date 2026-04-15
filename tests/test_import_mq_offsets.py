from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "import_mq_offsets.py"


def load_module():
    spec = importlib.util.spec_from_file_location("import_mq_offsets", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load import_mq_offsets module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ParseHeaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_parse_header_extracts_basic_define(self) -> None:
        text = "#define pinstLocalPlayer_x 0x14059F820\n"
        result = self.module.parse_header(text)
        self.assertIn("pinstLocalPlayer", result)
        self.assertEqual(result["pinstLocalPlayer"], 0x14059F820)

    def test_parse_header_strips_x_suffix(self) -> None:
        text = "#define CharacterZoneClient__CastSpell_x 0x1401015d0\n"
        result = self.module.parse_header(text)
        self.assertIn("CharacterZoneClient__CastSpell", result)
        self.assertNotIn("CharacterZoneClient__CastSpell_x", result)

    def test_parse_header_multiple_defines(self) -> None:
        text = (
            "#define pinstLocalPlayer_x 0x14059F820\n"
            "#define pinstTarget_x 0x14059F830\n"
        )
        result = self.module.parse_header(text)
        self.assertEqual(len(result), 2)
        self.assertIn("pinstLocalPlayer", result)
        self.assertIn("pinstTarget", result)

    def test_parse_header_skips_non_x_suffix_defines(self) -> None:
        text = "#define MAX_SPELLS 300\n"
        result = self.module.parse_header(text)
        # MAX_SPELLS has no _x suffix, should not be parsed
        self.assertEqual(result, {})

    def test_parse_header_skips_non_hex_values(self) -> None:
        text = "#define SomeName_x 12345\n"
        result = self.module.parse_header(text)
        # Must be 0x-prefixed hex
        self.assertEqual(result, {})

    def test_parse_header_handles_uppercase_hex(self) -> None:
        text = "#define pinstLocalPlayer_x 0x14059FABCD\n"
        result = self.module.parse_header(text)
        self.assertEqual(result["pinstLocalPlayer"], 0x14059FABCD)

    def test_parse_header_handles_lowercase_hex(self) -> None:
        text = "#define pinstLocalPlayer_x 0x14059fabcd\n"
        result = self.module.parse_header(text)
        self.assertEqual(result["pinstLocalPlayer"], 0x14059FABCD)

    def test_parse_header_empty_input(self) -> None:
        result = self.module.parse_header("")
        self.assertEqual(result, {})

    def test_parse_header_ignores_comments(self) -> None:
        text = (
            "// Comment line\n"
            "/* Block comment */\n"
            "#define pinstLocalPlayer_x 0x14059F820\n"
        )
        result = self.module.parse_header(text)
        self.assertIn("pinstLocalPlayer", result)
        self.assertEqual(len(result), 1)

    def test_parse_header_handles_leading_whitespace(self) -> None:
        text = "    #define pinstLocalPlayer_x 0x14059F820\n"
        result = self.module.parse_header(text)
        self.assertIn("pinstLocalPlayer", result)

    def test_parse_header_handles_mixed_eqlib_content(self) -> None:
        text = (
            "// EverQuest offsets\n"
            "#pragma once\n"
            "#define pinstLocalPlayer_x 0x14059F820\n"
            "#define pinstControlledPlayer_x 0x14059F828\n"
            "#define MAX_SPELLS 300\n"
            "#define SomeFlag 1\n"
            "#define __ClientDate_x 0xDEADBEEF\n"
        )
        result = self.module.parse_header(text)
        self.assertEqual(len(result), 3)
        self.assertIn("pinstLocalPlayer", result)
        self.assertIn("pinstControlledPlayer", result)
        self.assertIn("__ClientDate", result)


class FormatAddrTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_format_addr_typical_eq_address(self) -> None:
        # A 64-bit EQ function address
        result = self.module.format_addr(0x14059F820)
        self.assertTrue(result.startswith("0x"))
        self.assertIn("_", result)

    def test_format_addr_zero(self) -> None:
        result = self.module.format_addr(0)
        self.assertEqual(result, "0x0000_0000_0000")

    def test_format_addr_roundtrip(self) -> None:
        addr = 0x0001_400D_9F20
        formatted = self.module.format_addr(addr)
        # Remove prefix and underscores, parse back
        recovered = int(formatted.replace("0x", "").replace("_", ""), 16)
        self.assertEqual(recovered, addr)

    def test_format_addr_uses_underscores_for_readability(self) -> None:
        result = self.module.format_addr(0x14059F820)
        # Should have underscores grouping 4-char hex segments
        hex_part = result[2:]  # strip "0x"
        groups = hex_part.split("_")
        for g in groups:
            self.assertEqual(len(g), 4, f"Group {g!r} should be 4 chars")

    def test_format_addr_uppercase_hex(self) -> None:
        result = self.module.format_addr(0xABCDEF012345)
        # Should use uppercase hex digits
        hex_part = result[2:].replace("_", "")
        self.assertEqual(hex_part, hex_part.upper())


class UpdateOffsetsRsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def _make_offsets_rs(self, content: str) -> Path:
        """Write a temporary offsets.rs-like file and return its path."""
        tmp = tempfile.NamedTemporaryFile(
            mode="w", suffix=".rs", delete=False, encoding="utf-8"
        )
        tmp.write(content)
        tmp.close()
        return Path(tmp.name)

    def test_update_changes_matching_constant(self) -> None:
        content = "pub const CAST_SPELL: u64 = 0x0001_4010_15D0;\n"
        path = self._make_offsets_rs(content)
        try:
            mapping = {"CharacterZoneClient__CastSpell": "CAST_SPELL"}
            updates = {"CharacterZoneClient__CastSpell": 0x0001_4010_ABCD}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(len(changes), 1)
            new_content = path.read_text(encoding="utf-8")
            self.assertIn("0001_4010_ABCD", new_content)
        finally:
            path.unlink(missing_ok=True)

    def test_update_dry_run_does_not_write(self) -> None:
        original = "pub const CAST_SPELL: u64 = 0x0001_4010_15D0;\n"
        path = self._make_offsets_rs(original)
        try:
            mapping = {"CharacterZoneClient__CastSpell": "CAST_SPELL"}
            updates = {"CharacterZoneClient__CastSpell": 0x0001_4010_ABCD}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=True)
            self.assertEqual(len(changes), 1)
            # File should be unchanged in dry-run mode
            self.assertEqual(path.read_text(encoding="utf-8"), original)
        finally:
            path.unlink(missing_ok=True)

    def test_update_no_change_when_value_matches(self) -> None:
        addr = 0x0001_4010_15D0
        content = f"pub const CAST_SPELL: u64 = {self.module.format_addr(addr)};\n"
        path = self._make_offsets_rs(content)
        try:
            mapping = {"CharacterZoneClient__CastSpell": "CAST_SPELL"}
            updates = {"CharacterZoneClient__CastSpell": addr}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(changes, [])
        finally:
            path.unlink(missing_ok=True)

    def test_update_skips_unmapped_defines(self) -> None:
        content = "pub const CAST_SPELL: u64 = 0x0001_4010_15D0;\n"
        path = self._make_offsets_rs(content)
        try:
            mapping = {"CharacterZoneClient__CastSpell": "CAST_SPELL"}
            # Include a define not in the mapping
            updates = {
                "CharacterZoneClient__CastSpell": 0x0001_4010_ABCD,
                "UnknownDefine": 0x1234_5678,
            }
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            # Only the mapped constant should generate a change
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0][0], "CAST_SPELL")
        finally:
            path.unlink(missing_ok=True)

    def test_update_returns_tq_name_mq_name_old_new(self) -> None:
        original_addr = 0x0001_4010_15D0
        new_addr = 0x0001_4010_ABCD
        content = f"pub const CAST_SPELL: u64 = {self.module.format_addr(original_addr)};\n"
        path = self._make_offsets_rs(content)
        try:
            mapping = {"CharacterZoneClient__CastSpell": "CAST_SPELL"}
            updates = {"CharacterZoneClient__CastSpell": new_addr}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(len(changes), 1)
            tq_name, mq_name, old_val, new_val = changes[0]
            self.assertEqual(tq_name, "CAST_SPELL")
            self.assertEqual(mq_name, "CharacterZoneClient__CastSpell")
            self.assertEqual(old_val, original_addr)
            self.assertEqual(new_val, new_addr)
        finally:
            path.unlink(missing_ok=True)

    def test_update_multiple_constants(self) -> None:
        content = (
            "pub const CAST_SPELL: u64 = 0x0001_4010_15D0;\n"
            "pub const DO_ATTACK: u64 = 0x0001_4024_9530;\n"
        )
        path = self._make_offsets_rs(content)
        try:
            mapping = {
                "CharacterZoneClient__CastSpell": "CAST_SPELL",
                "PlayerZoneClient__DoAttack": "DO_ATTACK",
            }
            updates = {
                "CharacterZoneClient__CastSpell": 0x0001_4010_ABCD,
                "PlayerZoneClient__DoAttack": 0x0001_4024_EFEF,
            }
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(len(changes), 2)
            names = {c[0] for c in changes}
            self.assertIn("CAST_SPELL", names)
            self.assertIn("DO_ATTACK", names)
        finally:
            path.unlink(missing_ok=True)

    def test_update_skips_version_date_stamps(self) -> None:
        content = "pub const __CLIENT_DATE: u64 = 0x0000_0000_0000;\n"
        path = self._make_offsets_rs(content)
        try:
            # __ClientDate is in the EQGAME_MAPPING but the update function
            # explicitly skips version date stamps
            mapping = {"__ClientDate": "__CLIENT_DATE"}
            updates = {"__ClientDate": 0xDEADBEEF}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(changes, [])
        finally:
            path.unlink(missing_ok=True)

    def test_update_module_level_constant(self) -> None:
        content = (
            "pub mod zone_info {\n"
            "    pub const INST_EQ_ZONE_INFO: u64 = 0x0001_4059_F820;\n"
            "}\n"
        )
        path = self._make_offsets_rs(content)
        try:
            mapping = {"instEQZoneInfo": "zone_info::INST_EQ_ZONE_INFO"}
            updates = {"instEQZoneInfo": 0x0001_4059_FFFF}
            changes = self.module.update_offsets_rs(path, updates, mapping, dry_run=False)
            self.assertEqual(len(changes), 1)
            new_text = path.read_text(encoding="utf-8")
            self.assertIn("4059_FFFF", new_text)
        finally:
            path.unlink(missing_ok=True)


class MappingIntegrityTests(unittest.TestCase):
    """Verify that the built-in EQGAME_MAPPING and EQMAIN_MAPPING are well-formed."""

    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_eqgame_mapping_is_non_empty(self) -> None:
        self.assertGreater(len(self.module.EQGAME_MAPPING), 0)

    def test_eqmain_mapping_is_non_empty(self) -> None:
        self.assertGreater(len(self.module.EQMAIN_MAPPING), 0)

    def test_eqgame_mapping_values_are_strings(self) -> None:
        for key, value in self.module.EQGAME_MAPPING.items():
            self.assertIsInstance(value, str, f"Value for {key!r} should be a string")

    def test_eqgame_mapping_no_duplicate_values(self) -> None:
        values = list(self.module.EQGAME_MAPPING.values())
        self.assertEqual(len(values), len(set(values)), "Duplicate TQ constant names found")

    def test_eqmain_mapping_keys_have_eqmain_prefix(self) -> None:
        for key in self.module.EQMAIN_MAPPING:
            self.assertTrue(key.startswith("EQMain__"), f"Key {key!r} should start with EQMain__")


if __name__ == "__main__":
    unittest.main()
