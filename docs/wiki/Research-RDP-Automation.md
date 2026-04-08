# RDP / Remote Desktop Automation Research

**Date:** 2026-03-29
**Goal:** Programmatically control frostreaver (Windows 11) from macOS to launch EQ clients, verify UI state, click buttons, and take screenshots.

---

## Option Comparison

| Option | macOS Compatible | Scriptable Input | Screenshots | Setup Complexity | Reliability |
|--------|:---:|:---:|:---:|:---:|:---:|
| **1. Extend Existing Rust API (Win32 SendInput)** | Yes (via HTTP) | Excellent | Yes | Low | High |
| **2. VNC + vncdotool** | Yes | Excellent | Yes | Medium | High |
| **3. Apache Guacamole + Playwright** | Yes | Good | Yes | High | Medium |
| **4. FreeRDP (xfreerdp)** | Partial | Poor | Limited | Medium | Low |
| **5. Python RDP (aardwolf/pyrdp)** | Yes | Limited | Limited | Medium | Low |
| **6. AutoHotkey on Windows** | Yes (via TCP) | Excellent | Yes | Medium | Medium |

---

## 1. Extend Existing Rust Remote API (RECOMMENDED)

**Approach:** Add Win32 `SendInput` / `FindWindow` / screenshot endpoints to the existing `textquest` remote API already running on frostreaver.

### How It Works
- The remote API already runs on frostreaver (port 3000) in the desktop session
- Add new endpoints: `/screenshot`, `/click`, `/key`, `/find-window`, `/launch`
- Use Win32 APIs directly from Rust:
  - `SendInput` for keystrokes and mouse clicks
  - `BitBlt` / `PrintWindow` for screenshots
  - `FindWindowW` / `EnumWindows` for window discovery
  - `SetForegroundWindow` to activate windows before input
  - `CreateProcessW` to launch applications (already implemented)
- Call from macOS via `curl` or any HTTP client

### Rust Implementation Sketch
```rust
// Already have the windows crate. Key APIs:
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_MOUSE, INPUT_KEYBOARD};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleDC, CreateCompatibleBitmap};
```

### New Endpoints
```
POST /screenshot          -> returns PNG of desktop or specific window
POST /click               -> {x, y, button} sends mouse click via SendInput
POST /key                 -> {keys: "ctrl+a"} sends keystroke via SendInput
POST /find-window         -> {title: "EverQuest"} returns window handle + position
POST /activate-window     -> {hwnd} brings window to foreground
POST /window-screenshot   -> {hwnd} screenshot of specific window
```

### Pros
- **Zero new dependencies** -- builds on existing infrastructure
- **Already solves session 0 problem** -- API runs in desktop session via startup folder
- **Native Win32 = most reliable input** -- SendInput is what AutoHotkey uses internally
- **No network protocol overhead** -- direct API calls, no VNC/RDP encoding
- **Screenshots are instant** -- BitBlt is sub-millisecond
- **Full control** -- can target specific windows, not just "whatever is focused"
- **Cross-platform client** -- any HTTP client works from macOS

### Cons
- Requires writing ~200-300 lines of new Rust code for input simulation + screenshots
- Need to handle DPI awareness for correct coordinates
- Screenshots need encoding (PNG via `image` crate)

### Setup
```bash
# On frostreaver: just rebuild and restart the API
cargo build --release
# API already auto-starts via startup folder shortcut
```

---

## 2. VNC + vncdotool

**Approach:** Install a VNC server on frostreaver, use `vncdotool` from macOS to automate.

### How It Works
- Install TightVNC/UltraVNC/TigerVNC server on Windows
- Connect from macOS using `vncdotool` (Python)
- Send clicks, keystrokes, capture screenshots programmatically

### vncdotool Usage
```bash
# Install on macOS
pip install vncdotool

# Connect and automate
vncdo -s frostreaver::5900 type "hello" key enter
vncdo -s frostreaver::5900 move 500 300 click 1
vncdo -s frostreaver::5900 capture screenshot.png

# Python API
from vncdotool import api
client = api.connect('frostreaver', password='xxx')
client.captureScreen('shot.png')
client.mouseMove(500, 300)
client.mousePress(1)
client.keyPress('enter')
```

