# EverQuest INI Optimization Guide — 36-Box Multiboxing

> Target hardware: 24-core / 64GB RAM mini PC running 36 EQ clients simultaneously.
> Template INI: `config/eqclient_multibox.ini`

---

## Quick Start

1. Back up your existing `eqclient.ini` for each client
2. Copy settings from `config/eqclient_multibox.ini` into each client's INI
3. Set `eqclient.ini` to **read-only** (`attrib +r eqclient.ini`) to prevent EQ from reverting settings
4. For the one "driver" client you actively play, use the relaxed values in the Driver column below

---

## Settings by Impact Tier

### Tier 1 — CRITICAL (do these first, biggest impact)

| Setting | Section | Bot Value | Driver Value | Why |
|---------|---------|-----------|-------------|-----|
| `MaxBGFPS=10` | `[Options]` | `10` | `30` | Background FPS cap. THE single most impactful multibox setting. Each unfocused client renders at this rate. Don't go below 10 or macros/autofollow break. |
| `MaxFPS=30` | `[Options]` | `30` | `60` | Foreground FPS cap. 30 is smooth enough for bot clients. |
| `Sound=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Disables entire sound engine. Saves ~30-50MB RAM and ~2-3% CPU per client. |
| `AllLuclinPcModelsOff=TRUE` | `[Defaults]` | `TRUE` | `FALSE` | Forces classic low-poly character models. Saves ~100-200MB RAM per client. Luclin models are the single largest memory consumer per client. |
| `ShowSpellEffects=0` | `[Defaults]` | `0` | `1` | Disables all spell particle rendering. In group/raid content with 36 chars casting, particles can consume 15%+ CPU. |
| `SpellParticleOpacity=0.0` | `[Defaults]` | `0.0` | `0.5` | Makes spell particles fully transparent even if shown. Belt-and-suspenders with ShowSpellEffects. |
| `SpellParticleDensity=0.0` | `[Defaults]` | `0.0` | `0.25` | Zero particle density. |
| `StickFigures=1` | `[Defaults]` | `1` (optional) | `0` | Renders all PCs/NPCs as colored stick figures. Extreme GPU+RAM savings but you lose visual info. Best for pure background bots. |
| `CPUAffinity0=-1` (through 23) | `[Defaults]` | `-1` | `-1` | Lets Windows schedule across all cores. Without this, EQ pins every instance to core 0, causing massive contention. Use `ClientCore#=-1` on newer patches if `CPUAffinity` doesn't work. |
| `WindowedWidth=640` / `Height=480` | `[VideoMode]` | `640x480` | `1920x1080` | Smallest viable window. Saves ~50-80MB VRAM per client vs 1080p. |
| `WindowedMode=TRUE` | `[Defaults]` | `TRUE` | `TRUE` | Must be windowed for multiboxing. |

### Tier 2 — HIGH (significant savings, strongly recommended)

