# Epic #2121 — DoS + Input Hardening (Audit 2026-04-19)

**Parent:** https://github.com/Maleick/TextQuest/issues/2121
**Source audit:** audit/AUDIT-2026-04-19.md
**Decomposed:** 2026-04-21

## Summary

The web API (`textquest-web`) accepts unbounded payloads, deserializes JSON without structural constraints, has an unguarded read-modify-write race on the chat-log config file, and stores Discord webhook URLs without validation. All four gaps were identified in the 2026-04-19 audit.

## Child Issues

| #                                                         | Title                                                                                          | Severity | Size   | Labels                   |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------- | ------ | ------------------------ |
| [#2287](https://github.com/Maleick/TextQuest/issues/2287) | Add DefaultBodyLimit to Axum router — cap unbounded JSON payloads                              | MEDIUM   | size-s | security, bug, risk:high |
| [#2290](https://github.com/Maleick/TextQuest/issues/2290) | Add schema validation to serde_json deserialization — enforce max array size and string length | LOW      | size-m | security, bug            |
| [#2294](https://github.com/Maleick/TextQuest/issues/2294) | Serialize concurrent PUT /api/chat-log/settings writes — prevent last-write-wins data loss     | LOW      | size-s | security, bug            |
| [#2298](https://github.com/Maleick/TextQuest/issues/2298) | Validate Discord webhook URLs on PUT /api/config/discord — reject malformed or non-HTTPS URLs  | LOW      | size-s | security, bug            |

## Suggested Work Order

1. **#2287** — `DefaultBodyLimit` first: it is a global mitigation that reduces the attack surface for all other issues and is the lowest-effort change.
2. **#2294** — Chat-log write-lock second: isolated two-file change, mirrors existing `character_config_write_lock` pattern.
3. **#2298** — Discord URL validation: single-handler change, no shared state, safe to parallelize with #2294.
4. **#2290** — Schema validation last: requires identifying the riskiest endpoints and adding per-field validators; larger surface area than the others.

## Key Files

| File                                          | Relevance                                                                     |
| --------------------------------------------- | ----------------------------------------------------------------------------- |
| `textquest-web/src/main.rs` (`build_app`)     | Where `DefaultBodyLimit` layer and the new `chat_log_write_lock` field belong |
| `textquest-web/src/api/chat_log.rs`           | `put_chat_log_settings` — write-race fix                                      |
| `textquest-web/src/api/discord.rs`            | `put_settings` — URL validation                                               |
| `textquest-web/src/api/chat_pattern_rules.rs` | Import endpoint — highest-risk deserialization target                         |
| `textquest-web/src/api/loot.rs`               | Rule PUT — array-heavy deserialization target                                 |

## Risk Notes

- #2287 (`DefaultBodyLimit`) is a one-liner layer addition but touches the top-level router — run the full `cargo test -p textquest-web` suite after landing.
- #2290 (schema validation) has the widest footprint. A worker should audit all import/bulk-write endpoints before choosing which fields to guard; do not attempt to guard everything in one PR.
- #2294 and #2298 are independent and can be dispatched in parallel without merge conflicts.
- None of these changes require a protocol break or API version bump — all are additive rejections of previously-accepted (but malformed) input.
