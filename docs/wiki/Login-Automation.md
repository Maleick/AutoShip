# Login Automation

## Current Operator Workflow

There are two user-facing entry points:

- CLI: `dmft.exe login <account> [--server ...] [--character ...]`
- TUI: `:login`, `:login all`, `:login G<n>`, `:login <account>`

Account metadata comes from `config/accounts.toml`. Passwords are not stored there.

## Current State Model

Shared login phases are defined in `dmft-common/src/login.rs`:

- `NotStarted`
- `ProcessLaunching`
- `AtLoginScreen`
- `EnteringCredentials`
- `ServerSelecting`
- `CharacterSelecting`
- `Zoning`
- `InWorld`
- `PostLoginSetup`
- `Ready`
- `Failed { reason }`

## Current Implementation Split

### Orchestrator side

Handled under `dmft/src/launcher/`:

- `spawner.rs`: start EQ clients
- `login_sm.rs`: external login state machine and launch flow
- `coordinator.rs`: staggered launch coordination and failure handling
- `post_login.rs`: group join, buff, and navigate-to-camp sequencing

### DLL side

Handled under `dmft-dll/src/login/`:

- resolve login UI state
- write credentials into login widgets
- select server
- select character
- enter world

The DLL is the part that actually manipulates EQ's login UI.

## Credentials

Current repo behavior:

- `config/accounts.toml` stores account names, server, character, class, and group metadata
- encrypted credential storage lives under `dmft/src/credentials/`
- crypto uses Argon2id plus AES-256-GCM
- the DLL zeroizes stored password material after credential entry

## Post-Login Sequencing

The post-login sequencer in `dmft/src/launcher/post_login.rs` currently models:

- joining the designated group
- applying buffs
- navigating to camp
- reporting ready

Shared IPC commands already exist for:

- `JoinGroup`
- `ApplyBuffs`
- `ReportReady`

## Important Platform Note

The TUI `:login` flow is stubbed on non-Windows. In that environment it logs what would have launched instead of controlling live EQ.

## Current Behavior vs Roadmap

### Current behavior

- Login automation is no longer just a design note; the code has real phase models and in-client login logic.
- The launch coordinator already tracks stagger timing, retries, and pause conditions for mass failures.

### Validation notes

- Login selectors and widget offsets are some of the most EQ-patch-sensitive parts of the repo.
- Post-login group formation and buff/camp sequencing are structured in code, but they still need repeated live validation on Windows with actual clients.
