# Session Handoff — 2026-04-01 20:45 UTC

> **Auto-generated** by `scripts/gen-handoff.sh`. Do not edit manually.

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

MacroQuest reference code now lives in local git submodules at `third_party/eqlib`
and `third_party/macroquest`. After checkout, run
`git submodule update --init --recursive` before doing offset or struct work.

## Repository Stats

- **Branch:** `codex/continue-maptui-recovery`
- **Total commits:** 472
- **Rust lines:** ~71191

## Workspace Crates

```
dmft  (v0.5.0)
textquest-dll  (v0.5.0)
textquest-common  (v0.5.0)
```

## Recent Commits (last 20)

```
5051822f docs(third_party): align handoff with submodules
ebfd9de7 chore(third_party): convert eqlib and macroquest to submodules
75bd384d chore(third_party): strip vendor git metadata
8275b31f chore(third_party): vendor macroquest snapshot
eeaaf22b Squashed 'third_party/macroquest/' content from commit c7df19a1
f7b1374c chore(third_party): vendor eqlib snapshot
5de546c0 Squashed 'third_party/eqlib/' content from commit b5598ba
5a416622 Merge pull request #27 from Maleick/codex/audit-and-tui-improvements-0Ab7K
b82f19f2 fix: persist CH chain reorder and restore minimap overlay
fbca3454 fix: address review comments for CH chain and map UI
d2150a4a feat(maps): improve map rendering minimap labels layers and clustering
6d623658 fix: resolve parser warning cleanup and conflict artifacts
25fca0f4 Add TUI command aliases and persist command history
3495ca83 Merge pull request #26 from Maleick/claude/dmft-audit-tui-overhaul-Netyy
80fb12e0 chore: update Cargo.lock for new TUI widget dependencies
7eb277d9 feat(maps): zone maps, layer toggling, spawn clustering, rendering improvements
47101021 feat(tui): dropdown menus, CH chain panel, wizard, config panel, UX improvements
dbab4e00 deps: add ratatui third-party widget dependencies
61fa02e4 fix: complete Frostreaver rebrand audit and fix unsafe unwrap
90e1b30b Merge pull request #23 from Maleick/copilot/claudefix-clippy-warnings-and-audit-commits
```

## Test Results

```
running 1083 tests
test result: ok. 1083 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.97s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 22 tests
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 206 tests
test result: ok. 206 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 430 tests
test result: ok. 430 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Per-crate summary

| Crate | Passed | Failed |
|-------|--------|--------|
| — | 1083 | 0 |
| — | 0 | 0 |
| — | 22 | 0 |
| — | 206 | 0 |
| — | 430 | 0 |
| — | 0 | 0 |
| — | 0 | 0 |
| — | 0 | 0 |

## Key Offsets (from textquest-common/src/offsets.rs)

| Constant | Value |
|----------|-------|
pub const EQ_PREFERRED_BASE: u64 = 0x0001_4000_0000;
pub const PINST_LOCAL_PLAYER: u64 = 0x0001_40E8_E380;
pub const PINST_CONTROLLED_PLAYER: u64 = 0x0001_40E8_E430;
pub const PINST_TARGET: u64 = 0x0001_40E8_E428;
pub const PINST_SPAWN_MANAGER: u64 = 0x0001_40F0_CD90;
pub const PINST_LOCAL_PC: u64 = 0x0001_40E9_09A8;
pub const PINST_SPELL_MANAGER: u64 = 0x0001_40F0_E6F0;
pub const PINST_CDISPLAY: u64 = 0x0001_40E8_E450;
pub const PINST_CEVERQUEST: u64 = 0x0001_40F1_1758;
pub const CAST_SPELL: u64 = 0x0001_400D_9F20;
pub const DO_COMBAT_ABILITY: u64 = 0x0001_402E_D490;
pub const USE_SKILL: u64 = 0x0001_4010_52A0;
pub const CAN_USE_ITEM: u64 = 0x0001_400E_DDB0;
pub const DO_ATTACK: u64 = 0x0001_4031_B890;
pub const EXECUTE_CMD: u64 = 0x0001_4022_35B0;
pub const INTERPRET_CMD: u64 = 0x0001_4028_3FB0;
pub const PINST_CXWND_MANAGER: u64 = 0x0001_40F3_7B28;
pub const CHAR_LIST_ENTER_WORLD: u64 = 0x0001_400D_4B20;
pub const CHAR_LIST_SELECT_CHAR: u64 = 0x0001_400D_5D20;
pub const CLICKED_PLAYER: u64 = 0x0001_4027_24F0;
pub const ISSUE_PET_COMMAND: u64 = 0x0001_4028_56A0;
pub const GET_CON_LEVEL: u64 = 0x0001_402E_3C10;
pub const GET_PC_CLIENT: u64 = 0x0001_4030_7970;
pub const DO_LOOT: u64 = 0x0001_4022_C0D0;
pub const PINST_ACTIVE_CORPSE: u64 = 0x0001_40E8_E390;
pub const PROCESS_GAME_EVENTS: u64 = 0x0001_4028_E0F0;
pub const REAL_RENDER_WORLD: u64 = 0x0001_401A_4320;
pub const FIX_HEADING: u64 = 0x0001_4066_1520;
pub const GET_BEARING: u64 = 0x0001_4025_8850;
pub const FREE_TARGET_CAST_SPELL: u64 = 0x0001_402B_5740;

## Build Requirements

```bash
# One-time (all platforms): fetch reference trees used for offset/struct work
git submodule update --init --recursive

# macOS/Linux (development — demo mode)
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build
cargo run        # TUI with demo data
cargo test

# Windows (production — live EQ)
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build --release
```

### Dependencies

- Rust (edition 2024, stable MSVC toolchain on Windows)
- CMake 3.5+ (for navmesh C++ FFI shim)
- LLVM/Clang (Windows, for bindgen)

## Key References

- Local eqlib reference: `third_party/eqlib`
- Local MacroQuest reference: `third_party/macroquest`
- MacroQuest login code: `third_party/macroquest/src/login`
- MacroQuest routing code: `third_party/macroquest/src/routing`
- MQ2Nav: https://github.com/brainiac/MQ2Nav
- mqmesh.com — navmesh downloads + updater.json manifest
