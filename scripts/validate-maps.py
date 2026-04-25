#!/usr/bin/env python3
"""Validate Brewall-style zone map files."""

from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path


DEFAULT_MAP_DIR = Path("config/maps")


@dataclass
class Bounds:
    min_x: float | None = None
    max_x: float | None = None
    min_y: float | None = None
    max_y: float | None = None

    def add(self, x: float, y: float) -> None:
        self.min_x = x if self.min_x is None else min(self.min_x, x)
        self.max_x = x if self.max_x is None else max(self.max_x, x)
        self.min_y = y if self.min_y is None else min(self.min_y, y)
        self.max_y = y if self.max_y is None else max(self.max_y, y)

    def display(self) -> str:
        if self.min_x is None or self.max_x is None or self.min_y is None or self.max_y is None:
            return "n/a"
        return (
            f"x={_format_float(self.min_x)}..{_format_float(self.max_x)}, "
            f"y={_format_float(self.min_y)}..{_format_float(self.max_y)}"
        )


@dataclass
class MapFileResult:
    path: Path
    line_count: int = 0
    record_count: int = 0
    bounds: Bounds = field(default_factory=Bounds)
    errors: list[str] = field(default_factory=list)

    @property
    def is_valid(self) -> bool:
        return not self.errors


def _format_float(value: float) -> str:
    return f"{value:g}"


def _split_fields(line: str) -> list[str]:
    return [part.strip() for part in line[1:].strip().split(",")]


def _parse_finite_float(value: str, field_name: str) -> tuple[float | None, str | None]:
    try:
        parsed = float(value)
    except ValueError:
        return None, f"{field_name} is not a valid float: {value!r}"
    if not math.isfinite(parsed):
        return None, f"{field_name} must be finite: {value!r}"
    return parsed, None


def _parse_u8(value: str, field_name: str) -> tuple[int | None, str | None]:
    try:
        parsed = int(value, 10)
    except ValueError:
        return None, f"{field_name} is not a valid 0-255 integer: {value!r}"
    if not 0 <= parsed <= 255:
        return None, f"{field_name} out of range 0-255: {parsed}"
    return parsed, None


def validate_l_line(line: str, bounds: Bounds | None = None) -> str | None:
    """Validate: L x1, y1, z1, x2, y2, z2, r, g, b."""
    fields = _split_fields(line)
    if len(fields) != 9:
        return f"L line expected 9 fields, got {len(fields)}"

    coordinates: list[float] = []
    for index, field in enumerate(fields[:6], start=1):
        value, error = _parse_finite_float(field, f"L coordinate {index}")
        if error:
            return error
        assert value is not None
        coordinates.append(value)

    for index, field in enumerate(fields[6:], start=1):
        _, error = _parse_u8(field, f"L color {index}")
        if error:
            return error

    if bounds is not None:
        bounds.add(coordinates[0], coordinates[1])
        bounds.add(coordinates[3], coordinates[4])
    return None


def validate_p_line(line: str, bounds: Bounds | None = None) -> str | None:
    """Validate: P x, y, z, r, g, b, size, label."""
    fields = _split_fields(line)
    if len(fields) < 8:
        return f"P line expected at least 8 fields, got {len(fields)}"

    coordinates: list[float] = []
    for index, field in enumerate(fields[:3], start=1):
        value, error = _parse_finite_float(field, f"P coordinate {index}")
        if error:
            return error
        assert value is not None
        coordinates.append(value)

    for index, field in enumerate(fields[3:6], start=1):
        _, error = _parse_u8(field, f"P color {index}")
        if error:
            return error

    _, error = _parse_finite_float(fields[6], "P size")
    if error:
        return error

    label = ",".join(fields[7:]).strip()
    if not label:
        return "P label must not be empty"

    if bounds is not None:
        bounds.add(coordinates[0], coordinates[1])
    return None


def validate_map_file_result(filepath: Path) -> MapFileResult:
    """Validate one map file and return summary metadata plus errors."""
    result = MapFileResult(path=filepath)
    try:
        with filepath.open(encoding="utf-8", errors="replace") as handle:
            for line_number, raw_line in enumerate(handle, start=1):
                result.line_count = line_number
                stripped = raw_line.strip()
                if not stripped or stripped.startswith("#"):
                    continue

                if stripped.startswith("L "):
                    error = validate_l_line(stripped, result.bounds)
                elif stripped.startswith("P "):
                    error = validate_p_line(stripped, result.bounds)
                else:
                    error = f"unknown line type: {stripped[0]!r}"

                if error:
                    result.errors.append(f"line {line_number}: {error}")
                else:
                    result.record_count += 1
    except OSError as exc:
        result.errors.append(f"read error: {exc}")

    return result


def validate_map_file(filepath: Path) -> list[str]:
    """Validate a single map file. Returns line-numbered error strings."""
    return validate_map_file_result(filepath).errors


def validate_map_directory(map_dir: Path) -> tuple[list[MapFileResult], list[str]]:
    if not map_dir.exists():
        return [], [f"map directory not found: {map_dir}"]
    if not map_dir.is_dir():
        return [], [f"map path is not a directory: {map_dir}"]

    map_files = sorted(map_dir.glob("*.txt"))
    if not map_files:
        return [], [f"no .txt map files found in {map_dir}"]

    return [validate_map_file_result(path) for path in map_files], []


def print_report(results: list[MapFileResult], directory_errors: list[str]) -> int:
    if directory_errors:
        for error in directory_errors:
            print(f"✗ Error: {error}")
        print("Map validation summary: valid_files=0, invalid_files=0, invalid_lines=0")
        return 1

    valid_files = 0
    invalid_files = 0
    invalid_lines = 0

    for result in results:
        if result.is_valid:
            valid_files += 1
            print(
                f"✓ Valid: {result.path.name}, line_count={result.line_count}, "
                f"bounds=({result.bounds.display()})"
            )
            continue

        invalid_files += 1
        invalid_lines += len(result.errors)
        for error in result.errors:
            print(f"✗ Error: {result.path.name}, {error}")

    print(
        "Map validation summary: "
        f"valid_files={valid_files}, invalid_files={invalid_files}, invalid_lines={invalid_lines}"
    )
    return 0 if invalid_lines == 0 else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate Brewall-style TextQuest map files.")
    parser.add_argument(
        "map_dir",
        nargs="?",
        type=Path,
        default=DEFAULT_MAP_DIR,
        help="Directory containing zone .txt map files (default: config/maps).",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    results, directory_errors = validate_map_directory(args.map_dir)
    return print_report(results, directory_errors)


if __name__ == "__main__":
    sys.exit(main())
