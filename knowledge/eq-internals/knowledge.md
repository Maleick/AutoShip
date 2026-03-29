# EQ Internals — Facts & Patterns

## CXWndManager Layout Differences
- **eqmain.dll**: `pWindows` ArrayClass at `+0x010`, count at `+0x018`
- **eqgame.exe**: `pWindows` ArrayClass at `+0x008`, count at `+0x010`
- Source: MQ2 CXWnd.h line 1121, confirmed by hex dump calibration (2026-03-29)

## CXStr / CStrRep Structure
- CXStr = single pointer to CStrRep (8 bytes on x64)
- CStrRep layout: refCount(i32@0x00), alloc(u32@0x04), length(u32@0x08), encoding(u32@0x0c), freeList(usize@0x10), data(bytes@0x18)
- CStrRep MUST be allocated with HeapAlloc (Windows process heap), not Rust allocator — EQ manages CStrRep lifecycle
- Source: reverse engineering + MQ2 reference (2026-03-29)

## CSidlScreenWnd
- `SidlText` at `+0x270` — the SIDL window name string (CXStr)
- Used to identify windows: "CharacterListWnd", "ServerListWnd", etc.
- Source: MQ2 CXWnd.h line 912

## CXWnd Vtable (eqmain.dll)
- `WndNotification` at vtable offset `+0x110`
- `SetWindowText` at vtable offset `+0x280` (DOES NOT WORK in eqmain.dll — wrong function)
- Source: calibrated live (2026-03-29)

## Thread Model
- eqmain.dll has its OWN event loop — game loop hook (ProcessGameEvents) does NOT fire during login screen
- All UI/game-state mutations must happen on the game loop thread
- IPC thread can set atomics that the game loop picks up on next tick
- Button clicks crash EQ if called from IPC thread — use PENDING_BUTTON_CLICK atomic pattern
- Source: confirmed by crash testing (2026-03-29)

## DirectInput
- EQ uses DirectInput (IDirectInput8A) for keyboard — all PostMessageW/SendInput simulation fails
- WM_CHAR, VK_RETURN, WM_KEYDOWN are all ignored by EQ
- Source: confirmed by testing (2026-03-29)
