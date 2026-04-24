# Performance Profiling

TextQuest keeps performance profiling focused on hot paths that run during the
combat tick or packet loop. The current Criterion baseline covers ability
cooldown tracking because shared cooldown checks can run for every eligible
rotation entry.

## Criterion Benchmarks

Run the ability cooldown benchmark with:

```bash
cargo bench -p textquest-dll --bench ability_cooldowns
```

Criterion writes the profile and comparison reports under:

```text
target/criterion/ability_cooldowns/
```

The benchmark group records:

| Benchmark | Purpose |
| --- | --- |
| `can_use_string_shared_key` | Baseline for compatibility callers that pass shared timer names every tick. |
| `can_use_precomputed_shared_key` | Optimized path for rotation/config entries that precompute `SharedCooldownKey` once. |
| `tick_64_active_cooldowns` | Upper-bound tick cost for the maximum tracked active ability cooldown set. |

## Optimization Notes

- `SharedCooldownKey` lets hot paths precompute named shared cooldown buckets
  during rotation/config loading instead of hashing the timer name during every
  `can_use` evaluation.
- Existing string-based APIs remain supported and delegate through the
  precomputed-key implementation so behavior stays compatible.
- Ability cooldown tracking remains allocation-light in the common case:
  tracker storage is preallocated for 16 entries and capped at 64 active
  cooldown entries.

## Performance Budgets

Budgets are per `textquest-dll` release build on a developer workstation:

| Path | Budget |
| --- | --- |
| Precomputed shared cooldown availability check | Less than 75 ns/op. |
| String shared cooldown compatibility check | Less than 250 ns/op. |
| Tick 64 active cooldowns | Less than 1 us/tick. |
| Steady-state allocations in `can_use_with_shared_key` | Zero. |

Budget changes should be made in the same PR as Criterion output showing the
new baseline and the reason the budget moved.
