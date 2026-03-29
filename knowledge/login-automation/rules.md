# Login Automation — Confirmed Rules

## R1: Use inline IPC handler for credential entry, not game loop
- eqmain.dll has its own event loop — game loop hook doesn't fire during login
- Credential writes must happen inline on the IPC thread
- Button clicks must be queued to game loop (after eqmain unloads)

## R2: Clone CStrRep freeList pointer from donor
- When creating a new CStrRep for an empty CXStr (e.g., password field), copy the freeList pointer from an existing CStrRep (e.g., username field)
- Without this, EQ crashes on deallocation because it uses the freeList for its custom allocator

## R3: Use SidlText for window identification in eqgame.exe
- WindowText may not contain the expected text at character select
- SidlText (CSidlScreenWnd::SidlText at +0x270) is the reliable SIDL identifier
- Example: "CharacterListWnd" for character list, "ServerListWnd" for server list
