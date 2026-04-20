# Team B — Security Posture Audit

Generated: 2026-04-19

## Summary

- **11 findings**: 0 critical, 1 high, 4 medium, 6 low
- **Attack-surface map**: Web API (Axum, 32 endpoints) protected by optional static X-API-Token; Windows IPC surface for DLL injection (named pipes); SQLite credential store (encrypted); Discord webhook routing; CLI-based bot token loading from TOML config.
- **Top risk**: Missing request size limits on web API endpoints could enable DoS; optional API auth requires explicit environment variable to activate.

---

## Findings (STRIDE-tagged)

### [HIGH] [Tampering, Info Disclosure] Unsafe environment variable mutation in tests

- **File**: textquest-web/src/api.rs:1840
- **Threat**: Test helper uses `unsafe { std::env::set_var(...) }` to temporarily override `TEXTQUEST_CONFIG_PATH`. If test crashes or panics before `Drop`, environment variable remains poisoned in the test process, leaking into subsequent tests or affecting parallel test execution.
- **Evidence**: `unsafe { std::env::set_var("TEXTQUEST_CONFIG_PATH", path) }` with `ConfigPathGuard` relying on Drop semantics. Test isolation relies on panic-free execution.
- **Impact**: Cross-test contamination, config leakage if tests run in parallel, potential for test-to-test path confusion affecting live config reads.
- **Remediation**: Replace with thread-local storage or temporary config file path that doesn't mutate process-level environment. Use `thread_local!` or pass config path explicitly via function argument instead of env var.

---

### [MEDIUM] [Tampering] No request size limits on web API endpoints

- **File**: textquest-web/src/main.rs (Axum Router setup)
- **Threat**: No explicit `DefaultBodyLimit` or request size validation on any endpoint. Attacker can send arbitrarily large payloads (e.g., 1GB JSON body) to endpoints like PUT `/api/accounts/{name}` or POST `/api/auto-group/settings`, causing memory exhaustion or OOM panic.
- **Evidence**: Router uses `tower_http::CorsLayer` and `api_token_auth` middleware but no size-limit middleware; no `ContentLength` header validation in handlers.
- **Impact**: Denial of service via large request bodies; potential OOM crashes affecting all connected clients.
- **Remediation**: Add `axum::middleware::DefaultBodyLimit` with reasonable limit (e.g., 10MB) before route handlers. Apply per-endpoint if some endpoints legitimately need larger bodies.

---

### [MEDIUM] [Spoofing] Optional API authentication enabled only via environment variable

- **File**: textquest-web/src/main.rs:340-344
- **Threat**: API token authentication is **off by default**. If `TEXTQUEST_API_TOKEN` environment variable is not set, **all API endpoints are unauthenticated**. On a networked deployment, this exposes all admin/control endpoints (accounts, session control, Discord config, loot settings) to unauthenticated access.
- **Evidence**: Code logs "API endpoints are unauthenticated" when env var is unset; middleware is a no-op (`api_token_auth` checks `if let Some(ref expected_token)` and passes through if None).
- **Impact**: Unauthenticated access to sensitive APIs (session control, account management, economy settings) if operator forgets to set env var in production.
- **Remediation**: Default to **authentication enabled** with a random generated token if unset, or fail startup with a clear error. Document required `TEXTQUEST_API_TOKEN` in deployment guide. Consider logging a warning (not info) at startup when auth is disabled.

---

### [MEDIUM] [Info Disclosure] Master password plaintext in environment variable

- **File**: textquest-web/src/main.rs:336-339, accounts.rs
- **Threat**: `TEXTQUEST_MASTER_PASSWORD` is read from environment variable and used as the master key for credential encryption. If the process environment is readable (e.g., `/proc/<pid>/environ` on Linux, `Get-ChildItem Env:` on Windows with local access), or if process is debugged, the plaintext password is exposed. No mention of zeroizing the plaintext after derivation.
- **Evidence**: `std::env::var("TEXTQUEST_MASTER_PASSWORD")` read directly; password is passed to `accounts::CredentialStore::open(password)` which uses it with `Argon2` for key derivation but doesn't zero the plaintext password string after use.
- **Impact**: If system is compromised or debugged, master password compromises all encrypted account credentials.
- **Remediation**: Use `zeroize` crate to zero the password string after key derivation. Consider using OS keystore (Windows DPAPI, macOS Keychain, Linux libsecret) instead of environment variable for production deployments.

