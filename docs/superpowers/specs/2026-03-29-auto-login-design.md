# Auto-Login Design Spec

## Goal

Fully automated login from EQ process launch to in-world: credential entry, server selection, character selection, and Enter World — all driven by the injected DLL with orchestrator providing credentials via IPC.

## Approach: Widget Manipulation (MQ2-style)

Uses EQ's SIDL UI widget tree to find windows by XML name, set text fields, and click buttons. This is the same approach MQ2AutoLogin uses — widget names are stable across patches.

For server selection, uses `LoginServerAPI::JoinServer(serverID)` directly (bypasses UI list). For character selection, uses `CCharacterListWnd::SelectCharacter()` + `EnterWorld()`.

## Architecture

### DLL-side Login FSM

The DLL owns the login state machine. The orchestrator sends credentials once; the DLL handles all UI steps autonomously and reports progress back.

```
Idle → WaitForLoginScreen → EnteringCredentials → WaitForServerSelect
  → SelectingServer → WaitForCharSelect → SelectingCharacter → WaitForWorld → InWorld
```

Error states: `ErrorDialog` (wrong password, server down, etc.) with retry logic.

### IPC Changes

New commands in `dmft-common/src/ipc.rs`:

```rust
// Orchestrator → DLL
Command::StartLogin {
    account_name: String,
    password: String,      // Zeroized after use
    server_name: String,   // e.g. "Firiona Vie"
    character_name: String,
}

// DLL → Orchestrator (already exists, extend usage)
Response::LoginPhaseUpdate { phase: LoginPhase }
```

### eqmain.dll Discovery

The DLL must find eqmain.dll at runtime since it's loaded into the same process:

1. `GetModuleHandleW("eqmain.dll")` → base address
2. Resolve global pointers relative to eqmain base:
   - `pinstCSidlManager` at base + 0x3824C0
   - `pinstLoginServerAPI` at base + 0x17F4D0
   - `pinstCXWndManager` at base + 0x3824B8

### Widget Names (from EQ XML skins, stable)

| Widget | XML Name | Type | Purpose |
|--------|----------|------|---------|
| Username | `LOGIN_UsernameEdit` | CEditWnd | Account name field |
| Password | `LOGIN_PasswordEdit` | CEditWnd | Password field |
| Login button | `LOGIN_ConnectButton` | CButtonWnd | Submit credentials |
| Server list | `SERVERSELECT_ServerList` | CListWnd | Server listing |
| Character list | `Character_List` | CListWnd | Character listing |
| OK dialog | `okdialog` | CStmlWnd | Error/info dialogs |
| Yes/No dialog | `yesnodialog` | CXWnd | Confirmation dialogs |
| Splash screens | `dbgsplash`, `soesplash` | CXWnd | Dismiss on click |

### UI Manipulation Methods

**Set text in CEditWnd:**
```rust
// Write directly to CEditBaseWnd::InputText at offset 0x278
// InputText is a CXStr (EQ's string type)
unsafe {
    let input_text_ptr = edit_wnd_ptr + 0x278;
    write_cxstr(input_text_ptr, text);
}
```

**Click a button:**
```rust
// Call CXWnd::WndNotification(sender, XWM_LCLICK, data)
// XWM_LCLICK = 1
unsafe {
    let vftable = *(button_ptr as *const *const usize);
    let wnd_notification = *vftable.add(WNDNOTIFICATION_VFUNC_INDEX);
    let func: fn(*mut u8, *mut u8, u32, *mut u8) = transmute(wnd_notification);
    func(button_ptr, button_ptr, 1, null_mut());
}
```

**Find window by name:**
```rust
// Navigate CSidlManager's window map
// CSidlManager::FindScreenPieceTemplate(name) or
// CXWnd::GetChildItem(xml_data_mgr, name)
unsafe {
    let sidl_mgr = *(eqmain_base + 0x3824C0) as *mut CSidlManager;
    let param_mgr = sidl_mgr_get_param_manager(sidl_mgr);
    let wnd = get_child_item(parent_wnd, param_mgr, "LOGIN_PasswordEdit");
}
```

