# Launcher / Client / Credentials Modules

Files: `launcher/{coordinator,login_sm,mod,post_login,spawner}.rs`, `client/{affinity,death_camp,discovery,healing,manager,mod,session}.rs`, `credentials/{crypto,mod,prompt,store}.rs`.

These three together solve: "spin up N EQ clients safely, get each one to character-select-in-game, and track them as live sessions."

---

## `launcher/coordinator.rs`

- **Purpose.** Staggered login orchestration — queue clients, launch one per stagger interval, respect retry backoff, pause everything on mass failure.
- **Public API.**
  - `LaunchCoordinator::new(config, retry, server)`.
  - `enqueue(client_id, account)`.
  - `tick() -> Vec<CoordinatorEvent>`.
  - `report_login_event(client_id, event)` — inject external FSM events.
  - `resume()`, `is_paused()`, `pending_count()`, `active_count()`.
  - `CoordinatorEvent`: `ClientLaunched`, `ClientReady`, `ClientFailed`, `AllPaused`, `AllReady`.
- **Invariants.**
  - `should_launch_next` respects max-concurrent-logins, stagger interval, and per-client retry backoff simultaneously.
  - Mass-failure detection uses a 60 s rolling window; 5 failures in window → pause all.
  - Failed clients land in `retry_not_before`; re-enqueue respects it.
  - Terminal phases (`Ready`, `Failed`, `ProcessExiting`) stop being ticked.

## `launcher/login_sm.rs`

- **Purpose.** Per-client finite state machine covering process spawn → character select → in-game.
- **Public API.**
  - `LoginStateMachine::new(client_id, account)`.
  - `advance(event) -> LoginAction` for `LoginEvent::{ProcessStarted, LoginScreenDetected, CredentialsSent, ServerSelected, CharacterSelected, ZoneInComplete, PlayerDataConfirmed, ErrorDetected, DllReported}`.
  - `tick() -> Option<LoginAction>` — phase timeout driver.
  - `is_terminal()`.
- **Invariants.**
  - Class placeholder: `"UNK"` is valid during `PlayerDataConfirmed` validation (`account_class_is_unset`).
  - Each phase has a timeout; expiry triggers retry or abort depending on retry count.

## `launcher/spawner.rs`

- **Purpose.** Windows-specific `CreateProcessW` wrapper that launches `eqgame.exe`.
- **Public API.** `spawn_eq_client(...)` returning `SpawnedProcess { pid }`.
- **Invariants / security note.** Command line includes `/login:account_name`, which makes the account name visible in `ps`/task manager. This is known (see prior audit); passwords are sent through the UI layer, not the command line.

## `launcher/post_login.rs`

- **Purpose.** Post-login sequencer — issues group/raid join commands, buff requests, camp commands after a client reports `Live`.

## `launcher/mod.rs`

- Re-exports the public API above and wires the `SpawnerPlatform` trait to the real or stub implementation.

---

## `client/manager.rs`

- **Purpose.** Registry of every live EQ client: `ClientId ↔ PID` map, session state, injection + health coordination.
- **Public API.**
  - `ClientManager::new(process_name)`.
  - `with_discovery_config(config)` — enable UDP multicast peer discovery.
  - `discover() -> Vec<ClientId>` — scan OS processes, register new PIDs.
  - `track_client(client_id, pid)` — register/update mapping.
  - `inject(client_id, dll_path)` / `inject_all(dll_path)`.
  - `check_health() -> Vec<ClientId>` — return crashed clients.
  - `initiate_camp_out(client_id)` / `check_camp_outs() -> Vec<ClientId>` — graceful `/camp desktop` → force-kill.
  - `get`, `get_mut`, `active_sessions`, `all_sessions`, `remove`.
