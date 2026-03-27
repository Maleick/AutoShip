# DMFT Security Audit — 2026-03-27

## Summary

18 findings: 0 Critical, 4 High, 10 Medium, 3 Low, 1 Info

## High Priority

### H1: Named pipe IPC has no authentication

- **Files:** `dmft-dll/src/ipc/pipe.rs:44`, `dmft/src/ipc/pipe.rs`
- **STRIDE:** Spoofing / Tampering / Elevation of Privilege
- **Issue:** Pipe created with default security. Any process running as same user can connect to `\\.\pipe\dmft_cmd_{N}` and send arbitrary commands.
- **Fix:** Restrictive security descriptor + session token handshake at injection time.

### H2: Shared memory has no access control

- **Files:** `dmft-dll/src/ipc/shared.rs:48`, `dmft/src/ipc/shared.rs:49`
- **STRIDE:** Spoofing / Tampering / Information Disclosure
- **Issue:** Predictable names, default security. Any process can read game state or write fake data.
- **Fix:** Restrictive DACL + randomized names + integrity check.

### H3: Account names exposed in process command line

- **File:** `dmft/src/launcher/spawner.rs:21-31`
- **STRIDE:** Information Disclosure
- **Issue:** `/login:{account}` visible via Task Manager. Accepted risk (EQ requires it).
- **Fix:** Consider PEB scrubbing after process creation.

### H4: Master key not zeroized in memory

- **File:** `dmft/src/credentials/store.rs:12`
- **STRIDE:** Information Disclosure
- **Issue:** `[u8; 32]` key persists after drop. Memory dumps could expose it.
- **Fix:** Use `zeroize` crate, `Zeroizing<[u8; 32]>`.

## Medium Priority

### M1: Per-account salt generated but unused in encryption

- **File:** `dmft/src/credentials/store.rs:45-46`
- **Fix:** Derive per-account key from master_key + salt, or remove unused salt.

### M2: No SQLite file permissions enforcement

- **File:** `dmft/src/credentials/store.rs:34`
- **Fix:** Set owner-only permissions after creation. Add `*.db` to `.gitignore`.

### M3: No IPC message size limit

- **File:** `dmft-common/src/protocol.rs`
- **Fix:** Add MAX_MESSAGE_SIZE constant, reject frames > 64KB.

### M4: Encryption functions panic on failure

- **File:** `dmft/src/credentials/crypto.rs:10,16,30`
- **Fix:** Return `Result<>` instead of `.expect()`.

### M5: Decrypted passwords persist as String

- **File:** `dmft/src/credentials/store.rs:63-73`
- **Fix:** Return `Zeroizing<String>` from `secrecy` crate.

### M6: Unsafe Send/Sync on shared memory

- **Files:** `dmft-dll/src/ipc/shared.rs:23-24`, `dmft/src/ipc/shared.rs:24-25`
- **Fix:** Use AtomicU32 for payload length, add SAFETY comments.

### M7: No IPC command validation or rate limiting

- **File:** `dmft-dll/src/ipc/pipe.rs:61-89`
- **Fix:** Validate parameters, add rate limiting, consider sequence numbers.

### M8: Account name logged in plaintext

- **File:** `dmft/src/launcher/spawner.rs:57`
- **Fix:** Redact account name in tracing output.

### M9: Mutex poisoning with unwrap() in nav module

- **File:** `dmft-dll/src/nav/mod.rs:21,28,37,45`
- **Fix:** Use `.unwrap_or_else(|e| e.into_inner())` like combat module.

### M10: MovementController uses potentially-stale player pointer

- **File:** `dmft-dll/src/hooks/movement.rs:51-54,62-64,73-78,91-94`
- **Fix:** Re-read player base from pinstLocalPlayer each tick, add null check.

## Low Priority

### L1: DLL filename randomization uses timestamp

- **File:** `dmft/src/inject/dll_prep.rs:30-48`
- **Fix:** Use OsRng instead of SystemTime.

### L2: WaitForSingleObject timeout not checked

- **File:** `dmft/src/inject/loader.rs:97`
- **Fix:** Check return value, don't free remote buffer on timeout.

### L3: Credential DB path not in .gitignore

- **Fix:** Add `*.db`, `*.sqlite`, `*.sqlite3` to `.gitignore`.

### L4: DLL injection uses PROCESS_ALL_ACCESS

- **File:** `dmft/src/inject/loader.rs:34`
- **Fix:** Use minimum required flags.