### Pros
- **Mature automation library** -- vncdotool is battle-tested
- **Visual verification** -- can capture screenshots and compare with `expectScreen()`
- **Image matching** -- wait for specific UI elements to appear
- **Simple protocol** -- VNC is straightforward, well-documented
- **Works from any OS** -- macOS, Linux, etc.

### Cons
- **Extra service on frostreaver** -- VNC server must run alongside everything else
- **Performance overhead** -- VNC encodes the entire screen, CPU cost
- **Security** -- VNC passwords are weak (8 char max on some servers); must tunnel over SSH
- **Coordinate-based** -- no window awareness, just raw screen coordinates
- **Bandwidth** -- full screen transfer for every screenshot
- **Conflicts with RDP** -- VNC and RDP can fight over the desktop session

### Setup Complexity: Medium
```powershell
# On frostreaver:
# 1. Download and install TightVNC server
# 2. Configure password and port
# 3. Allow through Windows Firewall
# 4. (Recommended) Set up SSH tunnel for security
```

---

## 3. Apache Guacamole + Playwright

**Approach:** Run Guacamole as a web-based RDP gateway, then use Playwright to automate the browser-based remote desktop.

### How It Works
- Deploy Guacamole (Docker) on frostreaver or a separate machine
- Guacamole connects to frostreaver via RDP and renders in HTML5 canvas
- Playwright controls the browser, clicking on the canvas to interact

### Pros
- **Browser-based** -- accessible from anywhere with a browser
- **Playwright is powerful** -- mature automation framework
- **Supports RDP, VNC, SSH** -- protocol-agnostic gateway
- **Good for visual testing** -- Playwright has built-in screenshot comparison

### Cons
- **Double indirection** -- Playwright -> Browser -> Guacamole -> RDP -> Windows
- **Complex setup** -- Docker, Guacamole server, database, RDP config
- **Latency** -- multiple encoding/decoding layers add delay
- **Canvas-based** -- Playwright clicks on an HTML canvas, not real UI elements
- **Fragile** -- canvas coordinates can shift with browser zoom, window size
- **Overkill** -- massive infrastructure for what we need

### Setup Complexity: High
```bash
# Docker compose with guacamole, guacd, postgres
# Configure RDP connection to frostreaver
# Set up Playwright on macOS
# Write automation scripts targeting the canvas element
```

---

## 4. FreeRDP (xfreerdp)

**Approach:** Use the open-source FreeRDP client from macOS to connect and script interactions.

### How It Works
- Install xfreerdp via Homebrew on macOS
- Connect to frostreaver's RDP
- Use action scripts for automation

### Pros
- **Open source** -- full RDP protocol support
- **Homebrew available** -- `brew install freerdp`
- **Action scripts** -- can trigger shell scripts on hotkey combos

### Cons
- **Not designed for automation** -- xfreerdp is an interactive client, not a scripting tool
- **No programmatic input API** -- can't send clicks/keystrokes from external scripts
- **macOS support is X11-based** -- requires XQuartz, not native
- **No screenshot API** -- no built-in way to capture the session programmatically
- **Action scripts are limited** -- only triggered by keyboard combos, not scriptable
- **Would need xdotool** -- to send input to the xfreerdp window, adding another layer

### Setup Complexity: Medium (but poor automation)
```bash
brew install freerdp
xfreerdp /v:frostreaver /u:user /p:pass /w:1920 /h:1080
# Then... pray you can script it with xdotool
```

---

## 5. Python RDP Libraries (aardwolf / pyrdp)

**Approach:** Use Python libraries that implement the RDP protocol to connect and automate.

### aardwolf
- Headless async RDP client for Python
- Can connect and receive screen updates
- Limited input simulation capabilities
- Experimental / research quality

### pyrdp
- Designed as RDP MITM tool, not an automation client
- Can inject keystrokes and run commands on new connections
- "Hackish" automation, not reliable for production use
- Primary use case is security research

### Pros
- **Pure Python** -- no external dependencies
- **Headless** -- no GUI needed
- **Cross-platform** -- works on macOS

### Cons
- **Immature for automation** -- neither library is designed for UI automation
- **Limited input support** -- basic keystrokes possible, mouse interaction spotty
- **RDP protocol complexity** -- bugs and edge cases in pure-Python RDP
- **No window awareness** -- screen-level only
- **pyrdp is a security tool** -- not meant for legitimate automation

### Setup Complexity: Medium
```bash
pip install aardwolf  # or clone from GitHub
# Limited documentation, expect experimentation
```

