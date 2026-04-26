#!/usr/bin/env python3
"""Collect reproducible patch-day evidence from repo-local Ghidra exports.

This script reads the sibling TextQuest-Ghidra repo, summarizes the available
live/test exports, computes stable hashes for the symbol-oriented JSON files,
and optionally copies those symbol files into a destination directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path
from typing import Any


SYMBOL_FILES = [
    "metadata.json",
    "metadata_raw.json",
    "functions.json",
    "classes.json",
    "namespaces.json",
    "imports.json",
    "exports.json",
    "decompiled_index.json",
]

COUNTABLE_JSON_FILES = {
    "functions.json",
    "classes.json",
    "namespaces.json",
    "imports.json",
    "exports.json",
    "decompiled_index.json",
}

DETECTION_FUNCTION_TARGETS = [
    {
        "id": "system_fingerprint",
        "display_name": "SystemFingerprint",
        "name_patterns": {"systemfingerprint", "system_fingerprint", "fun_140594840"},
        "address_patterns": {"0x140594840"},
    },
    {
        "id": "cheater_ld_flag",
        "display_name": "CheaterLdFlag",
        "kind": "string",
        "string_patterns": {"cheaterldflag"},
    },
    {
        "id": "vm_detection_enum_firmware_tables",
        "display_name": "GetSystemFirmwareTable",
        "kind": "import",
        "name_patterns": {"getsystemfirmwaretable"},
    },
    {
        "id": "vm_detection_query_firmware_tables",
        "display_name": "EnumSystemFirmwareTables",
        "kind": "import",
        "name_patterns": {"enumsystemfirmwaretables"},
    },
    {
        "id": "module_enum_create_snapshot",
        "display_name": "CreateToolhelp32Snapshot",
        "kind": "function_or_import",
        "name_patterns": {"createtoolhelp32snapshot"},
    },
    {
        "id": "module_enum_k32_enum_processes",
        "display_name": "K32EnumProcesses",
        "kind": "function_or_import",
        "name_patterns": {"k32enumprocesses"},
    },
    {
        "id": "module_enum_k32_enum_modules",
        "display_name": "K32EnumProcessModules",
        "kind": "function_or_import",
        "name_patterns": {"k32enumprocessmodules"},
    },
    {
        "id": "module_enum_open_process",
        "display_name": "OpenProcess",
        "kind": "function_or_import",
        "name_patterns": {"openprocess"},
    },
    {
        "id": "module_enum_query_full_image_name",
        "display_name": "QueryFullProcessImageNameA",
        "kind": "function_or_import",
        "name_patterns": {"queryfullprocessimagenamea"},
    },
]


def _normalize_address(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    value = value.strip().lower()
    if value.startswith("0x"):
        value = value[2:]
    try:
        int(value, 16)
    except ValueError:
        return None
    return f"0x{value}"


def _load_json_if_exists(path: Path) -> Any | None:
    if not path.exists():
        return None
    try:
        return load_json(path)
    except json.JSONDecodeError:
        return None


def _string_match(text: Any, patterns: set[str]) -> bool:
    if not isinstance(text, str):
        return False
    lowered = text.lower()
    return any(pattern in lowered for pattern in patterns)


def _fingerprint_payload(payload: Any) -> str:
    return hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def _find_entry_by_patterns(
    items: Any,
    name_patterns: set[str] | None = None,
    address_patterns: set[str] | None = None,
) -> dict[str, Any] | None:
    if not isinstance(items, list):
        return None
    for item in items:
        if not isinstance(item, dict):
            continue

        name = item.get("name")
        if name_patterns and isinstance(name, str):
            if name.lower() in name_patterns:
                return item

        address = _normalize_address(item.get("address"))
        if address_patterns and address and address in address_patterns:
            return item
    return None


def _collect_function_signature(item: dict[str, Any], export_dir: Path) -> dict[str, Any]:
    signature_payload: dict[str, Any] = {
        "name": item.get("name"),
        "address": item.get("address"),
    }
    signature: dict[str, Any] = {
        "source": "functions.json",
        "name": signature_payload["name"],
        "address": signature_payload["address"],
        "decompiled_file": item.get("file"),
    }
    if "file" in item and isinstance(item["file"], str):
        decomp_root = (export_dir / "decompiled").resolve()
        decomp_path = (decomp_root / item["file"]).resolve()
        is_within_decomp_root = decomp_path == decomp_root or decomp_root in decomp_path.parents
        if is_within_decomp_root:
            signature["decompiled_file"] = str(decomp_path)
        if is_within_decomp_root and decomp_path.is_file():
            signature_payload["decompiled_sha256"] = sha256_file(decomp_path)
            with decomp_path.open("r", encoding="utf-8", errors="ignore") as handle:
                signature["decompiled_line_count"] = len(handle.readlines())
    signature["fingerprint"] = _fingerprint_payload(signature_payload)
    return signature


def _collect_import_signature(item: dict[str, Any], name: str) -> dict[str, Any]:
    return {
        "source": "imports.json",
        "name": name,
        "address": item.get("address"),
        "fingerprint": _fingerprint_payload(item),
    }


def _collect_string_signature(entries: list[dict[str, Any]], patterns: set[str]) -> dict[str, Any]:
    normalized = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        value = entry.get("value")
        if _string_match(value, patterns):
            normalized.append(
                {
                    "address": entry.get("address"),
                    "value": entry.get("value"),
                }
            )
    normalized.sort(key=lambda item: (str(item.get("address")), str(item.get("value"))))
    return {
        "source": "strings.json",
        "matches": normalized,
        "fingerprint": _fingerprint_payload(
            [entry["address"] for entry in normalized]
            + [entry["value"] for entry in normalized],
        ),
    }


def collect_detection_signatures(export_dir: Path) -> dict[str, Any]:
    functions = _load_json_if_exists(export_dir / "functions.json") or []
    decompiled = _load_json_if_exists(export_dir / "decompiled_index.json") or []
    imports = _load_json_if_exists(export_dir / "imports.json") or []
    strings = _load_json_if_exists(export_dir / "strings.json") or []

    signatures: dict[str, Any] = {}
    for target in DETECTION_FUNCTION_TARGETS:
        target_id = target["id"]
        name_patterns = set(
            entry.lower()
            for entry in target.get("name_patterns", [])
        )
        address_patterns = set(target.get("address_patterns", []))
        kind = target.get("kind", "function")

        found_entry: dict[str, Any] | None = None
        signature_payload: dict[str, Any] | None = None

        if kind in {"function", "function_or_import"}:
            found_entry = _find_entry_by_patterns(
                decompiled, name_patterns=name_patterns, address_patterns=address_patterns
            )
            if found_entry is None:
                found_entry = _find_entry_by_patterns(
                    functions, name_patterns=name_patterns, address_patterns=address_patterns
                )
            if found_entry is not None:
                signature_payload = _collect_function_signature(found_entry, export_dir)
        if kind in {"import", "function_or_import"} and signature_payload is None:
            if isinstance(imports, list):
                for item in imports:
                    if not isinstance(item, dict):
                        continue
                    name = item.get("name")
                    if (
                        isinstance(name, str)
                        and name_patterns
                        and name.lower() in name_patterns
                    ):
                        signature_payload = _collect_import_signature(item, name)
                        break

        if kind == "string" and signature_payload is None:
            if isinstance(strings, list):
                patterns = set(target.get("string_patterns", []))
                candidate = _collect_string_signature(strings, patterns)
                if candidate["matches"]:
                    signature_payload = candidate

        if signature_payload is not None:
            signatures[target_id] = {
                "found": True,
                "target": target_id,
                "display_name": target["display_name"],
                "kind": kind,
                "signature": signature_payload,
            }
        else:
            signatures[target_id] = {
                "found": False,
                "target": target_id,
                "display_name": target["display_name"],
                "kind": kind,
                "signature": {},
            }

    return signatures


def compare_detection_signatures(
    live: dict[str, Any],
    test: dict[str, Any],
) -> dict[str, Any]:
    comparison: dict[str, Any] = {
        "targets": {},
        "changed_targets": [],
        "only_in_live": [],
        "only_in_test": [],
        "same": [],
    }

    for target_id in sorted(set(live) | set(test)):
        live_item = live.get(target_id, {"found": False, "signature": {}})
        test_item = test.get(target_id, {"found": False, "signature": {}})
        live_found = bool(live_item.get("found"))
        test_found = bool(test_item.get("found"))

        if live_found and test_found:
            changed = (
                live_item.get("signature", {}).get("fingerprint")
                != test_item.get("signature", {}).get("fingerprint")
            )
            comparison["targets"][target_id] = {
                "status": "changed" if changed else "unchanged",
                "same": not changed,
                "live": live_item,
                "test": test_item,
            }
            if changed:
                comparison["changed_targets"].append(target_id)
            else:
                comparison["same"].append(target_id)
            continue

        if live_found:
            comparison["only_in_live"].append(target_id)
            comparison["targets"][target_id] = {
                "status": "missing_in_test",
                "same": False,
                "live": live_item,
                "test": test_item,
            }
            continue

        if test_found:
            comparison["only_in_test"].append(target_id)
            comparison["targets"][target_id] = {
                "status": "missing_in_live",
                "same": False,
                "live": live_item,
                "test": test_item,
            }
            continue

    comparison["blocked"] = len(comparison["changed_targets"]) > 0
    return comparison


def compare_detection_gate(anti_cheat_comparison: dict[str, Any]) -> dict[str, Any]:
    return {
        "blocked": anti_cheat_comparison.get("blocked", False),
        "changed_count": len(anti_cheat_comparison.get("changed_targets", [])),
        "changed_targets": anti_cheat_comparison.get("changed_targets", []),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--ghidra-root",
        type=Path,
        default=Path(__file__).resolve().parents[2] / "TextQuest-Ghidra",
        help="Path to the TextQuest-Ghidra repo (default: sibling repo).",
    )
    parser.add_argument(
        "--variants",
        nargs="+",
        default=["live", "test"],
        help="Variants to inspect (default: live test).",
    )
    parser.add_argument(
        "--modules",
        nargs="+",
        default=["eqgame", "eqmain", "eqgraphics"],
        help="Modules to inspect (default: eqgame eqmain eqgraphics).",
    )
    parser.add_argument(
        "--manifest-out",
        type=Path,
        required=True,
        help="Where to write the JSON evidence manifest.",
    )
    parser.add_argument(
        "--copy-symbols",
        type=Path,
        help="Optional output directory for copied symbol JSON files.",
    )
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def count_entries(path: Path) -> int | None:
    if path.name not in COUNTABLE_JSON_FILES:
        return None
    payload = load_json(path)
    if isinstance(payload, list):
        return len(payload)
    if isinstance(payload, dict):
        return len(payload)
    return None


def collect_file_info(path: Path) -> dict[str, Any]:
    info: dict[str, Any] = {
        "exists": path.exists(),
        "path": str(path),
    }
    if not path.exists():
        return info

    info["size_bytes"] = path.stat().st_size
    info["sha256"] = sha256_file(path)
    count = count_entries(path)
    if count is not None:
        info["count"] = count
    return info


def collect_module_summary(ghidra_root: Path, variant: str, module: str) -> dict[str, Any]:
    module_root = ghidra_root / variant / module
    export_dir = module_root / "ghidra-export"
    summary: dict[str, Any] = {
        "exists": export_dir.exists(),
        "module_root": str(module_root),
        "export_dir": str(export_dir),
    }
    if not export_dir.exists():
        return summary

    json_files = sorted(p.name for p in export_dir.glob("*.json"))
    summary["json_file_count"] = len(json_files)
    summary["json_files"] = json_files

    metadata_path = export_dir / "metadata.json"
    if metadata_path.exists():
        metadata = load_json(metadata_path)
        count_map = metadata.get("counts", {}) if isinstance(metadata.get("counts"), dict) else {}
        summary["metadata"] = {
            "program_name": metadata.get("program_name"),
            "image_base": metadata.get("image_base"),
            "memory_size": metadata.get("memory_size"),
            "function_count": metadata.get("function_count", count_map.get("functions")),
            "symbol_count": metadata.get("symbol_count"),
            "ghidra_version": metadata.get("ghidra_version"),
        }

    files: dict[str, Any] = {}
    for name in SYMBOL_FILES:
        files[name] = collect_file_info(export_dir / name)
    summary["files"] = files
    summary["detection_signatures"] = collect_detection_signatures(export_dir)

    harvest_info_path = ghidra_root / variant / "harvest-info.json"
    if harvest_info_path.exists():
        summary["harvest_info"] = load_json(harvest_info_path)

    return summary


def compare_modules(live: dict[str, Any], test: dict[str, Any]) -> dict[str, Any]:
    comparison: dict[str, Any] = {}
    live_files = live.get("files", {})
    test_files = test.get("files", {})
    all_names = sorted(set(live_files) | set(test_files))

    changed = []
    only_live = []
    only_test = []
    for name in all_names:
        live_exists = live_files.get(name, {}).get("exists", False)
        test_exists = test_files.get(name, {}).get("exists", False)
        if live_exists and test_exists:
            if live_files[name].get("sha256") != test_files[name].get("sha256"):
                changed.append(name)
        elif live_exists:
            only_live.append(name)
        elif test_exists:
            only_test.append(name)

    comparison["changed_files"] = changed
    comparison["only_in_live"] = only_live
    comparison["only_in_test"] = only_test

    comparison["metadata"] = {}
    live_meta = live.get("metadata", {})
    test_meta = test.get("metadata", {})
    for key in sorted(set(live_meta) | set(test_meta)):
        comparison["metadata"][key] = {
            "live": live_meta.get(key),
            "test": test_meta.get(key),
            "same": live_meta.get(key) == test_meta.get(key),
        }

    live_signatures = live.get("detection_signatures", {})
    test_signatures = test.get("detection_signatures", {})
    anti_cheat_comparison = compare_detection_signatures(
        live_signatures, test_signatures
    )
    comparison["anti_cheat_diffs"] = anti_cheat_comparison
    comparison["detection_change_gate"] = compare_detection_gate(anti_cheat_comparison)

    return comparison


def copy_symbol_files(summary: dict[str, Any], destination: Path) -> None:
    export_dir = Path(summary["export_dir"])
    destination.mkdir(parents=True, exist_ok=True)
    for name in SYMBOL_FILES:
        source = export_dir / name
        if source.exists():
            shutil.copy2(source, destination / name)


def main() -> int:
    args = parse_args()

    manifest: dict[str, Any] = {
        "ghidra_root": str(args.ghidra_root),
        "variants": {},
        "comparisons": {},
    }

    for variant in args.variants:
        variant_summary: dict[str, Any] = {}
        for module in args.modules:
            module_summary = collect_module_summary(args.ghidra_root, variant, module)
            variant_summary[module] = module_summary
            if args.copy_symbols and module_summary.get("exists"):
                copy_symbol_files(
                    module_summary,
                    args.copy_symbols / variant / module,
                )
        manifest["variants"][variant] = variant_summary

    if "live" in manifest["variants"] and "test" in manifest["variants"]:
        for module in args.modules:
            live_summary = manifest["variants"]["live"].get(module, {})
            test_summary = manifest["variants"]["test"].get(module, {})
            if live_summary.get("exists") and test_summary.get("exists"):
                manifest["comparisons"][module] = compare_modules(live_summary, test_summary)

    args.manifest_out.parent.mkdir(parents=True, exist_ok=True)
    args.manifest_out.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    print(f"Wrote manifest: {args.manifest_out}")
    if args.copy_symbols:
        print(f"Copied symbol files to: {args.copy_symbols}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
