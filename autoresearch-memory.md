# Autoresearch Memory

## Run

- Run id: `debug-packets-2026-04-09`
- Goal: make the Debug screen, ghidra.db Explorer, and PacketMonitor usable for live Test debugging
- Verify command:

```bash
cargo test -p textquest-common offset_db -- --nocapture && \
cargo test -p textquest --bin textquest -- --nocapture && \
cargo build -p textquest -p textquest-dll
```

## Current Slice (2026-04-10): Test Runtime Data Completeness

- Goal: stabilize runtime-read completeness for zone, target/self-target, class, and stand-state surfaces in Test-facing TUI/CLI output.
- Proof artifacts consulted:
  - `/Users/maleick/Projects/TextQuest-Ghidra/test/harvest-info.json` (`2026-04-09T01:42:16Z`)
  - local runtime-diagnostics docs in `docs/wiki/Research-Test-Offset-Reconciliation.md`
  - local patch-day runbook in `docs/wiki/Research-Patch-Day-Reproduction.md`
- Implemented in this slice:
  - Added target-read diagnostic reason states (`none`, `invalid_pointer`, `read_failed`, `self_target_fallback`) in external spawn reader paths.
  - Added zone-read source diagnostics (`inst_direct`, `inst_indirect`, `zone_guide`, `none`) with explicit unresolved candidate logging.
  - Hardened zone-name normalization so placeholder `Unknown` does not masquerade as valid source data.
  - Prevented unresolved selected-client zone reads from stalling on `Unknown` by forcing a re-read while unresolved.
  - Prevented window-title `Unknown` values from overwriting a valid runtime zone name.
  - Updated TUI character summary and CLI dump/status output to make target/zone/class/stand degradation visible.
- What still needs live proof:
  - Fresh in-world Test capture that exercises `self_target_fallback` on the current patch binary.
  - Fresh confirmation that map/named completeness improves when zone/target reads stop collapsing to unresolved states.
  - Fresh Windows proof that PacketMonitor shows rows after pipe-open recovers the on-disk session token.
- Verification command set for this slice:
  - `cargo fmt --all`
  - `cargo test -p textquest-common offset_db -- --nocapture`
  - `cargo test -p textquest --bin textquest -- --nocapture`
  - `cargo build -p textquest -p textquest-dll`
  - `/verify` if available; this run confirms availability at `/Users/maleick/.local/bin/verify`.

## Current Slice (2026-04-10): Test Map/Data Completeness and Tactical Map UX

- Goal: make Test and Mac demo map behavior explicit, predictable, and easier to verify on patch day.
- Root cause confirmed:
  - `config/maps` currently only contains `eastwastes`, `ecommons`, `freportw`, `greatdivide`, and `permafrost` assets.
  - `crescent` map files are absent locally, which explains the missing or partial outline/overlay behavior when the UI expects those zones.
  - Mac demo bounds clamping skips zones without loadable map data; before this slice, that path was only debug-logged.
- Implemented in this slice:
  - Tactical map load status now distinguishes loaded geometry from point-only overlays that have no drawable linework.
  - Tactical map view now shows explicit banners for hidden geometry, missing geometry, and unavailable navmesh data.
  - Demo map bounds loading now warns when map data is missing so Mac demo regressions are visible in logs.
  - Tactical map reset remains on `r`; `[` and `]` stay reserved for previous/next client navigation.
  - Map help/status text already reflects the current `+ / -`, `< / >`, and `r` bindings.
- Current blocker:
  - Full map asset restoration is still blocked on an approved `crescent` map source. The code now fails visibly instead of pretending the data exists.
  - No immutable runtime capture was archived for this slice; proof is currently mechanical verification plus the patch-day runbook, not a saved end-to-end runtime artifact.
- Verification command set for this slice:
  - `cargo fmt --all`
  - `cargo build -p textquest`
  - `cargo test -p textquest -- --nocapture`
  - Manual Test and Mac demo runtime checks merged into `docs/wiki/Research-Patch-Day-Reproduction.md` under the historical/reference-only Tactical map verification section

## Retained Learning

- Historical frostreaver runtime notes cited below are not backed by archived raw dump/log artifacts. Treat them as operator notes, not archived proof, until `TextQuest-Ghidra/snapshots/<date>-runtime-proof/` contains the raw bundle. Tracking issue: `Maleick/TextQuest#713`.

- The first stable Test correction is not `pinstLocalPlayer`; it is the `LocalPC -> me` chain.
- Current local proof:
  - `PINST_LOCAL_PC = 0x140EA9A68`
  - `character_zone::ME = 0x2848`
