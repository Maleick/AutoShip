# WinEQ 2022 Research Notes

Research into [LavishSoftware/JMB-WinEQ-2022](https://github.com/LavishSoftware/JMB-WinEQ-2022) — the open-source WinEQ 2022 agent for Joe Multiboxer (JMB). This documents techniques relevant to Frostreaver's 36-client multibox setup.

## Key Finding: WinEQ 2022 is a Thin Agent

WinEQ 2022 is **not** a standalone application. It's a LavishScript/ISS agent that runs inside the Joe Multiboxer (JMB) runtime. The heavy lifting — process injection, Direct3D hooking, window management APIs — lives in **JMB's closed-source core** (Inner Space / Lavish platform). WinEQ 2022 is essentially a configuration layer and GUI that calls into JMB's built-in primitives.

This means: the *techniques* are visible in how WinEQ configures things, but the *implementations* are in JMB's proprietary engine. Still, the configuration patterns reveal exactly what knobs matter for multiboxing.

---

## 1. Resource Throttling

### FPS Limiting (Foreground/Background)

WinEQ exposes separate FPS caps for foreground and background windows.

**Settings** (`WEQ2.Settings.iss`):
```
variable uint ForegroundFPS=60
variable uint BackgroundFPS=60
```

**Session-side implementation** (`WEQ2022.Session.iss`):
```iss
method SetForegroundFPS(uint newValue)
    Settings.ForegroundFPS:Set[${newValue}]
    if ${newValue}
        maxfps -fg -calculate ${newValue}
    else
        maxfps -fg -disable

method SetBackgroundFPS(uint newValue)
    Settings.BackgroundFPS:Set[${newValue}]
    if ${newValue}
        maxfps -bg -calculate ${newValue}
    else
        maxfps -bg -disable
```

`maxfps` is a **JMB built-in command** that hooks DirectX present calls to throttle frame rate. The `-calculate` flag suggests it computes frame timing rather than using a simple sleep. Setting 0 disables the limit.

**Frostreaver relevance**: We need foreground/background FPS caps. For 36 clients:
- Foreground (active window): 30-60 FPS
- Background (35 idle clients): 5-10 FPS or lower
- Implementation: Hook `IDirect3DDevice9::Present` or use `D3DPRESENT_INTERVAL` + `Sleep()` in game loop hook

### EQPlayNice / Render Strobing

This is the classic EQ multibox optimization, originally a standalone utility from the early 2000s.

**Settings**:
```
variable bool UseEQPlayNice=FALSE
variable float RenderStrobeInterval=1.0  // seconds
```

**Session-side implementation**:
```iss
method SetUseEQPlayNice(bool newValue)
    Settings.UseEQPlayNice:Set[${newValue}]
    if ${newValue}
        EQPlayNice:Enable
        EQPlayNice:SetCeiling["${Math.Calc[${Settings.RenderStrobeInterval}*1000]}"]
    else
        EQPlayNice:Disable
```

`EQPlayNice` is another JMB built-in. The "render strobe" concept: **only render the 3D world at a configurable interval** (default 1.0 second). Between render frames, the game still processes network messages, AI ticks, and input — it just skips the expensive GPU render pass.

The GUI tooltip confirms this:
> "Improves EverQuest framerate (FPS), allowing better movement speed and auto-follow precision, as well as PC and multi-boxing performance. This is achieved by only allowing the game world to render at a specified interval."

**Frostreaver relevance**: This is the single most impactful optimization for 36 clients. Implementation approach:
- Hook `ProcessGameEvents` (we already do this in `textquest-dll/src/hooks/game_loop.rs`)
- Track time since last render; if below threshold, skip the render call chain
- Or hook `IDirect3DDevice9::BeginScene` / `EndScene` and short-circuit
- Background clients could render once every 2-5 seconds; they only need network/game logic ticks

### CPU Throttling / Affinity / Priority

**Not present in WinEQ 2022 source.** WinEQ does NOT do:
- CPU affinity pinning
- Process priority management
- Working set trimming
- Memory limits

These are either handled by JMB's core (unlikely — no configuration exposed), left to the OS scheduler, or handled by users via external tools. Our existing `affinity.rs` module is already ahead of WinEQ here.

### GPU Adapter Selection

WinEQ allows per-profile GPU adapter selection:

```iss
variable int Adapter=-1  // -1 = default

method SetForceAdapter(int numAdapter)
    noop ${Direct3D8:SetAdapter[${numAdapter}]}
          ${Direct3D9:SetAdapter[${numAdapter}]}
          ${Direct3D11:SetMonitor["${Display.Monitor[${numAdapter.Inc}]~}"]}
```

**Frostreaver relevance**: Interesting for multi-GPU setups. Could split rendering load across GPUs if the Forge AI machine has an iGPU + dGPU. Low priority for now.

---

## 2. Window Management

### Window Presets

WinEQ's core window management is through "presets" — named window configurations:

```iss
objectdef weq2preset
    variable string Name
    variable int X
    variable int Y
    variable bool FullScreen
    variable bool AlwaysOnTop
    variable bool LockPosition
    variable bool LockSize
    variable bool Border
    variable float Scale=1.0
```

Default presets:
- **Normal**: position (0,15), scale 100%, border, not always-on-top
- **Tiny**: position (0,4), scale 20%, always-on-top, border
- **Full Screen (emulated)**: position (0,0), scale 100%, no border, fullscreen emulation

### Window Positioning

```iss
method ApplyWindowPreset()
    variable int monitorX=${Display.Monitor.Left}
    variable int monitorY=${Display.Monitor.Top}

    if ${CurrentPreset.FullScreen}
        WindowCharacteristics -stealth -size -viewable fullscreen
                              -pos -viewable ${monitorX},${monitorY}
                              -frame none -visibility foreground
        return

    // Calculate scaled size
    sizeX:Set[${Display.Width}*${useScale}]
    sizeY:Set[${Display.Height}*${useScale}]
    posX:Set[${monitorX}+${CurrentPreset.X}]
    posY:Set[${monitorY}+${CurrentPreset.Y}]

    WindowCharacteristics -pos -viewable ${posX},${posY}
                          -size -viewable ${sizeX}x${sizeY}
                          ${useAlwaysOnTop}${useBorder}
```

Key techniques:
- **Emulated fullscreen**: Forces windowed mode even when game requests fullscreen (`ForceWindowed=TRUE` by default). Intercepts DirectX fullscreen transitions.
- **Scaling**: Windows can be rendered at reduced scale (e.g., 20% for "Tiny" preset). The game renders at full resolution internally but the window is scaled down.
- **Border control**: Can strip window borders (`-frame none`) for borderless windowed mode, or use `thin` (no resize grip) or `thick` (resizable).
- **Lock characteristics**: `windowcharacteristics -lock` prevents the game from moving/resizing its own window.

### Force Windowed Mode

```iss
method SetForceWindowed(bool value)
    noop ${Direct3D8:SetForceWindowed[${value}]}
         ${Direct3D9:SetForceWindowed[${value}]}
         ${Direct3D10:SetForceWindowed[${value}]}
         ${Direct3D11:SetForceWindowed[${value}]}
```

This intercepts DirectX device creation to prevent exclusive fullscreen. Critical for multiboxing — exclusive fullscreen would steal the display from other clients.

**Frostreaver relevance**: We should hook `IDirect3DDevice9::Reset` and `CreateDevice` to force `D3DPRESENT_PARAMETERS.Windowed = TRUE`. Could also be done via eqclient.ini (`VideoMode=2` for windowed).

### Focus / Session Switching

```iss
method NextWindow()
    variable uint nextSlot=${This.GetNextSlot}
    if !${Display.Window.IsForeground}
        return
    uplink focus "jmb${nextSlot}"
    relay "jmb${nextSlot}" "Event[OnHotkeyFocused]:Execute"

method OnSlotHotkey(uint numSlot)
    uplink focus "jmb${numSlot}"
    relay "jmb${numSlot}" "Event[OnHotkeyFocused]:Execute"
```

Session switching uses JMB's `uplink focus` + `relay` system:
- `uplink focus "jmb3"` — tell the uplink (main process) to bring slot 3's window to foreground
- `relay "jmb3" "..."` — send a command to slot 3's session to execute

Global hotkeys: `Ctrl+Alt+1` through `Ctrl+Alt+8` for direct slot access, `Ctrl+Alt+X/Z` for next/prev cycling.

**Frostreaver relevance**: Our IPC (shared memory + named pipes) can do the same. Add a `FocusWindow` IPC command that calls `SetForegroundWindow`. Global hotkey registration via `RegisterHotKey` Win32 API.

### Gamma Lock

```iss
method SetLockGamma(bool newValue)
    if ${newValue}
        GammaLock on
    else
        GammaLock off
```

Prevents EQ from changing system gamma. When multiboxing, one client changing gamma affects all displays. Minor but nice QoL.

---

## 3. Hooks and Patches

### What JMB (Inner Space) Hooks

WinEQ 2022 itself does NOT inject code or modify EQ memory. However, the **JMB/Inner Space runtime** that hosts it does the following (inferred from the API calls in the scripts):

1. **DirectX hooks**: `Direct3D8`, `Direct3D9`, `Direct3D10`, `Direct3D11` objects are all available — JMB hooks DirectX device creation and rendering
2. **Window hooks**: `Display.Window`, `WindowCharacteristics`, `windowtext` — JMB hooks window creation and management
3. **Input hooks**: LGUI2 input binding system — JMB hooks keyboard/mouse input
4. **Process management**: `JMB.Slot[N]:Launch` — JMB manages process creation
5. **Virtual file system**: Per-profile `eqclient.ini` and `eqlsPlayerData.ini` via file redirection:

```json
"virtualFiles": [
    {
        "pattern": "*/eqclient.ini",
        "replacement": "{1}/custom_eqclient.ini"
    },
    {
        "pattern": "*/eqlsPlayerData.ini",
        "replacement": "{1}/eqlsPlayerData.WinEQProfile0.ini"
    }
]
```

This virtual file system is how WinEQ supports running multiple EQ instances from the same install directory with different settings. Each profile can have its own eqclient.ini without copying the entire EQ folder.

**Frostreaver relevance**: We already do DLL injection and game loop hooking. The virtual file system concept is clever — we could implement this via `CreateFileW` hook to redirect file opens per-client. However, our current approach of separate eqclient.ini paths in the TOML config achieves the same goal more simply.

---

## 4. Configuration Format

### Settings Storage

JSON file (`WinEQ2022.Settings.json`) with this structure:

```json
{
    "Indicator": true,
    "IndicatorX": 20,
    "IndicatorY": 28,
    "LockGamma": false,
    "ForceWindowed": true,
    "LockWindow": false,
    "EQPlayNice": true,
    "RenderStrobeInterval": 1.0,
    "BackgroundFPS": 60,
    "ForegroundFPS": 60,
    "Hotkeys": { ... },
    "Profile1": { ... },
    "Profile2": { ... },
    "Preset1": { ... },
    "Preset2": { ... }
}
```

### Per-Profile Settings

- `Name`, `EQPath`, `EQClientINI` — basic identity
- `Preset` — default window preset number
- `GlobalHotkey` — "AUTO" (assigns `Ctrl+Alt+N` based on slot) or explicit combo
- `Adapter` — GPU adapter index (-1 = default)
- `Sound` — enable/disable audio
- `Patch` — use launchpad.exe vs direct eqgame.exe
- `Locale` — us/kr/tw/jp/de/fr/cn
- `FGTileScale` / `BGTileScale` — foreground/background tile scaling (0.6 / 0.5)
- `PIP` — picture-in-picture mode with position, border, and scale settings

### Capacity Limits

- **30 profiles** (hardcoded `Profiles:Resize[30]`)
- **10 window presets** (hardcoded `Presets:Resize[10]`)
- **20 global hotkeys** (slots 1-20)
- **10 preset hotkeys**

For our 36-client setup, WinEQ's 30-profile limit would be insufficient.

---

## 5. Actionable Takeaways for Frostreaver

### High Priority — Implement These

| Feature | WinEQ Approach | Frostreaver Implementation |
|---------|---------------|---------------------------|
| **Background FPS cap** | `maxfps -bg -calculate N` (DirectX present hook) | Hook `IDirect3DDevice9::Present`, insert `Sleep()` or timer-based throttle. Target 5-10 FPS for background clients. |
| **Render strobing** | `EQPlayNice:SetCeiling[ms]` (skip render passes) | In game loop hook, track render timer. Skip `BeginScene`/`EndScene` if below interval. Let game logic tick continue. |
| **Force windowed** | `Direct3D9:SetForceWindowed[TRUE]` | Hook `CreateDevice`/`Reset`, force `Windowed=TRUE` in present params. Or enforce via eqclient.ini. |
| **Window positioning** | `WindowCharacteristics -pos -size` | `SetWindowPos` Win32 API from orchestrator. Pre-compute a tiling layout for 36 windows. |

### Medium Priority — Nice to Have

| Feature | Notes |
|---------|-------|
| **Window lock** | Prevent EQ from moving/resizing its window. Hook `SetWindowPos` or `MoveWindow`. |
| **Gamma lock** | Hook `SetDeviceGammaRamp` to no-op. Prevents one client from blasting brightness. |
| **Borderless windowed** | `SetWindowLong` to strip `WS_BORDER` / `WS_CAPTION` styles. |
| **Per-client eqclient.ini** | Already supported in our config. Could add `CreateFileW` hook for transparent redirection. |

### Low Priority / Not Needed

| Feature | Why Skip |
|---------|----------|
| **PIP (picture-in-picture)** | Complex, not needed for automated multibox |
| **GUI overlay** | We use TUI dashboard instead |
| **Global hotkeys for switching** | Automated clients don't need manual switching |
| **INI import from WinEQ 2** | We have our own config format |

### Key Insight: Render Strobing Math for 36 Clients

If each EQ client renders at 60 FPS, 36 clients = 2,160 frames/second of GPU work.

With render strobing at 2-second intervals for 35 background clients + 30 FPS foreground:
- Foreground: 30 FPS
- Background: 35 clients × 0.5 FPS = 17.5 FPS total
- **Total GPU load: ~48 frames/second** (97.8% reduction)

This is why EQPlayNice render strobing was the killer feature for multiboxing since 2002. It's the single most important optimization we need.
