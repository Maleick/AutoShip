# Epic #2122 — Secrets at Rest: Master Password + Temp Tokens + Error Leaks

**Parent:** https://github.com/Maleick/TextQuest/issues/2122
**Source audit:** docs/audits/2026-04-19/team-b-security.md
**Decomposed:** 2026-04-21

## Scope

Four secrets-hygiene defects identified in the 2026-04-19 audit:

1. `unsafe env::set_var` in tests — cross-test env poisoning risk
2. `TEXTQUEST_MASTER_PASSWORD` not zeroized after Argon2 derivation — readable via `/proc/<pid>/environ`
3. Temp session token files (`login_token_{pid}`) have no ownership or permission check
4. Error messages in API handlers leak full filesystem paths to clients

## Child Issues

| #                                                         | Title                                                                                             | Severity | Size   | Labels                     |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | -------- | ------ | -------------------------- |
| [#2277](https://github.com/Maleick/TextQuest/issues/2277) | Fix unsafe env::set_var in tests — eliminate cross-test env poisoning (api.rs:1840)               | HIGH     | size-s | security, testing, p1-high |
| [#2282](https://github.com/Maleick/TextQuest/issues/2282) | Zeroize TEXTQUEST_MASTER_PASSWORD plaintext after Argon2 key derivation                           | MEDIUM   | size-s | security, p2-medium        |
| [#2288](https://github.com/Maleick/TextQuest/issues/2288) | Add file ownership check and 0700 permissions to temp session token files                         | MEDIUM   | size-m | security, p2-medium        |
| [#2291](https://github.com/Maleick/TextQuest/issues/2291) | Sanitize error messages in chat_log.rs, loot.rs, admin_config.rs — stop leaking config file paths | LOW      | size-s | security, p3-low           |

## Suggested Work Order

1. **#2277** — Fix `unsafe env::set_var` first. Highest severity; also unblocks clean parallel test runs needed to verify later fixes.
2. **#2282** — Zeroize master password. Self-contained change in `main.rs` + `accounts.rs`; no cross-cutting dependencies.
3. **#2288** — Add temp token file ownership check. Touches `live_ipc.rs` and platform-specific file permission APIs; larger surface, do after simpler fixes are merged.
4. **#2291** — Sanitize error messages. Lowest risk; introduces a shared helper, can be done in parallel with #2288 since they touch different files.

## Risk Notes

- **#2277** (env poisoning): the `ConfigPathGuard` Drop approach is inherently unsafe for panicking tests. The fix must remove `unsafe` entirely — do not patch around it.
- **#2282** (zeroize): confirm `zeroize` crate is already in `Cargo.toml`; if not, add it. The `Zeroizing<String>` wrapper is the idiomatic approach.
- **#2288** (temp token permissions): Windows file ACL validation is more complex than Unix `stat`. Worker should test on both platforms or scope Windows to a best-effort directory ACL and document the limitation.
- **#2291** (path leaks): the shared `internal_error` helper should log at `error!` level so ops can still diagnose failures server-side.

## Non-Goals (out of scope for this epic)

- Migrating `TEXTQUEST_MASTER_PASSWORD` to OS keystore (Windows DPAPI / macOS Keychain) — that is a larger architectural change; file a separate issue if desired.
- Constant-time token comparison centralization — tracked under #2255 (part of epic #2117).