- That proof comes from `TextQuest-Ghidra/test/eqgame/decompiled/140309ad0_FUN_140309ad0.c`, where the player constructor stores the created player pointer at `LocalPC + 0x2848`.
- The same global at `0x140EA9A68` also appears in the send-path research. Keep the name ambiguity explicit in docs until the owning object is fully resolved.
- Current Test `eqmain` login globals appear shifted by `+0x1010` versus the prior baseline:
  - `LOGIN_SERVER_API = 0x1801804E0`
  - `PINST_LOGIN_CLIENT = 0x1801804F0`
  - `PINST_LOGIN_CONTROLLER = 0x180180500`
- `eqlib` remains useful for patch triage, but not for final promotions. The recurring checklist is still `eqgame.h`, `eqmain.h`, and the player/UI/login header cluster.
- The orchestrator warning `No session token for client` is not a direct read of `%TEMP%/textquest/login_token_{pid}.bin`; it comes from the orchestrator's in-memory `session_tokens` map. Treat it as a registration symptom, not as proof that the token file is missing.
- The DLL-side progress marker is `%TEMP%/textquest/token_{pid}.bin`. If that file still exists after a supposedly successful inject, the DLL did not reach `generate_session_token()` and therefore did not start authenticated IPC for that session.
- The current debug build now writes `%TEMP%/textquest/textquest-dll-init-{pid}.log` breadcrumbs so we can distinguish `callback_enter`, `initialize_enter`, `tracing_initialized`, `session_token_ready`, `initialize_done`, `already_initialized`, `initialize_err`, and `initialize_panic`.
- We added earlier attach-stage breadcrumbs too:
  - `dll_process_attach`
  - `thread_library_calls_disabled`
  - `thread_pool_submitted`
  - `thread_pool_submit_err`
  - `dll_process_detach`
- The current DLL handoff no longer closes the one-shot thread-pool work item immediately after submit. Leaking that single work item for process lifetime is acceptable and avoids losing the callback before any init breadcrumb is written.
- Remote launch transport is now proven:
  - direct SSH `Start-Process eqgame.exe patchme` lands in session 0
  - an SSH-created interactive scheduled task lands GUI processes in console session 1
  - that pattern was proven with both `notepad.exe` and `eqgame.exe patchme`
- For future autonomous tests, the correct launch shape is to trigger a desktop-session task that runs `textquest.exe autologin --spawn ...` locally on frostreaver.
- Current autonomy blocker on frostreaver is credentials, not launch code:
  - `config/accounts.toml` exists
  - `data/credentials.db` was absent in the checked workspaces
  - `TEXTQUEST_MASTER_PASSWORD` was not set in the probed environment
- The remote coding workspace on frostreaver can drift behind the local repo. Before trusting any Windows runtime result, verify the remote source contains the latest `cli.rs`, `offsets.rs`, and DLL startup changes.
- The frostreaver workspace root can also contain stale `textquest.exe` / `textquest_dll.dll` copies even after `target/release` and the desktop drop are refreshed. If a Windows run prints `Using DLL: C:\Users\xmale\Projects\TextQuest-...\textquest_dll.dll`, that run is using the stale root-level binaries, not the fresh release build.
- The patch-day-safe fix is now in `scripts/build-windows-release.ps1`: `-CopyToDesktop` rewrites the `Desktop\\TextQuest-Test` launchers so they always execute the fresh desktop-drop EXE instead of a workspace-root copy.
- `C:\Users\xmale\Desktop\TextQuest-Test` is the authoritative Windows Test retest surface. `README.md` still shows raw `target\release` CLI examples, but those are build-only/workspace guidance and should not be used as Test-retest authority.
- Direct workspace-root launches and raw `target\release` retests are stale/unsafe for Test validation because they bypass the staged desktop drop.
- `scripts/sync-frostreaver.ps1` is unsafe for `test`-branch parity because it runs `git pull origin master` on frostreaver; do not use it when branch fidelity matters.
- The same desktop packaging step must also ship `data/ghidra.db`; otherwise the Windows Debug explorer will fall back to `No ghidra.db loaded` even if the local repo DB is current.
- The current clean debugger DB is rebuilt from `TextQuest-Ghidra/test/eqgame/ghidra-export` only after deleting the old `data/ghidra.db` first; `import_ghidra` appends into an existing file.
- A fresh desktop drop is still not enough if the remote source tree is mixed. On 2026-04-09, `frostreaver` had fresh desktop timestamps but stale `app.rs`, `event.rs`, `spawns.rs`, and packet-hook files because the sync only overlaid selected files into a non-git workspace.
- Patch-day-safe remote rebuilds must start from a full tracked-tree sync plus a second ignored-assets archive for `config/maps`, `data/meshes`, and `data/ghidra.db`.
- The manual TUI attach path needs file-backed token adoption. `Orchestrator::register_client()` now prefers an existing `login_token_{pid}.bin`, and `tui/run.rs` seeds orchestrator tokens from disk during live-process scans so packet polling does not spam `No session token for client` when a DLL was already injected manually.
- The stable frostreaver release-build fix is:
  - delete `target/release/build/recastnavigation-sys-*`
  - set `CMAKE_GENERATOR=Visual Studio 17 2022`
  - run `cargo +nightly build --release -p textquest -p textquest-dll`
