# Session Handoff — 2026-04-07 22:07 UTC

> **Auto-generated** by `scripts/gen-handoff.sh`. Do not edit manually.

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

Optional local reference trees may live at `third_party/eqlib` and
`third_party/macroquest`. Routine `cargo build` / `cargo test` work does not
require them, but offset or struct work may use them when they are present in
the workspace.

## Repository Stats

- **Branch:** `docs-metrics-update`
- **Total commits:** 1284
- **Rust lines:** ~128050

## Reference Trees

- `third_party/eqlib` — not present
- `third_party/macroquest` — not present

## Workspace Crates

```
textquest  (v0.6.0)
textquest-dll  (v0.6.0)
textquest-common  (v0.6.0)
textquest-web  (v0.1.0)
```

## Recent Commits (last 20)

```
10d4201ea Remove stale MCP config and harden README metrics test counting
b8910e887 Use a single required PR gate job (#664)
a3ec46407 [WIP] Add command correlation IDs for IPC causality (#630)
704a6a960 Classify navmesh route failures and block unsafe fallback shortcuts (#642)
22cc96e8f Load combat actions from per-toon config files (#649)
19d137e1a [WIP] Add web UI for loot rules and distribution configuration (#656)
8588e13de Fix duplicate PR gate check names (#663)
152a670fc [WIP] Translate KissAssist capabilities into DMFT TUI slices (#639)
e7db0c62b Stabilize post-merge sync and HWBP tests (#662)
d5bc46bc4 [WIP] Add MQ2Nav destination command parity for nav target loc door item (#636)
4334943ef Guard pre-clear for bare target slash commands (#661)
b1959918a Add loot rules and distribution configuration (#544)
7f5e21a55 Intercept /door and target click commands (#595)
bb0dbe499 Align M10 roadmap docs and stabilize hosted CI (#610)
631a4e53d [WIP] Build milestone-level anti-cheat gates from official signals (#638)
48e124be5 [WIP] Add optional UDP multicast peer discovery mode (#628)
43b994c62 [WIP] Implement door and object interaction command path (#660)
9ec9a4d5a [WIP] Add casting parser options and target selectors (#645)
807cd47b2 Add MQ2MoveUtils-style /stick and /follow command handling with warp/summon break guards (#643)
b5e49c224 [WIP] Add MQ2Cast -bandolier gear swap option for cast workflows (#634)
```

## Test Results

```
running 1527 tests
test result: ok. 1527 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.84s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 22 tests
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 502 tests
test result: ok. 502 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 842 tests
test result: FAILED. 841 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### Test summary

| Passed | Failed |
|--------|--------|
| 1527 | 0 |
| 0 | 0 |
| 2 | 0 |
| 22 | 0 |
| 502 | 0 |
| 841 | 1 |

## Key Offsets (from textquest-common/src/offsets.rs)

| Constant | Value |
|----------|-------|
| EQ_PREFERRED_BASE | `0x0001_4000_0000` |
| PINST_LOCAL_PLAYER | `0x0001_40E8_E380` |
| PINST_CONTROLLED_PLAYER | `0x0001_40E8_E430` |
| PINST_TARGET | `0x0001_40E8_E428` |
| PINST_SPAWN_MANAGER | `0x0001_40F0_CD90` |
| PINST_LOCAL_PC | `0x0001_40E9_09A8` |
| PINST_SPELL_MANAGER | `0x0001_40F0_E6F0` |
| PINST_CDISPLAY | `0x0001_40E8_E450` |
| PINST_CEVERQUEST | `0x0001_40F1_1758` |
| PINST_CCHAT_WINDOW_MANAGER | `0x0001_40F2_2B20` |
| PINST_CINV_SLOT_MGR | `0x0001_40DD_D5F0` |
| CAST_SPELL | `0x0001_400D_9F20` |
| DO_COMBAT_ABILITY | `0x0001_402E_D490` |
| USE_SKILL | `0x0001_4010_52A0` |
| CAN_USE_ITEM | `0x0001_400E_DDB0` |
| DO_ATTACK | `0x0001_4031_B890` |
| EXECUTE_CMD | `0x0001_4022_35B0` |
| INTERPRET_CMD | `0x0001_4028_3FB0` |
| RIGHT_CLICKED_ON_PLAYER | `0x0001_4029_6D30` |
| PINST_EVERQUEST | `0x0001_40F1_1758` |
| PCCLIENT_EXTENDED_TARGET_LIST | `0x2e98` |
| PCCLIENT_IN_COMBAT | `0x2eac` |
| XTARGET_LIST_SLOTS_OFFSET | `0x08` |
| XTARGET_LIST_AUTO_ADD_HATERS | `0x20` |
| ARRAY_CLASS_LENGTH | `0x00` |
| ARRAY_CLASS_ARRAY_PTR | `0x08` |
| XTARGET_SLOT_SIZE | `0x4c` |
| XTARGET_SLOT_TYPE | `0x00` |
| XTARGET_SLOT_STATUS | `0x04` |
| XTARGET_SLOT_SPAWN_ID | `0x08` |

## Build Requirements

```bash
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

- Rust (edition 2024, nightly MSVC toolchain on Windows for live/release validation)
- CMake 3.5+ (for navmesh C++ FFI shim)
- LLVM/Clang (Windows, for bindgen)

## Key References

- Local eqlib reference: `third_party/eqlib`
- Local MacroQuest reference: `third_party/macroquest`
- MacroQuest login code: `third_party/macroquest/src/login`
- MacroQuest routing code: `third_party/macroquest/src/routing`
- MQ2Nav: https://github.com/brainiac/MQ2Nav
- mqmesh.com — navmesh downloads + updater.json manifest
