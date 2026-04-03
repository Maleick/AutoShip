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

## Exposure Review

Use these categories when anti-cheat work needs a bounded review instead of vague “stealth” language:

| Category | Current repo surface | Confidence | Why it matters |
| --- | --- | --- | --- |
| Module presence | `dmft-dll` is injected into `eqgame.exe` and remains loaded in-process | High | A loaded third-party DLL is itself an exposure surface. |
| Detour hooks | The DLL installs the game-loop and render hooks | High | Hooked code paths should be reviewed separately from normal operator behavior. |
| In-process control paths | DMFT uses internal function calls, widget clicks, and login helpers inside the client | High | Internal control differs from external input simulation and needs explicit risk labeling. |
| IPC naming and authentication | Session-derived names, raw-token auth, and current-user DACLs | High | These harden local IPC but do not erase host-level risk. |
| Timing variation | Command jitter and movement humanization | Medium | Useful hardening, but not proof of safety against a specific detection path. |
| Operator environment | Live-machine cleanliness, runner boundaries, and artifact handling | High | Official Daybreak policy is broader than one session or one executable. |
| Community detection claims | Forum and community reporting | Low | Good for validation hypotheses only, not safety guarantees. |

### Confidence rules

- `High`: directly grounded in current code or official Daybreak policy
- `Medium`: current-code behavior with anti-detection value inferred rather than proven
- `Low`: community reporting or speculative interpretation

## Operator Hygiene Checklist

### Live machine hygiene

- keep live-play machines free of unrelated cheat tooling and stale test binaries
- avoid reusing stale DLLs, copied token files, or mixed old/new build artifacts
- treat `%TEMP%/dmft` logs and token-bearing files as sensitive operational data
- separate speculative packet or exploit-adjacent research from normal live-play hosts

### Runner and build hygiene

- keep the self-hosted runner focused on build, test, wiki, and release work
- do not assume a clean runner proves that live-play hosts are also clean
- prefer reproducible branch builds over hand-copied binaries

### Runtime discipline

- verify the exact branch, commit, and config before injection
- choose the lowest-exposure control path that still satisfies the task
- keep operator-visible behavior conservative enough that reports do not become the main risk vector
- record whether a conclusion came from official policy, repo observation, or community reporting

### Escalation triggers

- new hooks, broader module footprint, or riskier movement/control paths should be labeled as an exposure increase before implementation
- claims backed only by community reporting should stay provisional and become validation tasks instead of “facts”
- if a machine cannot be shown to be clean, treat that as a blocker

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
