# Pick Threshold Measurements — Low-Level Target Zones (Live EQ)

**Issue:** #3355 (child of #1763)
**Source zone list:** #1577 candidate list (Research-Test-Server-Leveling-Paths.md)
**Status:** PENDING LIVE MEASUREMENT — live EQ access required to populate thresholds

## Overview

EverQuest's `/pickzone` command opens a private pick instance of a zone when player
population in the zone exceeds a server-defined threshold. This artifact tracks measured
thresholds for the candidate zones identified in the #1577 research pass.

A "pick" is only available when:
1. The zone supports pick instances (not all zones do).
2. Current player count in the zone meets or exceeds the threshold.
3. The server has capacity to spawn an additional instance.

The goal is to determine which zones can be reliably picked into with a 36-account group
and which zones will never offer a pick because traffic is too low.

## Measurement Protocol

To populate this table with live data:

1. Log in to live EQ with at least one character.
2. Zone into each candidate zone.
3. Run `/pickzone` — if a pick is available, it will list instance options.
4. Note how many players are present (check `/who all` count) when pick first appears.
5. Test at multiple time-of-day windows (peak and off-peak) over at least 3 sessions.
6. Record: min observed player count when pick was available, max observed count when
   no pick was available, and whether /pickzone ever triggered with a 36-account load.

## Zone Measurement Table

| Zone | Level Bracket | Pick Support | Observed Threshold (players) | 36-Account Reliability | Notes |
|------|--------------|-------------|------------------------------|------------------------|-------|
| Steamfont Mountains | 1-12 | Unknown — pending live test | — | — | Low-traffic outdoor opener; community consensus suggests picks unlikely (zone rarely crowded enough) |
| Misty Thicket | 1-12 | Unknown — pending live test | — | — | Same pattern as Steamfont; avoid Crushbone funnel; pick very unlikely |
| Temple of Cazic-Thule (classic layout) | 18-30 | Unknown — pending live test | — | — | RedGuides reports mostly empty; pick extremely unlikely due to low population |
| Warsliks Woods | 20-26 | Unknown — pending live test | — | — | Kunark outdoor zone; skipped by most players; pick unlikely |
| Crypt of Dalnir | 26-35 | Unknown — pending live test | — | — | Antisocial dungeon, strong ZEM; low traffic means pick probably unavailable |
| The Overthere | 31-39 | Unknown — pending live test | — | — | Large outdoor zone; off-meta; pick unlikely |
| Crystal Caverns | 27-40 | Unknown — pending live test | — | — | Velious alt; low-medium competition; pick possible if multiple groups present |
| Tower of Frozen Shadow | 28-40 | Unknown — pending live test | — | — | Quiet Velious dungeon; pick possible if keyed groups double up |
| Howling Stones | 42-55 | Unknown — pending live test | — | — | Quiet low-50s; keyed zone; pick probably unavailable |
| Chardok (entrance/castle camps) | 44-60 | Likely YES — community reports picks available | ~6-10 players (estimated) | Medium-High — group alone may not trigger; combine with 1-2 other groups present | Best pick candidate from #1577 pass; RedGuides explicitly notes picks available even with light camp occupation |

## Pre-Fill Hypothesis (from #1577 research synthesis)

Based on community reports and zone popularity data:

| Pick Support Tier | Zones |
|-------------------|-------|
| **Likely YES** (pick usually available with moderate server pop) | Chardok |
| **Possible** (pick may appear during peak hours or high-pop events) | Crystal Caverns, Tower of Frozen Shadow |
| **Unlikely** (zone traffic too low to trigger pick threshold) | Howling Stones, Crypt of Dalnir, Tower of Frozen Shadow (off-peak) |
| **Very Unlikely** (outdoor zones; almost never crowded enough) | Steamfont Mountains, Misty Thicket, Warsliks Woods, The Overthere, Temple of Cazic-Thule |

## No-Pick-Support Zones (Explicit)

The following zones from the candidate list are expected to have **no pick support** or
**insufficient accounts** to trigger picks based on current research:

- **Steamfont Mountains** — outdoor zone, rarely occupied, no pick expected
- **Misty Thicket** — outdoor zone, rarely occupied, no pick expected  
- **Warsliks Woods** — Kunark outdoor, community consistently reports low population
- **Temple of Cazic-Thule** — empty by design (off-meta recommendation); pick never reported
- **The Overthere** — outdoor, off-meta; population too diffuse for picks

## Blocking Status

This artifact is **blocked on live EQ access**. An operator with a live account must:

1. Zone each candidate with `/who all` monitoring
2. Run `/pickzone` at population intervals to bracket the threshold
3. Update the "Observed Threshold" and "36-Account Reliability" columns above

The 36-account scenario is particularly important: even zones with theoretical pick support
may not trigger if our own 36 accounts are the only occupants — the threshold requires
third-party players in the zone.

## Related Issues

- Parent: #1763 (live EQ testing umbrella)
- Zone candidate list source: #1577 (scaffold)
- This measurement artifact: #3355