**Server selection (direct API):**
```rust
unsafe {
    let login_api = *(eqmain_base + 0x17F4D0) as *mut LoginServerAPI;
    // Find server ID by iterating LoginClient::ServerList at offset 0x178
    let join_server: fn(*mut u8, i32, *mut u8, i32) -> u32 =
        transmute(eqmain_base + 0x18050);
    join_server(login_api, server_id, null_mut(), 10);
}
```

### Screen Detection

The DLL detects which screen is active by checking:

1. **Game state enum**: EQ has a global `gGameState` — `GAMESTATE_PRECHARSELECT` (login/server/charselect) vs `GAMESTATE_CHARSELECT` vs `GAMESTATE_INGAME`
2. **Window visibility**: Check if specific windows are visible (e.g., `LOGIN_ConnectButton` visible = login screen)
3. **Local player pointer**: Non-null = in world

### Login Flow

```
1. Orchestrator spawns eqgame.exe
2. Orchestrator injects DLL
3. DLL hooks game loop, starts IPC listener
4. Orchestrator sends StartLogin { account, password, server, character }
5. DLL stores credentials in memory (zeroized on use)

6. DLL detects login screen (LOGIN_ConnectButton visible):
   a. Find LOGIN_UsernameEdit, set InputText = account_name
   b. Find LOGIN_PasswordEdit, set InputText = password
   c. Find LOGIN_ConnectButton, send XWM_LCLICK
   d. Zero password from memory
   e. Report LoginPhaseUpdate(EnteringCredentials)

7. DLL detects server select screen:
   a. Find server by name in LoginClient::ServerList
   b. Call LoginServerAPI::JoinServer(server_id)
   c. Report LoginPhaseUpdate(ServerSelecting)

8. DLL detects character select screen:
   a. Find character by name in Character_List CListWnd
   b. Call SelectCharacter(index)
   c. Wait 1-2 seconds (human-like delay)
   d. Call EnterWorld()
   e. Report LoginPhaseUpdate(CharacterSelecting)

9. DLL detects local_player != null:
   a. Report LoginPhaseUpdate(InWorld)
   b. Login FSM transitions to idle

10. Orchestrator receives InWorld, starts PostLoginSequencer
```

### Error Handling

| Error | Detection | Response |
|-------|-----------|----------|
| Wrong password | `okdialog` visible with error text | Dismiss, report LoginError::WrongPassword |
| Account locked | `okdialog` with "locked" text | Report LoginError::AccountLocked, stop |
| Server down | Server status flags | Wait and retry (30s intervals) |
| Server full | Server status flags | Wait and retry |
| Character not found | Name not in list | Report LoginError::CharacterNotFound |
| Timeout | No progress for 60s | Retry from current phase, max 3 attempts |
| "Already logged in" | `yesnodialog` visible | Click Yes to kick existing session |

### Security

- Password stored in DLL memory only between receiving `StartLogin` and clicking Login button
- Zeroized immediately after setting the CEditWnd text
- IPC pipe has DACL restricted to current user SID (already implemented)
- No password logging (tracing skips password fields)

## Files to Create/Modify

### New files
- `dmft-dll/src/login/mod.rs` — DLL-side login FSM
- `dmft-dll/src/login/eqmain.rs` — eqmain.dll discovery + widget access
- `dmft-dll/src/login/widgets.rs` — CEditWnd/CButtonWnd/CListWnd manipulation

### Modified files
- `dmft-common/src/ipc.rs` — Add `StartLogin` command
- `dmft-common/src/offsets.rs` — Add eqmain.dll offsets
- `dmft-dll/src/hooks/game_loop.rs` — Integrate login FSM tick
- `dmft-dll/src/ipc/mod.rs` — Handle `StartLogin` command
- `dmft/src/launcher/coordinator.rs` — Send credentials via IPC after injection

## Testing Strategy

- Unit tests for login FSM state transitions (macOS-compatible with stubs)
- Integration test: mock IPC command → FSM processes → correct phase updates emitted
- Live test on frostreaver: full login cycle with one account
- Then scale to 6-account test

## Open Questions

1. **eqmain.dll offset validation**: The offsets from MQ2 headers (CSidlManager at 0x3824C0, etc.) need validation against the March 10, 2026 EQ build. May need adjustment.
2. **CXStr memory layout**: Need to verify EQ's string type layout for safe writes. MQ2 uses its own CXStr wrapper.
3. **GameState detection**: Need to find the gGameState global pointer in eqgame.exe offsets.