| Setting | Section | Bot Value | Driver Value | Why |
|---------|---------|-----------|-------------|-----|
| `Shadows=0` | `[Defaults]` | `0` | `0` | Disables all shadow rendering. ~5-10% GPU savings per client. |
| `ShadowClipPlane=0` | `[Options]` | `0` | `0` | Shadow draw distance to zero. |
| `ClipPlane=5` | `[Options]` | `5` | `20` | World geometry draw distance. Lower = fewer triangles drawn per frame. |
| `ActorClipPlane=5` | `[Options]` | `5` | `15` | NPC/PC model draw distance. Bot clients don't need to see far. |
| `ShowGrass=FALSE` | `[Defaults]` | `FALSE` | per pref | Grass is purely cosmetic. ~2-5% GPU savings. |
| `TextureQuality=0` | `[Defaults]` | `0` | per pref | Lowest texture resolution. Saves ~50-100MB VRAM per client. |
| `MipMapping=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Disables mipmapping. Saves ~20-50MB VRAM per client. |
| `ShowDynamicLights=FALSE` | `[Defaults]` | `FALSE` | per pref | Disables torch/lightstone glow. ~2-5% CPU per frame. |
| `MultiPassLighting=FALSE` | `[Defaults]` | `FALSE` | `FALSE` | Skips extra lighting passes. Reduces draw call count. |
| `PostEffects=0` | `[Defaults]` | `0` | per pref | Disables bloom, color grading, screen effects. ~3-5% GPU. |
| `Bloom=0` | `[Defaults]` | `0` | per pref | Requires PostEffects. Disable on bots. |
| `ParticleDensity=0` | `[Options]` | `0` | per pref | Global particle density override. |
| `ServerFilter=1` | `[Defaults]` | `1` | `1` | Server-side message filtering. Reduces network traffic + CPU processing in crowded zones. |
| `EnvironmentParticleOpacity=0.0` | `[Defaults]` | `0.0` | `0.5` | Environment particle effects (rain, etc). |
| `ActorParticleOpacity=0.0` | `[Defaults]` | `0.0` | `0.5` | Character buff/aura particles. |

### Tier 3 — MODERATE (incremental savings that add up across 36 clients)

| Setting | Section | Bot Value | Driver Value | Why |
|---------|---------|-----------|-------------|-----|
| `VertexShaders=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Disables vertex shaders. Trade-off: moves work from GPU to CPU. With 36 clients sharing one GPU, less GPU contention wins. If GPU has headroom, enable to offload from CPU. |
| `20PixelShaders=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Same trade-off as vertex shaders. |
| `14PixelShaders=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Same. |
| `1xPixelShaders=FALSE` | `[Defaults]` | `FALSE` | `TRUE` | Same. |
| `Sky=0` | `[Options]` | `0` | per pref | Disables sky rendering entirely. Saves a few draw calls. |
| `LODBias=0` | `[Options]` | `0` | per pref | Most aggressive LOD — simplest meshes at all distances. |
| `Log=FALSE` | `[Defaults]` | `FALSE` | per pref | Disables chat logging. Reduces disk I/O across 36 clients. |
| `LoadSocialAnimations=FALSE` | `[Defaults]` | `FALSE` | per pref | Skips loading emote animation data. |
| `LoadVeliousArmorsWithLuclin=FALSE` | `[Defaults]` | `FALSE` | `FALSE` | Skips Velious armor textures. |
| `UseNewUIEngine=0` | `[Defaults]` | `0` | `0` | Disables new UI engine. Reported to fix frame limiter issues and reduce UI rendering overhead. |
| `FogScale=0.5` | `[Options]` | `0.5` | per pref | Reduce fog complexity. |
| `SkyUpdateInterval=60000` | `[Defaults]` | `60000` | `1000` | Update sky once per minute instead of every frame. |
| `TerrainTextureQuality=0` | `[Defaults]` | `0` | per pref | Lowest terrain textures. |
| `FullscreenBitsPerPixel=16` | `[VideoMode]` | `16` | `32` | 16-bit color saves VRAM. Only applies if fullscreen. |

### Tier 4 — LOW (minor, but free performance across 36 clients)

| Setting | Section | Bot Value | Why |
|---------|---------|-----------|-----|
| `ShowNamesLevel=1` | `[Defaults]` | `1` | Show only group names. Less text rendering. |
| `Show3dTargetIndicator=0` | `[Options]` | `0` | Skip 3D target ring rendering. |
| `DisableTattoos=TRUE` | `[Defaults]` | `TRUE` | Skip tattoo textures. |
| `InspectOthers=FALSE` | `[Defaults]` | `FALSE` | Skip inspect processing. |
| `AllowAutoDuck=0` | `[Defaults]` | `0` | Disable auto-crouch. |
| `EnableVoiceChat=FALSE` | `[Defaults]` | `FALSE` | Disable voice chat subsystem. |
| All combat message filters | `[Options]` | `0` | See template INI. ~1-2% CPU in heavy combat. |
| HitsMode floaters | `[HitsMode]` | `0` | Disable floating damage numbers. |

