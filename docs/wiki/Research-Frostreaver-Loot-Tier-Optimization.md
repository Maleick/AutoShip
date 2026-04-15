# Research: Frostreaver Loot Tier Optimization

This packet answers [TextQuest#1591](https://github.com/Maleick/TextQuest/issues/1591) and feeds the raid-economics parent issue [#1524](https://github.com/Maleick/TextQuest/issues/1524).

## Scope

Question: if Frostreaver inherits Mischief-style randomized raid loot, which Velious raid target is the best "anchor farm" inside each loot bucket for a 36-character box raid?

This packet focuses on:

- observed Velious raid loot buckets from Mischief/Teek data
- practical anchor targets to farm per bucket
- normalized active-raid loot/hour and Krono/hour modeling
- encounter-locking implications for parallel tag play

This packet does not try to reverse-engineer the hidden Daybreak bucket formula beyond what public data supports.

## Executive Summary

- Frostreaver is confirmed to launch with `Scars of Velious`, `Randomized Loot`, `Free Trade`, and `Encounter Locking`.
- The exact bucket formula is not published in the Frostreaver rules post. Observed Mischief/Teek raid data shows that Velious raid loot is not exact same-level matching. It resolves into four curated raid pools, and the widest observed bucket spans level 55 through 70.
- Best farm anchors for a 36-box raid are:
  - Pool 1: `Lord Yelinak`
  - Pool 2: `Dozekar the Cursed`
  - Pool 3: `Klandicar`
  - Pool 4: `Lodizal`
- Encounter locking does allow multiple simultaneous locks because Daybreak's published FAQ says there is no hard cap on locked NPCs. That helps with split-tagging outdoor spawns, but it does not enable indefinite parking because long-held locks semi-unlock and any unlocked encounter can be contested again.

## Source Stack And Confidence

| Source | Use | Confidence |
| --- | --- | --- |
| [Votes are in!](https://www.everquest.com/news/eq-2026-tlp-polls-outcome) | Confirms Frostreaver launch rules | High |
| [Fangbreaker Rulesets FAQ](https://www.everquest.com/guides/eq-2025-tlp-ruleset-faq) | Official encounter-locking mechanics | High |
| [Mischief/Teek Randomization Name/Raids](https://docs.google.com/spreadsheets/d/1o9tyiIjWnXYII1sNs1szDo3hAwJyghnP8Ybbrt19Cb0/edit) | Public bucket map used by TLP players | High for observed buckets, medium for hidden formula |
| [Mischief progression tracker](https://tlptracker.com/mischief/) | First-kill timing used as a proxy for practical accessibility | Medium |
| [Frostreaver TLP Farming & XP Guide](Frostreaver-Farming-Guide.md) | Internal 36-box roster and prior raid-difficulty assumptions | Medium |

## Method

1. Treat the Bobbybick Mischief/Teek sheet as the best public evidence of observed Velious raid bucket membership.
2. Use the existing 36-box roster in [Frostreaver TLP Farming & XP Guide](Frostreaver-Farming-Guide.md) as the raid baseline:
   - 3 WAR, 1 SK, 2 PAL
   - 6 CLR, 6 BRD, 4 SHM
   - 8 MNK plus utility DPS and support
3. Model profitability in normalized active-raid time, not wall-clock spawn downtime.
   - Reason: open-world respawn windows dominate wall-clock yield and vary with pick state, competition, and whether AoC lockouts are available.
   - Use this packet to decide which boss to prioritize once the spawn is available, not to forecast idle-time ROI.
4. Use "loot package/hour" as the comparison unit.
   - One loot package = one successful raid-boss kill from that bucket.
   - Krono/hour then becomes a sensitivity model: `loot packages/hour * average Krono value per kill package`.

## Observed Velious Raid Buckets

### Pool 1

Observed span: level 66-70

Observed members:

- Tunare
- The Avatar of War
- King Tormax
- Lord Yelinak
- Vulak'Aerr
- Cazic Thule 2.0

Recommendation: `Lord Yelinak`

Why this is the anchor:

- It is the cleanest single-target fight in the bucket for a 36-box raid.
- It avoids the deeper Temple of Veeshan wing progression required for `Vulak'Aerr`.
- It is materially simpler to script and recover than `Tunare` or `Avatar of War`.
- Mischief progression timing shows broader guild access to `Lord Yelinak` earlier than the true apex fights in the same pool.

Planning estimate:

- Active cycle time: `24 minutes`
- Loot packages/hour: `2.5`

### Pool 2

Observed span: level 66-70

Observed members:

- Cekenar
- Dozekar the Cursed
- Jorlleag
- Lady Mirenilla
- Lady Nevederia
- Lord Feshlak
- Lord Vyemm
- Lord Koi'Doken
- Sevalak
- Master of the Guard
- The Final Arbiter
- The Progenitor
- Kildrukaun the Ancient
- Tjudawos the Ancient
- Vyskudra the Ancient
- Zeixshi-Kar the Ancient
- Hraashna the Warder
- Nanzata the Warder
- Tukaarak the Warder
- Ventani the Warder
- Dagarn the Destroyer

Recommendation: `Dozekar the Cursed`

Why this is the anchor:

- It is the most practical pool-2 kill that does not force full Sleeper's Tomb or deeper NToV-style progression.
- It sits in Halls of Testing, which the existing guide already treats as realistic 36-box content.
- Mischief progression timing shows `Dozekar` being killed immediately when Velious opened, which is the strongest public signal that it is the most accessible pool-2 farm.

Planning estimate:

- Active cycle time: `20 minutes`
- Loot packages/hour: `3.0`

### Pool 3

Observed span: level 59-70

Observed members:

- Statue of Rallos Zek
- Wuoshi
- Velketor the Sorcerer
- Dain Frostreaver IV
- Klandicar
- Sontalak
- Lord Doljonijiarnimorinar
- Eashen of the Sky
- Gozzrem
- Ikatiar the Venom
- Lendiniara the Keeper
- Telkorenar

Recommendation: `Klandicar`

Why this is the anchor:

- It is an outdoor dragon with far less setup overhead than `Velketor`, `Dain`, or Temple of Veeshan dragons in the same pool.
- It avoids the Kael faction and clear overhead that makes `Statue of Rallos Zek` look better on paper than it often is in practice.
- `Sontalak` is the other realistic outdoor option, but `Klandicar` is the more conservative choice for a box raid because the pull and recovery pattern is simpler.

Planning estimate:

- Active cycle time: `15 minutes`
- Loot packages/hour: `4.0`

### Pool 4

Observed span: level 55-70

Observed members:

- Galiel Spirithoof
- Sarik the Fang
- Ordro
- a thifling orator
- Grahl Strongback
- Ail the Elder
- Ancient Totem
- Fayl Everstrong
- Rumbleroot
- Treah Greenroot
- Casalen
- Essedera
- Grozzmel
- Krigara
- Lepethida
- Midayor
- Tavekalem
- Ymmeln
- keeper of the glades
- Undogo Digolo
- Lodizal
- Derakor the Vindicator
- Prince Thirneg

Recommendation: `Lodizal`

Why this is the anchor:

- It is the cleanest outdoor single-target kill in the bucket.
- It removes most of the zone-clear and faction overhead tied to Plane of Growth minis, Temple of Veeshan entrance dragons, and `Derakor the Vindicator`.
- The wide level spread in this bucket is the clearest proof that Velious raid randomization is bucketed, not exact same-level matching.

Planning estimate:

- Active cycle time: `12 minutes`
- Loot packages/hour: `5.0`

## Practical Ranking

If the question is "which bucket gives the safest repeatable farm pattern for a 36-box crew," the order is:

1. `Pool 4 / Lodizal`
2. `Pool 3 / Klandicar`
3. `Pool 2 / Dozekar`
4. `Pool 1 / Lord Yelinak`

If the question is "which bucket has the highest likely sale value per successful kill package," the order flips:

1. `Pool 1`
2. `Pool 2`
3. `Pool 3`
4. `Pool 4`

The real economics question is where sale value per kill grows faster than cycle time.

## Loot/Hour And Krono/Hour Model

### Active-Raid Model

Use:

`loot packages/hour = 60 / cycle_minutes`

`Krono/hour = loot packages/hour * average saleable Krono value per kill package`

Base planning basket values for early Velious:

- Pool 4 package: `0.35 Krono`
- Pool 3 package: `0.70 Krono`
- Pool 2 package: `1.25 Krono`
- Pool 1 package: `2.00 Krono`

These are not claimed market facts. They are planning weights so the economics work can be compared before Frostreaver bazaar data exists. Replace them with observed medians once the server opens.

### Sensitivity Table

| Pool | Anchor farm | Cycle minutes | Loot packages/hour | Low market (`0.5x`) | Base market (`1.0x`) | Fresh-launch spike (`2.0x`) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Lord Yelinak | 24 | 2.5 | 2.5 K/hr | 5.0 K/hr | 10.0 K/hr |
| 2 | Dozekar the Cursed | 20 | 3.0 | 1.9 K/hr | 3.8 K/hr | 7.5 K/hr |
| 3 | Klandicar | 15 | 4.0 | 1.4 K/hr | 2.8 K/hr | 5.6 K/hr |
| 4 | Lodizal | 12 | 5.0 | 0.9 K/hr | 1.8 K/hr | 3.5 K/hr |

## Break-Even Read

- Pool 1 beats Pool 2 if its average saleable basket is at least `1.33x` a Pool 2 basket.
- Pool 2 beats Pool 3 if its average basket is at least `1.25x` a Pool 3 basket.
- Pool 3 beats Pool 4 if its average basket is at least `1.25x` a Pool 4 basket.

That means the choice is mostly market-driven once kill execution is stable. The easier bucket is not automatically the best Krono bucket.

## Encounter Locking Implications

Daybreak's published encounter-locking FAQ matters for open-world farm routing:

- The first player added to hate owns the lock.
- The owning group or raid can change membership mid-fight.
- There is `no` cap on how many NPCs a player can have locked at once.
- Long-held locks semi-unlock after extended time.
- `/yell` semi-unlocks a target, and rare or high-difficulty NPCs can only be `/yell`ed by the group or raid leader.

Practical consequence for this issue:

- A 36-box force can split into tag teams and temporarily secure multiple independent open-world raid spawns.
- This helps with outdoor bucket anchors such as `Lodizal`, `Klandicar`, and `Sontalak`.
- It does not create a stable "bank every spawn forever" strategy, because semi-unlock rules and hate-list maintenance still require live control.
- For indoor bosses with real clear overhead, FTE is less important than pathing, staging, and recovery time.

## Answer To The Key Question

How granular are the tiers?

- Not strict same-level only.
- The public Velious raid data strongly supports curated buckets inside the expansion.
- The clearest example is Pool 4, which mixes level 55-60 Plane of Growth and Temple of Veeshan minis with `Lodizal` at level 60, `Prince Thirneg` and `keeper of the glades` at level 65, and `Derakor the Vindicator` at level 70.

Working rule for Frostreaver planning:

- Treat randomized raid loot as `same expansion + observed bucket`, not `same exact mob level`.

## Recommended Operating Playbook

1. Use `Lodizal` as the low-risk bucket-4 sell-through farm.
2. Use `Klandicar` as the mid-tier outdoor bucket-3 anchor.
3. Use `Dozekar` once the raid can clear Halls of Testing cleanly.
4. Graduate to `Lord Yelinak` when the raid wants top-bucket exposure without committing to `Vulak'Aerr`, `Tunare`, or `Avatar of War`.
5. Refresh the Krono basket assumptions with real bazaar medians once Frostreaver has two weeks of market data.

## Sources

- [Votes are in!](https://www.everquest.com/news/eq-2026-tlp-polls-outcome)
- [Fangbreaker Rulesets FAQ](https://www.everquest.com/guides/eq-2025-tlp-ruleset-faq)
- [Mischief/Teek Randomization Name/Raids](https://docs.google.com/spreadsheets/d/1o9tyiIjWnXYII1sNs1szDo3hAwJyghnP8Ybbrt19Cb0/edit)
- [Mischief progression tracker](https://tlptracker.com/mischief/)
- [Frostreaver TLP Farming & XP Guide](Frostreaver-Farming-Guide.md)
