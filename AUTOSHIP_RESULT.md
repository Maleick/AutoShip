# Result: #1143 — #866: Account Safety & Detection Framework [PARENT]
Status: DONE
- Implemented account safety plumbing in orchestrator: chat-sourced ban/suspension detection via `textquest_common::account_safety`, immediate client isolation (`/camp desktop` + removal), and ban registry tracking via `is_banned_client`.
- Added GM/CSR alert rate limiting for chat-based GM tells (120s cooldown) to prevent spam.
- Added server-status-aware launch pause in `LaunchCoordinator` using `server.status_url` with periodic blocking HTTP checks and automatic resume when status recovers.
- Prevented relaunch/re-registration of banned clients by purging/guarding in `OrchestratorLoop` health and launch readiness paths.
- Validation: `cargo check` passed.
COMPLETE
