#!/usr/bin/env python3
"""Export Ghidra-derived byte patterns as TextQuest scan entries.

This helper is intentionally usable outside Ghidra for tests and patch-day
automation. Feed it JSON records with at least `name` and `bytes`; `address`
is optional, and when omitted the emitted `expected_preferred` field is `null`.
It emits entries shaped like `textquest_common::pattern_db::ScanEntry`.

Example input record:

```json
{
  "name": "pinstLocalPlayer",
  "address": "0x140123456",
  "bytes": "48 8B 05 11 22 33 44 48 85 C0",
  "category": "Global",
  "resolve": {"RipRelative": {"disp_offset": 3}},
  "wildcards": [[3, 4]]
}
```
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterable


DEFAULT_WINDOW_LEN = 16

# Mirror of `textquest_common::pattern_db::ScanModule` and `ScanCategory` enums.
VALID_MODULES = ("EqGame", "EqMain", "EqGraphics")
VALID_CATEGORIES = ("Function", "Global")


def _normalize_module(value: str) -> str:
    for candidate in VALID_MODULES:
        if candidate.lower() == value.lower():
            return candidate
    raise ValueError(
        f"unsupported module {value!r}; expected one of {VALID_MODULES}"
    )


def _normalize_category(value: Any) -> str:
    if not value:
        return "Function"
    text = str(value)
    for candidate in VALID_CATEGORIES:
        if candidate.lower() == text.lower():
            return candidate
    raise ValueError(
        f"unsupported category {value!r}; expected one of {VALID_CATEGORIES}"
    )


def _parse_addr(value: Any) -> int | None:
    if value is None:
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        cleaned = value.replace("_", "").strip()
        # Accept 0x-prefixed, binary/octal (via base 0), or bare hex strings
        # (e.g. Ghidra often emits "14028E0F0"). Fall back to hex parsing when
        # the base-0 parse fails so bare hex addresses still round-trip.
        try:
            return int(cleaned, 0)
        except ValueError:
            return int(cleaned, 16)
    raise TypeError(f"unsupported address value: {value!r}")


def _parse_bytes(value: Any, window_len: int = DEFAULT_WINDOW_LEN) -> list[int] | None:
    if value is None:
        return None
    if isinstance(value, list):
        return [int(v) & 0xFF for v in value[:window_len]]
    if isinstance(value, str):
        tokens = re.findall(r"[0-9A-Fa-f]{2}", value)
        if not tokens:
            return None
        return [int(token, 16) for token in tokens[:window_len]]
    raise TypeError(f"unsupported bytes value: {value!r}")


def _wildcard_indexes(
    wildcard_ranges: Iterable[tuple[int, int] | list[int]] | None,
) -> set[int]:
    indexes: set[int] = set()
    for start, length in wildcard_ranges or []:
        indexes.update(range(int(start), int(start) + int(length)))
    return indexes


def ida_pattern(
    bytes_: Iterable[int],
    wildcard_ranges: Iterable[tuple[int, int] | list[int]] | None = None,
) -> str:
    """Format bytes as an IDA pattern, replacing wildcard ranges with `??`."""
    wildcards = _wildcard_indexes(wildcard_ranges)
    return " ".join(
        "??" if idx in wildcards else f"{int(byte) & 0xFF:02X}"
        for idx, byte in enumerate(bytes_)
    )


def _normalize_resolve(value: Any) -> str | dict[str, dict[str, int]]:
    if value is None:
        return "Direct"
    if value == "Direct":
        return "Direct"
    if isinstance(value, dict) and "RipRelative" in value:
        disp_offset = int(value["RipRelative"]["disp_offset"])
        return {"RipRelative": {"disp_offset": disp_offset}}
    raise ValueError(f"unsupported resolve mode: {value!r}")


def build_scan_entry(
    symbol: dict[str, Any],
    module: str,
    window_len: int = DEFAULT_WINDOW_LEN,
) -> dict[str, Any] | None:
    """Build a TextQuest scan entry from a symbol record.

    Returns `None` for records without byte data, allowing callers to export
    partially populated Ghidra symbol dumps without failing the whole run.
    """
    bytes_ = _parse_bytes(symbol.get("bytes"), window_len=window_len)
    if not bytes_:
        return None

    entry: dict[str, Any] = {
        "name": str(symbol["name"]),
        "module": _normalize_module(module),
        "pattern": ida_pattern(bytes_, symbol.get("wildcards")),
        "category": _normalize_category(symbol.get("category")),
        "resolve": _normalize_resolve(symbol.get("resolve")),
        "expected_preferred": _parse_addr(symbol.get("address")),
    }
    return entry


def parse_symbols(text: str) -> list[dict[str, Any]]:
    """Parse symbol JSON as either a list or an object with `symbols`/`functions`."""
    data = json.loads(text)
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        for key in ("symbols", "functions"):
            value = data.get(key)
            if isinstance(value, list):
                return value
    raise ValueError("expected a JSON list or object containing symbols/functions")


def export_patterns(
    symbols: Iterable[dict[str, Any]],
    module: str,
    window_len: int = DEFAULT_WINDOW_LEN,
) -> list[dict[str, Any]]:
    """Export all symbols with byte windows as scan entries."""
    entries: list[dict[str, Any]] = []
    for symbol in symbols:
        entry = build_scan_entry(symbol, module=module, window_len=window_len)
        if entry is not None:
            entries.append(entry)
    return entries


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Export Ghidra symbol byte windows as TextQuest scan entries"
    )
    parser.add_argument("input", help="Input JSON path, or - for stdin")
    parser.add_argument(
        "--module",
        default="EqGame",
        choices=VALID_MODULES,
        help="ScanModule name to emit",
    )
    parser.add_argument(
        "--window-len",
        type=int,
        default=DEFAULT_WINDOW_LEN,
        help="Maximum number of bytes to include per pattern",
    )
    parser.add_argument("--output", help="Output JSON path; defaults to stdout")
    args = parser.parse_args()

    if args.input == "-":
        text = sys.stdin.read()
    else:
        text = Path(args.input).read_text(encoding="utf-8")

    entries = export_patterns(parse_symbols(text), module=args.module, window_len=args.window_len)
    payload = json.dumps(entries, indent=2) + "\n"

    if args.output:
        Path(args.output).write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
