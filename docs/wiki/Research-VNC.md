# VNC Remote Desktop Control Research

Research into using VNC from macOS (Claude Code) to programmatically control Windows 11 desktop.

## 1. Existing VNC MCP Servers

Three production-ready VNC MCP servers already exist:

### Option A: hrrrsn/mcp-vnc (TypeScript) — RECOMMENDED
- **GitHub**: https://github.com/hrrrsn/mcp-vnc
- **Stars**: 31 | **Language**: TypeScript
- **Install**: `npm install -g @hrrrsn/mcp-vnc`
- **Tools provided**:
  - `vnc_screenshot` — capture screen with configurable delay (0-300000ms)
  - `vnc_click` — click at coordinates (left/right/middle, double-click)
  - `vnc_move_mouse` — position cursor at X,Y
  - `vnc_key_press` — send individual keys or combos like "Ctrl+Alt+Delete"
  - `vnc_type_text` — type single-line text with optional Enter
  - `vnc_type_multiline` — type multiple lines in sequence
- **Config**: Set VNC host, port, password as env vars in Claude Desktop/VS Code config
- **Supports**: Any VNC server (TightVNC, TigerVNC, RealVNC, UltraVNC)
- **Verdict**: Most mature, best tool coverage, easy npm install. **This is the one to use.**

### Option B: leonszimmermann/mcp-vnc (Python)
- **GitHub**: https://github.com/leonszimmermann/mcp-vnc
- **Stars**: 7 | **Language**: Python
- **Install**: pip install fastmcp, pillow, vncdotool
- **Uses**: vncdotool under the hood
- **Config**: VNC host + password as env vars, requires absolute paths in config
- **Note**: Recommends 1024x768 resolution for optimal performance
- **Verdict**: Simpler/lighter, but less mature than hrrrsn version.

### Option C: mayflower/vnc-use (Python, LLM-driven)
- **GitHub**: https://github.com/mayflower/vnc-use
- **Stars**: Unknown | **Language**: Python
- **What it does**: Full autonomous desktop agent — observe-propose-act loop
  - Captures screenshots, uses LLM to decide next action, executes it
  - Supports Gemini 2.5 Computer Use and Claude Haiku 4.5
- **MCP mode**: Exposes `execute_vnc_task` tool via FastMCP 2.0
- **Docker**: Pre-configured on ports 5901 (VNC), 6901 (web), 8001 (MCP)
- **Verdict**: Overkill for our needs — we want Claude Code to be the brain, not a nested LLM agent. But interesting for reference.

### Option D: Windows Remote Control MCP (nut.js-based)
- **PulseMCP**: https://www.pulsemcp.com/servers/cheffromspace-windows-remote-control
- **Stars**: 306 | **Uses**: nut.js library
- **What it does**: Mouse, keyboard, screen capture — but runs locally on Windows, not over VNC
- **Verdict**: Requires running the MCP server ON the Windows machine. Not useful for remote control from macOS.

## 2. VNC Servers for Windows 11

### TightVNC — RECOMMENDED for simplicity
- Free, open source, lightweight
- Supports Windows 11 (32-bit and 64-bit)
- Simple installation, minimal resource usage
- Good compatibility with vncdotool and MCP servers
- Download: https://www.tightvnc.com/

### UltraVNC — Best for Windows-specific features
- Free, open source, Windows-only
- Full Windows 11 support (through Server 2026)
- MS Logon authentication (Active Directory)
- DSM encryption plugins
- File transfer, multi-monitor, chat
- Best choice if you need Windows-specific features

### TigerVNC — Best cross-platform
- Fork of TightVNC, well-maintained
- Better performance than TightVNC
- More Linux-focused, but works on Windows
- Good option if also controlling Linux boxes

### RealVNC — Commercial
- Free tier available, commercial for full features
- Most polished UI and setup experience
- Cloud connectivity (no port forwarding needed)
- Overkill for a local network automation setup

**Recommendation**: TightVNC for initial setup (simplest), upgrade to UltraVNC if we need encryption or multi-monitor.

## 3. VNC Automation from macOS (without MCP)

### vncdotool (Python)
- **PyPI**: https://pypi.org/project/vncdotool/
- **Docs**: https://vncdotool.readthedocs.io/
- Command-line VNC client + Python library
- Features: screenshots, keystrokes, mouse clicks, recording sessions
- Install: `pip install vncdotool`
- Example: `vncdo -s vnc-host type "hello" key enter capture screenshot.png`
- Can embed in Python scripts via `vncdotool.client.VNCDoToolClient`
- **Used by**: leonszimmermann/mcp-vnc under the hood

