# Research: Patch-Day Reproduction Workflow

This document is the reproducible patch-day runbook for TextQuest.

It is meant to keep Test and Live refreshes fast when EverQuest patches land, especially on the monthly Test patch line and the following Live patch line.

## Current Status

- Source-of-truth raw exports live in the sibling repo:
  - `/Users/maleick/Projects/TextQuest-Ghidra`
- Canonical immutable evidence for the historical/reference-only Test run lives in the evidence repo snapshot:
  - `/Users/maleick/Projects/TextQuest-Ghidra/snapshots/2026-04-09-patch-day/ghidra-manifest.json`
  - `/Users/maleick/Projects/TextQuest-Ghidra/snapshots/2026-04-09-patch-day/symbols`
- TextQuest keeps the runbook, workflow guidance, and pointers to that snapshot. It is not the canonical home for immutable manifests or copied symbol trees.
- The evidence pack is intentionally split into Test and Live so partial Live coverage does not get mistaken for full patch-day parity.
- Raw authority remains `TextQuest-Ghidra/test/*`.
- Current copied symbol package includes:
  - `live/eqgame`
  - `test/eqgame`
  - `test/eqmain`
  - `test/eqgraphics`
- Current Test export counts from the manifest:
  - `eqgame`: `19,994` functions, `143,929` symbols, base `0x140000000`
  - `eqmain`: `4,591` functions, `27,680` symbols, base `0x180000000`
  - `eqgraphics`: `6,310` functions, `6,501` symbols, base `0x180000000`
- Current live baseline in repo-local exports is incomplete:
  - `live/eqgame` exists
  - `live/eqmain` and `live/eqgraphics` do not yet exist in `TextQuest-Ghidra`
- The local debugger database consumed by the TUI currently lives at:
  - `/Users/maleick/Projects/TextQuest/data/ghidra.db`
  - current counts: `23120` functions, `0` globals, `427` imports, `21980` strings, `0` opcodes
  - it is a runtime/debug cache rebuilt from `../TextQuest-Ghidra/test/eqgame/ghidra-export/`
  - it is not the full patch-day evidence store
  - under current policy it is intentionally not retained as a long-term Test database artifact; only the lightweight baseline note survives in `TextQuest-Ghidra`
  - current Debug behavior: the Ghidra Explorer is row-oriented but still function-heavy; EQ Internals still comes from compiled offsets because the `globals` table is empty
- The older Windows-side raw export bundle is still present on `frostreaver`:
  - `C:\Users\xmale\Projects\TextQuest\data\ghidra-export\ghidra-export`
  - it contains the raw April 3 live export (`functions.json`, `strings.json`, `imports.json`, `bookmarks.json`, etc.)
  - it does not contain `decompiled/` or `decompiled_index.json`
  - its `19,542` functions plus `21,982` strings explain the earlier “about 40,000” count memory
- Current Test snapshot integrity warning:
  - `eqgame` `metadata.json` reports `19994` functions, while `functions.json` and `decompiled_index.json` report `19578`
  - `eqmain` `metadata.json` reports `4591`, while `functions.json` and `decompiled_index.json` report `4362`
  - `eqgraphics` is aligned at `6310`
  - `metadata_raw.json` is byte-identical to `metadata.json` for all three Test modules
  - treat these as integrity warnings only; do not promote alternate counts blindly without module-level raw evidence context

## Audit Structure

Use the same four buckets for every patch-day run:

- Inputs: exact binaries, host, transport, and harvest timestamp.
- Evidence pack: the canonical dated manifest plus copied symbol trees under `TextQuest-Ghidra/snapshots/<date>-patch-day/`.
- Runtime validation: fresh Test retest, fresh Live retest when Live binaries exist, and a Windows desktop-drop sanity check.
- Blockers: missing module harvests, stale remote workspace contents, missing credentials, unresolved globals/functions, or a skipped retest.

The current manifest does not populate a top-level `generated_at`, so use the filename plus each variant's `harvest_info.harvested_at` field as the timestamp source until the generator is updated.

Current audit split:

- Test: harvest-complete for `eqgame`, `eqmain`, and `eqgraphics`.
- Live: partial only; `live/eqgame` exists, while `live/eqmain` and `live/eqgraphics` are still missing.
- Runtime retest: blocked until a fresh interactive Live session is available on frostreaver.
- Any cited frostreaver runtime outcomes without an archived artifact path should be treated as historical runtime notes only. Missing-runtime-proof archival discipline is tracked in `Maleick/TextQuest#713`.