---

### [MEDIUM] [Spoofing] Weak timestamp-based token extraction in temp directory

- **File**: textquest-web/src/live_ipc.rs:180-189
- **Threat**: Session tokens are written to temp files named `login_token_{pid}` without file ownership/permission validation. A local attacker with access to `%TEMP%\textquest` (or `/tmp/textquest` on Linux) can enumerate and read other processes' session tokens, or replace them if directory permissions are weak.
- **Evidence**: Code reads from `std::env::temp_dir().join("textquest")` and extracts PIDs from filename to locate tokens. No file permission check (`stat` mode) or ownership validation before reading token bytes.
- **Impact**: Local privilege escalation; token theft from other TextQuest processes running under different users.
- **Remediation**: Validate file ownership (must match current process or root/SYSTEM). Restrict temp directory permissions to 0700 (rwx------). Use monotonic session ID instead of PID in temp filename to avoid enumeration.

---

### [LOW] [Tampering] Constant-time token comparison only at API layer

- **File**: textquest-web/src/main.rs:182-192
- **Threat**: Constant-time comparison (`constant_time_eq_str`) is implemented and used for API token validation. However, this is **only applied at the Axum middleware layer**. If any internal subsystem or future feature accepts the token directly (e.g., WebSocket upgrade, non-REST endpoint), it may use a standard string comparison, leaking token length via timing.
- **Evidence**: `constant_time_eq_str` is defined and applied in `api_token_auth` middleware, but no other callers guard against timing attacks.
- **Impact**: Low risk for now (token is static), but future development should centralize token comparison. Timing attack on token length (trivial for 32-char hex strings).
- **Remediation**: Document this protection; consider wrapping the token in a `SecretString` type that implements `ConstantTimeEq` and use it consistently across all auth checks.

---

### [LOW] [Info Disclosure] Verbose error messages leak config file paths

- **File**: textquest-web/src/api/chat_log.rs:19, loot.rs, admin_config.rs
- **Threat**: Error responses return the full file path of config files (e.g., `"Failed to read /home/user/config/textquest.toml: permission denied"`). On a networked API, this leaks the full directory structure and filesystem layout to unauthenticated clients.
- **Evidence**: Error messages in handlers include `path.display()` and return via JSON: `"Failed to read {}: {error}"`.
- **Impact**: Information disclosure; attacker learns filesystem layout and config file locations for social engineering or targeted attacks.
- **Remediation**: Log the full path server-side (for debugging) but return a generic error message to clients (e.g., `"Failed to load configuration"`). Use correlation IDs for tracing.

---

### [LOW] [Spoofing] Admin session snapshot readable without authentication

- **File**: textquest-web/src/api/admin.rs:63-70
- **Threat**: `GET /api/admin/sessions` returns admin-level session inventory (session IDs, character names, groups, lifecycle state). This endpoint is protected by optional `X-API-Token` auth, but if auth is disabled (the default), it is **unauthenticated**. Returns full session inventory including group IDs and character names.
- **Evidence**: Handler reads from `state.admin_session_snapshot_path`, calls `read_admin_sessions(path)`, and returns JSON directly. Auth middleware is optional.
- **Impact**: Unauthenticated access to session inventory; attacker learns all active sessions, group composition, and character names.
- **Remediation**: Audit all endpoints for auth requirements. Ensure API token is **always required** for admin endpoints, not optional. Add `@admin` or `@auth_required` decorator/attribute to handlers.

---

### [LOW] [Info Disclosure] Session logs keyed by session_id without access control

