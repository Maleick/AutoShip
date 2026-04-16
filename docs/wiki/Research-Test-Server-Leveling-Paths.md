# Research: Test-Server Leveling Paths and Overlooked Zones

This page tracks the `TextQuest#1576` research pass for pre-launch leveling routes on EverQuest Test or low-pop Live servers.

## Purpose

The goal is not to find the absolute fastest public leveling path. The goal is to find routes that still level well when the obvious guide-driven hotspots are crowded, camped, or too chaotic for unattended automation.

## Validation Status

- This document is a research synthesis, not an in-game stopwatch report.
- No EverQuest client or Test/Live account was available in-session, so exact XP/hour numbers and live `/pickzone` threshold measurements remain unvalidated from this repo pass.
- XP guidance below is therefore a relative tier: `High`, `Medium-High`, `Medium`, or `Low`, based on guide consensus and zone ratings rather than direct timed pulls.

## Negative-Context Baseline

These are the zones that mainstream leveling guides keep recommending and that should be treated as the crowded baseline:

- `Paludal Caverns`, `Unrest`, `City of Mist`, `Lower Guk`, `Karnor's Castle`, `Old Sebilis`, and `Velketor's Labyrinth` all show up repeatedly in Almar's and EQProgression's mainstream leveling brackets.
- RedGuides discussions about "out of the way" leveling explicitly start from the assumption that those mainstream dungeons are where everyone already expects to go.

If the goal is low-intervention automation on Test or a quiet Live server, the useful question is not "what is the best zone?" It is "what still works when we intentionally refuse the obvious answer?"

## Recommended Overlooked Shortlist

| Zone | Suggested bracket | Why it survives the negative-context filter | Expected XP | Density / pull management | Competition | Automation safety | Pick-zone potential |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Steamfont Mountains` or `Misty Thicket` | 1-12 | Lesser-traveled Faydwer/Antonica openers that avoid the Crushbone / Field of Bone funnel while still keeping short pull lines and clear escape space. | Medium | Outdoor singles and small camps; easy to reset bad pulls. | Low | High | Very low |
| `Temple of Cazic-Thule` (classic layout) | 18-30 | RedGuides calls it a "great untouched place" for high teens to low 30s, and a Befallen -> Najena -> CT chain was reported as mostly empty. | Medium-High | Dense enough to chain pulls, but runners and maze turns require tighter camp anchoring than outdoor zones. | Low | Medium | Low |
| `Warsliks Woods` | 20-26 | RedGuides specifically recommends Kunark outdoor zones because most players ignore them in favor of faster dungeons. | Medium | Wide pull lanes and easy singles around the giant fort / sarnak edges. | Low | High | Very low |
| `Crypt of Dalnir` | 26-35 | Almar flags it as an antisocial dungeon with an amazing ZEM, and EQProgression keeps it in the 26-33 bracket. | High | Moderate density by floor; manageable if the group stays on one ramp/floor segment at a time. | Low | Medium | Low |
| `The Overthere` | 31-39 | Repeatedly recommended in the RedGuides off-meta discussion as a place players skip while chasing faster dungeon XP. | Medium | Large outdoor lanes with clear line-of-sight and simple pull geometry, especially around safer edge camps. | Low-Medium | High | Very low |
| `Crystal Caverns` | 27-40 | Velious alternative that EQProgression rates well in the high 20s / low 30s but that is still overshadowed by Unrest / Lower Guk / City of Mist talk. | High | Split orc and geonid pockets let groups hold stable camps without zone-wide pathing. | Low-Medium | Medium-High | Low-Medium |
| `Tower of Frozen Shadow` | 28-40 | Quiet Velious dungeon with strong bracket coverage and floor-based camp splits that are easier to reserve than mainstream open dungeons. | High | Moderate per-floor density; keying is the operational cost, but floor boundaries make pull management predictable. | Low-Medium | Medium | Low |
| `Howling Stones` | 42-55 | RedGuides still lists it as a quiet low-50s answer when Karnor's / Sebilis are the default conversation. | High | Wing-based pulls with good density, but keying and caster-heavy packs raise the attention floor. | Low-Medium | Medium | Low |
| `Chardok` (entrance -> castle camps) | 44-60 | RedGuides notes multiple less-popular camps, fast respawns, good trash cash, and active picks even when only a few camps are taken. | High | Very dense. Favor entrance, kennel-adjacent, or castle-side camps instead of the most contested named loops. | Medium | Medium | Medium-High |

## Best-Fit Paths

### Path A: Lowest-Intervention Outdoor Route

Use this when the priority is stable pulling, clear recovery lines, and minimal pathing surprises.

1. `Steamfont Mountains` or `Misty Thicket` through level 10-12.
2. `Warsliks Woods` from 20-26.
3. `The Overthere` from 31-39.
4. `Crystal Caverns` from the high 20s into the upper 30s if more density is needed.
5. `Chardok` entrance or safer side camps from the mid 40s onward if a private or low-traffic pick is available.

Why this path works:

- It avoids the standard "everyone goes here" dungeon ladder.
- The outdoor segments have the most forgiving pull geometry for unattended automation.
- The later pivot to `Crystal Caverns` or `Chardok` restores XP density only after the group has the toolkit to survive bad pulls.

### Path B: Highest XP Without Defaulting to the Crowd

Use this when the group can handle indoor camps but still wants to dodge the headline zones.

1. `Steamfont Mountains` or `Misty Thicket` through level 10-12.
2. `Temple of Cazic-Thule` from the high teens through the 20s.
3. `Crypt of Dalnir` from 26-35.
4. `Crystal Caverns` or `Tower of Frozen Shadow` through the upper 30s.
5. `Howling Stones` or `Chardok` from the low 40s into the 50s.

Why this path works:

- `Temple of Cazic-Thule`, `Crypt of Dalnir`, and `Tower of Frozen Shadow` all keep better-than-average XP while sitting outside the loudest public leveling meta.
- The route climbs from simple openers into keyed or floor-split dungeons only after the group has more tools for recovery and crowd control.

### Path C: Pick-First Fallback

Use this when the operator wants the best chance of landing a low-traffic or private instance.

1. Prefer `Chardok` first.
2. Consider `Velketor's Labyrinth` inner sections only if competition is lighter than the usual named camps.
3. Treat most "quiet because nobody is there" zones as bad pick candidates.

