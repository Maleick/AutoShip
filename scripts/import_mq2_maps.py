#!/usr/bin/env python3
"""
import_mq2_maps.py — Import and convert MQ2 community map files to TextQuest format.

MQ2 map format (same as TextQuest format):
  L x1, y1, z1, x2, y2, z2, r, g, b   — line segment
  P x, y, z, r, g, b, size, label      — point/label

Usage:
  # Download and convert from a local MQ2 map directory:
  python3 scripts/import_mq2_maps.py --source /path/to/mq2/maps --dest config/maps/

  # Convert a single file:
  python3 scripts/import_mq2_maps.py --file /path/to/mq2/maps/gfaydark.txt --dest config/maps/

  # List zones available in source directory:
  python3 scripts/import_mq2_maps.py --source /path/to/mq2/maps --list

MQ2 map files are directly compatible with TextQuest — no conversion is needed
for the line/point data itself.  This script copies and validates files,
optionally filtering to a specific zone list.

The canonical MQ2 community map source is:
  https://github.com/macroquest/macroquest (Resources/MQ2Map/maps/)
  or hosted mirrors of the EQ map packs.
"""

import argparse
import re
import shutil
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Zone name → short filename mapping (MQ2 convention)
# Add entries here to expand the import list.
# ---------------------------------------------------------------------------
ZONE_LIST = [
    # Classic overworld
    ("West Commonlands", "wcommons"),
    ("East Commonlands", "ecommons"),
    ("Qeynos Hills", "qeytoqrg"),
    ("Greater Faydark", "gfaydark"),
    ("Lesser Faydark", "lfaydark"),
    ("Crushbone", "crushbone"),
    ("Blackburrow", "blackburrow"),
    ("Highpass Hold", "highpass"),
    ("Oasis of Marr", "oasis"),
    ("South Karana", "southkarana"),
    ("North Karana", "northkarana"),
    ("Eastern Plains of Karana", "eastkarana"),
    ("Kithicor Forest", "kithicor"),
    ("Lake Rathe", "lakerathe"),
    ("Rathe Mountains", "rathemtn"),
    ("Lavastorm Mountains", "lavastorm"),
    ("Nektulos Forest", "nektulos"),
    ("Commonlands (old)", "commons"),
    ("Butcherblock Mountains", "butcher"),
    ("Ocean of Tears", "oot"),
    ("Feerrott", "feerrott"),
    ("Innothule Swamp", "innothule"),
    ("Misty Thicket", "mistythicket"),
    ("Rivervale", "rivervale"),
    # Classic dungeons
    ("Nagafen's Lair (SolB)", "solb"),
    ("Solusek's Eye (SolA)", "sola"),
    ("Lower Guk", "gukbottom"),
    ("Upper Guk", "guktop"),
    ("Runnyeye", "runnyeye"),
    ("Unrest", "unrest"),
    ("Kedge Keep", "kedge"),
    ("Najena", "najena"),
    # Kunark
    ("Field of Bone", "fieldofbone"),
    ("Warslik's Woods", "warslikswood"),
    ("Lake of Ill Omen", "lakeofillomen"),
    ("Frontier Mountains", "frontiermtns"),
    ("Burning Wood", "burningwood"),
    ("Chardok", "chardok"),
    ("Dalnir", "dalnir"),
    ("Old Sebilis", "sebilis"),
    ("Trakanon's Teeth", "trakanon"),
    ("Timorous Deep", "timorous"),
    ("Firiona Vie", "firiona"),
    # Velious
    ("Eastern Wastes", "eastwastes"),
    ("Western Wastes", "westwastes"),
    ("Crystal Caverns", "crystal"),
    ("Kael Drakkel", "kael"),
    ("Skyshrine", "skyshrine"),
    ("Temple of Veeshan", "templeveeshan"),
    ("Velketor's Labyrinth", "velketor"),
]

