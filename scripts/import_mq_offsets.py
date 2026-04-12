#!/usr/bin/env python3
"""Import MQ eqlib offset headers into TextQuest's offsets.rs.

Usage:
    # From a local eqlib checkout:
    python3 scripts/import_mq_offsets.py path/to/eqlib/include/eqlib/offsets/eqgame.h

    # Fetch directly from GitHub (requires curl):
    curl -sL https://raw.githubusercontent.com/macroquest/eqlib/live/include/eqlib/offsets/eqgame.h \
        | python3 scripts/import_mq_offsets.py -

    # Also import eqmain.h:
    python3 scripts/import_mq_offsets.py --eqmain path/to/eqmain.h path/to/eqgame.h

    # Dry-run (show changes without writing):
    python3 scripts/import_mq_offsets.py --dry-run path/to/eqgame.h

This script parses #define lines from MQ's offset headers and updates the
corresponding constants in textquest-common/src/offsets.rs. It only updates
offsets that have a known mapping — new offsets must be added manually.

Designed for patch-day workflow: wait for MQ to publish updated eqgame.h,
run this script, then `cargo test -p textquest-common` to validate.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# ─── MQ #define name → TextQuest constant name mapping ───────────────────────
# Format: MQ define name (without _x suffix) → TQ constant path in offsets.rs
# The _x suffix is stripped during parsing.

EQGAME_MAPPING: dict[str, str] = {
    # Global pointers
    "pinstLocalPlayer": "PINST_LOCAL_PLAYER",
    "pinstControlledPlayer": "PINST_CONTROLLED_PLAYER",
    "pinstTarget": "PINST_TARGET",
    "pinstPlayerManager": "PINST_SPAWN_MANAGER",
    "pinstLocalPC": "PINST_LOCAL_PC",
    "pinstSpellManager": "PINST_SPELL_MANAGER",
    "pinstCDisplay": "PINST_CDISPLAY",
    "pinstCEverQuest": "PINST_CEVERQUEST",
    "pinstCChatWindowManager": "PINST_CCHAT_WINDOW_MANAGER",
    "pinstCInvSlotMgr": "PINST_CINV_SLOT_MGR",
    "pinstCXWndManager": "PINST_CXWND_MANAGER",
    "pinstActiveCorpse": "PINST_ACTIVE_CORPSE",
    "pinstSGraphicsEngine": "PINST_SGRAPHICSENGINE",
    "pinstCContextMenuManager": "PINST_CONTEXT_MENU_MANAGER",
    "instEQZoneInfo": "zone_info::INST_EQ_ZONE_INFO",
    # Functions
    "CharacterZoneClient__CastSpell": "CAST_SPELL",
    "PcZoneClient__DoCombatAbility": "DO_COMBAT_ABILITY",
    "CharacterZoneClient__UseSkill": "USE_SKILL",
    "CharacterZoneClient__CanUseItem": "CAN_USE_ITEM",
    "PlayerZoneClient__DoAttack": "DO_ATTACK",
    "__ExecuteCmd": "EXECUTE_CMD",
    "CEverQuest__InterpretCmd": "INTERPRET_CMD",
    "CEverQuest__RightClickedOnPlayer": "RIGHT_CLICKED_ON_PLAYER",
    "CEverQuest__ClickedPlayer": "CLICKED_PLAYER",
    "CEverQuest__IssuePetCommand": "ISSUE_PET_COMMAND",
    "PcClient__GetConLevel": "GET_CON_LEVEL",
    "PlayerClient__GetPcClient": "GET_PC_CLIENT",
    "__do_loot": "DO_LOOT",
    "__ProcessGameEvents": "PROCESS_GAME_EVENTS",
    "CEverQuest__dsp_chat": "DSP_CHAT",
    "CDisplay__RealRender_World": "REAL_RENDER_WORLD",
    "__FixHeading": "FIX_HEADING",
    "__get_bearing": "GET_BEARING",
    "CCharacterListWnd__EnterWorld": "CHAR_LIST_ENTER_WORLD",
    "CCharacterListWnd__SelectCharacter": "CHAR_LIST_SELECT_CHAR",
    "FreeTargetTracker__CastSpell": "FREE_TARGET_CAST_SPELL",
    "PlayerZoneClient__ChangeHeight": "CHANGE_HEIGHT",
    "ZoneGuideManagerClient__Instance": "ZONE_GUIDE_MANAGER",
    "CChatWindowManager__GetRGBAFromIndex": "CCHAT_MGR_GET_RGBA",
    "CChatWindowManager__InitContextMenu": "CCHAT_MGR_INIT_CONTEXT_MENU",
    "CChatWindowManager__FreeChatWindow": "CCHAT_MGR_FREE_CHAT_WINDOW",
    "CChatWindowManager__SetLockedActiveChatWindow": "CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT",
    "CChatWindowManager__CreateChatWindow": "CCHAT_MGR_CREATE_CHAT_WINDOW",
    "CInvSlotMgr__FindInvSlot": "INV_SLOT_MGR_FIND_SLOT",
    "CInvSlotMgr__MoveItem": "INV_SLOT_MGR_MOVE_ITEM",
    "CInvSlotMgr__SelectSlot": "INV_SLOT_MGR_SELECT_SLOT",
    "CInvSlot__GetItemBase": "INV_SLOT_GET_ITEM_BASE",
    "CSpellBookWnd__MemorizeSet": "SPELL_BOOK_WND_MEMORIZE_SET",
    "CContextMenuManager__HandleMenu": "CONTEXT_MENU_MGR_HANDLE_MENU",
    # Version stamps
    "__ClientDate": "__CLIENT_DATE",
    "__ExpectedVersionDate": "__EXPECTED_VERSION_DATE",
}

EQMAIN_MAPPING: dict[str, str] = {
    "EQMain__pinstCSidlManager": "eqmain::SIDL_MANAGER",
    "EQMain__pinstLoginServerAPI": "eqmain::LOGIN_SERVER_API",
    "EQMain__pinstCXWndManager": "eqmain::CXWND_MANAGER",
    "EQMain__LoginServerAPI__JoinServer": "eqmain::JOIN_SERVER",
    "EQMain__pinstCLoginViewManager": "eqmain::LOGIN_VIEW_MANAGER",
    "EQMain__pinstLoginClient": "eqmain::PINST_LOGIN_CLIENT",
    "EQMain__pinstLoginController": "eqmain::PINST_LOGIN_CONTROLLER",
    "EQMain__LoginController__GiveTime": "eqmain::LOGIN_CONTROLLER_GIVE_TIME",
}


def parse_header(text: str) -> dict[str, int]:
    """Parse #define lines from an MQ offset header.

    Returns a dict of {name_without_x: address}.
    """
    results: dict[str, int] = {}
    define_re = re.compile(
        r"^\s*#define\s+(\w+?)_x\s+(0x[0-9A-Fa-f]+)\s*$", re.MULTILINE
    )
    for m in define_re.finditer(text):
        name = m.group(1)
        addr = int(m.group(2), 16)
        results[name] = addr
    return results


def format_addr(addr: int) -> str:
    """Format address as Rust hex literal matching offsets.rs style.

    Example: 0x0001_400D_9F20 (12 hex digits, 3 groups of 4)
    """
    # offsets.rs uses 12 hex digits (padded to 48 bits) for EQ addresses
    hex_str = f"{addr:012X}"
    # Group into 4-char segments with underscores: 0001_400D_9F20
    parts = [hex_str[i : i + 4] for i in range(0, len(hex_str), 4)]
    return "0x" + "_".join(parts)


def update_offsets_rs(
    offsets_path: Path,
    updates: dict[str, int],
    mapping: dict[str, str],
    dry_run: bool = False,
) -> list[tuple[str, str, int, int]]:
    """Update constants in offsets.rs with new values.

    Returns list of (tq_name, mq_name, old_value, new_value) for changed offsets.
    """
    content = offsets_path.read_text()
    changes: list[tuple[str, str, int, int]] = []

    for mq_name, addr in updates.items():
        tq_name = mapping.get(mq_name)
        if tq_name is None:
            continue

        # Skip version date stamps (not hex addresses).
        # Only exclude the specific date/version defines — NOT function names
        # like __ExecuteCmd, __do_loot, __ProcessGameEvents which are real offsets.
        if mq_name in ("__ClientDate", "__ExpectedVersionDate", "__ClientTime",
                        "__ActualVersionDate", "__ActualVersionTime"):
            continue

        # Build regex to find the constant declaration
        # Handle both top-level (e.g., PINST_LOCAL_PLAYER) and module-level (e.g., zone_info::INST_EQ_ZONE_INFO)
        if "::" in tq_name:
            # Module-level constant — search for just the constant name part
            const_name = tq_name.split("::")[-1]
        else:
            const_name = tq_name

        # Match: pub const NAME: u64 = 0xHEX_ADDR;
        pattern = re.compile(
            rf"(pub\s+const\s+{re.escape(const_name)}\s*:\s*u64\s*=\s*)(0x[0-9A-Fa-f_]+)(\s*;)"
        )

        match = pattern.search(content)
        if match is None:
            print(f"  WARNING: could not find {tq_name} in offsets.rs", file=sys.stderr)
            continue

        old_hex = match.group(2)
        old_value = int(old_hex.replace("_", ""), 16)

        if old_value == addr:
            continue  # No change

        new_hex = format_addr(addr)
        new_content = content[: match.start(2)] + new_hex + content[match.end(2) :]
        content = new_content
        changes.append((tq_name, mq_name, old_value, addr))

    if not dry_run and changes:
        offsets_path.write_text(content)

    return changes


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Import MQ eqlib offset headers into TextQuest offsets.rs"
    )
    parser.add_argument(
        "eqgame_h",
        help="Path to eqgame.h (or - for stdin)",
    )
    parser.add_argument(
        "--eqmain",
        help="Path to eqmain.h (optional)",
    )
    parser.add_argument(
        "--offsets-rs",
        default=None,
        help="Path to offsets.rs (auto-detected from repo root)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show changes without writing",
    )
    args = parser.parse_args()

    # Find offsets.rs
    if args.offsets_rs:
        offsets_path = Path(args.offsets_rs)
    else:
        # Try to find it relative to script location
        script_dir = Path(__file__).resolve().parent
        offsets_path = script_dir.parent / "textquest-common" / "src" / "offsets.rs"

    if not offsets_path.exists():
        print(f"ERROR: offsets.rs not found at {offsets_path}", file=sys.stderr)
        return 1

    # Parse eqgame.h
    if args.eqgame_h == "-":
        eqgame_text = sys.stdin.read()
    else:
        eqgame_text = Path(args.eqgame_h).read_text()

    eqgame_defines = parse_header(eqgame_text)
    print(f"Parsed {len(eqgame_defines)} #define values from eqgame.h")

    # Count how many map to TQ constants
    mapped = sum(1 for k in eqgame_defines if k in EQGAME_MAPPING)
    print(f"  {mapped} have TextQuest mappings ({len(EQGAME_MAPPING)} total mappable)")

    # Apply updates
    changes = update_offsets_rs(offsets_path, eqgame_defines, EQGAME_MAPPING, args.dry_run)

    # Parse eqmain.h if provided
    if args.eqmain:
        eqmain_text = Path(args.eqmain).read_text()
        eqmain_defines = parse_header(eqmain_text)
        print(f"\nParsed {len(eqmain_defines)} #define values from eqmain.h")
        eqmain_changes = update_offsets_rs(
            offsets_path, eqmain_defines, EQMAIN_MAPPING, args.dry_run
        )
        changes.extend(eqmain_changes)

    # Report
    if not changes:
        print("\nNo changes needed — all offsets match.")
        return 0

    print(f"\n{'[DRY RUN] ' if args.dry_run else ''}Updated {len(changes)} offset(s):\n")
    for tq_name, mq_name, old_val, new_val in changes:
        print(f"  {tq_name}:")
        print(f"    MQ define: {mq_name}_x")
        print(f"    old: {format_addr(old_val)}")
        print(f"    new: {format_addr(new_val)}")
        print()

    if not args.dry_run:
        print(f"Written to {offsets_path}")
        print("\nNext steps:")
        print("  1. Update EXPECTED_CLIENT_DATE in scan_engine.rs")
        print("  2. Update client_date in offset_db.rs from_compiled_offsets()")
        print("  3. cargo test -p textquest-common")
        print("  4. cargo clippy --all-targets --all-features -- -D warnings")

    return 0


if __name__ == "__main__":
    sys.exit(main())
