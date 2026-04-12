<div align="center">
  <img src="assets/beacon-banner.png" alt="Beacon" width="600" />

# Beacon: Autonomous Multi-Agent GitHub Orchestration

[![Version](https://img.shields.io/badge/version-v0.1-blue?style=flat-square)](https://github.com/Maleick/beacon/releases)
[![GitHub Stars](https://img.shields.io/github/stars/Maleick/beacon?style=flat-square&color=yellow)](https://github.com/Maleick/beacon/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/Maleick/beacon?style=flat-square)](https://github.com/Maleick/beacon/issues)
[![Platform](https://img.shields.io/badge/platform-Claude%20Code-blueviolet?style=flat-square)](https://claude.ai/code)
[![Plugin](https://img.shields.io/badge/type-plugin-orange?style=flat-square)](https://github.com/Maleick/beacon)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

</div>

**Beacon** is a Claude Code plugin that autonomously dispatches AI agents to resolve GitHub issues — routing work to Codex, Gemini, or Claude based on complexity and quota, verifying results, and merging PRs with zero human intervention.

## How It Works

Beacon runs a four-tier orchestration loop inside Claude Code:

1. **Monitor** — Three bash watchers poll GitHub issues (60s), PR status (30s), and agent completion (5s) via the Monitor tool
2. **Triage** — Claude Haiku interprets events and writes prioritized actions to an event queue
3. **Execute** — Claude Sonnet consumes the queue, dispatches agents to isolated git worktrees, and drives the verification pipeline
4. **Advise** — Claude Opus is spawned only at strategic decision points (UltraPlan, phase checkpoints, repeated failures)

```
GitHub Issues → Monitor Scripts → Haiku Triage → Sonnet Executor → Opus Advisor
                     ↑                                  ↓
              PR/CI/agent events           Codex · Gemini · Grok · Claude agents
                                                         ↓
                                          Verify → Simplify → PR → Merge
```

## Key Features

**Third-party first dispatch.** Beacon routes simple and medium issues to Codex, Gemini, or Grok before burning Claude quota. Claude Haiku and Sonnet serve as reliable fallbacks, with Opus reserved for strategic decisions only.

**Worktree isolation.** Each issue gets its own git worktree and tmux pane. Agents work concurrently without stepping on each other. Completion is detected via structured status words (`COMPLETE`, `BLOCKED`, `STUCK`) captured in pane logs — no polling required.

**Tiered escalation.** Haiku handles simple issues (2-3 files). On failure, it retries once with context, then auto-promotes to Sonnet. Sonnet escalates to Opus after two failures. PR comments are triaged the same way: Haiku fixes nits, Sonnet handles bugs, Opus handles design escalations.

**Verification pipeline.** Every completed issue runs through: Sonnet review against acceptance criteria → simplify pass → PR creation → CI monitoring → auto-merge (simple) or human review gate (complex).

**Discord integration.** Webhook events trigger issue dispatch in real time. A command channel accepts operator commands (`work on`, `skip`, `pause`, `resume`) without leaving Discord.

**Quota tracking.** A decay-based quota estimator updates `quota.json` after each dispatch and surfaces usage in `/beacon:status` as ASCII progress bars.

## Installation

Requires Claude Code with plugin support.

```bash
# Install via plugin marketplace
/install-plugin https://github.com/maleick/beacon

# Or install locally during development
/install-plugin /path/to/beacon
```

**Prerequisites:**

- `jq` — `brew install jq`
- `gh` — GitHub CLI, authenticated (`gh auth login`)
- At least one AI CLI tool: `codex`, `gemini`, or `grok` (optional but recommended for quota efficiency)

## Commands

| Command          | Purpose                                                     |
| ---------------- | ----------------------------------------------------------- |
| `/beacon:start`  | Launch orchestration for the current repo                   |
| `/beacon:status` | Show running agents, quota bars, and progress               |
| `/beacon:stop`   | Gracefully stop all agents                                  |
| `/beacon:plan`   | Analyze issues and build execution plan without dispatching |

## Status Display

```
● Beacon — 3 active · 7 complete · 1 blocked
─────────────────────────────────────────────
  #12  feature: add dark mode       [Codex  ] 4m
  #15  fix: auth token expiry       [Haiku  ] 2m
  #18  refactor: query optimizer    [Sonnet ] 11m

  Quota  Claude  ████████████░░░░░░░░  61%
         Codex   ██████░░░░░░░░░░░░░░  31%
```

## Architecture

| Role             | Model                  | Trigger                                                     |
| ---------------- | ---------------------- | ----------------------------------------------------------- |
| Executor         | Sonnet                 | Every event from queue                                      |
| Advisor          | Opus                   | UltraPlan · phase checkpoint · 2+ failures · LOW_CONFIDENCE |
| Worker (simple)  | Haiku / Codex / Gemini | 1-3 file changes                                            |
| Worker (medium)  | Sonnet / Gemini Pro    | Multi-file, moderate complexity                             |
| Worker (complex) | Sonnet + autoresearch  | Cross-cutting, architectural                                |
| Triage           | Haiku                  | Every Monitor event                                         |
| Reviewer         | Sonnet                 | Post-completion verification                                |

State persists in `.beacon/state.json` (local) and GitHub labels (durable across sessions).

## Plugin Structure

```
commands/beacon.md          — /beacon entry point
skills/beacon/              — Core orchestration protocol
skills/beacon-dispatch/     — Agent dispatch (third-party first)
skills/beacon-verify/       — Post-completion pipeline
skills/beacon-status/       — Status display with quota bars
skills/beacon-poll/         — GitHub issue sync safety net
agents/haiku-triage.md      — Haiku event interpreter
agents/monitor.md           — CI/PR monitor agent
agents/reviewer.md          — Sonnet verification reviewer
hooks/beacon-init.sh        — Initialize .beacon/ state directory
hooks/detect-tools.sh       — Detect available AI CLIs + quota
hooks/monitor-agents.sh     — Agent completion watcher (5s)
hooks/monitor-prs.sh        — PR CI/merge status watcher (30s)
hooks/monitor-issues.sh     — GitHub issue watcher (60s)
hooks/cleanup-worktree.sh   — Remove worktree + branch on close
hooks/sweep-stale.sh        — Clean orphaned worktrees on startup
```

## Design Decisions

Eleven v3 design decisions are documented in [wiki/Design-Decisions.md](wiki/Design-Decisions.md). Key choices:

- **Sonnet as executor, not Opus** — Opus is expensive; Sonnet handles reactive event processing at scale
- **Monitor over CronCreate** — Monitor tool streams events; cron polling misses rapid state changes
- **Status words over pane_dead** — `COMPLETE`/`BLOCKED`/`STUCK` are reliable; pane death is ambiguous
- **Third-party first** — Preserving Claude Max quota for complex work extends daily capacity significantly

See [BEACON_SPEC.md](BEACON_SPEC.md) for the full specification and [wiki/Architecture.md](wiki/Architecture.md) for the complete four-tier model.
