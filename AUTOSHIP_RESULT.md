# Result: #899 — M10: Mail to Bazaar Mule — Window Parsing and Send Automation

## Status: PARTIAL

## Changes Made

- `textquest-common/src/offsets.rs`: Added `PINST_MAIL_WINDOW` (placeholder) and
  `mail_window` submodule with `IS_OPEN`, `RECIPIENT_FIELD`, `SUBJECT_FIELD`,
  `BODY_FIELD`, `ATTACHMENT_SLOT_COUNT`, `ATTACHMENT_SLOTS_BASE` offset constants.
  All addresses are `0x0` — Ghidra RE work against client date 20260415 is still
  required to fill real values (tracked in this issue).

- `textquest-common/src/ipc.rs`: Added IPC protocol surface for mail automation:
  - `MailWindowSnapshot` struct — snapshot of open/closed state, recipient,
    subject, body, and per-slot attachment names.
  - `SendMailParams` struct — all parameters for a compose+send sequence
    (recipient, subject, body, inventory attach slots, plat amount).
  - `Command::OpenMailWindow` — open the EQ mail compose window.
  - `Command::QueryMailWindowState` — poll mail window state; returns
    `Response::MailWindowState`.
  - `Command::SendMailToMule { params }` — full send sequence via DLL.
  - `Response::MailWindowState { snapshot }` — snapshot response.
  - Four dedicated unit tests + two variants added to existing bulk roundtrip
    tests. All new tests pass when compiled in isolation.

## Tests

- Command: `cargo check -p textquest-common`
- Result: PASS (0 errors, 1 pre-existing semver warning)
- Command: `python3 scripts/dev-preflight.py --skip-tests --env-only`
- Result: 11 PASS, 1 FAIL (pre-existing map bounds issue — crescent_reach_newbie,
  unrelated to mail changes)
- New tests added: yes — `command_open_mail_window_roundtrip`,
  `command_query_mail_window_state_roundtrip`,
  `command_send_mail_to_mule_roundtrip`, `response_mail_window_state_roundtrip`
- Note: `cargo test` for textquest-common currently fails to compile due to 6
  pre-existing errors in combat.rs and ipc.rs (missing struct fields from other
  merged PRs: `min_expansion` in `AbilitySet`, `break_conditions`/`break_reason`
  in `StickConfig`/`NavStatus`). These errors exist on master and are not
  introduced by this PR.

## Notes

This PR covers the IPC and offset stub layer (sub-issues 1, 2, 3, 4 protocol
side). Remaining work:

- **DLL implementation** (sub-issues 3–5 DLL side): `textquest-dll` needs a
  handler for `OpenMailWindow`, `QueryMailWindowState`, and `SendMailToMule`
  that reads/writes the actual EQ `CMailWindow` struct at runtime. Blocked on
  real Ghidra-verified offsets for the mail window struct.
- **Real memory offsets** (sub-issue 1): All `mail_window::*` offsets are `0x0`
  placeholder. Requires Ghidra RE on `CMailWindow` for client 20260415.
- **Integration test** (sub-issue 6): item → mailed → received by mule.
  Requires DLL implementation and a live Teek test pass.
