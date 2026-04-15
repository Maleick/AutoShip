# Research: Frostreaver Raid Economics

Curated `M10` economy note for issues `#1524` and `#1520`.

This document models early-phase Velious raid profitability on Frostreaver and recommends a loot-distribution policy for NToV progression. It is a planning model, not a live market capture. The current evidence state is `Research-backed` for the structural trade-offs and `Needs Live Proof` for actual Frostreaver sale prices, clear times, and post-launch item saturation.

## Scope

- early-phase Velious raiding, with NToV as the hard target and Kael or ToV feeder content as the setup path
- GDKP versus static loot distribution for 6-, 12-, 24-, and 36-character raid footprints
- breakeven math, opportunity-cost thresholds, and prep-timeline guidance

Out of scope:

- live bazaar price tracking
- exact krono exchange rates
- automated loot-policy implementation details in the TUI or economy loops

## Source Stack

Primary repo inputs:

- `docs/implementation-roadmap.md`
- `docs/wiki/Roadmap-and-Known-Gaps.md`
- `docs/wiki/Frostreaver-Farming-Guide.md`

Planning seed:

- GitHub issues `#1524` and `#1520`

## Unit Normalization

Issue `#1524` mixes a `50k krono/run` note with gearing costs denominated in plat. For modeling purposes, normalize all returns to **plat-equivalent gross loot value per clear**.

That keeps the math internally consistent:

- gearing costs are already quoted in plat
- farm alternatives in the existing Frostreaver guide are quoted in plat per hour
- krono only matters later as a storage or exchange unit once a live server exchange rate exists

## Baseline Assumptions

| Parameter | Default | Why it matters |
| --- | --- | --- |
| Gross loot value per clear | `50,000pp` plat-equivalent | Imported from issue `#1524` as the Aradune-style benchmark |
| Payout ratio under GDKP | `90%` to raiders, `10%` retained | Covers raid bank, unsold items, consumables, and payout friction |
| Gear investment per character | `40,000pp` to `120,000pp` | Imported from issue `#1524` |
| Raid duration | `3.0h` | Reasonable early-phase window for forming, moving, killing, looting, and liquidating |
| Comparable steady farm rate | `500pp/h` to `1,500pp/h` per active character-equivalent | Matches the Kael, Sebilis, Western Wastes, and ToV ranges already documented in the farming guide |
| Roster footprints modeled | `6`, `12`, `24`, `36` | Covers one group, two groups, and full-raid dilution |

## Core Formulas

Use these formulas to recompute the model when the first live Frostreaver clears happen.

- `per_char_payout = gross_loot_value * payout_ratio / roster_size`
- `breakeven_runs = gear_cost / per_char_payout`
- `raid_pph_per_char = per_char_payout / raid_hours`
- `gross_value_needed_to_beat_farming = roster_size * raid_hours * farm_pph / payout_ratio`

These formulas show the key economic truth: GDKP gets weaker linearly as roster size rises.

## GDKP Breakeven at the 50k Baseline

Assume a `50,000pp` clear, `90%` payout ratio, and a `3.0h` raid.

| Roster size | Payout per character | Breakeven at 40k gear | Breakeven at 120k gear | Raid value per hour per character |
| --- | --- | --- | --- | --- |
| `6` | `7,500pp` | `5.3` clears | `16.0` clears | `2,500pp/h` |
| `12` | `3,750pp` | `10.7` clears | `32.0` clears | `1,250pp/h` |
| `24` | `1,875pp` | `21.3` clears | `64.0` clears | `625pp/h` |
| `36` | `1,250pp` | `32.0` clears | `96.0` clears | `417pp/h` |

Implications:

- one- and two-group raids can justify GDKP as a real income surface if the 50k baseline is real
- full-raid GDKP at the same gross value is not an income engine; it is a slow rebate on a progression raid
- a 36-character NToV force needs either much higher gross value or a much smaller payout pool to beat normal farm alternatives

## When GDKP Actually Beats Farming

Assume the same `3.0h` raid and `90%` payout ratio.

| Roster size | Gross clear value needed to beat `500pp/h` farming | Gross clear value needed to beat `1,000pp/h` farming | Gross clear value needed to beat `1,500pp/h` farming |
| --- | --- | --- | --- |
| `6` | `10,000pp` | `20,000pp` | `30,000pp` |
| `12` | `20,000pp` | `40,000pp` | `60,000pp` |
| `24` | `40,000pp` | `80,000pp` | `120,000pp` |
| `36` | `60,000pp` | `120,000pp` | `180,000pp` |

This is the decision gate for issue `#1524`:

- if the raid cannot reliably clear above the threshold for its live roster size, raiding is not the best money-maker
- once the raid does clear above that threshold, GDKP becomes economically rational again