---

## 6. AutoHotkey on Windows Side

**Approach:** Run an AHK script on frostreaver that listens for TCP commands and executes automation.

### How It Works
- AutoHotkey v2 script runs on frostreaver
- Listens on a TCP socket for JSON commands
- Executes Win32 automation (clicks, keystrokes, screenshots)
- Returns results over the socket

### Pros
- **Powerful automation** -- AHK is purpose-built for Windows UI automation
- **Window-aware** -- can target specific windows by title/class
- **Mature ecosystem** -- decades of Windows automation scripts
- **Image search** -- built-in `ImageSearch` for visual verification

### Cons
- **Another runtime** -- need to install AutoHotkey on frostreaver
- **Another language** -- AHK scripting language is quirky
- **TCP server is DIY** -- AHK's socket support is community-maintained, not built-in
- **Duplicates what Rust can do** -- SendInput from AHK == SendInput from Rust
- **No HTTP** -- would need a custom protocol or wrapper
- **Maintenance burden** -- now maintaining AHK scripts alongside Rust code

### Setup Complexity: Medium
```
# Install AutoHotkey v2 on frostreaver
# Write TCP server script
# Write automation handlers
# Set up auto-start
```

---

## Recommendation

### Winner: Option 1 -- Extend the Existing Rust Remote API

The best option is overwhelmingly to extend the remote API that's already running on frostreaver. Here's why:

1. **Infrastructure already exists** -- The API server is running, auto-starts on login, and is accessible from macOS. We just need new endpoints.

2. **Native Win32 is the gold standard** -- `SendInput` is what AutoHotkey, VNC servers, and every other Windows automation tool uses under the hood. Calling it directly from Rust eliminates every middleware layer.

3. **Window-aware automation** -- Unlike VNC/RDP (which only see pixels), Win32 APIs can find windows by title/class, get their positions, activate them, and send targeted input. This is critical for managing 36 EQ clients.

4. **No new dependencies** -- The `windows` crate is already in the project. No VNC server, no Guacamole, no Python, no AHK.

5. **Screenshots without screen sharing** -- `BitBlt`/`PrintWindow` capture window contents directly from the compositor, faster and more reliable than any remote desktop protocol.

6. **Already proven** -- The existing `/launch-eq`, `/inject`, `/kill-eq` endpoints demonstrate this pattern works perfectly.

### Runner-up: Option 2 (VNC + vncdotool)

If we need visual verification beyond what Win32 screenshots provide (e.g., "does this screen look right?" image comparison), VNC + vncdotool is the best supplementary tool. The `expectScreen()` feature for waiting until a specific UI state appears is genuinely useful. Could be added later as a complement.

### Implementation Priority

```
Phase 1: Core automation (extend Rust API)
  - POST /screenshot (desktop + per-window)
  - POST /click, /key (SendInput wrappers)
  - POST /find-window, /activate-window
  ~200-300 lines of Rust

Phase 2: Visual verification (if needed)
  - Install TightVNC on frostreaver
  - pip install vncdotool on macOS
  - Write image-matching helpers

Phase 3: Complex flows (if needed)
  - Chain API calls into login/launch sequences
  - Add OCR for reading UI text (tesseract)
  - Add template matching for UI element detection
```

---

## Sources

- [FreeRDP GitHub](https://github.com/FreeRDP/FreeRDP)
- [xfreerdp man page](https://www.mankier.com/1/xfreerdp)
- [vncdotool GitHub](https://github.com/sibson/vncdotool)
- [vncdotool PyPI](https://pypi.org/project/vncdotool/)
- [vncdotool Python API docs](https://vncdotool.readthedocs.io/en/latest/library.html)
- [Apache Guacamole](https://guacamole.apache.org/)
- [Guacamole Architecture](https://guacamole.apache.org/doc/gug/guacamole-architecture.html)
- [aardwolf - Headless RDP client](https://github.com/skelsec/aardwolf)
- [pyrdp - RDP MITM tool](https://github.com/GoSecure/pyrdp)
- [Win32 SendInput (Rust)](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/KeyboardAndMouse/fn.SendInput.html)
- [Win32 SendInput (Microsoft)](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
- [AHKsock - AHK TCP](https://github.com/jleb/AHKsock)
- [RemoteKeyStrokes](https://github.com/U53RW4R3/RemoteKeyStrokes)