- **Invariants.**
  - Two HashMaps (`sessions` + `sessions_by_pid`) are kept in sync on every mutation.
  - Camp-out timeout defaults to 45 s; force-kill only after timeout.
  - Health checks are skipped while a client is `CampingOut`, `Exited`, or `Relaunching` (avoids spurious resurrections).
  - Successful `inject` sets `hook_status = Injected`.
  - `remove` triggers `dll_prep::cleanup_dll` so staged files do not leak.

## `client/session.rs`

- **Purpose.** Per-client state bag: PID, character name, hook status, health monitor, last game state, account binding, post-login phase, slot lifecycle, camp-out tracker.
- **Public API.** `EqSession::{new, begin_camp_out, is_camp_out_timed_out, update_state}` + `SlotLifecycle` enum: `Configured → Launching → WaitingForLogin → EnteringWorld → Live → Recovering/Blocked → CampingOut → Exited/Relaunching`.
- **Invariants.** `update_state` validates character name against binding and rejects mismatches (defence against character-slot swap).

## `client/discovery.rs`

- **Purpose.** UDP multicast peer discovery between orchestrator instances (e.g., multiple hosts in a fleet).
- **Invariants.** Peers expire after a TTL window; rediscovery is cheap.

## `client/healing.rs`

- **Purpose.** Health monitor: "is this PID still alive and responsive?" based on exit-code + memory-read probes.
- **Invariants.** Crashes are signalled once per client (edge-triggered) so upstream does not re-queue twice.

## `client/affinity.rs`

- **Purpose.** Set CPU affinity / priority for spawned EQ processes (perf hygiene on multi-core hosts).

## `client/death_camp.rs`

- **Purpose.** Special-case state tracker for "camp out from dead" — interacts with `orchestrator_loop` auto-camp.

---

## `credentials/crypto.rs`

- **Purpose.** Argon2id KDF + AES-256-GCM primitives, all wrapped in `Zeroizing`.
- **Public API.**
  - `derive_key(master_password, salt) -> Zeroizing<[u8; 32]>`.
  - `derive_key_from_master(master_key, salt) -> Zeroizing<[u8; 32]>`.
  - `encrypt(plaintext, key) -> (ciphertext, nonce)`.
  - `decrypt(ciphertext, key, nonce) -> Vec<u8>`.
  - `generate_salt() -> [u8; 32]`.
- **Invariants.**
  - Argon2id params: `m=65536, t=3, p=4`.
  - Two-level key hierarchy: master key (from master password + master salt), per-account key (from master key + per-account salt).
  - Nonce is random per encryption, 12 bytes, stored next to the ciphertext.
  - Keys wrapped in `Zeroizing<T>` so they zero on drop.

## `credentials/store.rs`

- **Purpose.** SQLite-backed persistence for the encrypted credentials.
- **Public API.** `CredentialStore::{open, open_default(master_password), add_account, get_password, list_accounts, remove_account}`.
- **Invariants.**
  - `credential_store_meta` table stores the master salt so it persists across sessions.
  - `credentials` table schema: `account_name (UNIQUE), password_enc (BLOB), nonce (BLOB), salt (BLOB), created_at, updated_at`.
  - `PRAGMA journal_mode = WAL`.
  - Default path is `data/credentials.db`; `open_default` bootstraps/loads the master salt.

## `credentials/prompt.rs`

- **Purpose.** `rpassword`-based master-password prompt so it does not show up in `ps`/task manager.

## `credentials/mod.rs`

- Re-exports the store + crypto API and provides thin convenience wrappers (`open`, `derive_master_key`).

---

## Cross-module flow

```
enqueue → LaunchCoordinator.tick (stagger check)
        → spawner::spawn_eq_client (Windows CreateProcessW)
        → ClientManager.track_client(client_id, pid)
        → LoginStateMachine.advance(ProcessStarted…PlayerDataConfirmed)
        → ClientManager.inject(client_id, dll_path)
            └─ credentials::open_default → get_password → LoginStateMachine input

HealthMonitor → ClientManager.check_health
             → initiate_camp_out → (timeout) force-kill
             → orchestrator_loop requeues with retry backoff
```