### Apache Guacamole + Playwright (Alternative approach)
- Guacamole exposes VNC/RDP as HTML5 web page
- Playwright MCP is already available in Claude Code
- Could interact with the VNC session through the web interface
- **Verdict**: Adds unnecessary complexity (Guacamole server + Playwright). Direct VNC MCP is simpler.

## 4. Recommended Architecture

```
┌─────────────────┐     VNC Protocol     ┌──────────────────┐
│  macOS (Claude)  │ ──────────────────── │  Windows 11 PC   │
│                  │     Port 5900        │  (frostreaver)   │
│  Claude Code     │                      │                  │
│    ↕ MCP         │                      │  TightVNC Server │
│  hrrrsn/mcp-vnc  │                      │  EverQuest       │
│  (TypeScript)    │                      │  Frostreaver DLL │
└─────────────────┘                      └──────────────────┘
```

### Setup Steps

#### Windows side (frostreaver mini PC):
1. Download and install TightVNC Server from https://www.tightvnc.com/
2. Set a VNC password during installation
3. Configure to start as Windows service (auto-start on boot)
4. Open port 5900 in Windows Firewall (or use SSH tunnel)
5. Set resolution to 1920x1080 (or 1024x768 for faster screenshots)

#### macOS side:
1. Install the MCP server:
   ```bash
   npm install -g @hrrrsn/mcp-vnc
   ```

2. Add to Claude Code MCP config (`~/.claude/settings.json` or project `.mcp.json`):
   ```json
   {
     "mcpServers": {
       "vnc": {
         "command": "mcp-vnc",
         "env": {
           "VNC_HOST": "frostreaver-ip",
           "VNC_PORT": "5900",
           "VNC_PASSWORD": "your-vnc-password"
         }
       }
     }
   }
   ```

3. Restart Claude Code — VNC tools should appear

#### Usage from Claude Code:
- `vnc_screenshot` — see the current desktop
- `vnc_click(x, y)` — click on UI elements
- `vnc_type_text("notepad")` — type text
- `vnc_key_press("win")` — open Start menu
- `vnc_key_press("enter")` — confirm dialogs

### Security Considerations
- VNC traffic is unencrypted by default
- For local network (same room/house): acceptable risk
- For remote access: use SSH tunnel (`ssh -L 5900:localhost:5900 frostreaver`)
- Or switch to UltraVNC with DSM encryption plugin

## 5. Comparison Summary

| Approach | Setup Complexity | Automation Quality | MCP Ready | Notes |
|----------|-----------------|-------------------|-----------|-------|
| **hrrrsn/mcp-vnc + TightVNC** | Low | High | Yes | **Best option** |
| leonszimmermann/mcp-vnc | Low | High | Yes | Python alternative |
| vnc-use (mayflower) | Medium | Very High | Yes | Overkill — nested LLM |
| Guacamole + Playwright | High | Medium | Partial | Unnecessary indirection |
| Windows Remote Control | N/A | High | Yes | Must run on Windows |
| Custom MCP + vncdotool | Medium | High | DIY | Only if others don't work |

## 6. Limitations & Considerations

- **Screenshot latency**: VNC screenshots take 0.5-2s depending on resolution and network
- **No OCR built-in**: Claude must interpret screenshots visually (which it's good at)
- **Resolution matters**: Lower resolution = faster screenshots but less screen real estate
- **Coordinate precision**: Need to know exact X,Y coords — screenshot → visual analysis → click
- **No window management API**: Can't enumerate windows or read text programmatically — it's all visual
- **Complements, doesn't replace**: VNC is for GUI tasks (launching apps, clicking through installers). For command-line tasks, SSH/WinRM/PowerShell remoting is better.

## 7. When to Use VNC vs Other Methods

| Task | Best Method |
|------|-------------|
| Run PowerShell commands | WinRM / SSH |
| Install software silently | WinRM + `winget` or `choco` |
| Launch EverQuest | VNC (need to click GUI) |
| Navigate EQ login screens | VNC |
| Edit config files | WinRM / SSH |
| Monitor desktop visually | VNC screenshot |
| Inject DLL into process | WinRM (command-line) |
| Interact with EQ UI | VNC |