## Test / Demo Consolidation Notes

### What worked

- Repo-local Test harvest and evidence packaging are reproducible from the current workspace.
- The current `data/ghidra.db` rebuild path is stable after deleting the old database first.
- The Windows desktop-drop helper now copies the fresh EXE, DLL, `data/ghidra.db`, and injection verifier into `C:\Users\xmale\Desktop\TextQuest-Test`.
- Demo mode remains a stable local Mac workflow for UI and map smoke testing.

### What failed

- Live evidence remains partial because `live/eqmain` and `live/eqgraphics` are still missing.
- The manifest generator still leaves top-level `generated_at` unset, so timestamp audit must rely on the dated filename plus per-variant harvest timestamps.
- A mixed remote workspace on frostreaver previously produced misleading Windows results even when the desktop drop looked fresh.
- PacketMonitor still needs a fresh live-inject proof row after the rebuilt desktop drop.
- Tactical-map `r` reset previously collided with repeat-command handling outside the focused map pane; the code path is now hardened, but live UX still needs a fresh Windows check.

### Exact commands and expected evidence

1. Test harvest:

```bash
cd /Users/maleick/Projects/TextQuest-Ghidra
python scripts/harvest_mcp.py test
```

Evidence:

- `TextQuest-Ghidra/test/harvest-info.json`
- `TextQuest-Ghidra/test/eqgame/ghidra-export/*`
- `TextQuest-Ghidra/test/eqmain/ghidra-export/*`
- `TextQuest-Ghidra/test/eqgraphics/ghidra-export/*`

2. Evidence pack copy:

```bash
python3 /Users/maleick/Projects/TextQuest/scripts/collect_patch_evidence.py --manifest-out /Users/maleick/Projects/TextQuest-Ghidra/snapshots/<date>-patch-day/ghidra-manifest.json --copy-symbols /Users/maleick/Projects/TextQuest-Ghidra/snapshots/<date>-patch-day/symbols
```

Evidence:

- dated manifest in `TextQuest-Ghidra/snapshots/<date>-patch-day/`
- copied symbol tree in `TextQuest-Ghidra/snapshots/<date>-patch-day/symbols`
- `TextQuest` retains runbook pointers only and should not be treated as the canonical home for snapshot evidence

3. Debugger DB rebuild:

```bash
cd /Users/maleick/Projects/TextQuest
rm -f data/ghidra.db
cargo run --bin import_ghidra -- data/ghidra.db ../TextQuest-Ghidra/test/eqgame/ghidra-export/
sqlite3 data/ghidra.db 'select count(*) from functions; select count(*) from globals; select count(*) from imports; select count(*) from strings; select count(*) from opcodes;'
```

Evidence:

- current April 9 / 10 snapshot counts: `23120 / 0 / 427 / 21980 / 0`
- `globals` and `opcodes` remain empty, which is why EQ Internals still comes from compiled offsets