LINE_RE = re.compile(
    r"^L\s+"
    r"(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*"
    r"(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*"
    r"(\d+),\s*(\d+),\s*(\d+)\s*$"
)
POINT_RE = re.compile(
    r"^P\s+"
    r"(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*(-?\d+\.?\d*),\s*"
    r"(\d+),\s*(\d+),\s*(\d+),\s*"
    r"(\d+),\s*(.+)\s*$"
)


def validate_map_file(path: Path) -> tuple[int, int, list[str]]:
    """Return (line_count, point_count, errors)."""
    lines = 0
    points = 0
    errors: list[str] = []
    with open(path, encoding="utf-8", errors="replace") as fh:
        for lineno, raw in enumerate(fh, 1):
            raw = raw.rstrip("\r\n")
            if not raw or raw.startswith("#"):
                continue
            if raw.startswith("L ") or raw.startswith("L\t"):
                if LINE_RE.match(raw):
                    lines += 1
                else:
                    errors.append(f"  line {lineno}: bad L record: {raw[:80]!r}")
            elif raw.startswith("P ") or raw.startswith("P\t"):
                if POINT_RE.match(raw):
                    points += 1
                else:
                    errors.append(f"  line {lineno}: bad P record: {raw[:80]!r}")
            else:
                errors.append(f"  line {lineno}: unknown record: {raw[:80]!r}")
    return lines, points, errors


def import_file(src: Path, dest_dir: Path, dry_run: bool = False) -> bool:
    """Validate and copy src into dest_dir.  Returns True on success."""
    lines, points, errors = validate_map_file(src)
    if errors:
        print(f"  WARN  {src.name}: {len(errors)} parse errors (first 3 shown)")
        for e in errors[:3]:
            print(e)
        if lines + points == 0:
            print(f"  SKIP  {src.name}: no valid records")
            return False
    print(f"  OK    {src.name}: {lines} L-records, {points} P-records")
    if not dry_run:
        shutil.copy2(src, dest_dir / src.name)
    return True


def cmd_list(source: Path) -> None:
    txt_files = sorted(source.glob("*.txt"))
    if not txt_files:
        print(f"No .txt files found in {source}")
        sys.exit(1)
    print(f"Found {len(txt_files)} map files in {source}:")
    for f in txt_files:
        print(f"  {f.name}")


def cmd_import_dir(source: Path, dest: Path, dry_run: bool) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    txt_files = sorted(source.glob("*.txt"))
    if not txt_files:
        print(f"No .txt files found in {source}")
        sys.exit(1)
    ok = 0
    skip = 0
    for f in txt_files:
        dest_file = dest / f.name
        if dest_file.exists():
            print(f"  EXIST {f.name} — already present, skipping")
            skip += 1
            continue
        if import_file(f, dest, dry_run=dry_run):
            ok += 1
        else:
            skip += 1
    print(f"\nImported {ok} files, skipped {skip}.")


def cmd_import_file(src: Path, dest: Path, dry_run: bool) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    import_file(src, dest, dry_run=dry_run)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Import MQ2 community map files into TextQuest config/maps/"
    )
    parser.add_argument(
        "--source", metavar="DIR",
        help="Directory containing MQ2 .txt map files"
    )
    parser.add_argument(
        "--file", metavar="FILE",
        help="Single MQ2 .txt map file to import"
    )
    parser.add_argument(
        "--dest", metavar="DIR", default="config/maps",
        help="Destination directory (default: config/maps)"
    )
    parser.add_argument(
        "--list", action="store_true",
        help="List zones available in --source (no import)"
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Validate files without copying"
    )
    args = parser.parse_args()

    dest = Path(args.dest)

    if args.list:
        if not args.source:
            parser.error("--list requires --source")
        cmd_list(Path(args.source))
    elif args.file:
        cmd_import_file(Path(args.file), dest, dry_run=args.dry_run)
    elif args.source:
        cmd_import_dir(Path(args.source), dest, dry_run=args.dry_run)
    else:
        parser.print_help()
        print("\nSupported zone short names (MQ2 filename stems):")
        for name, stem in ZONE_LIST:
            print(f"  {stem:<25} {name}")
        sys.exit(0)


if __name__ == "__main__":
    main()
