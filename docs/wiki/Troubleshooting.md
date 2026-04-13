# Troubleshooting

## No EQ clients found

Symptoms:

- TUI starts in demo mode
- CLI `inject` says no `eqgame.exe` processes were found

Checks:

- confirm you are on Windows for live mode
- confirm `eqgame.exe` is actually running
- confirm `process_name` in `config/textquest.toml` matches your client

## TUI says demo mode unexpectedly

Current expected behavior:

- macOS and Linux always use demo mode
- Windows also falls back to demo mode when no live EQ clients are attached

If you expected live mode on Windows:

- start EQ first
- verify the process is visible
- verify the DLL was injected
- use `client-status-all` to confirm shared-memory reads

## Missing session token

Typical error:

`No session token for PID ... Inject the DLL first to create authenticated IPC state.`

What it means:

- the orchestrator could not find `%TEMP%/textquest/login_token_<pid>.bin`

Fix:

1. re-run injection for that PID
2. confirm the token files exist under `%TEMP%/textquest`
3. retry the CLI or TUI command

## Cannot open shared memory

Symptoms:

- `status` fails
- navigation or live state views remain empty

Checks:

- confirm injection succeeded
- confirm the DLL log is updating
- confirm the client is still running
- confirm the session token matches the current injected session

## Injection succeeded but commands do nothing

Checks:

- review `logs/textquest.log`
- review `%TEMP%/textquest/textquest-dll.log`
- confirm the pipe authentication token was sent
- confirm you are not using the TUI `:inject` placeholder as if it were the real injector

For live injection work, prefer the CLI `inject` path.

## Nav or map data missing

Checks:

- confirm the zone has a matching file under `config/maps`
- confirm navmesh cache files are present when expecting navmesh-backed routing
- remember that straight-line fallback is expected when mesh data is unavailable
- remember that non-Windows builds intentionally skip live navmesh overlay loading

## Login automation issues

Checks:

- confirm `config/accounts.toml` entry names match what you are launching
- confirm encrypted credentials were stored separately
- review DLL login logs after EQ patches
- expect widget- and offset-related failures first after live client updates

## Wiki publish fails with "Repository not found"

If `python scripts/sync_wiki.py --push` reports that the wiki remote does not exist yet:

1. open the repository Wiki tab in GitHub
2. create the first page in the UI
3. rerun `python scripts/sync_wiki.py --push`

That is the one-time bootstrap path when wiki support is enabled but the wiki git repo is not yet initialized.

## Current Behavior vs Roadmap

### Current behavior

- Demo mode, missing token files, and missing shared memory are common and expected failure modes with straightforward checks.

### Validation notes

- After EQ patches, offset and login failures should be assumed first until proven otherwise.