At the current `50,000pp` benchmark:

- `6`-character GDKP comfortably beats steady farms
- `12`-character GDKP is competitive with strong farms
- `24`- or `36`-character GDKP loses to good Kael, Sebilis, or ToV farm loops unless actual gross values are far higher than the issue seed

## Static Loot Distribution Economics

Static loot does not create immediate plat payout. Its return comes from **replacement cost avoided** and **future clear value unlocked**.

Static distribution is economically correct when:

- one main-tank, cleric, bard, or resist upgrade materially increases raid survivability
- the raid is still wipe-prone or slow enough that unlocking future kills matters more than today’s sale price
- the roster is stable enough that core upgrades stay inside the team rather than walking out the door

Static distribution is weak when:

- attendance is flexible and the roster changes weekly
- many drops are off-class, duplicates, or randomization-driven misfits for the current team
- the raid is already on farm and the real bottleneck is liquid capital, not kill reliability

## Distribution System Comparison

| Decision factor | GDKP | Static priority or rotation |
| --- | --- | --- |
| converts every tradable drop into immediate plat | Strong | Weak |
| handles flexible attendance or plug characters | Strong | Weak |
| accelerates tank, healer, and utility progression | Medium | Strong |
| benefits most from randomized off-class drops | Strong | Medium |
| keeps admin overhead low | Weak | Strong |
| works well at full-raid size on a 50k gross clear | Weak | Strong |

## Recommended Policy for Early-Phase NToV

For the specific issue `#1524` setup, the optimal answer is **not** pure day-one GDKP.

Use a **progression-first static policy with a later hybrid pivot**:

1. **Lockouts 1-4:** static priority on main-tank, cleric, bard, enchanter, and resist-gating pieces.
2. **All duplicate or clearly off-path drops:** liquidate them instead of rotting them.
3. **After the raid becomes repeatable:** convert non-core upgrades and surplus pieces into a payout pool.
4. **Only switch to full GDKP** when actual gross clear value beats the farming threshold for the live roster size.

Why this wins:

- early NToV is progression-gated, not just market-gated
- full-roster GDKP dilution is severe at the issue’s current `50,000pp` benchmark
- randomized-loot servers still reward liquidating duplicates, so a hybrid policy captures both progression and sale value

## Opportunity-Cost Readout

Issue `#1524` explicitly asks whether a character in raid is giving up more plat than it gains.

At the current baseline:

- a `36`-character GDKP raid at `50,000pp` gross produces only `417pp/h` per character-equivalent
- the farming guide already lists multiple steady loops at `500pp/h` to `1,500pp/h`
- that means a full early-phase raid is economically worse than farming unless the raid is buying future progression

So the answer is:

- **small roster:** raid can be primary income
- **large roster:** raid is secondary income until clear value rises
- **full early-phase NToV:** treat the raid as progression first and money second

## Prep Timeline

This timeline assumes Velious-start conditions and the `40k` to `120k` per-character gearing burden from issue `#1524`.

| Window | Priority | Economic implication |
| --- | --- | --- |
| Week `1-2` | level to `60`, establish plat float, finish keys or access work | farming dominates; raid income is negligible |
| Week `2-4` | buy or camp pre-BiS, resist pieces, and Kael or HoT feeder upgrades | use plat to compress time-to-raid-ready |
| Week `4-6` | first NToV attempts, easy dragons, and feeder clears | static priority has the highest leverage here |
| Week `6+` | repeatable clears, duplicate drops, cleaner sale channels | hybrid or full GDKP becomes viable if gross value clears the threshold table |

Default interpretation:

- **week 4 raids** are reasonable as scouting or first-kill attempts
- **week 4 profitable GDKP** is optimistic unless the roster is small or the market is much hotter than the seed assumptions

## Default Answers to the Open Questions

Absent better live data, issue `#1524` should proceed with these defaults:

- **Raid frequency:** `1x` major NToV night plus optional feeder-content farming, not `2x` major raids immediately
- **Loot distribution:** progression-first static with hybrid sale handling, not pure GDKP
- **Roster size:** keep the economic core as small as kill reliability allows; every extra raider dilutes GDKP sharply
- **Timeline:** plan for week `4` attempts and later monetization, not week `4` farm-mode clears
- **Primary vs secondary income:** treat raiding as secondary income until live clear values beat the threshold table

## Live-Proof Tasks Before Promoting to Policy

This page should be updated after the first real Frostreaver lockouts with:

- actual gross loot value per clear in plat-equivalent terms
- actual raid duration from form-up to loot liquidation
- actual roster size for the first stable NToV clears
- how many drops were true core upgrades versus sale-only pieces

Until then, this document should be treated as the bounded planning model for `M10`, not as a claim that Frostreaver will definitely support a profitable full-roster GDKP on day one.
