#!/usr/bin/env python3
"""Validate camp coordinates against zone map bounds."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import argparse
import math
import sys
import tomllib
from typing import Final, Sequence


TARGET_CAMPS: Final[tuple[str, ...]] = (
    "crescent_reach_newbie",
    "crescent_reach_undead",
    "crushbone_entrance",
    "crushbone_throne",
    "lguk_live_side",
    "lguk_dead_side",
    "mistmoore_entrance",
    "mistmoore_castle",
    "sebilis_disco",
    "unrest_basement",
    "unrest_yard",
)


@dataclass
class CampConfig:
    name: str
    zone: str
    camp_center: tuple[float, float, float]
    pull_point: tuple[float, float, float]
    pull_radius: float


@dataclass
class Bounds:
    min_x: float
    max_x: float
    min_y: float
    max_y: float


@dataclass
class CampResult:
    camp_name: str
    zone: str
    map_file: Path | None
    bounds: Bounds | None
    center_ok: bool
    center_distance: float
    pull_ok: bool
    pull_distance: float
    pull_circle_ok: bool
    errors: list[str]
    warnings: list[str]


def parse_floats(value: str, count: int) -> list[float]:
    parts = [part.strip() for part in value.split(",")]
    if len(parts) < count:
        raise ValueError(f"expected {count} values, got {len(parts)}")

    return [float(part) for part in parts[:count]]


def nearest_distance_to_bounds(x: float, y: float, bounds: Bounds) -> float:
    if x < bounds.min_x:
        dx = bounds.min_x - x
    elif x > bounds.max_x:
        dx = x - bounds.max_x
    else:
        dx = 0.0

    if y < bounds.min_y:
        dy = bounds.min_y - y
    elif y > bounds.max_y:
        dy = y - bounds.max_y
    else:
        dy = 0.0

    return math.hypot(dx, dy)


def parse_map_bounds(map_path: Path) -> tuple[Bounds, list[str]]:
    points: list[tuple[float, float]] = []
    errors: list[str] = []

    try:
        text = map_path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        return Bounds(0.0, 0.0, 0.0, 0.0), [f"Read error: {exc}"]

    for line_num, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("*") or line.startswith("#"):
            continue

        if line.startswith("L "):
            try:
                coords = parse_floats(line[2:], 6)
                points.append((coords[0], coords[1]))
                points.append((coords[3], coords[4]))
            except ValueError as exc:
                errors.append(f"Line {line_num}: invalid L line — {exc}")
            continue

        if line.startswith("P "):
            try:
                coords = parse_floats(line[2:], 3)
                points.append((coords[0], coords[1]))
            except ValueError as exc:
                errors.append(f"Line {line_num}: invalid P line — {exc}")
            continue

    if not points:
        return Bounds(0.0, 0.0, 0.0, 0.0), ["Map file contains no parseable point coordinates."]

    xs, ys = zip(*points)
    return Bounds(min(xs), max(xs), min(ys), max(ys)), errors


def load_camps(camp_dir: Path) -> dict[str, CampConfig]:
    camps: dict[str, CampConfig] = {}
    for camp_file in sorted(camp_dir.glob("*.toml")):
        try:
            data = tomllib.loads(camp_file.read_text(encoding="utf-8", errors="replace"))
        except OSError as exc:
            raise RuntimeError(f"Failed to read {camp_file.name}: {exc}") from exc

        name = str(data.get("name", "")).strip()
        zone = str(data.get("zone", "")).strip()
        if not name or not zone:
            raise RuntimeError(f"{camp_file.name}: missing required `name` or `zone`")

        camp_center = data.get("camp_center")
        pull_point = data.get("pull_point")
        pull_radius = data.get("pull_radius")
        if not isinstance(camp_center, Sequence) or len(camp_center) != 3:
            raise RuntimeError(f"{camp_file.name}: `camp_center` must be a 3-value list")
        if not isinstance(pull_point, Sequence) or len(pull_point) != 3:
            raise RuntimeError(f"{camp_file.name}: `pull_point` must be a 3-value list")
        if not isinstance(pull_radius, (int, float)):
            raise RuntimeError(f"{camp_file.name}: `pull_radius` must be numeric")

        camps[name] = CampConfig(
            name=name,
            zone=zone,
            camp_center=(float(camp_center[0]), float(camp_center[1]), float(camp_center[2])),
            pull_point=(float(pull_point[0]), float(pull_point[1]), float(pull_point[2])),
            pull_radius=float(pull_radius),
        )
    return camps


def find_map_file(map_dir: Path, zone: str) -> Path | None:
    direct = map_dir / f"{zone}.txt"
    if direct.exists():
        return direct

    variant = map_dir / f"{zone}_1.txt"
    if variant.exists():
        return variant

    zone_prefix = zone.lower() + "_"
    for candidate in map_dir.glob("*.txt"):
        if candidate.stem.lower().startswith(zone_prefix):
            return candidate
    return None


def in_bounds(x: float, y: float, bounds: Bounds, margin_ratio: float) -> bool:
    span_x = bounds.max_x - bounds.min_x
    span_y = bounds.max_y - bounds.min_y
    tol_x = span_x * margin_ratio / 2.0
    tol_y = span_y * margin_ratio / 2.0
    return (
        bounds.min_x - tol_x <= x <= bounds.max_x + tol_x
        and bounds.min_y - tol_y <= y <= bounds.max_y + tol_y
    )


def pull_radius_fits(x: float, y: float, radius: float, bounds: Bounds) -> bool:
    return (
        x - radius >= bounds.min_x
        and x + radius <= bounds.max_x
        and y - radius >= bounds.min_y
        and y + radius <= bounds.max_y
    )


def project_to_grid(x: float, y: float, bounds: Bounds, width: int, height: int) -> tuple[int, int]:
    span_x = bounds.max_x - bounds.min_x
    span_y = bounds.max_y - bounds.min_y
    if span_x == 0:
        span_x = 1.0
    if span_y == 0:
        span_y = 1.0

    col = int((x - bounds.min_x) / span_x * (width - 1))
    row = int((bounds.max_y - y) / span_y * (height - 1))
    col = max(0, min(width - 1, col))
    row = max(0, min(height - 1, row))
    return row, col


def render_ascii(camp: CampConfig, pull_point: tuple[float, float], bounds: Bounds) -> str:
    inner_width, inner_height = 42, 18
    width = inner_width + 2
    height = inner_height + 2
    grid = [[" "] * width for _ in range(height)]

    for col in range(1, width - 1):
        grid[0][col] = "-"
        grid[height - 1][col] = "-"
    for row in range(1, height - 1):
        grid[row][0] = "|"
        grid[row][width - 1] = "|"
    grid[0][0] = "+"
    grid[0][width - 1] = "+"
    grid[height - 1][0] = "+"
    grid[height - 1][width - 1] = "+"

    def set_cell(x: float, y: float, value: str) -> None:
        row, col = project_to_grid(x, y, bounds, inner_width, inner_height)
        grid[row + 1][col + 1] = value

    set_cell(camp.camp_center[0], camp.camp_center[1], "C")
    set_cell(pull_point[0], pull_point[1], "P")

    if camp.pull_radius > 0:
        for angle in range(0, 360, 10):
            rx = pull_point[0] + camp.pull_radius * math.cos(math.radians(angle))
            ry = pull_point[1] + camp.pull_radius * math.sin(math.radians(angle))
            row, col = project_to_grid(rx, ry, bounds, inner_width, inner_height)
            ch = "o"
            existing = grid[row + 1][col + 1]
            if existing == "C":
                ch = "X"
            elif existing == "P":
                ch = "P"
            grid[row + 1][col + 1] = ch

    return "\n".join("".join(row) for row in grid)


def evaluate_camp(camp: CampConfig, map_dir: Path, margin: float) -> CampResult:
    map_file = find_map_file(map_dir, camp.zone)
    if map_file is None:
        return CampResult(
            camp_name=camp.name,
            zone=camp.zone,
            map_file=None,
            bounds=None,
            center_ok=False,
            center_distance=0.0,
            pull_ok=False,
            pull_distance=0.0,
            pull_circle_ok=False,
            errors=[f"Missing map file for zone {camp.zone!r}."],
            warnings=[],
        )

    bounds, parse_errors = parse_map_bounds(map_file)
    warnings: list[str] = []
    errors = list(parse_errors)
    if parse_errors:
        return CampResult(
            camp_name=camp.name,
            zone=camp.zone,
            map_file=map_file,
            bounds=None,
            center_ok=False,
            center_distance=0.0,
            pull_ok=False,
            pull_distance=0.0,
            pull_circle_ok=False,
            errors=errors,
            warnings=warnings,
        )

    center_ok = in_bounds(camp.camp_center[0], camp.camp_center[1], bounds, 0.0)
    pull_ok = in_bounds(camp.pull_point[0], camp.pull_point[1], bounds, margin)
    center_distance = nearest_distance_to_bounds(camp.camp_center[0], camp.camp_center[1], bounds)
    pull_distance = nearest_distance_to_bounds(camp.pull_point[0], camp.pull_point[1], bounds)

    pull_circle_ok = pull_radius_fits(camp.pull_point[0], camp.pull_point[1], camp.pull_radius, bounds)
    if not pull_circle_ok:
        warnings.append("Pull radius circle does not fully fit inside map bounds.")

    return CampResult(
        camp_name=camp.name,
        zone=camp.zone,
        map_file=map_file,
        bounds=bounds,
        center_ok=center_ok,
        center_distance=center_distance,
        pull_ok=pull_ok,
        pull_distance=pull_distance,
        pull_circle_ok=pull_circle_ok,
        errors=errors,
        warnings=warnings,
    )


def format_mark(value: bool) -> str:
    return "✓" if value else "✗"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Validate camp coordinates vs map bounds.")
    parser.add_argument("camp_dir", nargs="?", default="config/camps")
    parser.add_argument("map_dir", nargs="?", default="config/maps")
    parser.add_argument(
        "--margin",
        type=float,
        default=0.20,
        help="Pull-point tolerance margin as fraction of map span (default 0.20).",
    )
    args = parser.parse_args(argv)

    camp_dir = Path(args.camp_dir).resolve()
    map_dir = Path(args.map_dir).resolve()

    if not camp_dir.exists():
        print(f"ERROR: camp config directory not found: {camp_dir}")
        return 1
    if not map_dir.exists():
        print(f"ERROR: map directory not found: {map_dir}")
        return 1

    try:
        camp_configs = load_camps(camp_dir)
    except Exception as exc:
        print(f"ERROR: failed to load camp configs: {exc}")
        return 1

    missing_required: list[str] = [name for name in TARGET_CAMPS if name not in camp_configs]
    if missing_required:
        print("ERROR: required camps missing from camp config directory.")
        for missing in missing_required:
            print(f"  - {missing}")
        return 1

    results: list[CampResult] = []
    fail_count = 0
    warning_count = 0

    for camp_name in TARGET_CAMPS:
        camp = camp_configs[camp_name]
        result = evaluate_camp(camp, map_dir, args.margin)
        results.append(result)
        if result.errors:
            fail_count += 1
            continue
        if not (result.center_ok and result.pull_ok):
            fail_count += 1
        if result.warnings:
            warning_count += len(result.warnings)

    for result in results:
        print("")
        print(f"Camp: {result.camp_name}")
        print(f"  Zone: {result.zone}")
        camp = camp_configs[result.camp_name]
        print(f"  Camp center: {camp.camp_center}")
        if result.map_file is None:
            print(f"  Map bounds: not found")
        else:
            print(f"  Map file: {result.map_file}")

        if result.bounds is None:
            print("  Map bounds: unavailable")
        else:
            print(
                f"  Map bounds: [{result.bounds.min_x:.1f}, {result.bounds.max_x:.1f}, "
                f"{result.bounds.min_y:.1f}, {result.bounds.max_y:.1f}]"
            )

        for error in result.errors:
            print(f"  Error: {error}")

        if result.bounds is not None:
            print(
                f"  Camp within bounds? {format_mark(result.center_ok)}"
                + (f" (distance {result.center_distance:.2f})" if not result.center_ok else "")
            )
            print(
                f"  Pull point within ±20% margin? {format_mark(result.pull_ok)}"
                + (f" (distance {result.pull_distance:.2f})" if not result.pull_ok else "")
            )
            print(
                f"  Pull radius circle fits map: "
                f"{'✓' if result.pull_circle_ok else '⚠'}"
                + ("" if result.pull_circle_ok else " (warning)")
            )
            for warning in result.warnings:
                print(f"  Warning: {warning}")
            print("  Diagram:")
            print(render_ascii(camp, camp.pull_point[:2], result.bounds).replace("\n", "\n    "))

    if fail_count == 0:
        print(f"\nPASS: all {len(results)} target camp locations validated.")
        if warning_count:
            print(f"PASS with {warning_count} warning(s).")
        return 0

    print(f"\nFAIL: {fail_count}/{len(results)} target camps failed validation.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
