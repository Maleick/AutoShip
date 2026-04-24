#!/usr/bin/env python
"""Export labeled or bookmarked Ghidra function prologues as JSON.

Run from Ghidra headless with optional script arguments:

    analyzeHeadless <project_dir> <project_name> \
        -process eqgame.exe \
        -scriptPath scripts \
        -postScript ghidra_export_function_prologues.py output.json 64

The output is a JSON array of objects with name, preferred-base address,
function size, and the first N bytes of machine code as uppercase hex.
"""

from __future__ import print_function

import json
import re


DEFAULT_BYTE_COUNT = 64
MIN_BYTE_COUNT = 32
MAX_BYTE_COUNT = 64
DEFAULT_NAME_RE = re.compile(r"^(FUN|SUB)_[0-9A-Fa-f]+$")


def address_to_hex(address):
    """Return a Ghidra address as the 0x-prefixed offset used in exports."""
    return "0x%X" % address.getOffset()


def function_size(function):
    """Return the number of addresses in the function body."""
    body = function.getBody()
    if hasattr(body, "getNumAddresses"):
        return int(body.getNumAddresses())
    if hasattr(function, "size"):
        return int(function.size)
    return 0


def is_default_function_name(name):
    """True when a function name looks auto-generated rather than labeled."""
    return DEFAULT_NAME_RE.match(name or "") is not None


def format_byte_string(values):
    """Format signed or unsigned byte values as uppercase two-digit hex."""
    return " ".join("%02X" % (int(value) & 0xFF) for value in values)


def clamp_byte_count(value):
    """Keep requested prologue length in the issue-requested 32-64 byte range."""
    try:
        count = int(value)
    except (TypeError, ValueError):
        count = DEFAULT_BYTE_COUNT
    if count < MIN_BYTE_COUNT:
        return MIN_BYTE_COUNT
    if count > MAX_BYTE_COUNT:
        return MAX_BYTE_COUNT
    return count


def make_byte_buffer(count):
    """Create a byte buffer that works in Ghidra Jython and CPython tests."""
    try:
        import jarray

        return jarray.zeros(count, "b")
    except ImportError:
        return bytearray(count)


def read_function_bytes(function, memory, byte_count):
    """Read up to byte_count bytes from a function entry point."""
    count = min(function_size(function), clamp_byte_count(byte_count))
    if count <= 0:
        return []

    buffer = make_byte_buffer(count)
    read_count = memory.getBytes(function.getEntryPoint(), buffer)
    if read_count < 0:
        read_count = 0
    read_count = min(int(read_count), count)
    return [int(buffer[index]) for index in range(read_count)]


def build_export_record(function, memory, byte_count):
    """Build one JSON-serializable export record for a Ghidra function."""
    return {
        "name": function.getName(),
        "address": address_to_hex(function.getEntryPoint()),
        "size": function_size(function),
        "bytes": format_byte_string(read_function_bytes(function, memory, byte_count)),
    }


def has_bookmark(function, bookmark_manager):
    """Return true when any bookmark exists across the function body.

    Ghidra's ``BookmarkManager.getBookmarksIterator`` signature is
    ``(Address, boolean)`` rather than ``(AddressSetView, boolean)``. Walk every
    address in the function body and short-circuit on the first hit so we
    actually cover the full range instead of only the entry point.
    """
    if bookmark_manager is None:
        return False

    body = function.getBody()
    if hasattr(bookmark_manager, "getBookmarksIterator"):
        address_iter = body.getAddresses(True)
        while address_iter.hasNext():
            iterator = bookmark_manager.getBookmarksIterator(address_iter.next(), True)
            if iterator.hasNext():
                return True
        return False

    if hasattr(bookmark_manager, "getBookmarks"):
        address_iter = body.getAddresses(True)
        while address_iter.hasNext():
            bookmarks = bookmark_manager.getBookmarks(address_iter.next())
            if len(bookmarks) > 0:
                return True
        return False

    return False


def should_export_function(function, bookmark_manager):
    """Export functions that are labeled by name or covered by a bookmark."""
    return (not is_default_function_name(function.getName())) or has_bookmark(
        function, bookmark_manager
    )


def iter_functions(function_manager):
    """Yield all functions from a Ghidra FunctionManager."""
    iterator = function_manager.getFunctions(True)
    while iterator.hasNext():
        yield iterator.next()


def export_records(program, byte_count):
    """Collect export records from all labeled/bookmarked functions."""
    memory = program.getMemory()
    function_manager = program.getFunctionManager()
    bookmark_manager = program.getBookmarkManager()
    records = []

    for function in iter_functions(function_manager):
        if should_export_function(function, bookmark_manager):
            records.append(build_export_record(function, memory, byte_count))

    records.sort(key=lambda record: int(record["address"], 16))
    return records


def write_records(records, output_path):
    """Write export records as stable pretty-printed JSON."""
    with open(output_path, "w") as handle:
        json.dump(records, handle, indent=2, sort_keys=True)
        handle.write("\n")


def parse_script_args(args):
    """Parse Ghidra script args as output path plus optional byte count."""
    if not args:
        raise ValueError("usage: ghidra_export_function_prologues.py <output.json> [32-64]")
    output_path = args[0]
    byte_count = DEFAULT_BYTE_COUNT
    if len(args) > 1:
        byte_count = clamp_byte_count(args[1])
    return output_path, byte_count


def main(program, args):
    output_path, byte_count = parse_script_args(args)
    records = export_records(program, byte_count)
    write_records(records, output_path)
    print("Exported %d function prologue(s) to %s" % (len(records), output_path))


if "currentProgram" in globals():
    main(currentProgram, getScriptArgs())
