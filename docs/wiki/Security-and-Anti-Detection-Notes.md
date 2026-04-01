# Security and Anti-Detection Notes

## Current Security Measures

From the current codebase:

- session tokens are 32 random bytes generated from OS entropy
- pipe and shared-memory names are derived from that per-session token
- named pipes require the raw token as the first authenticated message
- token comparison in the DLL is constant-time
- shared memory uses a current-user DACL
- named pipes use restrictive current-user security attributes
- login passwords are zeroized after use inside the DLL
- staged DLL filenames are randomized before injection

## Current Operator Implications

- authenticated IPC is per injected session, not just per PID
- reconnect-style commands depend on the retained `login_token_<pid>.bin` file
- if you copy or relaunch clients, do not assume an old token file is still valid

## Anti-Detection Notes

Current practical measures include:

- randomized DLL staging names
- session-derived IPC names rather than a simple static pipe prefix path
- movement humanization in the navigation layer
- personality and timing variation in higher-level behavior
- render strobing for background clients

## Important Nuance About Prefixes

The repo still contains legacy prefix constants in `dmft-common/src/ipc.rs`, but the active naming helpers currently derive names from the session token:

- `pipe_name(session_id, client_id)`
- `shared_memory_name(session_id, client_id)`

When documenting current behavior, prefer the active helper behavior over the legacy constant names.

## Known Risks and Limits

- restriction to the current user is helpful, but it does not eliminate all local forensic or anti-cheat risk
- login UI automation remains patch-sensitive
- any EQ patch can invalidate offsets or widget assumptions
- operator mistakes such as stale tokens, wrong build artifacts, or using placeholder UI actions can still expose brittle behavior

## Current Behavior vs Roadmap

### Current behavior

- The repository already contains real security-minded implementation details, not only future ideas.

### Roadmap and validation notes

- Hardening is an ongoing area rather than a finished milestone.
- Treat all live-EQ anti-detection claims as bounded and operational, not as guarantees.
