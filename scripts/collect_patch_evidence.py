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