Important caveat:

- `/pickzone` only opens if a pick is currently available in the zone.
- Bonzz notes that most players use Test to experiment, not to live there full time.
- That means the exact trait that makes a zone attractive for quiet automation usually makes it worse for forcing a private pick.

## Zones To De-Prioritize For Unattended Automation

| Zone | Why it does not make the primary recommendation list |
| --- | --- |
| `Temple of Droga` | EQProgression still rates the bracket, but Almar's Droga notes describe runners and healers through walls. Good XP is not enough if every bad split cascades. |
| `Kaesora` | The bracket is correct on paper, but the multi-level undead layout is less forgiving than `Crypt of Dalnir` or `Crystal Caverns`. Better as an attended boxer detour than a default automation route. |
| `The Hole` | RedGuides still points to it as a quiet answer, but the vertical geometry and expensive recovery paths make it a poor unattended default even when the crowd level is ideal. |
| Deep `Mistmoore` | Almar repeatedly recommends Mistmoore in the broad leveling meta, but also warns that deeper sections jump in difficulty fast. That makes it the opposite of "consistent mob difficulty." |

## Practical Takeaways

- The strongest off-meta core is `Warsliks Woods -> Crypt of Dalnir -> Crystal Caverns -> Chardok`.
- If the priority is raw automation safety over XP, bias toward outdoor zones first and only add indoor density when the group outgrows the safer brackets.
- If the priority is forced privacy via picks, quiet zones are not enough by themselves. `Chardok` is the best candidate from this pass because community reports still mention open camps and available picks.
- `Temple of Cazic-Thule` is worth validating early because it is the clearest "everybody says go elsewhere, but this is still empty" recommendation in the source set.

## Sources

- [EQProgression zone leveling guide](https://www.eqprogression.com/zone-leveling-guide/) for broad level brackets and zone ratings.
- [Almar's progression server leveling guides](https://www.almarsguides.com/EQ/Leveling/Kunark/) and [later-era leveling guide index](https://www.almarsguides.com/eq/leveling/omens/) for "antisocial", "weakest mobs", and mainstream baseline comparisons.
- [RedGuides thread: Suggestions for out of the way leveling on TLP?](https://www.redguides.com/community/threads/suggestions-for-out-of-the-way-leveling-on-tlp.73310/) for quiet-zone community heuristics from automation-minded players.
- [Bonzz Test Server guide](https://www.bonzz.com/test.htm) for current Test server behavior and population context.
- [RedGuides command reference for `/pickzone`](https://www.redguides.com/docs/commands/) for pick-availability behavior.
- Repo-local references: [Frostreaver Farming Guide](Frostreaver-Farming-Guide.md) and [P99 Zone Guide](P99-Zone-Guide.md).
