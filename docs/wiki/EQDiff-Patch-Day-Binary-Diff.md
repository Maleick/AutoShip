# EQDiff Patch-Day Binary Diff

`eqdiff` is a workspace CLI for comparing two `eqgame.exe` builds and carrying known TextQuest function offsets forward when patch-day code moves.

## Usage

```bash
cargo run -p eqdiff -- old/eqgame.exe new/eqgame.exe --offsets config/offsets.json
```

By default the tool writes:

- `config/offsets.updated.json` with the same JSON shape consumed by `textquest-common::offset_db::OffsetDatabase`.
- `config/offsets.eqdiff-report.md` with a human-readable function match table.

Use `--output <path>` or `--report <path>` to choose explicit paths.

## Matching Strategy

Phase 1 matches function-like code RVAs by shared string references:

- Extract printable strings from `.rdata` and `.data`.
- Disassemble executable sections and find instructions that reference those strings.
- Match old and new code RVAs by shared strings, preferring the candidate with the most shared evidence.
- Rebase matching entries in the `functions` section from the old PE image base to the new PE image base.

Struct layout maps and unrelated globals are preserved because Phase 1 does not prove layout changes or global pointer movement.

## Review Expectations

Treat `eqdiff` output as patch-day evidence, not an automatic merge decision. Review the report, prioritize high-confidence matches with distinctive strings, and reconcile any unmatched or ambiguous offsets with canonical Ghidra evidence before updating checked-in offsets.
