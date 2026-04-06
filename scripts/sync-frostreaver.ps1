# sync-frostreaver.ps1 — Run on Frostreaver to sync Claude Code settings and rebuild TextQuest
# Usage: powershell -ExecutionPolicy Bypass -File sync-frostreaver.ps1

$ErrorActionPreference = "Continue"

Write-Host "=== TextQuest Frostreaver Sync ===" -ForegroundColor Cyan

# 1. Pull latest master and rebuild
Write-Host "`n[1/4] Pulling master and rebuilding..." -ForegroundColor Yellow
Set-Location "C:\Users\xmale\Projects\TextQuest"
git pull origin master
cargo build --release -p textquest -p textquest-dll

# 2. Update Claude Code settings.json
Write-Host "`n[2/4] Updating Claude Code settings..." -ForegroundColor Yellow
$settingsPath = "C:\Users\xmale\.claude\settings.json"
$settings = @{
    env = @{
        CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS = "1"
        ENABLE_LSP_TOOLS = "1"
        # 1M context re-enabled (removed CLAUDE_CODE_DISABLE_1M_CONTEXT)
    }
    hooks = @{
        PostToolUse = @(
            @{
                matcher = "Write|Edit"
                hooks = @(
                    @{
                        type = "command"
                        command = "C:/Users/xmale/.claude/hooks/auto-format.sh"
                    }
                )
            }
        )
        SessionStart = @(
            @{
                matcher = "startup"
                hooks = @(
                    @{
                        type = "command"
                        command = "C:/Users/xmale/.claude/hooks/env-summary.sh"
                    },
                    @{
                        type = "command"
                        command = "C:/Users/xmale/.claude/hooks/serena-init.sh"
                    }
                )
            }
        )
    }
    enabledPlugins = @{
        "claude-md-management@claude-plugins-official" = $true
        "commit-commands@claude-plugins-official" = $true
        "superpowers@claude-plugins-official" = $true
        "hookify@claude-plugins-official" = $true
        "code-review@claude-plugins-official" = $true
        "feature-dev@claude-plugins-official" = $true
        "learning-output-style@claude-plugins-official" = $true
        "serena@claude-plugins-official" = $true
        "code-simplifier@claude-plugins-official" = $true
        "frontend-design@claude-plugins-official" = $true
        "playwright@claude-plugins-official" = $true
        "discord@claude-plugins-official" = $true
        "rust-analyzer-lsp@claude-plugins-official" = $true
        "autoresearch@autoresearch" = $true
        "codex@openai-codex" = $true
    }
    extraKnownMarketplaces = @{
        "claude-plugins-official" = @{
            source = @{
                source = "github"
                repo = "anthropics/claude-plugins-official"
            }
        }
        "autoresearch" = @{
            source = @{
                source = "github"
                repo = "Maleick/claude-autoresearch"
            }
            autoUpdate = $true
        }
        "openai-codex" = @{
            source = @{
                source = "github"
                repo = "openai/codex-plugin-cc"
            }
        }
    }
    autoDreamEnabled = $true
    skipDangerousModePermissionPrompt = $true
}
$settings | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding UTF8
Write-Host "  Settings written to $settingsPath" -ForegroundColor Green

# 3. Verify settings
Write-Host "`n[3/4] Verifying settings..." -ForegroundColor Yellow
$verify = Get-Content $settingsPath | ConvertFrom-Json
if ($verify.extraKnownMarketplaces.autoresearch) {
    Write-Host "  autoresearch marketplace: OK" -ForegroundColor Green
} else {
    Write-Host "  autoresearch marketplace: MISSING" -ForegroundColor Red
}
if ($verify.enabledPlugins."autoresearch@autoresearch") {
    Write-Host "  autoresearch plugin: OK" -ForegroundColor Green
} else {
    Write-Host "  autoresearch plugin: MISSING" -ForegroundColor Red
}
if (-not $verify.env.CLAUDE_CODE_DISABLE_1M_CONTEXT) {
    Write-Host "  1M context: ENABLED (no disable flag)" -ForegroundColor Green
} else {
    Write-Host "  1M context: DISABLED" -ForegroundColor Red
}

# 4. Summary
Write-Host "`n[4/4] Done!" -ForegroundColor Cyan
Write-Host "  - Master pulled and built"
Write-Host "  - Settings synced (autoresearch added, 1M context enabled)"
Write-Host "  - Next: Start Claude Code Desktop and open C:\Users\xmale\Projects\TextQuest"
Write-Host "  - Kara Vanguard will use Discord identity from that session"
