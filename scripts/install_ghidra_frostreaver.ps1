# Ghidra + GhidraMCP Installation Script for Frostreaver
# Run as xmale in PowerShell (Admin not required)

$ErrorActionPreference = "Stop"
$ProgressPreference = "Continue"

# Config
$GhidraVersion = "11.3.2"
$GhidraUrl = "https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_${GhidraVersion}_build/ghidra_${GhidraVersion}_PUBLIC_20250415.zip"
$GhidraMcpUrl = "https://github.com/LaurieWired/GhidraMCP/releases/download/GhidraMCP-1.4/GhidraMCP-1-4.zip"
$InstallDir = "C:\ghidra"
$DownloadsDir = "$env:USERPROFILE\Downloads"

# Ensure install dir
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

# Download Ghidra
$GhidraZip = "$DownloadsDir\ghidra_${GhidraVersion}_PUBLIC.zip"
if (!(Test-Path $GhidraZip)) {
    Write-Host "Downloading Ghidra ${GhidraVersion}..."
    Invoke-WebRequest -Uri $GhidraUrl -OutFile $GhidraZip -UseBasicParsing
} else {
    Write-Host "Ghidra zip already downloaded."
}

# Extract Ghidra
Write-Host "Extracting Ghidra to ${InstallDir}..."
Expand-Archive -Path $GhidraZip -DestinationPath $InstallDir -Force

# Find extracted folder (usually ghidra_${GhidraVersion}_PUBLIC)
$GhidraHome = (Get-ChildItem -Path $InstallDir -Directory | Select-Object -First 1).FullName
Write-Host "Ghidra home: $GhidraHome"

# Download GhidraMCP
$McpZip = "$DownloadsDir\GhidraMCP-1-4.zip"
if (!(Test-Path $McpZip)) {
    Write-Host "Downloading GhidraMCP 1.4..."
    Invoke-WebRequest -Uri $GhidraMcpUrl -OutFile $McpZip -UseBasicParsing
} else {
    Write-Host "GhidraMCP zip already downloaded."
}

# Extract GhidraMCP
$McpDir = "$InstallDir\GhidraMCP"
if (!(Test-Path $McpDir)) {
    New-Item -ItemType Directory -Path $McpDir | Out-Null
}
Expand-Archive -Path $McpZip -DestinationPath $McpDir -Force

# Verify Java
$java = Get-Command java -ErrorAction SilentlyContinue
if (!$java) {
    Write-Warning "Java not found in PATH. Ghidra requires JDK 17+. Please install from https://adoptium.net/"
} else {
    Write-Host "Java found: $($java.Source)"
}

Write-Host ""
Write-Host "=== INSTALLATION COMPLETE ==="
Write-Host "Ghidra home: $GhidraHome"
Write-Host "GhidraMCP plugin: $McpDir"
Write-Host ""
Write-Host "Next steps (manual):"
Write-Host "1. Launch Ghidra:  ${GhidraHome}\ghidraRun.bat"
Write-Host "2. File -> Install Extensions -> + -> Select ${McpDir}\GhidraMCP-1-4.zip"
Write-Host "3. Restart Ghidra"
Write-Host "4. File -> Configure -> Developer -> Enable GhidraMCPPlugin"
Write-Host "5. Load eqgame.exe, run auto-analysis"
Write-Host "6. Enable HTTP server: Edit -> Tool Options -> GhidraMCP HTTP Server"
Write-Host ""
Write-Host "MCP bridge config for Claude Code:"
Write-Host "  python $McpDir\bridge_mcp_ghidra.py --ghidra-server http://127.0.0.1:8080/"