- **File**: textquest-web/src/main.rs:146
- **Threat**: `session_logs` is a `HashMap<u32, Vec<String>>` of session logs accessible via API (presumed from handlers). If an endpoint like `GET /api/admin/logs/{session_id}` is unauthenticated or if a user can guess session IDs, they can read logs of any session.
- **Evidence**: `pub session_logs: tokio::sync::RwLock<HashMap<u32, Vec<String>>>` in AppState suggests API handlers have direct access. Session IDs are small integers (u32) — easily enumerable.
- **Impact**: Information disclosure; attacker can enumerate session IDs and read operational logs, chat history, or debug output from any session.
- **Remediation**: Restrict log access to authenticated users **and** enforce session ownership (user can only read logs for sessions they own). Use opaque session tokens, not sequential u32 IDs.

---

### [LOW] [Tampering] File write race condition in config updates

- **File**: textquest-web/src/api/chat_log.rs:74-105
- **Threat**: Config writes use a temp file + rename pattern (`atomic write`), which is good, but there's no distributed lock. If two concurrent requests both call `put_chat_log_settings`, both may read the same config, modify it independently, and the second write wins (last-write-wins). This can lose settings from concurrent requests.
- **Evidence**: `write_chat_log_settings_to_disk` reads `path`, modifies, writes to temp file, and renames. No file-level or application-level lock across the read-modify-write cycle.
- **Impact**: Settings loss under concurrent requests; race condition between web API and orchestrator updates.
- **Remediation**: Use application-level mutex (`Mutex<()>`) guarding the entire read-modify-write cycle, similar to what's done for `character_config_write_lock`. Apply to all config write handlers.

---

### [LOW] [Spoofing] JSON deserialization without schema validation

- **File**: textquest-web/src/api/admin_config.rs:40
- **Threat**: Config files are deserialized from JSON with `serde_json::from_slice` without validating schema constraints (e.g., array size limits, string length limits). A malformed or adversarially crafted config file could cause panics or resource exhaustion.
- **Evidence**: `serde_json::from_slice::<Vec<...>>(&payload)` in `read_live_sessions` and similar patterns throughout API handlers.
- **Impact**: Denial of service via malformed config files; potential panics if array size is unexpectedly large.
- **Remediation**: Add schema validation after deserialization (e.g., max array size, max string length). Use `serde(deny_unknown_fields)` to reject unexpected config keys. Log and reject invalid configs with meaningful error.

---

### [LOW] [Tampering] No validation of webhook URLs before storing

- **File**: textquest-web/src/api/discord.rs:36-41
- **Threat**: Discord webhook URLs are stored in `DiscordSettings` without validation. An operator could paste a typo'd URL, or an attacker (if auth is disabled) could replace webhook URLs with attacker-controlled ones, causing alerts to be sent to wrong Discord server.
- **Evidence**: `pub webhook_url: String` and `pub channels: HashMap<String, String>` accept any string; no `Url` parsing or Discord webhook validation.
- **Impact**: Alerts and messages sent to wrong destination; potential for attacker to intercept/redirect alerts if API auth is disabled.
- **Remediation**: Parse and validate webhook URLs with the `url` crate. Enforce `https://discord.com/api/webhooks/` scheme and path pattern. Log and reject invalid URLs with clear error.

---

## Dependency notes

**Cargo.toml (textquest-web):**

- `axum = "0.8"`, `tokio`, `tower-http = "0.6"` — Well-maintained web framework; no known critical CVEs.
- `rusqlite = "0.32"` with bundled SQLite — Uses parameterized queries (good); bundled SQLite is automatically patched when crate updates.
- `aes-gcm = "0.10"`, `argon2 = "0.5"`, `zeroize`, `rand` — Cryptographic primitives look sound. `argon2` is used for key derivation; `aes-gcm` for encryption.
- No `unsafe` crate deps; only unsafe in Rust code for Windows API calls (expected).
- **No obvious supply-chain risks** in the dependency tree shown.

---

## CI/workflow notes

**GitHub Actions (`.github/workflows/`):**

