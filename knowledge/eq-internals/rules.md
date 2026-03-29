# EQ Internals — Confirmed Rules (Apply by Default)

## R1: Always use HeapAlloc for CStrRep allocation
- **Rule**: Never use Rust's allocator for CStrRep — always use `HeapAlloc(GetProcessHeap(), ...)`
- **Why**: EQ manages CStrRep lifecycle and will crash if the buffer is on a different heap
- **Confirmations**: 5+ (every credential entry uses this)
- **Promoted from**: Direct observation (2026-03-29)

## R2: Queue UI mutations to game loop thread
- **Rule**: Never call WndNotification, EnterWorld, or any UI function from the IPC thread
- **Why**: EQ's UI is single-threaded — cross-thread UI calls crash eqgame.exe
- **Confirmations**: 3 (button click crash, EnterWorld peer review, MQ2 reference)
- **Promoted from**: H-crash-ipc-thread (2026-03-29)

## R3: Use eqgame.exe offsets for eqgame.exe windows, eqmain.dll offsets for eqmain.dll windows
- **Rule**: Never mix CXWndManager offsets between DLLs — they have different struct layouts
- **Why**: eqmain.dll pWindows at +0x010, eqgame.exe pWindows at +0x008
- **Confirmations**: 2 (Phase 3 bug fix, MQ2 source verification)
- **Promoted from**: Phase 3 root cause analysis (2026-03-29)
