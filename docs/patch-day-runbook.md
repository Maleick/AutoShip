# EQ Patch Day Runbook

How to update TextQuest offsets after an EQ live patch.

## Timeline

EQ patches every Tuesday during maintenance (~6am-11am PT). After the patch:

1. **T+0h**: EQ servers come up with new client build
2. **T+1-6h**: MQ community publishes updated `eqlib/offsets/eqgame.h`
3. **T+5min after MQ publish**: We run the import script and rebuild

## Quick Path (Recommended)

Wait for MQ to publish updated offsets, then import them.

```bash
# 1. Fetch the latest eqgame.h from MQ's eqlib (live branch)
curl -sL https://raw.githubusercontent.com/macroquest/eqlib/live/include/eqlib/offsets/eqgame.h \
    -o /tmp/eqgame.h

# 2. Optionally fetch eqmain.h
curl -sL https://raw.githubusercontent.com/macroquest/eqlib/live/include/eqlib/offsets/eqmain.h \
    -o /tmp/eqmain.h

# 3. Dry-run to preview changes
python3 scripts/import_mq_offsets.py --dry-run /tmp/eqgame.h

# 4. Apply changes
python3 scripts/import_mq_offsets.py --eqmain /tmp/eqmain.h /tmp/eqgame.h

# 5. Update version stamps manually (all three must move together):
#    - textquest-common/src/offsets.rs: CLIENT_DATE
#    - textquest-common/src/offsets.rs: ACTUAL_VERSION_DATE (address)
#    - textquest-common/src/offsets.rs: EXPECTED_VERSION_DATE (string)
#
#    CLIENT_DATE promotion gate: do not bump CLIENT_DATE from upstream MQ
#    headers alone. Bump it only after a local Ghidra or GhidraMCP run has
#    proven the new ACTUAL_VERSION_DATE address points at the expected
#    EXPECTED_VERSION_DATE string in the binary. See
#    docs/wiki/Research-Test-Offset-Reconciliation.md for the exact
#    verification commands and the ZoneGuideManagerClient false-promotion
#    incident from 2026-04-09 that motivates this gate.

# 6. Validate
cargo test -p textquest-common
cargo clippy --all-targets --all-features -- -D warnings
python3 scripts/validate_offsets_sync.py

# 7. Full preflight
python3 scripts/dev-preflight.py
```

See `docs/wiki/Offset-Placeholder-Audit.md` for the current list of
`0x0` placeholder constants that still need Ghidra-backed values.

## Manual Path (If MQ Hasn't Published Yet)

If you need offsets before MQ publishes, use binary diffing.

### Option A: Bulk Delta (Most Patches)

Most EQ patches shift all code by a constant delta (e.g., +0x1000).

```bash
# 1. Check one known function in the new binary (e.g., ProcessGameEvents)
#    Open new eqgame.exe in Ghidra, find ProcessGameEvents by string xref or pattern
#    Calculate: delta = new_address - old_address

# 2. If delta is consistent across 3+ functions, apply it globally:
#    For each constant in offsets.rs: new_value = old_value + delta
#    Bulk-delta automation is tracked by issue #763
#    ("Pattern Scanning & Patch-Day Workflow Parity"):
#    https://github.com/Maleick/TextQuest/issues/763

# 3. Validate a few key offsets manually before trusting the bulk delta
```

### Option B: Full Ghidra RE

For patches that reorganize code (rare):

1. Load new `eqgame.exe` into Ghidra
2. Run auto-analysis
3. Use Ghidra's Version Tracking to match functions from old session
4. Export matched addresses
5. Update `offsets.rs` manually

## What Gets Updated

### Address offsets (in `offsets.rs`)

| Category | Count | Source |
|----------|-------|--------|
| Global pointers (`PINST_*`) | 17 | `eqgame.h` / `eqmain.h` |
| Function addresses | 40 | `eqgame.h` / `eqmain.h` |
| Anti-cheat/network functions | 6 | Manual Ghidra RE only |
| Anti-cheat globals (msg counters) | 2 | Manual Ghidra RE only |

### Struct field offsets (in `offsets.rs` modules)

These change less frequently (only when EQ adds new struct fields):

| Category | Count | Notes |
|----------|-------|-------|
| `player_base::*` | 14 | Spawn position, name, ID |
| `player_zone::*` | 15 | HP, mana, level, class |
| `buff_slots::*` | 10 | Buff array layout |
| `profile::*` | 10 | Spellbook, memorized spells |
| UI widget offsets | ~40 | Chat, inventory, context menus |

Struct offsets are NOT in MQ's `eqgame.h` — they're in the C++ headers throughout eqlib.
The import script only handles address offsets, not struct fields.

### Version stamp (manual)

After updating addresses:

1. `textquest-common/src/offsets.rs`: update `CLIENT_DATE` to `"YYYYMMDD"`

`CLIENT_DATE` is the canonical client-date value used by both the scan engine
(`EXPECTED_CLIENT_DATE`) and `OffsetDatabase` (`client_date`), so no separate
edits are needed.

## Offset Name Mapping

Full mapping between TextQuest constant names and MQ `#define` names is in
`scripts/import_mq_offsets.py` (the `EQGAME_MAPPING` and `EQMAIN_MAPPING` dicts).

## Validation

After updating offsets:

1. `cargo test -p textquest-common` — offset consistency tests
2. `cargo test` — full workspace (no regressions)
3. If TEXTQUEST_SCAN_OFFSETS=1 is set, the DLL will log version detection and
   scan validation at startup — check `%TEMP%/textquest/textquest-dll.log`

## Future Automation

- **Pattern scanning** (#746-#750): Once real byte patterns replace placeholder stubs,
  the scan engine can auto-detect most address offsets without manual updates
- **Binary diff tool** (#757-#760): Automated function matching between old/new binaries
- **Ghidra export** (#748): Script to export function prologues for pattern authoring