---

## Memory Budget for 36 Clients

| Configuration | RAM/Client | Total (36) | Notes |
|---------------|-----------|------------|-------|
| EQ defaults (all features on) | ~1.2-1.5 GB | 43-54 GB | Does not fit in 64GB with OS overhead |
| Our optimized template | ~400-600 MB | 14-22 GB | Comfortable in 64GB |
| Absolute minimum (StickFigures + MemMode Least + 640x480) | ~250-350 MB | 9-13 GB | Extreme, but viable for pure background bots |

**64GB budget breakdown (optimized template):**
- 36 clients @ ~500MB = ~18 GB
- Windows 11 + drivers = ~4-6 GB
- Frostreaver orchestrator + DLLs = ~1-2 GB
- Headroom for zone loads, spikes = ~38 GB free

### VRAM Budget (shared GPU)

| Setting | VRAM/Client | Notes |
|---------|------------|-------|
| 640x480 + low textures + no mip | ~80-150 MB | Viable |
| 1080p + high textures | ~200-400 MB | Not viable for 36 clients |

With integrated graphics (typical mini PC), VRAM is carved from system RAM. At ~100MB/client VRAM, that's ~3.6GB additional from the 64GB pool.

---

## Background FPS Deep Dive

`MaxBGFPS` deserves special attention because it controls what happens to 35 of your 36 clients at any given time.

| MaxBGFPS | Behavior | Use Case |
|----------|----------|----------|
| `0` | Unlimited | **Never use for multibox** |
| `5` | Very low | Risky — macros/autofollow may stutter |
| `10` | Low, reliable | **Recommended for bot clients** — macros execute reliably |
| `20-25` | Moderate | Good for semi-active bots (driver alts) |
| `30` | Normal | Only for the driver client |
| `60` | High | Waste of resources for multibox |

The `/framelimiter` in-game command can also adjust this at runtime. Frostreaver's DLL can override this per-client programmatically.

---

## Settings That Can Cause Crashes

These settings have reported stability issues at extreme values:

| Setting | Risk | Safe Value |
|---------|------|------------|
| `ClipPlane=0` | Crashes in some zones with zero geometry | Use `5` minimum |
| `MaxBGFPS < 5` | Client may become unresponsive | Use `10` minimum |
| `TextureCache=FALSE` | Longer zone times, rare crashes on low VRAM | Test per system |
| `UseNewUIEngine=0` | Some UI skins may break | Use `default` skin |
| All shaders `FALSE` | Visual corruption on some GPUs | Test first, enable if issues |

---

## In-Game Settings (must configure manually)

These cannot be set via INI and must be configured in-game per client:

1. **Options > Display > Memory Mode**: Set to **"Least"** — reduces texture cache and model caching. Characters take a few seconds to render after zoning but saves ~100-200MB per client.
2. **Options > uncheck "Enable Journal"** — NPC journal records every NPC emote; massive CPU/RAM drain over time.
3. **Advanced Display > Clip Plane slider**: Set to minimum.
4. **Advanced Display > Particle settings**: All to "Off" (supplements INI settings).
5. **`/showgrass off`** — Confirms `ShowGrass=FALSE`.
6. **Set `eqclient.ini` to read-only** after configuring — prevents EQ from reverting settings on exit.

---

## Deployment Strategy for Frostreaver

Each EQ client has its own `eqclient.ini` in its install directory. For 36 clients:

1. **Template approach**: Maintain `config/eqclient_multibox.ini` as the master template
2. **Launcher integration**: The M2.5 launcher (`dmft/src/launcher/`) can deploy the INI to each client directory before launch
3. **Per-client overrides**: The driver client gets a relaxed profile; all other 35 get the aggressive bot profile
4. **Read-only lock**: After deploying, mark INIs read-only so EQ doesn't revert settings on exit

### Shader Trade-off Decision

With 36 clients sharing one GPU, the shader decision depends on your hardware:

