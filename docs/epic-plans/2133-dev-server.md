# Epic #2133: Dev server setup + backend hosting decoupled from Frostreaver

**Epic issue:** https://github.com/Maleick/TextQuest/issues/2133
**Created:** 2026-04-21
**Status:** In decomposition

## Goal

Decouple textquest-web (Axum backend) from Frostreaver (Windows gaming rig) so the backend
runs always-on on a stable host. The DLL stays on Frostreaver — Windows named pipes to
eqgame.exe are immovable. Only the dll↔backend communication channel changes.

## Architecture constraints

| Component                            | Stays on                       | Reason                                      |
| ------------------------------------ | ------------------------------ | ------------------------------------------- |
| textquest-dll                        | Frostreaver                    | Named pipes to eqgame.exe are Windows-local |
| textquest-web (Axum)                 | Remote host (TBD)              | Goal of this epic                           |
| web/ (Vite SPA)                      | CDN or served by Axum          | Static assets, host-agnostic                |
| Discord bot + SQLite + session state | Remote host with textquest-web | Backend responsibilities                    |

## Child issues

### Decision phase

| Issue | Title                                                                                                  | Labels                      | Depends on |
| ----- | ------------------------------------------------------------------------------------------------------ | --------------------------- | ---------- |
| #2247 | Research: hosting comparison matrix (Mac mini vs DO vs Fly.io vs Hetzner vs Oracle Free)               | agent:ready, mode:research  | —          |
| #2249 | Research: Frostreaver↔backend transport (Tailscale vs WireGuard vs WebSocket-TLS vs Cloudflare Tunnel) | agent:ready, mode:research  | —          |
| #2252 | ADR: decide backend host and document decision                                                         | agent:ready, human:required | #2247      |
| #2253 | ADR: decide Frostreaver↔backend transport and document decision                                        | agent:ready, human:required | #2249      |

### Implementation phase

| Issue | Title                                                                   | Labels                     | Depends on   |
| ----- | ----------------------------------------------------------------------- | -------------------------- | ------------ |
| #2256 | Refactor: textquest-dll outbound WebSocket to config-driven backend URL | agent:ready, enhancement   | #2253        |
| #2258 | Refactor: textquest-web authenticated WebSocket API for dll control     | agent:ready, enhancement   | #2253        |
| #2260 | DevOps: backend deploy pipeline (Docker + GHA → chosen host)            | agent:ready, enhancement   | #2252        |
| #2262 | Local dev: document and wire .claude/launch.json for decoupled stack    | agent:ready, documentation | #2256, #2258 |

## Dependency graph

```
#2247 (host research)   →  #2252 (host ADR)    →  #2260 (deploy pipeline)
#2249 (transport research)  →  #2253 (transport ADR)  →  #2256 (dll refactor)
                                                          →  #2258 (web refactor)
                                                              ↓
                                                          #2262 (local dev docs)
```

Research issues (#2247, #2249) can run in parallel immediately.

## Non-goals

- Moving textquest-dll off Frostreaver
- Replacing SQLite with Postgres (separate scope)
- Changing the eqgame.exe↔dll named pipe protocol

## Deliverable sequence

1. Research issues complete → tradeoff matrices in docs/architecture/
2. ADRs accepted (human review required)
3. Implementation PRs land (dll + web refactors + CI pipeline)
4. Local dev workflow documented
5. Cutover: point production dll at remote backend with fallback plan
