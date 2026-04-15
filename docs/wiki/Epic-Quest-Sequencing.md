# Epic Quest Sequencing

This page captures the roster-level plan for parallelizing the Shaman and Beastlord epic quests on the Frostreaver 36-box roster. The goal is to keep the Truespirit bottleneck moving for Shamans without wasting spare account time that can already be spent on Beastlord item collection.

## Scope

- Shaman epic: **Spear of Fate**
- Beastlord epic: **Claw of the Savage Spirit**
- Roster assumption: 6-8 Shamans and multiple Beastlords sharing the same leveling, raid, and farm windows
- Era assumption: Velious-start progression with Kunark and Velious farming lanes opening during the same ramp-up window
- Beastlord availability assumption: this sequencing assumes Frostreaver enables Beastlords and their epic during this window; otherwise, the Beastlord lane begins only when Beastlords are available on the server ruleset
- Out of scope: full quest walkthroughs, NPC dialog transcripts, and exact live spawn timers

## Evidence State

| Planning item | State | Why it matters |
| --- | --- | --- |
| Shaman epic is the critical path because Truespirit faction and hand-ins do not scale cleanly across many characters | Research-backed | This is the main sequential bottleneck in the roster plan |
| Beastlord epic progress can be spread across extra accounts because it is item-heavy rather than faction-gated | Research-backed | Spare groups can keep making progress instead of waiting on Shaman hand-ins |
| Exact Shaman turn-in throughput on Frostreaver, including failure recovery and weekly completion rate | Needs Live Proof | This determines how aggressively later Shaman waves can start |
| Exact Beastlord camp map, respawn variance, and group-vs-solo efficiency for each blocker item | Needs Live Proof | This determines how much parallel farm capacity the roster can absorb |

## Bottleneck Comparison

| Epic | Primary gate | Parallelism | Failure cost | Best roster use |
| --- | --- | --- | --- | --- |
| Shaman: Spear of Fate | Truespirit faction and precise quest hand-ins | Low | High; a missed turn-in or faction mistake slows the whole queue | Start early, stagger starts, and keep the work on the most reliable characters first |
| Beastlord: Claw of the Savage Spirit | Named drops, quest pieces, and zone access | High | Medium; missed drops cost time but not the entire lane | Use overflow groups and off-hours farm windows to fill a shared bank of pieces |

The practical implication is simple: do **not** try to advance every Shaman at once. Treat the Shaman epic as the sequential lane and Beastlord progress as the overflow lane that soaks up extra farming capacity.

## Recommended Sequencing

| Window | Shaman lane | Beastlord lane | Roster intent |
| --- | --- | --- | --- |
| Weeks 0-2 | No epic hand-ins yet; finish leveling, pre-BiS, travel, and shared utility prep | Bank any opportunistic quest pieces without diverting prime groups | Preserve leveling momentum and avoid early bottlenecks |
| Weeks 3-4 | Start the oldest or most raid-critical Shamans on **Spear of Fate** | Continue passive banking only | Open the sequential lane as soon as the roster can support it |
| Weeks 4-8 | Keep the first Shaman wave moving; stagger new starts by several days instead of stacking them on one window | Start active **Claw of the Savage Spirit** farming once the relevant zones are open | Turn spare groups into parallel throughput instead of idle wait time |
| Weeks 8+ | Start the second Shaman wave only after the first wave proves the real turn-in cadence | Keep Beastlord farming active until the rare-drop blockers are done | Let real completion rate, not optimism, decide when to widen the queue |

### Why this order wins

1. The first Shaman wave converts the hardest bottleneck into real data early.
2. Beastlords can absorb spare group time without blocking Shaman completions.
3. Later Shamans start only after the faction lane has a measured pace instead of a guessed pace.

## Shaman Lane Rules

- Prioritize the oldest, most raid-critical, or best-geared Shamans first.
- Keep a single checklist for each Shaman before every important turn-in:
  - faction level confirmed
  - all required items present
  - no mixed bags between characters
  - hand-in owner and escort support confirmed
- Stagger Shaman starts instead of stacking them. The goal is steady completions, not simultaneous partial progress.
- Reserve experienced operator attention for the hand-in windows. This lane has the highest irreversible mistake cost.

## Beastlord Lane Rules

Beastlord progress should be organized by **variance**, not by narrative quest order.

| Component bucket | Examples from the current issue scope | Recommended handling |
| --- | --- | --- |
| High-variance blockers | `Draz Nurakk's Head`, `Jagged Claws`, `Khati Sha Seal` | Put dedicated groups on these first once the zones unlock and keep loot ownership centralized |
| Shared bank materials | `Gems of the Void`, `Acrylia Sphere`, `Dense Fungal Padding` | Collect these opportunistically during overflow farming and store them in a shared epic bank |

This keeps the roster from over-investing in easily replaceable pieces while the true blocker drops remain unresolved.

## Group Allocation Model

| Group budget | Assignment | When to use it |
| --- | --- | --- |
| 1-2 groups | Shaman faction / escort / hand-in support | During active Shaman push windows |
| 2-3 groups | Beastlord blocker camps | After required zones unlock and the first Shaman wave is stable |
| Remaining groups | XP, pre-BiS, plat, and general roster upkeep | Always; do not starve baseline progression for quest work |

The sequencing fails if every group gets pulled into epic work too early. Keep enough groups on general progression so the roster keeps improving while the quest lanes advance.

## Risk Register

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Shaman hand-in failure or wrong-character turn-in | High | Use per-character checklists and keep a single operator responsible for critical turn-ins |
| Shaman faction cadence slower than expected | High | Delay the second Shaman wave until the first wave establishes a measured completion rate |
| Beastlord blocker drops have worse variance than expected | Medium | Start the blocker camps before the low-variance combine pieces dominate group time |
| Zone unlock timing slips | Medium | Do not commit to Beastlord completion dates until the required zones are confirmed live |
| Too many groups leave leveling and gear progression for epic work | Medium | Cap epic-dedicated groups and keep spare groups on baseline roster improvement |

## Validation Still Required

The plan above is a sequencing guide, not a guaranteed completion SLA. Before treating it as fixed policy, validate the following on live Frostreaver:

- actual Truespirit turn-in throughput per week once the first Shaman wave starts
- exact camp map for each Beastlord blocker item
- respawn and competition variance for the rare-drop Beastlord pieces
- whether full-group support materially outperforms solo or small-box handling on each epic step

## Related Issues

- Parent milestone tracker: [#1520](https://github.com/Maleick/TextQuest/issues/1520)
- Sibling economy/planning issues: [#1525](https://github.com/Maleick/TextQuest/issues/1525), [#1526](https://github.com/Maleick/TextQuest/issues/1526), [#1527](https://github.com/Maleick/TextQuest/issues/1527)