- The frostreaver sync/build loop has an additional trap: syncing source files via `tar` preserved older mtimes, and Cargo skipped rebuilding `textquest_dll` even when the file contents changed.
  - `scripts/build-windows-release.ps1` now refreshes Rust source mtimes under `textquest-common/src`, `textquest/src`, and `textquest-dll/src` before the release build so the desktop DLL actually updates.
- The current Test actor/class evidence is stronger than the old direct `PlayerZoneClient::CharClass` read:
  - `PlayerClient::mActorClient` is at `0x0FE0` in the current reference headers
  - `ActorBase::Race`, `RaceOverride`, and `Class` imply absolute player offsets `0x0FF4`, `0x0FF8`, and `0x0FFC`
  - a historical frostreaver runtime note reported `actor_class=3` and `direct_class=0` for the local player, and local-player class now resolves to `PAL`
  - `PcProfile::Class = 0x17CC` is the current fallback when actor/direct class is unavailable
- The current Test zone surface is still partially unresolved:
  - `instEQZoneInfo` direct short/long strings were blank in the live dump even while `LocalPC->me` and spawn enumeration were valid
  - the EXE and DLL now try both direct and pointer-indirect `instEQZoneInfo` reads before falling back to `ZoneGuideManagerClient`
  - the dump path now logs both direct and indirect zone-name candidates so the next in-world run can prove which shape Test is using
- `/verify` is available in this run environment as `/Users/maleick/.local/bin/verify`; mechanical verification should include it and capture any fail list before declaring this slice complete.
- Historical frostreaver runtime notes before EQ exited reported:
  - `pinstLocalPlayer = 0`
  - `pinstLocalPC` and `LocalPC->me` valid
  - `pinstSpawnManager = 0`
  - linked-list fallback enumerated `688` spawns successfully
  - many NPC class reads improved from `?c0?` to plausible values like `WAR`, `SHM`, and `WIZ`
- The Debug screen no longer owns a spawn list; Map owns spawn selection and `h`/`x` opens raw spawn memory into Hex.
- `App::load_ghidra_database()` searches the current directory and executable ancestors for `data/ghidra.db`.
- Current DB snapshot is `23120` functions, `0` globals, `427` imports, `21980` strings, `0` opcodes.
- Because `globals` and `opcodes` are empty, the Ghidra Explorer is row-oriented but still function-heavy, and EQ Internals continues to use compiled offsets.
- Packet capture now hooks `send`, `recv`, `WSASend`, and `WSARecv`, and forwards `payload_preview` bytes over IPC.
- The latest concrete Test zone bug was a symbol-selection error, not just a missing proof:
  - wrong TextQuest value: `0x1403571F0`
  - correct `openvanilla` Test singleton: `ZoneGuideManagerClient__Instance_x = 0x1403572C0`
  - `zoneGuideManager` now belongs in globals, not functions.
- The packet-hook implementation now needs to stay on `retour::GenericDetour` for Windows release builds. The `static_detour!` version compiled locally on non-Windows but blew up under MSVC release recursion during the real `frostreaver` rebuild.
- PacketMonitor operator behavior is: `Space` pause/resume, `j/k` or arrows select, `PgUp/PgDn` scroll, `c` clear, and the right sidebar shows the selected preview bytes.
- PacketMonitor polling now has two recovery/diagnostic layers:
  - live-process scans seed in-memory session tokens from disk
  - `Orchestrator::get_pipe()` retries the on-disk token path if in-memory state is empty
- The map reset control is now plain `r` in addition to the platform-specific `Home` key, so Mac keyboards are no longer blocked from resetting the tactical viewport.
- Tactical map controls are:
  - `<` / `>` depth
  - `+` / `-` zoom
  - `r` / `Home` reset
  - `v` viewport cycle
  - `n` navmesh toggle
  - `A` self-target
  - `[` / `]` still switch clients globally
