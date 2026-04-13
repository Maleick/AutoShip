#!/usr/bin/env python3
"""Validate all zone map files in config/maps/ directory."""

from __future__ import annotations

import sys
from pathlib import Path


def validate_l_line(line: str) -> str | None:
    """Validate L (line) format: L x1, y1, z1, x2, y2, z2, r, g, b"""
    parts = line[1:].strip().split(",")
    if len(parts) != 9:
        return f"L line: expected 9 fields, got {len(parts)}"

    for i, part in enumerate(parts):
        try:
            if i < 6:  # coordinates
                v = float(part.strip())
                if not (-1e6 < v < 1e6):
                    return f"L line: coordinate out of bounds: {v}"
            else:  # color channels
                v = int(part.strip())
                if not (0 <= v <= 255):
                    return f"L line: color {i - 6} out of range: {v}"
        except ValueError:
            return f"L line: field {i} not numeric: {part!r}"
    return None


def validate_p_line(line: str) -> str | None:
    """Validate P (point) format: P x, y, z, r, g, b, size, label"""
    parts = line[1:].strip().split(",", 7)  # max 8 parts (label can contain commas)
    if len(parts) < 8:
        return f"P line: expected >=8 fields, got {len(parts)}"

    for i, part in enumerate(parts[:7]):
        try:
            if i < 3:  # coordinates
                float(part.strip())
            elif i < 6:  # color
                v = int(part.strip())
                if not (0 <= v <= 255):
                    return f"P line: color field out of range: {v}"
            else:  # size
                v = int(part.strip())
                if not (0 < v < 256):
                    return f"P line: size out of range: {v}"
        except ValueError:
            return f"P line: field {i} not numeric: {part!r}"
    return None


def validate_map_file(filepath: Path) -> list[str]:
    """Validate a single map file. Returns list of error strings."""
    errors: list[str] = []
    with open(filepath) as f:
        for num, raw_line in enumerate(f, 1):
            line = raw_line.rstrip()
            if not line or line.startswith("#"):
                continue

            if line.startswith("L "):
                err = validate_l_line(line)
            elif line.startswith("P "):
                err = validate_p_line(line)
            else:
                err = f"Unknown line type: {line[0]!r}"

            if err:
                errors.append(f"Line {num}: {err}")

    return errors


def main() -> int:
    map_dir = Path("config/maps")
    if not map_dir.exists():
        print(f"Map directory not found: {map_dir}")
        return 1

    map_files = sorted(map_dir.glob("*.txt"))
    if not map_files:
        print(f"No .txt map files found in {map_dir}")
        return 0

    all_errors: dict[str, list[str]] = {}
    for map_file in map_files:
        errors = validate_map_file(map_file)
        if errors:
            all_errors[map_file.name] = errors

    if all_errors:
        print("Map validation FAILED")
        for fname, errors in all_errors.items():
            print(f"\n{fname}:")
            for err in errors[:10]:  # show first 10 errors per file
                print(f"  {err}")
            if len(errors) > 10:
                print(f"  ... and {len(errors) - 10} more errors")
        return 1

    print(f"All {len(map_files)} map files validated OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