1. **CI (ci.yml):**
   - Runs on self-hosted Linux + Windows runners; no GitHub-hosted runners (good practice for security).
   - Cargo `fmt`, `clippy`, and coverage checks before merge.
   - **No hardcoded secrets in workflows.** Environment variables are loaded implicitly.
   - `pull_request_target` is **not used**; uses standard `pull_request` trigger (safe).
   - TruffleHog OSS secret scanning runs on every CI pass.
   - **Recommendation**: Add `cargo audit` to CI gate (currently manual only).

2. **Automation (automation.yml):**
   - Uses `actions/github-script@v8` for GitHub API automation (labels, PR closing, auto-merge).
   - Inline JavaScript runs GitHub API calls with context tokens (standard scoped token, not admin).
   - **No pull_request_target misuse** — PR automation uses standard events.
   - **Risk**: If a branch name is crafted as `"../../../etc/passwd"`, the script could be injected via branch name in context variables. Low risk due to allowlist of branch patterns (`codex/`, `claude/` prefixes).

3. **Action versions:**
   - `actions/checkout@v4`, `dtolnay/rust-toolchain@nightly`, `Swatinem/rust-cache@v2` — All well-maintained and pinned to specific versions (good).
   - `actions/github-script@v8` pinned (good).
   - **No unpinned actions** found.

4. **Secrets handling:**
   - No `secrets.` references in workflows (good — no hardcoded secret injection).
   - Bot token, API tokens are loaded via environment at runtime, not baked into CI.

**Recommendation**: Explicitly document the `TEXTQUEST_API_TOKEN` and `TEXTQUEST_MASTER_PASSWORD` environment variables in the CI setup docs; ensure they are never logged or exported in workflow output.

---

## Recommendations (Priority Order)

1. **[HIGH]** Enable API authentication by default. Change opt-out (disabled by default) to opt-in (enabled by default).
2. **[HIGH]** Add request size limits to Axum router (DefaultBodyLimit middleware).
3. **[MEDIUM]** Replace unsafe env var mutation in tests with thread-local or explicit config passing.
4. **[MEDIUM]** Validate master password is zeroized after key derivation; consider OS keystore for production.
5. **[MEDIUM]** Add file ownership/permission validation to temp token files.
6. **[LOW]** Centralize and document all token/auth comparisons; ensure constant-time checks everywhere.
7. **[LOW]** Replace filesystem paths in error messages with generic descriptions; log full paths server-side only.
8. **[LOW]** Add schema validation constraints (max array size, string length) to all serde deserialization.
9. **[LOW]** Validate Discord webhook URLs before storing; parse and verify against Discord's expected pattern.
10. **[LOW]** Add read-modify-write locks to all config persistence operations (not just character config).

---

## Risk Surface Map

```
┌─ Web API (Axum + Tokio)
│  ├─ /api/* endpoints (32 handlers)
│  │  ├─ Auth: X-API-Token (optional, default off) ⚠️
│  │  ├─ Size limits: None ⚠️
│  │  └─ Session/admin endpoints: Unauthed data exposure risk
│  │
│  ├─ WebSocket (/ws)
│  │  └─ Assumes same-origin; no explicit CSRF token
│  │
│  └─ Static SPA (index.html, assets)
│
├─ IPC (Windows named pipes)
│  ├─ Pipe format: `textquest-{session_id}-{pid}`
│  ├─ Tokens: Temp files in %TEMP%\textquest (weak access control)
│  └─ Risk: Local elevation of privilege, token theft
│
├─ Credential Store (SQLite)
│  ├─ Encryption: AES-GCM (key from TEXTQUEST_MASTER_PASSWORD)
│  ├─ Master password: Plaintext env var (could be debugged)
│  └─ Risk: Credential exposure if master password leaked
│
├─ Discord Integration
│  ├─ Bot: Serenity (token from TOML config, not env)
│  ├─ Webhooks: Stored in DiscordSettings, no validation
│  └─ Risk: Webhook hijacking if API auth disabled, token interception
│
└─ Config Files (TOML)
   ├─ Path: Configurable via TEXTQUEST_CONFIG_PATH env var
   ├─ Deserialization: Unvalidated JSON from snapshots
   └─ Risk: Schema injection, DoS via large arrays
```
