# Running AI Coding Agents Locally on Frostreaver (Windows 11)

Research date: 2026-03-29

## Problem Statement

Currently we develop on macOS and remote-control frostreaver via SSH/API. This causes **session isolation** — GUI apps launched from SSH run in session 0 (invisible on the desktop). We need a solution where an AI coding agent can:

1. Edit Rust code and run `cargo build`
2. Launch `eqgame.exe` (visible on desktop)
3. Run the DLL injector (`textquest.exe --inject`)
4. Read DLL log files
5. Iterate autonomously (build -> test -> check logs -> fix -> repeat)

---

## Option 1: Claude Code Native on Windows (RECOMMENDED)

**Feasibility: HIGH — This works TODAY**

Claude Code runs natively on Windows 10/11. No WSL required (though WSL is also supported). It uses Git Bash internally for shell commands but can be launched from PowerShell or CMD.

### How It Works

- Claude Code is a CLI tool that runs in a terminal
- On Windows, it uses Git Bash internally to execute commands
- It can run any shell command, including launching GUI apps
- Since it runs in the desktop session, **no session isolation**
- Auto-updates in the background

### Setup Steps

1. Install [Git for Windows](https://git-scm.com/downloads/win) on frostreaver
2. Install Claude Code via PowerShell:
   ```powershell
   irm https://claude.ai/install.ps1 | iex
   ```
   Or via WinGet:
   ```powershell
   winget install Anthropic.ClaudeCode
   ```
3. Install Rust toolchain (`rustup`) on frostreaver
4. Clone the repo: `git clone https://github.com/<repo> C:\Projects\TextQuest`
5. Run `claude` in the project directory
6. Authenticate with Anthropic account

### Can It Launch GUI Apps?

**Yes.** Claude Code's Bash tool executes commands in the desktop session. Running `Start-Process eqgame.exe` or calling it directly will launch the game visibly on the desktop. No session isolation because Claude Code IS running on the desktop.

### Pros

- Zero session isolation — runs in the user's desktop session
- Full access to cargo, git, all CLI tools
- Can launch eqgame.exe, run textquest.exe --inject, read logs
- Auto-updates, auto-installs, zero-config
- Same CLAUDE.md / HANDOFF.md workflow we already use
- Native Windows binary, signed by Anthropic
- Can use the same API key / Max subscription

### Cons

- Requires someone to RDP in or use the physical console to start it (one-time)
- Git Bash shell, not native PowerShell (PowerShell is opt-in preview)
- No GUI interaction (can't click buttons in EQ) — CLI only
- Needs a terminal open on the desktop

### Verdict

**This is the primary recommendation.** It solves the session isolation problem completely. Claude Code on frostreaver can do everything our macOS Claude Code does, PLUS launch GUI apps that are actually visible.

---

## Option 2: Claude Computer Use (FUTURE — macOS only today)

**Feasibility: NOT YET on Windows**

Computer Use lets Claude see the screen, move the mouse, and click buttons. This would be the dream — Claude could literally play EQ.

### Current Status (March 2026)

- Computer Use launched in Cowork and Claude Code Desktop on **macOS only**
- Available to Claude Pro ($20/mo) and Claude Max ($100-200/mo) subscribers
- Windows support is **"coming soon"** — no confirmed date
- Cowork got Windows support in Feb 2026, so Computer Use on Windows is likely Q2 2026

### What It Could Do

- See the EQ window on screen
- Click login buttons, navigate menus
- Read on-screen text, observe game state visually
- Full desktop automation — no DLL injection needed for basic control

### Pros

- Ultimate solution — Claude can see and interact with the game
- Could handle login flows, character selection visually
- No need for memory reading for basic observation

### Cons

- **Not available on Windows yet**
- Still in "research preview" — may be unreliable
- Would be slow compared to direct memory reading / function calls
- Can't replace DLL injection for combat automation (too slow)
- Unclear if it can handle fullscreen DirectX windows

### Verdict

**Watch this space.** When Computer Use ships on Windows, it becomes a powerful complement to our DLL approach — useful for login automation and visual verification, but not a replacement for the core TextQuest architecture. Check back monthly.

---

## Option 3: Claude Code via WSL2 on Frostreaver

**Feasibility: HIGH — works, but Option 1 is simpler**

Run Claude Code inside WSL2 (Ubuntu), use WSL interop to launch Windows executables.

### How It Works

- Install WSL2 + Ubuntu on frostreaver
- Install Claude Code in WSL: `curl -fsSL https://claude.ai/install.sh | bash`
- Claude Code runs in Linux, but can call Windows binaries via interop:
  ```bash
  cmd.exe /C "C:\path\to\eqgame.exe"
  powershell.exe -Command "Start-Process eqgame.exe"
  ```
- WSL2 can access Windows filesystem at `/mnt/c/`

### Session Isolation

**Partially solved.** WSL interop launches Windows processes in the desktop session IF WSL was started from the desktop. If WSL is started via SSH, the same session 0 problem may occur.

### Pros

- Full Linux environment (better tool compatibility)
- WSL2 supports sandboxing for Claude Code
- Can cross-compile or use Windows cargo via interop
- Access to both Linux and Windows toolchains

### Cons

- Extra complexity (WSL + Windows interop)
- Performance overhead for cross-filesystem access
- PowerShell interop adds ~700ms per invocation
- Cargo/Rust would need to be installed on Windows side anyway (for MSVC target)
- Session isolation still possible if WSL started remotely
- Known issue: Claude Code in WSL2 spawns powershell.exe 38+ times on startup causing freezes

### Verdict

**Unnecessary if Option 1 works.** WSL adds complexity without clear benefit. Use native Windows Claude Code instead. WSL is a good fallback if native Windows has issues.

---

## Option 4: Cursor IDE on Frostreaver

**Feasibility: MEDIUM — works but overkill**

Cursor is a VS Code fork with built-in AI (uses Claude or GPT). It has a terminal and can run commands.

### How It Works

- Install Cursor on frostreaver
- Open the TextQuest project
- Use Cursor's AI chat + terminal to build, test, iterate
- Terminal runs in the desktop session — can launch GUI apps

### Pros

- Nice GUI IDE experience
- Terminal access in desktop session (no isolation)
- Built-in AI code editing
- Good for visual code review

### Cons

- Cursor's AI is less capable than Claude Code for autonomous iteration
- No equivalent of CLAUDE.md / HANDOFF.md workflow
- Can't run headless / unattended as easily
- Monthly subscription ($20/mo) on top of Claude subscription
- Less control over which model is used
- No equivalent of Claude Code's agent loop / tool use

### Verdict

**Not recommended as primary tool.** Cursor is great for interactive coding but can't match Claude Code's autonomous capabilities. Could be useful as a secondary IDE for manual work on frostreaver.

---

## Option 5: Hybrid — Claude Code on Frostreaver + GitHub Sync

**Feasibility: HIGH — this is how to operationalize Option 1**

### The Workflow

1. **Development** happens on macOS with Claude Code (current workflow)
2. **Testing/deployment** happens on frostreaver with its own Claude Code instance
3. GitHub is the sync mechanism — push from macOS, pull on frostreaver
4. Each machine has its own CLAUDE.md but shares the repo

### Concrete Setup

```
macOS (development):
  - Edit code, run cargo build, run tests
  - Push to GitHub

frostreaver (deployment + live testing):
  - git pull
  - cargo build --release
  - Launch eqgame.exe
  - Run textquest.exe --inject
  - Check logs, iterate on bugs
  - Push fixes back to GitHub
```

### Using HANDOFF.md

The existing HANDOFF.md approach works perfectly:
1. macOS Claude Code writes HANDOFF.md with context
2. Push to GitHub
3. frostreaver Claude Code reads HANDOFF.md, has full context
4. Can continue work autonomously

### Pros

- Best of both worlds — develop on macOS, deploy on Windows
- No remote control needed
- Each Claude Code instance is fully autonomous
- GitHub provides audit trail
- Can run simultaneously on both machines

### Cons

- Two separate Claude Code sessions (two context windows)
- Need to manage sync discipline (push/pull)
- API costs for two concurrent sessions

### Verdict

**This is the recommended operational model.** Use Option 1 (native Claude Code on frostreaver) with GitHub sync. macOS for development, frostreaver for live testing.

---

## Recommendation Summary

| Option | Feasibility | Session Isolation Solved? | Recommended? |
|--------|------------|--------------------------|--------------|
| 1. Claude Code Native Windows | HIGH | YES | **PRIMARY** |
| 2. Claude Computer Use | NOT YET | YES (when available) | WATCH |
| 3. Claude Code via WSL2 | HIGH | PARTIAL | FALLBACK |
| 4. Cursor IDE | MEDIUM | YES | NO |
| 5. Hybrid (macOS + frostreaver) | HIGH | YES | **OPERATIONAL MODEL** |

### Immediate Action Plan

1. **RDP into frostreaver** (or use physical console)
2. **Install Git for Windows**: `winget install Git.Git`
3. **Install Claude Code**: `irm https://claude.ai/install.ps1 | iex`
4. **Install Rust**: `winget install Rustlang.Rustup`
5. **Clone repo**: `git clone <repo-url> C:\Projects\TextQuest`
6. **Run Claude Code**: Open PowerShell, `cd C:\Projects\TextQuest`, `claude`
7. **Test**: Ask Claude Code to run `cargo build` and launch a test `.exe`

### Long-term

- Monitor Claude Computer Use for Windows release (likely Q2 2026)
- When available, Computer Use + Claude Code = full autonomous EQ control
- Consider setting up a persistent terminal session (Windows Terminal) that auto-starts Claude Code on boot

---

## Sources

- [Claude Code Advanced Setup (Official)](https://code.claude.com/docs/en/setup)
- [Claude Code Desktop App](https://code.claude.com/docs/en/desktop)
- [Claude Computer Use in Cowork](https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork)
- [Claude Computer Use API Docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)
- [WSL Interop Documentation](https://learn.microsoft.com/en-us/windows/wsl/filesystems)
- [Claude Code Windows Native Install Guide](https://smartscope.blog/en/generative-ai/claude/claude-code-windows-native-installation/)
- [Claude Code WinGet Install](https://windowsforum.com/threads/install-claude-code-on-windows-11-with-winget-fast-node-js-free-setup.398164/)
- [Claude Code WSL2 Known Issue](https://github.com/anthropics/claude-code/issues/29672)
- [Anthropic March 2026 Updates](https://www.builder.io/blog/claude-code-updates)
