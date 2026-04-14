#!/usr/bin/env python3
"""Validate config/offsets.json against compiled constants in offsets.rs."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
JSON_PATH = REPO_ROOT / "config" / "offsets.json"
OFFSETS_RS_PATH = REPO_ROOT / "textquest-common" / "src" / "offsets.rs"

TOP_LEVEL_FIELDS = [
    ("client_date", "CLIENT_DATE"),
    ("eq_preferred_base", "EQ_PREFERRED_BASE"),
]

GLOBAL_FIELDS = [
    ("pinstLocalPlayer", "PINST_LOCAL_PLAYER"),
    ("pinstControlledPlayer", "PINST_CONTROLLED_PLAYER"),
    ("pinstTarget", "PINST_TARGET"),
    ("pinstSpawnManager", "PINST_SPAWN_MANAGER"),
    ("pinstLocalPC", "PINST_LOCAL_PC"),
    ("pinstSpellManager", "PINST_SPELL_MANAGER"),
    ("pinstCDisplay", "PINST_CDISPLAY"),
    ("pinstCEverQuest", "PINST_CEVERQUEST"),
]

PLAYER_BASE_FIELDS = [
    ("next", "NEXT"),
    ("prev", "PREV"),
    ("y", "Y"),
    ("x", "X"),
    ("z", "Z"),
    ("heading", "HEADING"),
    ("speedCurrent", "SPEED_CURRENT"),
    ("speedRun", "SPEED_RUN"),
    ("speedHeading", "SPEED_HEADING"),
    ("name", "NAME"),
    ("displayedName", "DISPLAYED_NAME"),
    ("type", "TYPE"),
    ("spawnId", "SPAWN_ID"),
    ("lastName", "LASTNAME"),
]

PLAYER_ZONE_FIELDS = [
    ("hpMax", "HP_MAX"),
    ("hpCurrent", "HP_CURRENT"),
    ("manaMax", "MANA_MAX"),
    ("manaCurrent", "MANA_CURRENT"),
    ("level", "LEVEL"),
    ("charClass", "CHAR_CLASS"),
    ("enduranceCurrent", "ENDURANCE_CURRENT"),
    ("enduranceMax", "ENDURANCE_MAX"),
    ("standState", "STANDSTATE"),
]

SPAWN_MANAGER_FIELDS = [("playerList", "PLAYER_LIST")]


def parse_rust_literal(raw: str) -> object:
    raw = raw.strip()
    if raw.startswith('"'):
        return json.loads(raw)
    if raw in {"true", "false"}:
        return raw == "true"
    return int(raw.replace("_", ""), 0)


def extract_const(source: str, name: str) -> object:
    pattern = re.compile(rf"^[ \t]*pub const {re.escape(name)}: [^=]+ = (?P<value>[^;]+);$", re.M)
    match = pattern.search(source)
    if not match:
        raise RuntimeError(f"missing constant {name} in offsets.rs")
    return parse_rust_literal(match.group("value"))


def extract_module_body(source: str, module_name: str) -> str:
    marker = f"pub mod {module_name} {{"
    start = source.find(marker)
    if start < 0:
        raise RuntimeError(f"missing module {module_name} in offsets.rs")

    depth = 1
    index = start + len(marker)
    while index < len(source):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start + len(marker) : index]
        index += 1

    raise RuntimeError(f"unterminated module {module_name} in offsets.rs")


def extract_module_const(source: str, module_name: str, name: str) -> object:
    module_body = extract_module_body(source, module_name)
    return extract_const(module_body, name)


def load_json_offsets(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_compiled_offsets(path: Path) -> dict[str, object]:
    source = path.read_text(encoding="utf-8")
    return {
        "client_date": extract_const(source, "CLIENT_DATE"),
        "eq_preferred_base": extract_const(source, "EQ_PREFERRED_BASE"),
        "globals": {
            json_name: extract_const(source, rust_name)
            for json_name, rust_name in GLOBAL_FIELDS
        },
        "player_base": {
            json_name: extract_module_const(source, "player_base", rust_name)
            for json_name, rust_name in PLAYER_BASE_FIELDS
        },
        "player_zone": {
            json_name: extract_module_const(source, "player_zone", rust_name)
            for json_name, rust_name in PLAYER_ZONE_FIELDS
        },
        "spawn_manager": {
            json_name: extract_module_const(source, "spawn_manager", rust_name)
            for json_name, rust_name in SPAWN_MANAGER_FIELDS
        },
    }


def format_value(value: object) -> str:
    if isinstance(value, int):
        return f"0x{value:x}"
    return repr(value)


def compare_offsets(json_value: object, compiled_value: object, path: str = "") -> list[str]:
    diffs: list[str] = []
    if isinstance(json_value, dict) and isinstance(compiled_value, dict):
        json_keys = set(json_value)
        compiled_keys = set(compiled_value)
        for key in sorted(json_keys - compiled_keys):
            full_path = f"{path}.{key}" if path else key
            diffs.append(
                f"{full_path}: present only in JSON ({format_value(json_value[key])})"
            )
        for key in sorted(compiled_keys - json_keys):
            full_path = f"{path}.{key}" if path else key
            diffs.append(
                f"{full_path}: present only in compiled constants ({format_value(compiled_value[key])})"
            )
        for key in sorted(json_keys & compiled_keys):
            full_path = f"{path}.{key}" if path else key
            diffs.extend(compare_offsets(json_value[key], compiled_value[key], full_path))
        return diffs

    if json_value != compiled_value:
        diffs.append(
            f"{path}: JSON={format_value(json_value)} compiled={format_value(compiled_value)}"
        )
    return diffs


def main() -> int:
    if not JSON_PATH.exists():
        print(f"Offset snapshot not found: {JSON_PATH}")
        return 0
    if not OFFSETS_RS_PATH.exists():
        print(f"Compiled offsets source not found: {OFFSETS_RS_PATH}")
        return 1

    json_offsets = load_json_offsets(JSON_PATH)
    compiled_offsets = load_compiled_offsets(OFFSETS_RS_PATH)
    diffs = compare_offsets(json_offsets, compiled_offsets)

    if diffs:
        print("Offset sync FAILED")
        for diff in diffs:
            print(f"- {diff}")
        return 1

    print(f"Offset sync OK: {JSON_PATH} matches {OFFSETS_RS_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