- Tactical-screen `r` must stay reserved for map reset, not repeat-command replay.
- Map discovery now probes CWD ancestors, executable ancestors, and macOS bundle-style `Contents/Resources/config/maps`.
- Packet hook installation needs to live on the normal DLL initialize path, not just a constructor retry, so runtime logs and retries stay visible during real injects.
- A canonical evidence snapshot is now generated in `TextQuest-Ghidra/snapshots/<date>-patch-day/` via:
  - `python3 /Users/maleick/Projects/TextQuest/scripts/collect_patch_evidence.py ...`
- The canonical historical/reference-only Test snapshot path for this thread is `/Users/maleick/Projects/TextQuest-Ghidra/snapshots/2026-04-09-patch-day/ghidra-manifest.json` plus `/Users/maleick/Projects/TextQuest-Ghidra/snapshots/2026-04-09-patch-day/symbols/`; `TextQuest` keeps runbook pointers only.
- The app-local Test debugger DB at `/Users/maleick/Projects/TextQuest/data/ghidra.db` is a mutable runtime/debug cache derived from `/Users/maleick/Projects/TextQuest-Ghidra/test/eqgame/ghidra-export` with observed counts `23120 / 0 / 427 / 21980 / 0`; under current policy it is documented only as reference context and is not retained as a long-term Test DB artifact.
- No local artifact matching `geyser dump` was found under `/Users/maleick/Projects`; use the repo-local Ghidra exports as the reproducible symbol/evidence source until that dump exists locally.
- Reference-only local patch bundles also exist outside this repo:
  - `/Users/maleick/Projects/forkvanilla`
  - `/Users/maleick/Downloads/Archive`
- High-signal reference findings from those trees:
  - the MQ/forkvanilla April 7 Test patch set still treats `eqgraphics.h` as reviewed unchanged
  - `BuildType.h` had to be aligned from `LIVE` to `TEST` there so Test-only feature gates behaved correctly
  - MQ needed an evidence-backed `CXWnd` layout correction (`WindowText` drift) to stop startup/UI crashes after the same Test patch
  - archived `PlayerClient_verified.h` shows `PlayerManagerBase::m_PlayerList` at `+0x10`, which is useful as a structure clue if we need to re-prove the spawn-manager walk from local Ghidra
- Those external sources are still checklist/reference material only. They can suggest what to inspect next, but they do not override local `TextQuest-Ghidra` or GhidraMCP proof when promoting TextQuest offsets or layouts.
- The only `forkvanilla` material worth retaining in TextQuest is short historical MQ audit context:
  - clean baseline anchors
  - the five-file MQ patch surface
  - the unresolved `MQ2Main.dll+0x63A2C1` dump blocker and matching-PDB note
- Once those notes are preserved in TextQuest docs, `/Users/maleick/Projects/forkvanilla` is safe to remove.

## Current Blind Spots

- No direct local-Test proof yet for `PINST_SPAWN_MANAGER` or `spawn_manager::PLAYER_LIST`
- No direct local-Test proof yet for `PINST_CDISPLAY`, `PINST_CEVERQUEST`, or `PINST_CXWND_MANAGER`
- Several `eqgame` hook functions still exist only as older compile-time baselines until locally rebound
- No secure unattended master-password delivery path yet for `textquest autologin --spawn`
- No fresh Windows proof yet that PacketMonitor shows live traffic from the rebuilt desktop drop
- No fresh Windows proof yet that the new pipe-open token recovery path eliminates empty Packet tabs after manual attach
- No populated `globals` or `opcodes` tables yet in `data/ghidra.db`
- No fresh in-world proof yet for the new indirect `instEQZoneInfo` fallback because the live EQ process exited before the rebuilt desktop drop could be re-tested
- The current patch-day manifest still lacks a populated top-level `generated_at`; use the filename plus `harvest_info.harvested_at` for audit timestamps.
- Live patch-day coverage is still partial because `TextQuest-Ghidra` has `live/eqgame` only; `live/eqmain` and `live/eqgraphics` are still missing.

## Re-anchor Commands

```bash
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py check
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /get_function_by_address --query address=ram:1405654e0
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /get_xrefs_to --query address=ram:140ea9a68 --query limit=5
python3 /Users/maleick/Projects/ghidra-mcp-skill/scripts/ghidra_mcp.py call GET /disassemble_function --query address=ram:1402792b0
```

## Next Move

Use a desktop-session trigger on frostreaver for future autonomous tests. Direct SSH launch is now proven to be session 0 only. If credentials are provisioned, the preferred command path is `textquest.exe autologin --spawn ...` launched via an interactive scheduled task; otherwise continue with manual in-world login plus SSH-driven inject/dump/UI while the credential path remains unresolved.
