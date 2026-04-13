#!/usr/bin/env python3

"""Validate all zone map files in config/maps/ directory."""
import sys
from pathlib import Path


def validate_l_line(line_num, line):
    """Validate L (line) format: L x1, y1, z1, x2, y2, z2, r, g, b"""
    parts = line[1:].strip().split(',')
    if len(parts) != 9:
        return f"L line: expected 9 fields, got {len(parts)}"

    for i, part in enumerate(parts):
        try:
            if i < 6:  # coordinates
                v = float(part.strip())
                if not (-1e6 < v < 1e6):  # sanity check
                    return f"L line: coordinate out of bounds: {v}"
            else:  # color channels
                v = int(part.strip())
                if not (0 <= v <= 255):
                    return f"L line: color {i-6} out of range: {v}"
        except ValueError:
            return f"L line: field {i} not numeric: {part}"
    return None


def validate_p_line(line_num, line):
    """Validate P (point) format: P x, y, z, r, g, b, size, label"""
    parts = line[1:].strip().split(',', 7)  # max 8 parts (label can have commas)
    if len(parts) < 8:
        return f"P line: expected >=8 fields, got {len(parts)}"

    for i, part in enumerate(parts[:7]):
        try:
            if i < 3:  # coordinates
                v = float(part.strip())
            elif i < 6:  # color
                v = int(part.strip())
                if not (0 <= v <= 255):
                    return f"P line: color field out of range: {v}"
            else:  # size
                v = int(part.strip())
                if not (0 < v < 256):
                    return f"P line: size out of range: {v}"
        except ValueError:
            return f"P line: field {i} not numeric: {part}"
    return None


def validate_map_file(filepath):
    """Validate single map file."""
    errors = []
    try:
        with open(filepath) as f:
            for num, line in enumerate(f, 1):
                line = line.rstrip()
                if not line or line.startswith('#'):
                    continue

                if line.startswith('L '):
                    err = validate_l_line(num, line)
                elif line.startswith('P '):
                    err = validate_p_line(num, line)
                else:
                    err = f"Unknown line type: {line[0]}"

                if err:
                    errors.append(f"Line {num}: {err}")
    except IOError as e:
        return [f"File error: {e}"]

    return errors


def main():
    """Validate all map files in config/maps/."""
    map_dir = Path("config/maps")

    if not map_dir.exists():
        print("❌ Map validation FAILED: config/maps/ directory not found")
        return 1

    all_errors = {}

    for map_file in sorted(map_dir.glob("*.txt")):
        errors = validate_map_file(map_file)
        if errors:
            all_errors[map_file.name] = errors

    if all_errors:
        print("❌ Map validation FAILED")
        for fname, errors in all_errors.items():
            print(f"\n{fname}:")
            for err in errors[:10]:  # show first 10 errors per file
                print(f"  {err}")
        return 1
    else:
        map_count = len(list(map_dir.glob('*.txt')))
        print(f"✅ All {map_count} map files validated")
        return 0


if __name__ == "__main__":
    sys.exit(main())
