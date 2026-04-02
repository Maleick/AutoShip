# Security and Anti-Detection Notes

## Canonical Inputs

Use these sources in this order:

1. current code and repo docs
2. `docs/external-research/daybreak-detection-digest.md`
3. official Daybreak policy pages linked from that digest
4. clearly labeled secondary community reporting

Do not present community inference as official detection fact.

## Current Security Measures

From the current codebase:

- session tokens are 32 random bytes generated from OS entropy
- pipe and shared-memory names are derived from the per-session token
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
- runner and live-play machines should stay clean of unrelated cheat tooling because Daybreak policy is broader than one game client

## Anti-Detection Research Posture

Current practical measures include:

- randomized DLL staging names
- session-derived IPC names rather than a fixed simple prefix scheme
- movement humanization in the navigation layer
- personality and timing variation in higher-level behavior
- render strobing for background clients

Current documentation rules:

- official Daybreak guidance can tighten milestone gates immediately
- community reports can suggest validation tasks or provisional slices
- exploit-oriented travel or control claims remain low-confidence until corroborated
- no anti-detection measure should be documented as a guarantee

## Important Nuance About Prefixes

The repo still contains legacy prefix constants in `dmft-common/src/ipc.rs`, but the active naming helpers derive names from the session token:

- `pipe_name(session_id, client_id)`
- `shared_memory_name(session_id, client_id)`

When documenting behavior, prefer the active helper behavior over legacy constant names.

## Known Risks and Limits

- restriction to the current user is helpful, but it does not eliminate local forensic or anti-cheat risk
- login UI automation remains patch-sensitive
- any EQ patch can invalidate offsets or widget assumptions
- operator mistakes such as stale tokens, wrong build artifacts, or high-visibility behavior can still expose brittle paths
- packet or zoning research does not automatically mean a path is safe to execute live

## Current Behavior vs Roadmap

### Current behavior

- the repository already contains concrete security-minded implementation details
- the new `M7` anti-cheat milestone is about hardening gates, evidence handling, and validation discipline

### Roadmap and validation notes

- treat live-EQ anti-detection claims as bounded operational guidance, not guarantees
- use the Daybreak digest to separate official signals from community inference
