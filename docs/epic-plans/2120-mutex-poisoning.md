# Epic #2120: Mutex Poisoning Recovery — DLL + Metrics

**Source:** audit/AUDIT-2026-04-19.md  
**Parent issue:** https://github.com/Maleick/TextQuest/issues/2120

## Problem

Several call sites use `.expect()` or `.unwrap()` on `Mutex::lock()`. Once any caller panics while holding the lock, the Mutex becomes poisoned and every subsequent caller panics too — a cascading failure. In the DLL context (called from EQ game frames) this is undefined behaviour: a panic in a Windows DLL that crosses an FFI boundary is unsound.

The recovery pattern is already established in `textquest-dll/src/combat/mod.rs`:

```rust
.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
```

This pattern recovers the inner data from the poisoned guard and continues, logging the original panic separately rather than cascading.

## Scope

| Area                       | Files                                          | Sites                |
| -------------------------- | ---------------------------------------------- | -------------------- |
| tradeskill_trophy hot-path | `textquest-dll/src/tradeskill_trophy.rs`       | 4x `.expect()`       |
| Metrics collection         | `textquest-common/src/observability.rs`        | 7x `.unwrap()`       |
| Dialog rez/accept path     | `textquest-dll/src/dialog.rs`                  | ~12x `.expect()`     |
| Magician combat tick       | `textquest-dll/src/combat/classes/magician.rs` | 5x `.expect()`       |
| Loot ledger (production)   | `textquest/src/loot/ledger.rs`                 | 2x `.expect()`       |
| Audit logger               | `textquest-soul/src/audit.rs`                  | 1x `.expect()`       |
| CONTRIBUTING docs          | `CONTRIBUTING.md`                              | 0 (missing guidance) |

## Reference implementation

`textquest-dll/src/combat/mod.rs` uses `unwrap_or_else(PoisonError::into_inner)` consistently across all combat state locks — use this file as the canonical example.

## Child issues

| #                                                         | Title                                                                                  | Priority |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------- | -------- |
| [#2276](https://github.com/Maleick/TextQuest/issues/2276) | fix(mutex): recover from poison in tradeskill_trophy.rs hot-path                       | MEDIUM   |
| [#2281](https://github.com/Maleick/TextQuest/issues/2281) | fix(mutex): poison recovery for InMemoryCollector in textquest-common/observability.rs | MEDIUM   |
| [#2286](https://github.com/Maleick/TextQuest/issues/2286) | fix(mutex): poison recovery for dialog.rs static Mutex locks (DLL rez/accept path)     | MEDIUM   |
| [#2292](https://github.com/Maleick/TextQuest/issues/2292) | fix(mutex): poison recovery for magician runtime lock + loot ledger + audit logger     | MEDIUM   |
| [#2296](https://github.com/Maleick/TextQuest/issues/2296) | docs(contributing): mandate poison-recovery pattern for all static Mutex locks         | DOCS     |

## Suggested fix order

1. **#2296 (docs first)** — codify the rule before making changes, so reviewers have a reference
2. **#2276** — tradeskill_trophy (smallest, isolated, good warm-up)
3. **#2281** — observability (common crate, affects all consumers)
4. **#2286** — dialog.rs (most sites, but mechanically identical)
5. **#2292** — magician + ledger + audit (spans three crates, do together)

## Acceptance criteria for epic close

- [ ] All child issues closed
- [ ] Zero `.expect()` / `.unwrap()` on production-path Mutex locks outside of test code
- [ ] Test-only `.unwrap()` annotated with `// test-only` comment
- [ ] CONTRIBUTING.md Mutex section merged
- [ ] CI green on Windows cross-compile