- **Weak GPU (integrated)**: Disable all shaders (`FALSE`) — GPU is the bottleneck, move work to CPU where you have 24 cores
- **Decent GPU (discrete)**: Enable shaders (`TRUE`) — GPU handles shader work more efficiently than CPU, and 24 cores are the bottleneck with 36 clients

Monitor with Task Manager to determine which resource (CPU vs GPU) hits 100% first, then adjust.

---

## PlayNice / Render Skipping (CPU Throttling)

### What is "PlayNice"?

**PlayNice is NOT an eqclient.ini setting.** EQPlayNice is an external DLL made by Lavish Software (WinEQ2 / ISBoxer developer) that hooks the EQ game loop and inserts `Sleep()` calls to yield CPU time. It provides two modes:

- **CPU Limiting**: Yields a constant percentage of CPU per frame (e.g., "use 50% of a core")
- **Framerate Limiting**: Yields minimum CPU needed to hit an FPS target

The real magic is **Rendering Limiting** — it slows the rate at which the 3D world is redrawn while letting the UI redraw at full speed. Background clients process macros/plugins at full speed but skip expensive 3D rendering.

### MQ2 Equivalents

| Tool | Key Feature | Status |
|------|-------------|--------|
| **EQPlayNice** | DLL injection, Sleep-based CPU throttling + render skipping | External tool (Lavish Software) |
| **MQ2FPS** | `/maxfps fg\|bg #`, `/render bg 0` (skip world render in background) | Discontinued from mainline MQ |
| **MQ2EQWire** | Entirely stops the draw call for background windows. `BGRenderRate=0` | Most aggressive — near-zero GPU for background clients |

### What This Means for Frostreaver

Since we already have an injected DLL (`dmft-dll`), we can implement the same technique directly in our game loop hook (`dmft-dll/src/hooks/game_loop.rs`):

1. **Check if window is focused** (use `GetForegroundWindow()` or track via `WM_ACTIVATE`)
2. **When unfocused, skip the 3D render call** — the `ProcessGameEvents` hook can conditionally bypass the render/present portion
3. **Keep processing game logic** — macros, IPC commands, combat logic still execute at full speed
4. **Result**: GPU usage drops to near-zero for 35 background clients while maintaining full bot functionality

This is the single most impactful optimization beyond INI settings and should be a high-priority addition to `dmft-dll`.

### INI Settings That Interact

The built-in INI settings that provide partial PlayNice-like behavior (already in our template):

| Setting | Value | Effect |
|---------|-------|--------|
| `MaxBGFPS=10` | `10` | Background FPS cap — EQ's built-in throttle when unfocused |
| `NoFPSLimiter=0` | `0` | Keep FPS limiter enabled |
| `MaxFPS=30` | `30` | Foreground FPS cap |

`MaxBGFPS` is EQ's built-in "PlayNice lite" — it throttles rendering when the window loses focus. Our DLL render-skip would be even more aggressive (zero renders vs 10 FPS).

---

## Memory Hard Cap per Instance

### EQ Has No Built-in Memory Limit

There is no `MemoryLimit=` or similar eqclient.ini setting. The closest built-in control is the in-game **Memory Mode** (Least/Balanced/Most) which affects texture caching aggressiveness, but it is not a hard cap.

### Windows Job Objects — Enforced Memory Limits

Windows Job Objects can enforce a per-process memory hard cap. This is the recommended approach for Frostreaver.

#### How It Works

1. `CreateJobObjectW()` — create a job object per EQ client
2. `SetInformationJobObject()` with `JobObjectExtendedLimitInformation` — set limits
3. Set `JOB_OBJECT_LIMIT_PROCESS_MEMORY` flag + `ProcessMemoryLimit` in bytes
4. `AssignProcessToJobObject(job, process)` — attach the EQ process

#### What Happens When a Process Exceeds the Limit