4. Windows desktop-drop rebuild:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-windows-release.ps1 -CopyToDesktop
```

Evidence:

- `C:\Users\xmale\Desktop\TextQuest-Test\01-dump.cmd`
- `C:\Users\xmale\Desktop\TextQuest-Test\02-inject.cmd`
- `C:\Users\xmale\Desktop\TextQuest-Test\03-ui.cmd`
- `C:\Users\xmale\Desktop\TextQuest-Test\README.txt`
- `C:\Users\xmale\Desktop\TextQuest-Test\TEST-ORDER.txt`

5. Demo smoke test on macOS:

```bash
cargo build
cargo run
```

Evidence:

- TUI opens in demo mode
- `2` Map loads from `config/maps`
- `<` / `>` adjusts depth and `+` / `-` adjusts zoom
- `r` resets zoom, pan, and viewport
- `h` / `x` from Map hand raw spawn memory to Debug

## Historical / Reference-Only Tactical Map Verification (2026-04-10)

This section absorbs the unique operator procedure from the prior standalone draft. Keep using this runbook as the canonical TextQuest pointer document; the snapshot evidence itself remains in `TextQuest-Ghidra`.

### What worked

- `config/maps` discovery probes CWD ancestors, executable ancestors, and macOS bundle-style `Contents/Resources/config/maps`.
- `permafrost` loads as the known-good map baseline.
- Tactical map status distinguishes loaded geometry from point-only overlays.
- Tactical status banners call out hidden geometry, no drawable geometry, and unavailable navmesh.
- Tactical `r` remains the primary reset key.
- `[` and `]` remain reserved for previous/next client navigation.

### What failed

- `crescent` map assets were missing during this audit pass.
- Demo bounds clamping on macOS cannot use map bounds for zones without loadable map data.
- Zones that stay `Unknown` indicate a real lookup problem and are not valid map input.

### Repro: Test

1. Start the Test client and move to `permafrost`.
2. Launch the TUI with `cargo run -p textquest -- tui`.
3. Switch to Tactical view.
4. Confirm the status line reports loaded geometry rather than silent partial data.
5. Confirm geometry and spawns render together.
6. Press `+` and `-` to verify zoom changes.
7. Press `<` and `>` to verify depth changes.
8. Press `r` to verify the tactical map view resets.
9. Press `n` to verify the navmesh/status message stays explicit.

### Repro: Mac Demo

1. Ensure no live EQ clients are attached.
2. Start the local demo client in `permafrost`.
3. Launch the TUI in demo mode.
4. Switch to Tactical view and confirm the map bounds initialize correctly for the demo zone.
5. If data is missing, confirm the UI reports that explicitly instead of drawing a silent partial map.
6. Re-run the same `+` / `-`, `<` / `>`, `r`, and `n` checks used for Test.

### Commands

```bash
cargo fmt --all
cargo build -p textquest
cargo test -p textquest -- --nocapture
cargo run -p textquest -- tui
```

### Pass criteria

- `permafrost` shows geometry and spawns in both Test and demo.
- The Tactical header reports zoom and depth.
- `r` resets the tactical map view and prints an explicit reset confirmation.
- `[` and `]` continue to switch clients globally.
- Missing map files are reported as missing instead of becoming a silent partial map.
- Navmesh availability remains explicit.

### Fail criteria

- The zone displays `Unknown` when a resolvable zone name should exist.
- Geometry is partial or absent without an explicit diagnostic status.
- `r` fails to reset the Tactical map.
- `[` or `]` are repurposed.
- Demo mode on macOS stops loading demo map data or silently skips missing-bounds cases.

### Patch-day notes

- Record any missing filenames if `crescent` or another expected zone is absent.
- Treat point-only overlays differently from true no-data cases.
- Use the on-screen Tactical status line as the operator-facing source of truth.

### Next blocker state

- Test blocker: `PINST_SPAWN_MANAGER`, `spawn_manager::PLAYER_LIST`, `PINST_CDISPLAY`, `PINST_CEVERQUEST`, `PINST_CXWND_MANAGER`, and the remaining `eqgame` hook rebinding still need local proof.
- Test blocker: a fresh live-in-world packet row is still required after the rebuilt desktop drop.
- Demo blocker: none at the code level; the remaining gap is documentation discipline, not map-path implementation.
- Live blocker: missing `live/eqmain` and `live/eqgraphics`.

## What Worked

- The repo-local Ghidra harvest flow is stable through the local MCP-backed workflow:
  - `/Users/maleick/Projects/TextQuest-Ghidra/scripts/harvest_mcp.py`
- The current Test correction strategy is working better when driven from local Ghidra evidence instead of upstream-only offsets.
- `LocalPC -> me` is the highest-confidence local-player chain on the current Test patch:
  - `PINST_LOCAL_PC = 0x140EA9A68`
  - `character_zone::ME = 0x2848`
- The injector path is now stable on Test.
  - `prepare_dll_locked()` was holding the staged payload open in a way that made `LoadLibraryW` fail.
  - current injects stage with `prepare_dll()` instead, and `LoadLibraryW` success is verified via thread exit code.
- The immediate post-inject crash was caused by a stale render detour.
  - current Test logs showed `REAL_RENDER_WORLD = 0x1401A4320`, but that address is absent from the local Test export and conflicts with existing research notes.
  - the render hook is now disabled by default and only enabled when `TEXTQUEST_ENABLE_RENDER_HOOK=1` is set.
- The current Test `PlayerZoneClient` layout is now partially runtime-verified.
  - rebinding the `player_zone` field block to the current `PlayerClient.h` layout restored plausible spawn names, levels, HP, and positions in `textquest.exe --dump`
  - the key corrected fields are `HPCurrent`, `HPMax`, `StandState`, `Level`, `CharClass`, `SpellGemETA`, `ManaCurrent`, `ManaMax`, `EnduranceCurrent`, and `EnduranceMax`
- Even with `pinstSpawnManager` still null on Test, the linked-list fallback is good enough to enumerate the zone.
  - current dump on `frostreaver` found `682` spawns after falling back from the null manager pointer
- The remote Windows build problem on `frostreaver` is reproducible and fixable:
  - stale `recastnavigation-sys-*` CMake state under `target/release/build/`
  - mismatched default generator (`Visual Studio 18 2026`) vs older cache (`Visual Studio 17 2022`)
  - clearing those build dirs and forcing `CMAKE_GENERATOR=Visual Studio 17 2022` lets the release build proceed
- The Windows rebuild flow had another reproducibility trap:
  - syncing files over `tar` preserved older source mtimes
  - Cargo sometimes skipped rebuilding `textquest_dll` even after real code changes
  - `scripts/build-windows-release.ps1` now refreshes Rust source mtimes under `textquest-common/src`, `textquest/src`, and `textquest-dll/src` before the release build so the desktop DLL actually updates
- The current Test class fix is now runtime-backed:
  - local player class was still wrong while `PlayerZoneClient::CharClass` read as `0`
  - moving to `mActorClient.Class` (`0x0FFC`) with `PcProfile::Class` (`0x17CC`) as fallback restored the local player as `PAL` in dump mode
- The current Test zone fallback had a concrete symbol-selection bug:
  - wrong TextQuest value: `0x1403571F0`
  - correct Test singleton from the `openvanilla` checklist: `ZoneGuideManagerClient__Instance_x = 0x1403572C0`
  - correcting that singleton is now part of the reproducible patch checklist
- PacketMonitor no longer depends on only one Winsock API family:
  - TextQuest now hooks `send` / `recv` and `WSASend` / `WSARecv`
  - fresh packet-row proof still requires a new inject into a live client with the rebuilt DLL
- The current symbol-oriented JSON exports are usable as a reproducible patch artifact:
  - `functions.json`
  - `classes.json`
  - `namespaces.json`
  - `imports.json`
  - `exports.json`
  - `metadata.json`
  - `metadata_raw.json`
  - `decompiled_index.json`

## What Did Not Work

- The `frostreaver` coding workspace drifted behind the local repo.
  - Earlier Windows tests were using a stale `textquest.exe` built before the LocalPC-chain and Test offset work.
- The older Windows raw export bundle is not a substitute for the newer symbol/decompile repo.
  - it has no `decompiled/` directory and no `decompiled_index.json`
  - use it as a live-baseline/raw-count reference only
- Direct SSH launch of GUI apps is still wrong for live EQ work.
  - It lands the process in session 0, not the interactive desktop session.
- Full autonomous login is still blocked by credentials, not by launch code.
  - `config/accounts.toml` exists on `frostreaver`
  - `data/credentials.db` does not
  - `%APPDATA%\\TextQuest\\master-password.txt` does not
- A local artifact matching the name `geyser dump` was not found under `/Users/maleick/Projects`.
  - The reproducible evidence path currently uses the repo-local Ghidra exports instead.

## Patch Notes

- The current Test offset work should treat upstream `eqlib` only as a checklist surface.
- Promotion authority is still:
  - local `TextQuest-Ghidra` exports
  - local GhidraMCP verification
- `openvanilla` / `eqlib` headers are still useful as reference-only layout clues when Test lacks an older baseline.
  - The current useful reference examples were `PlayerClient::mActorClient`, `ActorBase::Class`, and `PcProfile::Class`.
  - Those values were not promoted blindly; the live Test dump had to agree before TextQuest changed its class reads.
- Patch checklist surfaces remain:
  - `eqgame.h`
  - `eqmain.h`
  - `eqgraphics.h`
  - the player/UI/login header cluster under `src/game`
- Current `live` vs `test` `eqgame` evidence from the manifest:
  - `7` symbol-oriented JSON files differ
  - `decompiled_index.json` is present only in the current Test package

## Helper Tools

- Harvest current binaries through the local Windows GhidraMCP workflow:
  - `/Users/maleick/Projects/TextQuest-Ghidra/scripts/harvest_mcp.py`
- Generate a portable evidence manifest and copy symbol files into TextQuest:
  - `/Users/maleick/Projects/TextQuest/scripts/collect_patch_evidence.py`
- Rebuild the local debugger database consumed by the TUI:
  - `cd /Users/maleick/Projects/TextQuest`
  - `cargo run --bin import_ghidra -- data/ghidra.db ../TextQuest-Ghidra/test/eqgame/ghidra-export/`
- Rebuild the Windows release artifacts with the known-good generator and recast cleanup:
  - `/Users/maleick/Projects/TextQuest/scripts/build-windows-release.ps1`
- Existing Windows validation helpers in TextQuest:
  - `/Users/maleick/Projects/TextQuest/scripts/test-windows.ps1`
  - `/Users/maleick/Projects/TextQuest/scripts/check_dll_log.ps1`
  - `/Users/maleick/Projects/TextQuest/scripts/verify_injection.bat`

## Reference-Only Research Inputs

These are useful checklist sources, but not promotion authority for TextQuest:

- `/Users/maleick/Projects/forkvanilla`
- `/Users/maleick/Downloads/Archive`

Current useful findings from those reference trees:

- `forkvanilla` confirms the same high-level Test patch pattern:
  - real source churn is concentrated in `eqlib`
  - top-level patch commits mostly just move changelog text plus the `src/eqlib` submodule
- the archived MQ patch bundles consistently treat `eqgraphics.h` as reviewed unchanged for the April 7 Test patch
- `forkvanilla` needed `BuildType.h` aligned from `LIVE` to `TEST`
  - that is specific to MQ/forkvanilla feature gating, not a TextQuest offset promotion
- MQ also needed an evidence-backed `CXWnd` layout fix after this Test patch
  - that is relevant to TextQuest as a warning that UI/login layouts may drift even when raw globals do not
- archived `PlayerClient_verified.h` shows `PlayerManagerBase::m_PlayerList` at `+0x10`
  - this is useful as a structural clue if TextQuest has to re-prove the spawn-manager/player-list walk from local Ghidra

### Historical notes retained before deleting `forkvanilla`

The following items are preserved only as historical audit context. They must not be used as promotion authority for TextQuest offsets, layouts, symbols, or runtime behavior.

- Clean MQ baseline anchors from the `forkvanilla` audit:
  - `8a2c07da...`
  - `980f219...`
- Final MQ patch surface observed there for the April 7 Test patch:
  - `CXWnd.h`
  - `LoginFrontend.h`
  - `PlayerClient.h`
  - `eqgame.h`
  - `eqmain.h`
- Unresolved MQ-specific dump blocker recorded there:
  - minidump: `eqgame_20260410_015631.dmp`
  - crash site: `MQ2Main.dll+0x63A2C1`
  - matching PDB GUID required: `{B2B29694-7D7D-4BCD-8788-A627B2727831}`
  - current best candidate noted there: `MQ2WindowInspector.cpp` `IsEmptyValue(const char*)`
- Historical MQ scoping rule retained here as reference-only context:
  - do not reopen `MQ2Anonymize.cpp` unless the clean 5-file build reproduces the startup/YAML crash
- Historical MQ clean-clone reproducibility rule retained here as reference-only context:
  - `data/resources/ItemDB.txt` must be zero-byte

These notes are retained only so the historical MQ audit is not lost when `/Users/maleick/Projects/forkvanilla` is removed. TextQuest promotion authority remains repo-local Ghidra exports, the local GhidraMCP workflow, checked-in TextQuest code, and live runtime validation.

## Steps To Reproduce

1. Update the binaries in `TextQuest-Ghidra`.
   - Put fresh patch binaries in the appropriate variant folder.
   - For Test: `test/eqgame`, `test/eqmain`, `test/eqgraphics`
   - For Live: at minimum `live/eqgame`
2. Run the repo-local Ghidra harvest.
   - `cd /Users/maleick/Projects/TextQuest-Ghidra`
   - `python scripts/harvest_mcp.py test`
   - On Live patch day, run the Live variant too when the binaries are present.
   - If the Live binaries are not present, record the missing module names and keep the audit explicitly partial.
3. Regenerate the canonical evidence-repo snapshot.
   - `python3 /Users/maleick/Projects/TextQuest/scripts/collect_patch_evidence.py --manifest-out /Users/maleick/Projects/TextQuest-Ghidra/snapshots/<date>-patch-day/ghidra-manifest.json --copy-symbols /Users/maleick/Projects/TextQuest-Ghidra/snapshots/<date>-patch-day/symbols`
4. Rebuild `data/ghidra.db` for the current patch.
   - `cd /Users/maleick/Projects/TextQuest`
   - remove the old DB first because `import_ghidra` appends into an existing file:
     - `rm -f data/ghidra.db`
   - `cargo run --bin import_ghidra -- data/ghidra.db ../TextQuest-Ghidra/test/eqgame/ghidra-export/`
   - Validate:
     - `sqlite3 data/ghidra.db 'select count(*) from functions; select count(*) from globals; select count(*) from imports; select count(*) from strings; select count(*) from opcodes;'`
     - expected current April 8 Test snapshot: `23120 / 0 / 427 / 21980 / 0`
   - Current source for that debugger DB:
     - `../TextQuest-Ghidra/test/eqgame/ghidra-export`
5. Export byte-pattern candidates for scan-engine review.
   - Use `scripts/export_ghidra_patterns.py` after the Ghidra harvest has produced symbol records with byte windows.
   - Input records must include `name`, `bytes`, `category`, and `resolve`. `address` is recommended but not required; if it is missing, the exporter will still emit an entry with `expected_preferred: null`. Bare hex addresses such as `14028E0F0` (no `0x` prefix) are accepted. `--module` must be one of `EqGame`, `EqMain`, or `EqGraphics`, and `category` is normalized case-insensitively to `Function` or `Global`. RIP-relative entries should include `wildcards`, for example `[[3, 4]]`, so displacement bytes become `??` in the IDA pattern.
   - Example:
     - `python3 scripts/export_ghidra_patterns.py ../TextQuest-Ghidra/test/eqgame/ghidra-export/pattern-symbols.json --module EqGame --output /tmp/textquest-scan-entries.json`
   - Review the output before promotion. A generated pattern is evidence for scan-engine testing, not an automatic offset promotion.
6. Review the current patch checklist surfaces.
   - Use `eqlib` history as a triage checklist only.
   - Promote offsets only when local Ghidra or GhidraMCP proves them.
   - If `ZoneGuideManagerClient` is involved, verify the singleton/instance symbol explicitly. A nearby function symbol was close enough to cause a false promotion on 2026-04-09.
7. Sync the current TextQuest source to `frostreaver`.
   - Do not assume the remote coding workspace is current.
   - Use a fresh full tracked-tree sync, not a hand-picked file overlay.
   - `scripts/sync-to-frostreaver.sh` does not satisfy this requirement today; it only syncs config/data/Ghidra surfaces and does not ship the tracked TextQuest source tree.
   - Recommended:
     - `git archive --format=tar.gz -o /tmp/textquest-head.tar.gz HEAD`
     - `tar -czf /tmp/textquest-assets.tar.gz config/maps data/meshes data/ghidra.db`
     - copy both archives to `frostreaver`
     - extract the tracked tree into a fresh remote workspace
     - extract the ignored assets archive into that same workspace
   - Verify the remote source contains the latest `app.rs`, `event.rs`, `spawns.rs`, packet-hook files, and DLL startup changes before building.
8. Rebuild the Windows artifacts on `frostreaver`.
   - Preferred:
     - `powershell -ExecutionPolicy Bypass -File scripts/build-windows-release.ps1 -CopyToDesktop`
   - Manual fallback:
     - remove `target/release/build/recastnavigation-sys-*`
     - refresh Rust source mtimes under `textquest-common/src`, `textquest/src`, and `textquest-dll/src`
     - set `CMAKE_GENERATOR=Visual Studio 17 2022`
     - run `cargo +nightly build --release -p textquest -p textquest-dll`
9. Refresh the desktop test folder.
   - The build helper now owns `C:\Users\xmale\Desktop\TextQuest-Test`.
   - It copies:
     - `target/release/textquest.exe`
     - `target/release/textquest_dll.dll`
     - `data/ghidra.db`
     - `scripts/verify_injection.bat`
   - It also rewrites:
     - `01-dump.cmd`
     - `02-inject.cmd`
     - `03-ui.cmd`
     - compatibility wrappers `04-ui-workspace.cmd`, `05-inject-workspace.cmd`, `06-dump-workspace.cmd`
     - `TEST-ORDER.txt`
     - `README.txt`
   - Those launchers intentionally target the fresh desktop-drop EXE, not any workspace-root copy.
   - `C:\Users\xmale\Desktop\TextQuest-Test` is the authoritative Windows Test retest surface.
   - `README.md` still shows raw `target\release` examples; treat those as build-only guidance, not authoritative Test retest instructions.
   - Direct workspace-root launches or raw `target\release` retests are stale/unsafe for Test validation because they bypass the staged desktop drop.
   - Legacy helper debt remains: `scripts/test-windows.ps1`, `scripts/test_autologin.bat`, `scripts/test_single_login.bat`, and `scripts/launch_and_login.bat` still bypass the authoritative staged desktop drop and are unsafe for patch-day Test retests until updated.
10. Run the Test validation pass.

- Launch EverQuest and get fully in game.
- Run:
  - `C:\Users\xmale\Desktop\TextQuest-Test\02-inject.cmd`
  - `C:\Users\xmale\Desktop\TextQuest-Test\01-dump.cmd`
  - `C:\Users\xmale\Desktop\TextQuest-Test\03-ui.cmd`
- Optional:
  - `C:\Users\xmale\Desktop\TextQuest-Test\verify_injection.bat`
- In `4` Debug, verify the three-pane layout: EQ Internals, Hex, and Ghidra Explorer.
- Press `Enter` on an internals row and on a Ghidra row; both should load bytes into Hex.
- If Debug says `No ghidra.db loaded`, stop and fix the desktop drop or working directory before trusting the debugger.
- To inspect raw spawn memory, use `2` Map, highlight a spawn, then press `h` or `x`; Debug no longer owns the spawn list.
- In `5` Packets, verify at least one inbound or outbound row appears after inject. Use `Space` to pause/resume, `j/k` or arrows to change selection, `PgUp/PgDn` to scroll, and confirm the sidebar shows a payload preview.
- Do not describe PacketMonitor as filtered/debug-decoded unless an operator-facing filter UI is added; today it is a live capture table plus selected-packet preview.
- Review:
  - `logs/textquest-dump.log.*`
  - `logs/textquest.log.*`
  - `%TEMP%\\textquest\\textquest-dll.log*`
  - `%TEMP%\\textquest\\textquest-dll-init-*.log`
- Manual TUI attach now adopts the existing `login_token_{pid}.bin` during live-process scans, and pipe-open now retries the on-disk token path if the in-memory map is empty, so packet polling should no longer warn forever about `No session token for client` after a clean manual inject.
- Do not launch `C:\Users\xmale\Projects\TextQuest-...\\textquest.exe` or `textquest_dll.dll` directly from the workspace root; stale root-level copies caused false Test results on 2026-04-09.

11. Run the Live validation pass when Live binaries and a fresh interactive session exist.

- Repeat the same symbol harvest, evidence pack refresh, and desktop-drop validation against Live only after `live/eqgame`, `live/eqmain`, and `live/eqgraphics` are all available.
- If Live is still missing `eqmain` or `eqgraphics`, document that as the blocker instead of treating the Live workflow as complete.

12. Record what changed.

- offsets promoted
- Test and Live evidence coverage
- missing module names or empty harvest slots
- manifest timestamp source
- unresolved globals/functions
- runtime failures
- build fixes needed on Windows

## Patch-Day Runtime Read Completeness Verification (Zone/Target/Class/State/Stand/Map/Named)

Run this block after any Test patch refresh touching runtime offsets or spawn reads.

1. Mechanical verification (repo-local):
   - `cargo fmt --all -- --check`
     - if it fails, run `cargo fmt --all` and retry the check
   - `cargo test -p textquest-common offset_db -- --nocapture`
   - `cargo test -p textquest --bin textquest -- --nocapture`
   - `cargo build -p textquest -p textquest-dll`
   - `verify`

- For the Tactical map/demo slice, use the `Historical / Reference-Only Tactical Map Verification (2026-04-10)` section in this runbook as the canonical operator checklist.

2. Fresh in-world runtime capture:
   - Restart EQ client fully (do not reuse a previously injected process).
   - Inject once, then run `textquest.exe --dump`.
   - Collect:
     - `logs/textquest-dump.log.*`
     - `logs/textquest.log.*`
     - `%TEMP%\\textquest\\textquest-dll.log*`
   - Limitation: no immutable runtime capture for the repaired Test/demo Tactical map path has been archived yet. Treat the runbook plus a fresh operator capture as required before promotion.
3. Zone surface checks:
   - Confirm zone read source is visible in logs/traces (`inst_direct`, `inst_indirect`, `zone_guide`, `none`).
   - If source is `none`, verify direct/indirect candidate values were logged for diagnosis.
   - Verify a window-title fallback of `Unknown` does not overwrite a valid runtime zone name.
   - Confirm TUI zone display is not silently stuck at `Unknown`.
4. Target/self-target checks:
   - In dump output, verify target-read reason appears for each target sample (`none`, `invalid_pointer`, `read_failed`, `self_target_fallback`).
   - Toggle target states in game: no target, NPC target, self target.
   - Confirm Character panel target line is either populated or explicitly reason-labeled, never silently blank.
5. Class/state checks:
   - Confirm dump diagnostics show actor/direct/profile class sources and plausible local player class.
   - Confirm `textquest.exe client-status <pid>` prints zone name + class ID + stand-state ID.
   - Confirm `textquest.exe client-status <pid>` prints `Nav: ...` and nav signal lines (sanity that the shared snapshot includes nav state).
6. Stand checks:
   - In game, toggle stance states (`Stand`, `Sit`, `FD`, `DEAD`) and rerun `textquest.exe client-status <pid>`.
   - Confirm the stand-state ID changes with stance and that the TUI Characters panel stand label matches the live stance transitions.
7. Map checks:
   - Open the TUI Map screen (`2`).
   - Confirm the map header shows `Map geometry loaded` (not `Directory missing`, `Invalid zone name`, or a stale previous-zone status).
   - Confirm spawn overlays render and the selected-spawn panel updates when moving selection.
   - Toggle at least one map surface:
     - `:mapfilter named off` then `:mapfilter named on` (named overlay category should visibly change when named spawns exist)
     - `:mapmarker set patchday_sanity` then `:mapmarker list` (marker should appear in the list)
8. Named checks:
   - Enable named alerts: `:watch named on`
   - Confirm the alert feed is live:
     - `:alerts count` shows a plausible number after play activity
     - `:alerts clear` clears it
   - Confirm named tracking and map surfaces consume populated spawn + target + zone surfaces (no blank-only behavior while spawn list is valid).
9. Adjacent completeness checks (non-map/demo ownership):
   - Do not treat PacketMonitor or map zoom/depth regressions as part of this slice unless they are direct consequences of runtime-read failures.

## 2026-04-09 Build Hygiene Note

- A partial remote overlay left `frostreaver` on a mixed source tree even though the desktop drop was fresh.
- The symptom was misleading: the Windows desktop artifacts had new timestamps, but the running app still showed the pre-Explorer Debug spawn list, did not auto-load `ghidra.db`, and the DLL still had the stub packet-hook path.
- The fix was a clean full-tree sync into `C:\\Users\\xmale\\Projects\\TextQuest` before rebuilding.
- The Windows packet-hook build also required replacing the `retour::static_detour!` packet hooks with `retour::GenericDetour`; otherwise MSVC release builds recursed indefinitely during macro expansion.
- The same Test pass also caught a symbol-selection error:
  - `ZoneGuideManagerClient__Instance_x` is `0x1403572C0`
  - TextQuest had incorrectly promoted `0x1403571F0`, which is a nearby non-singleton symbol

## Current Blind Spots

- No local proof yet for the current Test `pinstSpawnManager`
- No local proof yet for the current Test `pinstLocalPlayer`
- No local proof yet for several other `eqgame` globals and hook entry points
- No local `Geyser` dump artifact is currently available in the workspace for comparison
- No fully autonomous credential path is set up yet on `frostreaver`
- The currently running EQ process on `frostreaver` still holds whatever DLL version was injected before the last rebuild.
  - when DLL-side layout changes land, take a clean EQ restart before reinjecting if you want to validate shared memory or live DLL state

## Current Runtime Note

As of this document update:

- the refreshed Windows `textquest.exe` was rebuilt and copied to the desktop test folder
- the refreshed Windows `textquest_dll.dll` with the newer thread-pool handoff and early breadcrumbs was rebuilt and copied to the desktop test folder
- the desktop test folder now has repo-generated launchers that target the fresh desktop artifacts instead of stale workspace-root copies
- a fresh in-world Test retest is still required after these rebuilt artifacts because `eqgame.exe` was not running at the time of the rebuild/package step
- the next Test retest should confirm whether `instEQZoneInfo` is direct or pointer-indirect on the current patch; both shapes are now logged in dump mode
