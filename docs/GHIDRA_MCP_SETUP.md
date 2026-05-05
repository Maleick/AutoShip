# GhidraMCP Setup Guide for Frostreaver

## Prerequisites
- Windows 10/11 on Frostreaver
- Java JDK 17+ (download from https://adoptium.net/ if missing)
- eqgame.exe binary available

## Step 1: Install Ghidra

Download and extract:
```powershell
# Run in PowerShell as xmale
$GhidraVersion = "11.3.2"
$Url = "https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_${GhidraVersion}_build/ghidra_${GhidraVersion}_PUBLIC_20250415.zip"
Invoke-WebRequest -Uri $Url -OutFile "$env:USERPROFILE\Downloads\ghidra.zip" -UseBasicParsing
Expand-Archive -Path "$env:USERPROFILE\Downloads\ghidra.zip" -DestinationPath "C:\ghidra" -Force
```

## Step 2: Install GhidraMCP Plugin

```powershell
# Download GhidraMCP 1.4
$McpUrl = "https://github.com/LaurieWired/GhidraMCP/releases/download/GhidraMCP-1.4/GhidraMCP-1-4.zip"
Invoke-WebRequest -Uri $McpUrl -OutFile "$env:USERPROFILE\Downloads\ghidramcp.zip" -UseBasicParsing
Expand-Archive -Path "$env:USERPROFILE\Downloads\ghidramcp.zip" -DestinationPath "C:\ghidra\GhidraMCP" -Force
```

In Ghidra GUI:
1. File → Install Extensions
2. Click `+` button
3. Select `C:\ghidra\GhidraMCP\GhidraMCP-1-4.zip`
4. Restart Ghidra
5. File → Configure → Developer → Enable **GhidraMCPPlugin**

## Step 3: Load eqgame.exe

1. Create new project: File → New Project → Non-Shared
2. File → Import File → select `eqgame.exe`
3. Select "x86:LE:64:default" as language
4. Run auto-analysis (Analysis → Auto Analyze)

## Step 4: Enable GhidraMCP HTTP Server

1. Edit → Tool Options → GhidraMCP HTTP Server
2. Set port to **8080**
3. Enable the server
4. Verify: `curl http://localhost:8080/` should return JSON status

## Step 5: Network Access from Mac

Option A — Tailscale (recommended):
- Frostreaver already has Tailscale
- Ensure Tailscale is running on both Mac and Frostreaver
- Access via `http://100.90.7.126:8080/`

Option B — SSH tunnel:
```bash
# From Mac terminal
ssh -L 8080:localhost:8080 xmale@192.168.1.207
```

## Step 6: Claude Code MCP Configuration

Add to `.claude/settings.json` on your Mac:

```json
{
  "mcpServers": {
    "ghidra": {
      "command": "python",
      "args": [
        "/ABSOLUTE_PATH_TO/bridge_mcp_ghidra.py",
        "--ghidra-server",
        "http://100.90.7.126:8080/"
      ]
    }
  }
}
```

Or for SSE transport (Cline/VSC):
```bash
python bridge_mcp_ghidra.py --transport sse --mcp-host 127.0.0.1 --mcp-port 8081 --ghidra-server http://100.90.7.126:8080/
```

## Research Targets Checklist

- [ ] Movement agreement packet opcode and field layout
- [ ] Memshift detection opcodes (6 main loop opcodes every 3 min)
- [ ] A/B packet counter function location
- [ ] TLP anti-cheat system entry points
- [ ] CEverQuest::StartCasting validation (offset 0x14029D5A0)

## Troubleshooting

| Issue | Fix |
|-------|-----|
| Java not found | Install Eclipse Temurin JDK 17 from adoptium.net |
| Plugin won't load | Ensure GhidraMCP version matches Ghidra version |
| HTTP server not responding | Check Windows Firewall, allow port 8080 |
| Can't reach from Mac | Verify Tailscale connected on both machines |