Memory allocations are **denied** (VirtualAlloc/HeapAlloc returns NULL) — the process is **not terminated**. However, EQ does not handle allocation failures gracefully, so in practice the client will crash with an access violation or "out of memory" error. Job Objects act as a **crash-on-exceed** safety net rather than graceful degradation.

#### Recommended Limits

| Client Type | Memory Limit | Rationale |
|-------------|-------------|-----------|
| Bot clients (optimized INI) | **1 GB** | Typical usage ~400-600MB, provides headroom for zone transitions |
| Driver client | **2 GB** | Higher textures/models need more room |
| Aggregate (all 36) | **36 GB** | Optional: `JOB_OBJECT_LIMIT_JOB_MEMORY` for total cap |

Set limits with ~1.5-2x typical usage to avoid crashes during zone transitions (which temporarily spike memory).

#### Notification-Only Mode (Safer Alternative)

Instead of hard limits, use `JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION` to get **notified** when a process exceeds a threshold without denying allocations. The orchestrator can then decide: log it, alert, or restart the client. This is safer for production use.

#### Rust Implementation

Fits naturally alongside `dmft/src/client/affinity.rs`. Requires adding `"Win32_System_JobObjects"` to the `windows` crate features in `dmft/Cargo.toml`:

```rust
use windows::Win32::System::JobObjects::*;
use windows::Win32::System::Threading::*;
use std::mem;

pub fn create_memory_limited_job(memory_limit_bytes: usize) -> windows::core::Result<HANDLE> {
    unsafe {
        let job = CreateJobObjectW(None, None)?;

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        info.ProcessMemoryLimit = memory_limit_bytes;

        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;

        Ok(job)
    }
}

pub fn assign_process_to_job(job: HANDLE, pid: u32) -> windows::core::Result<()> {
    unsafe {
        let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)?;
        AssignProcessToJobObject(job, process)?;
        CloseHandle(process)?;
        Ok(())
    }
}
```

#### Bonus: Process Cleanup on Orchestrator Exit

Setting `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` on the job object means all 36 EQ clients terminate automatically if the Frostreaver orchestrator crashes or exits. This is a valuable safety net — no orphaned EQ processes consuming resources.

```rust
info.BasicLimitInformation.LimitFlags =
    JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
```

---

## Sources

- [RedGuides: Settings And You — Minimize CPU/Memory Usage](https://www.redguides.com/community/resources/settings-and-you-a-guide-to-adjusting-your-eq-settings-to-minimize-cpu-memory-usage.1292/)
- [RedGuides: Optimal eqclient.ini settings](https://www.redguides.com/community/threads/optimal-eqclient-ini-settings.42204/)
- [RedGuides: Modern Lag and StickFigures=1](https://www.redguides.com/community/threads/modern-lag-and-the-use-of-stickfigures-1.88346/)
- [Fanra's EverQuest Wiki: Graphics and Performance Settings](https://everquest.fanra.info/wiki/Graphics_and_performance_settings_guide)
- [Almar's Guides: EQ Client INI](https://www.almarsguides.com/eq/gettingstarted/boxing/PopularEQIssues/EverquestFiles/EqClientINI/)
- [Almar's Guides: Tips for Reducing CPU Usage](https://almarsguides.com/eq/gettingstarted/boxing/PopularEQIssues/ReduceYourLag/)
- [MacroQuest Docs: Multiboxing](https://docs.macroquest.org/main/multiboxing/)
- [MacroQuest Docs: MQ2FPS](https://docs.macroquest.org/plugins/discontinued/mq2fps/)
- [ISBoxer: Tweak Your Framerate](https://isboxer.com/wiki/HOWTO:Tweak_your_framerate)
- [Steam Community: EQ Stability and Performance Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=1917259530)
- [EQPlayNice — Lavish Software Wiki](https://www.lavishsoft.com/wiki/index.php/EQPlayNice)
- [MQ2EQWire — RedGuides](https://www.redguides.com/community/threads/mq2eqwire.66963/)
- [Windows Job Objects — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [JOBOBJECT_EXTENDED_LIMIT_INFORMATION — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)
