# Epic #2123 — DLL stealth + combat hygiene: audit followups

**Status:** Decomposed — 5 child issues filed  
**Source audit:** `audit/AUDIT-2026-04-19.md` (audit 2026-04-19)

---

## Problem summary

Full audit 2026-04-19 surfaced three categories of DLL and combat-class hygiene problems:

1. **Combat class panic** — `warrior.rs:62` hard-panics on malformed TOML, taking down all warrior-class clients at first combat rotation entry.
2. **Stealth helpers with silent-wrong-behavior fallbacks** — `is_trusted_origin` falls back to `cfg!(test)` for missing Origin headers; `stack_spoof.rs` uses `unwrap_or_default()` on `CString` which silently redirects `GetModuleHandleA` to the caller instead of the target.
3. **Test-cfg validation gap / stub UI** — Packet monitor UI ships with 5 hardcoded stubs making it non-functional in production; `etw_blind.rs` hardware breakpoint scope is thread-local with no documentation or enumeration.

---

## Child issues

| #     | Severity | Title                                                                                           | Category           |
| ----- | -------- | ----------------------------------------------------------------------------------------------- | ------------------ |
| #2275 | MEDIUM   | warrior.rs:62 panic on malformed TOML kills all warrior-class clients at first combat           | Combat class panic |
| #2278 | MEDIUM   | is_trusted_origin returns cfg!(test) for no-Origin requests — fragile if test binary deployed   | Stealth fallback   |
| #2280 | MEDIUM   | Packet monitor UI: 5 hardcoded stubs (peak_rate, no selection, no payload, PIDs not names)      | Stub UI            |
| #2284 | LOW      | stack_spoof.rs: CString::new.unwrap_or_default causes GetModuleHandleA to return caller         | Stealth fallback   |
| #2289 | LOW      | etw_blind.rs: hardware breakpoint only scoped to calling thread — document or enumerate threads | Test-cfg/scope gap |

---

## Implementation order

Work items ordered by risk and dependency:

1. **#2275** (MEDIUM) — warrior.rs panic. Highest blast radius; hits every warrior-class client. Fix first.
2. **#2278** (MEDIUM) — is_trusted_origin fallback. Security-sensitive; fix before any network-facing deploys.
3. **#2280** (MEDIUM) — Packet monitor stubs. Self-contained UI work; can parallelize with #2278.
4. **#2284** (LOW) — stack_spoof CString. Correctness fix; low blast radius, no external dependencies.
5. **#2289** (LOW) — etw_blind thread scope. Requires a design decision (document vs. enumerate) before coding.

---

## Acceptance criteria (epic level)

- [ ] All 5 child issues closed
- [ ] No `panic!` / `unwrap_or_default` in stealth or combat-class hot paths (enforced via clippy `#[deny(clippy::unwrap_used)]` or equivalent)
- [ ] `is_trusted_origin` denies no-Origin requests in non-test builds
- [ ] Packet monitor displays live data (no hardcoded stubs remain)
- [ ] `etw_blind.rs` thread-scope behavior documented or broadened
- [ ] CI green on all affected crates

---

## Notes

- All child issues tagged `agent:ready` — eligible for automated dispatch
- Source: `audit/AUDIT-2026-04-19.md`
- Parent: #2123
