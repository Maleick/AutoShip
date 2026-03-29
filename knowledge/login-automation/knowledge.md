# Login Automation — Facts & Patterns

## Working Login Chain (Phases 1-3)
1. Launch with `patchme /login:accountname` — bypasses EULA
2. DLL injection → find eqmain.dll CXWndManager
3. Write credentials to CEditWnd::InputText via CXStr direct write
4. Click Login button via WndNotification(XWM_LCLICK) vtable call
5. Wait for server select → click PLAY EVERQUEST! button
6. Wait for eqmain.dll unload → find CharacterListWnd by SidlText
7. Queue EnterWorld() to game loop thread

## Confirmed Working Methods
| Method | Status | Notes |
|--------|--------|-------|
| CXStr direct write to InputText +0x278 | WORKS | When CXStr is non-null |
| HeapAlloc CStrRep clone for password | WORKS | Clones from donor CStrRep |
| WndNotification(XWM_LCLICK) via vtable | WORKS | Must be on correct thread |
| CXWndManager window enumeration | WORKS | "2 before label" heuristic |
| `/login:account` command-line flag | WORKS | Bypasses EULA |
| SidlText window lookup at +0x270 | UNTESTED | New approach for Phase 3 |

## Does NOT Work
| Method | Status | Notes |
|--------|--------|-------|
| SetWindowText vtable 0x280 | WRONG FUNC | eqmain.dll vtable differs |
| PostMessageW WM_CHAR | IGNORED | EQ uses DirectInput |
| PostMessageW VK_RETURN | IGNORED | Same — DirectInput |
| EQLogin char array write alone | NO UI REFRESH | UI reads from CEditWnd |
| `/enterworld` slash cmd | NO EFFECT | Not recognized at char select |
