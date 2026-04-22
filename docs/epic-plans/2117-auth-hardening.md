# Epic #2117 — Auth Hardening (Audit 2026-04-19)

**Parent:** https://github.com/Maleick/TextQuest/issues/2117
**Source audit:** audit/AUDIT-2026-04-19.md
**Decomposed:** 2026-04-21

## Child Issues

| #                                                         | Title                                                                    | Severity | Size    | Labels                   |
| --------------------------------------------------------- | ------------------------------------------------------------------------ | -------- | ------- | ------------------------ |
| [#2245](https://github.com/Maleick/TextQuest/issues/2245) | Fix constant_time_eq_str timing oracle — early return leaks token length | HIGH     | size-s  | security, api, p1-high   |
| [#2246](https://github.com/Maleick/TextQuest/issues/2246) | Make API authentication default-ON — flip optional auth gate             | MEDIUM   | size-s  | security, api, p2-medium |
| [#2248](https://github.com/Maleick/TextQuest/issues/2248) | Guard admin session snapshot endpoint behind auth check                  | LOW      | size-xs | security, web            |
| [#2251](https://github.com/Maleick/TextQuest/issues/2251) | Add ownership check to session_logs keyed by enumerable u32 session_id   | LOW      | size-s  | security, api            |
| [#2255](https://github.com/Maleick/TextQuest/issues/2255) | Centralize constant-time comparison into a single shared utility         | LOW      | size-xs | security, api            |

## Suggested Work Order

1. **#2255** — Centralize constant-time comparison first; #2245 fix should land on top of this utility.
2. **#2245** — Fix the timing oracle using the new centralized utility.
3. **#2246** — Flip auth to default-ON; coordinate with integration test updates.
4. **#2251** — Add IDOR ownership check to session_logs.
5. **#2248** — Add per-endpoint auth guard to admin snapshot route.

## Risk Notes

- #2246 (default-ON auth) is the highest blast-radius change — it will break any integration test that omits tokens. Assign to a worker who can run the full test suite.
- #2245 and #2255 can be done by the same worker sequentially to avoid merge conflicts on the comparison utility module.
- #2248 and #2251 are isolated endpoint guards with no shared state — safe to parallelize.
