# Velious-Era Class Synergy Validation

> Research date: 2026-04-15
> Issue: `TextQuest#1593`
> Scope: independent validation of Jeff's locked 36-account Frostreaver composition for Velious launch

## Table of Contents

1. [Question Under Test](#question-under-test)
2. [Baseline Roster](#baseline-roster)
3. [Findings At A Glance](#findings-at-a-glance)
4. [External Evidence](#external-evidence)
5. [Jeff Comp vs Common Boxer Comps](#jeff-comp-vs-common-boxer-comps)
6. [Assessment Of The Key Tradeoffs](#assessment-of-the-key-tradeoffs)
7. [Verdict](#verdict)
8. [Sources](#sources)

---

## Question Under Test

Jeff's locked roster intentionally prefers survivability and recoverability over maximum theoretical DPS. This page validates whether that tradeoff still makes sense for a Velious-start TLP where the army is expected to run under automation instead of under constant high-attention manual play.

The specific questions from `TextQuest#1593` were:

- whether the "bard + heavy healing" backbone is the right automation core
- whether the roster is materially more defensive than common TLP multibox builds
- whether a DPS-first alternative should trim healers for more melee
- whether `8 Monks` is the right Velious-launch melee stack
- whether `6 Monks + 2 Berserkers` is even a valid launch-era alternative

## Baseline Roster

The canonical locked roster is documented in [Frostreaver Farming Guide](Frostreaver-Farming-Guide.md):

- 3 Warriors, 1 Shadowknight, 2 Paladins
- 6 Clerics, 6 Bards, 4 Shamans, 1 Druid, 1 Enchanter
- 8 Monks plus 1 Beastlord, 1 Ranger, 1 Wizard, and 1 Magician

Important nuance: Jeff's live roster is not literally "two healers in every group." The actual pattern is:

- 4 melee groups with `tank + cleric + bard + shaman + 2 monks`
- 1 utility group with `paladin + cleric + bard + druid + beastlord + ranger`
- 1 caster/logistics group with `paladin + cleric + bard + enchanter + wizard + magician`

So the roster really uses:

- a universal `bard + cleric` floor in all 6 groups
- a second healer or support-healer in 5 of 6 groups
- a single pure utility/CC group rather than 6 identical melee pods

## Findings At A Glance

- Jeff's comp is more conservative than the usual "manual boxer" TLP group, but it is still within the normal megabox design space.
- Public boxer advice strongly supports `one bard per group` as the default answer once the roster is large enough to build full groups on purpose.
- Community advice is less rigid on the second healer/support slot. That is where most people trim for DPS once they trust their tank, their gear, and their own reaction speed.
- `8 Monks` is the cleanest Velious-legal melee stack among the options discussed here.
- `6 Monks + 2 Berserkers` is not a Velious-launch option because Berserkers are a later-expansion class.

## External Evidence

### 1. Bard-per-group is mainstream boxer advice, not a Jeff-only quirk

The strongest cross-source consensus is on bards, not on extra healers.

- In a RedGuides megabox thread about building a 54-character TLP force, multiple posters treated `bard + cleric per group` as the baseline and argued that every group wants a bard for speed, resists, utility, and melee/caster support. One poster explicitly called bard-and-cleric-per-group "the staple," while others argued there is "really no sub" for a bard in each group on TLP.[^choosing-a-tlp]
- That same thread also shows the real point of disagreement: not whether bards are worth the slot, but how many additional shamans, tanks, or clerics you still need after the bard/cleric floor is in place.[^choosing-a-tlp]

That lines up well with Jeff's locked roster. Six bards is not excessive for this use case; it is consistent with public boxer heuristics for intentionally built raidable group blocks.

### 2. Public boxer advice usually trims the overlap slot for DPS once the player trusts the setup

The common small-group TLP boxer pattern is lighter than Jeff's.

- A RedGuides "Good group set up?" thread reduces the baseline to "get a tank and healer, and you can do most anything with the other spots," then fills the remaining slots with Bard and DPS/utility picks.[^good-group]
- A RedGuides "Which toon do I sit?" thread treats `Cleric/Shaman` and `Bard/Enchanter` as overlapping utility pairs and says the player should bench one of the overlap slots for more damage once the tank can live without it.[^which-toon]
- A RedGuides "Rate my 6 box" thread is the closest direct analogue to Jeff's tension. The poster says `SK / Bard / Cleric / Shaman / Monk / Monk` felt very safe but slower than a leaner DPS setup, and other replies recommend converting duplicated monk or support slots into synergy picks.[^rate-my-6-box]

This is the clearest independent validation of Jeff's own framing: the robustness pattern is real, and the cost is real. Public boxers regularly trim the second healer/support slot for more DPS once they are comfortable with the content.

### 3. Monk-centered melee stacks are well supported for Kunark/Velious-era play

For the actual Velious era, the monk-heavy part of Jeff's comp is easier to validate than the healer density.

- A Project 1999 Kunark/Velious duo discussion explicitly calls out monks as high-DPS melee that can also handle slowed non-raid targets surprisingly well, and it contrasts that strength against cleric-heavy setups that become very safe but very slow.[^p99-duo]
- An EverQuest forum boxing thread says Shaman is generally the better long-term partner for a monk than Druid, and it specifically mentions Bard or Enchanter as the next support layer if the player expands into a trio.[^paladin-boxing]

The common thread is straightforward:

- Monk scales well early without demanding late-expansion weapon or synergy support.
- Monk pairs naturally with Shaman slow/buffs and with Bard support.
- Cleric-heavy or dual-healer versions trade speed for safety.

That matches Jeff's four melee-core groups almost exactly.

### 4. Berserkers are not part of a Velious-launch decision tree

`6 Monks + 2 Berserkers` fails on era legality before it even reaches a DPS comparison.

- The official EverQuest progression-server news post for January 8, 2018 marks Gates of Discord as a later TLP unlock, well after Velious.[^god-unlock]
- Community EverQuest references identify Berserker as a Gates of Discord class rather than a Velious-era class.[^berserker-wiki]

For this issue, that means the real launch-era question is not "Monk or Berserker?" It is closer to:

- more monks versus more rogues/casters
- more monks versus trimming support/healing for another melee DPS slot

Jeff's choice to keep the 8 monks is therefore the correct Velious-era framing.

## Jeff Comp vs Common Boxer Comps

| Topic | Jeff's locked roster | Common manual 6-box pattern | Common megabox heuristic | Validation |
| ----- | -------------------- | --------------------------- | ------------------------ | ---------- |
| Bard density | 6 bards, 1 per group | Usually 1 bard if the player is building a "real" camp group | Often 1 bard per group once building raidable blocks | Strongly supported |
| Cleric density | 6 clerics, 1 per group | Often 0-1 cleric; many players use only 1 primary healer | Megabox threads frequently anchor each group or tank block with clerics | Supported, but conservative |
| Secondary healer/support | 4 shamans + 1 druid + 1 enchanter on top of the cleric floor | Commonly trimmed for DPS once tank survivability is proven | Debated; this is where posters min/max | Jeff is intentionally above the DPS meta here |
| Monk density | 8 monks | Usually 1-3 monks in a 6-box melee core | Monks frequently appear in melee pods; later-era groups add Berserkers/Rogues | Strongly supported for Velious |
| Tank density | 3 WAR + 1 SK + 2 PAL | Usually 1 real tank plus maybe 1 utility tank | Megabox posts still keep multiple geared tanks, but not more than needed | Reasonable for unattended raids and recovery |

The practical summary is:

- Jeff's roster is heavier on tank/heal coverage than the average player-run XP group.
- Jeff's roster is not unusually bard-heavy compared with multibox raid heuristics.
- The "extra safety" lives mostly in the second healer/support slots and the real tank floor, not in the monk count.

## Assessment Of The Key Tradeoffs

### Bard + 2 healers per group as the survivability core

This claim is directionally correct, but it needs one wording fix: Jeff's real roster is `bard + cleric everywhere`, with a second healer/support in most groups rather than in all groups.

Validation:

- `bard + cleric everywhere` is strongly consistent with public boxer advice
- the extra healer/support slot is the exact place where public boxers say they would cut for DPS if their tank and attention budget can support it
- because TextQuest's target mode is automation-first rather than reflex-heavy manual play, Jeff's conservative choice is justified even if it is not the server's absolute DPS ceiling

### DPS-optimized alternative

The public boxer meta does show the alternative clearly:

- trim one overlap slot
- keep the bard
- keep at least one real healer
- add another melee or synergy pick

In practical Velious terms, that would look more like:

- `tank / cleric / bard / monk / monk / flex DPS`
- `tank / shaman / bard / monk / monk / monk`
- or a mixed melee pod that replaces one support slot with rogue, ranger, mage, or beastlord utility depending on goals

That kind of roster will kill faster. The cost is:

- less wipe recovery
- less room for pathing or pull mistakes
- less slack for bad assist swaps or healer lag
- more dependence on human intervention when something breaks

For a manual boxer, that is often acceptable. For unattended automation, it is exactly the kind of optimization that tends to convert small mistakes into cascading failures.

### 8 Monks vs 6 Monks + 2 Berserkers

For Velious launch, keep the 8 monks.

Reasoning:

- Berserkers are not unlocked yet, so the proposed alternative is era-invalid.[^god-unlock] [^berserker-wiki]
- Monk is already well-supported by the public Kunark/Velious-era evidence as an early, low-friction melee DPS anchor.[^p99-duo] [^paladin-boxing]
- Monk also preserves backup pulling and utility value that pure DPS swaps do not always preserve

If the roster is revisited in a much later era, then Berserker, Rogue, and Beastlord distribution becomes a fresh question. That is not a Velious-launch question.

## Verdict

No change is justified to Jeff's locked Velious-launch roster based on the public sources reviewed here.

The external evidence supports three conclusions:

1. Jeff's bard floor is correct. Public boxer advice repeatedly converges on bard-per-group once the goal is stable multigroup play.
2. Jeff's healer/support density is above the common DPS-max pattern, but that is the right place to be conservative for unattended automation.
3. `8 Monks` is the right launch-era answer among the options in the issue, because the suggested Berserker swap is not available in Velious.

The strongest "optimize later" signal is not "replace monks." It is:

- revisit whether every farm group still needs the same amount of secondary healing/support once live data shows the real failure modes
- keep the bard + cleric floor intact
- trim only after the automation has proven it can recover cleanly from bad pulls, pathing drift, and healer desync without operator rescue

## Sources

[^choosing-a-tlp]: [RedGuides: "Choosing a TLP"](https://www.redguides.com/community/threads/choosing-a-tlp.78463/) — useful for cross-era boxer heuristics on bard, cleric, shaman, tank, and DPS distribution in large TLP box armies. Several replies discuss the survivability-versus-DPS trade directly.

[^good-group]: [RedGuides: "Good group set up?"](https://www.redguides.com/community/threads/good-group-set-up.92634/) — small-group boxer advice that reduces the foundation to a real tank plus healer and then fills the remaining slots with Bard and DPS/utility.

[^which-toon]: [RedGuides: "Which toon do I sit?"](https://www.redguides.com/community/threads/which-toon-do-i-sit.89357/) — useful evidence that public boxers see `Cleric/Shaman` and `Bard/Enchanter` as the main overlap slots to cut when shifting from safe to faster comps.

[^rate-my-6-box]: [RedGuides: "Rate my 6 box. Feedback / constructive criticism please"](https://www.redguides.com/community/threads/rate-my-6-box-feedback-constructive-criticism-please.94694/) — direct boxer report that `SK / Bard / Cleric / Shaman / Monk / Monk` is safe but slower than leaner DPS variants.

[^paladin-boxing]: [EverQuest Forums: "Paladin + ? for boxing?"](https://forums.daybreakgames.com/eq/index.php?threads/paladin-for-boxing.275121/) — public forum discussion that treats Shaman as the stronger long-term partner for Monk and points to Bard/Enchanter as the common next support layer.

[^p99-duo]: [Project 1999 archive: "Most solid duo for Kunark and Velious"](https://www.project1999.com/forums/archive/index.php/t-8691.html) — era-relevant community discussion that calls out Monk DPS strength and the speed loss that comes from substituting safer but lower-damage support.

[^god-unlock]: [EverQuest: "Gates of Discord is Now Available on Lockjaw and Ragefire!"](https://www.everquest.com/news/eq-god-unlock-lockjaw-ragefire-progression-servers-jan-2018) — official proof that Gates of Discord is a later TLP unlock than Velious.

[^berserker-wiki]: [EverQuest Wiki: Berserker](https://everquest.fandom.com/wiki/Berserker) — community reference identifying Berserker as a Gates of Discord class.

## Research Limits

- Public TLP Discord archives were not accessible in-session, so this write-up relies on public RedGuides, EverQuest forum, Project 1999, and official EverQuest sources.
- Several RedGuides megabox comments are later-era than pure Velious. They are used here only for multibox roster heuristics such as bard density, cleric floors, and the survivability-versus-DPS trade, not as direct proof of Velious-era class balance.
