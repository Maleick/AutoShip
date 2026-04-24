# Result: #950 — Verify/update packet scrambler offset for client 20260310

Status: DONE

Changes Made:
- Updated `OFFSET_PACKET_SCRAMBLER`, `OFFSET_HTON`, and `OFFSET_NETWORK_SEND` in `textquest-common/src/offsets.rs` for the 20260310 live client.
- Updated the active-hack offset comment with the verification date and canonical TextQuest-Ghidra snapshot.
- Added active-hack IDA scan entries in `textquest-common/src/pattern_db.rs` for the packet scrambler global, opcode hton helper, and network send function.

Tests:
- `cargo check -p textquest-common` passed.

Notes:
- Verification used the TextQuest-Ghidra `2026-04-11-live-working` snapshot and `staging/live/2026-04-11/eqgame.exe`, which reports `Mar 10 2026`.
- GhidraMCP was not running locally, so verification used exported Ghidra metadata plus direct PE pattern scans/disassembly.
- `cargo fmt --check` was attempted and failed on pre-existing unrelated formatting drift outside this issue (`textquest/src/tui/*`, `textquest-web/src/api/*`); touched files were formatted directly and pass `git diff --check`.

COMPLETE
