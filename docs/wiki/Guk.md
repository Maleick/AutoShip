# Guk Farming Validation

This page is the canonical validation ledger for issue `#3384` and the source
of truth for farming evidence in Upper Guk and Lower Guk (gukbottom).
It is a child of parent `#1780`.

Use this page to separate what the repository already supports from what still
needs live EverQuest proof. Do not promote Guk routing, spawn, DPS, or
automation claims beyond the evidence state recorded here.

## Scope

- **Upper Guk** (`upperguk`): froglok camps, levels 10-25, research baseline only
- **Lower Guk** (`gukbottom`): live side (levels 35-42) and dead side (levels 40-48), two checked-in camp configs

This page focuses on Lower Guk because `config/camps/lguk_live_side.toml` and
`config/camps/lguk_dead_side.toml` are already in the repo and the dead-side
camp feeds directly into `sebilis_disco` in the camp progression chain.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Lower Guk can absorb 3-4 groups across live side, dead side, and tunnel camps. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md`, `docs/wiki/P99-Zone-Guide.md` | Guides cite zone capacity, but no live TextQuest session data supports the group-count claim yet. |
| Lower Guk is a primary source of randomized-loot named kills at Classic tier. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md` | Frostreaver guide explicitly calls Lower Guk a high-named-density zone for randomized loot. No live kill-rate data exists. |
| TextQuest has two Guk camp configs with pull lists and thresholds. | Repo-verified | `config/camps/lguk_live_side.toml`, `config/camps/lguk_dead_side.toml` | Both configs define pull points, radii, mob filters, and mana thresholds. Values are planning inputs, not validated baselines. |
| TextQuest has named-mob configs for Lower Guk and Upper Guk with respawn windows and drop lists. | Repo-verified | `config/named_mobs/lowerguk.toml`, `config/named_mobs/upperguk.toml` | Ghoul Lord, Frenzied Ghoul, King Tranix, Ritualist of Hate, and Templar Visk are tracked. Timer windows come from research; none have live sample confirmation. |
| The dead-side camp is the final pre-Sebilis waypoint in the camp progression chain. | Repo-verified | `config/camps/lguk_dead_side.toml` (`next_camp = "sebilis_disco"`) | The dead-side to Sebilis transition is defined in config but has not been timed or tested under live conditions. |
| Epic-component camps in Lower Guk (Frenzied Ghoul for FBSS, Ghoul Lord for Ghoulbane) are viable with a dedicated group. | Research-backed | `config/named_mobs/lowerguk.toml`, `docs/wiki/P99-Zone-Guide.md` | Named configs list FBSS-source Frenzied Ghoul (28-36 min window) and Ghoulbane-source Ghoul Lord (28-36 min window). DPS floor and pull-sequence for sustained named cycling are not yet measured. |
| The HVT watchlist includes two Lower Guk entries. | Repo-verified | `config/hvt_watchlist.toml` | Two `zone = "lowerguk"` entries exist. Exact mobs and alert thresholds should be confirmed against live alert behavior. |

## Research-backed inputs already in repo

### Live-side camp (`lguk_live_side`)

- Zone: `gukbottom`
- Camp center: `[500.0, -300.0, -20.0]`
- Pull point: `[550.0, -250.0, -20.0]`
- Pull radius: `180`, camp radius: `25`, leash: `90`
- Mana thresholds: `rest_mana_pct = 65`, `pull_mana_pct = 35`
- Level range: `[35, 42]`
- Pull list: `a froglok ghoul knight`, `a froglok shin knight`, `a froglok gaz knight`
- Ignore: `the ghoul lord`
- Burn: `the ghoul lord`
- Progression: `prev_camp = "mistmoore_castle"`, `next_camp = "lguk_dead_side"`
- These defaults are planning inputs. No live pull-count, DPS, or mana-recovery baseline exists yet.

### Dead-side camp (`lguk_dead_side`)

- Zone: `gukbottom`
- Camp center: `[-400.0, 100.0, -40.0]`
- Pull point: `[-350.0, 150.0, -40.0]`
- Pull radius: `200`, camp radius: `30`, leash: `100`
- Mana thresholds: `rest_mana_pct = 70`, `pull_mana_pct = 40`
- Level range: `[40, 48]`
- Pull list: `a froglok realist`, `a froglok idealist`, `a froglok ton knight`
- Ignore: `the froglok king`
- Burn: `the froglok king`
- Progression: `prev_camp = "lguk_live_side"`, `next_camp = "sebilis_disco"`
- These defaults are planning inputs. No live DPS baseline or recovery test exists yet.

### Named-mob configs

**Lower Guk** (`config/named_mobs/lowerguk.toml`):

| Name | Level | Respawn window | Location | Drops | Priority |
| --- | --- | --- | --- | --- | --- |
| Ghoul Lord | 45 | 28-36 min | `[-308, -868, -81]` | Flowing Black Silk Sash, Ghoulbane | high |
| Frenzied Ghoul | 44 | 28-36 min | `[-215, -845, -81]` | Short Sword of the Ykesha | high |
| King Tranix | 46 | 28-36 min | `[-170, -1015, -110]` | Robe of the Ishva | medium |
| Ritualist of Hate | 42 | 22-28 min | `[-280, -940, -81]` | Idol of the Underking | low |
| Templar Visk | 43 | 22-28 min | `[-195, -900, -81]` | Dark Circlet | medium |

These timer windows are research-backed estimates, not live-validated samples.
On Frostreaver (randomized loot), named kills at this level tier can drop any
Classic rare of similar level — the drop list above reflects base drop theory
only.

**Upper Guk** (`config/named_mobs/upperguk.toml`):

| Name | Level | Respawn window | Drops | Priority |
| --- | --- | --- | --- | --- |
| Assassin | 30 | 16-22 min | Serrated Bone Dirk | medium |
| Supplier | 28 | 16-22 min | Woven Spider Silk | low |
| Herbalist | 27 | 16-22 min | Thick Fur Cloak | low |

### Map annotations

`config/maps/gukbottom.txt` already annotates `lguk_live_side_camp` and
`lguk_dead_side_camp` with their coordinates. No navigation waypoints or
waypoint-lattice have been validated for the corridor between the two camps.

## DPS Expectations (Epic-Component Camp)

### Current evidence state: Research-backed, not live-measured

The primary epic-component targets in Lower Guk are:

- **Ghoul Lord** → Ghoulbane (Paladin epic component), Flowing Black Silk Sash (FBSS, BiS haste belt)
- **Frenzied Ghoul** → Short Sword of the Ykesha (Warrior/Rogue BiS for its tier)

Research-backed DPS floor assumptions for level 40-48 Guk camps:

| Group comp assumption | Expected sustained DPS per group | Notes |
| --- | --- | --- |
| 1 tank + 1 healer + 4 DPS (levels 40-48) | ~300-500 DPS (estimated) | Research estimate from EQ community; not measured via TextQuest |
| Named mob HP (Ghoul Lord, level 45) | ~8,000-12,000 HP (estimated) | P99 wiki community figures; not confirmed on Frostreaver |
| Kill time on named at ~400 DPS | ~20-30 seconds (estimated) | Derived from HP and DPS estimates above |

These figures are research theory only. Live validation must measure actual kill
time per named and sustained damage output across a full pull rotation before
any DPS claim can be promoted beyond research-backed status.

## Loot-per-Kill Observations

### Current evidence state: Research-backed, not live-measured

On a normal TLP server, named-specific drop rates apply. On Frostreaver
(randomized loot), any Classic rare at the same level tier can drop from any
named in the same expansion.

| Target | Drop rate state | Notes |
| --- | --- | --- |
| Flowing Black Silk Sash (FBSS) | Research-backed theory | Historically camps from Frenzied Ghoul; on randomized loot, any same-tier Classic rare can yield FBSS |
| Ghoulbane | Research-backed theory | Historically from Ghoul Lord; randomized loot changes this to level-tier pool |
| Short Sword of the Ykesha | Research-backed theory | From Frenzied Ghoul at base; randomized pool widens the source set |
| Generic pp output per session | Not estimated | No live session output data exists; do not project pp/hour until a live sample is captured |

All loot-per-kill observations must be recorded from live sessions before any
output rate can be cited as evidence.

## Multibox Pull-Sequence

### Current evidence state: Config-defined, not live-tested

The checked-in pull configs define pull lists but do not specify multibox
pull-sequencing strategy. The following sequence is a research-backed proposal
only:

**Proposed live-side pull sequence (6-box, Warrior tank + Cleric + 4 DPS):**

1. Puller (Monk or Ranger) leaves camp radius and targets nearest pull-list mob.
2. Single-pull using FD (Monk) or ranged aggro (Ranger) to separate from groups.
3. Return mob to camp center `[500.0, -300.0, -20.0]`.
4. Tank picks up, DPS burns — healer holds med until `rest_mana_pct = 65` is met.
5. Next pull begins when mana is above `pull_mana_pct = 35`.
6. Named (`the ghoul lord`) triggers burn rotation; puller avoids pulling additional mobs while named is active.

**Proposed dead-side pull sequence (6-box, Warrior tank + Cleric + 4 DPS):**

1. Puller targets `a froglok realist`, `a froglok idealist`, or `a froglok ton knight`.
2. Dead side has narrower corridors — single pulls are critical to avoid room-aggro.
3. Leash radius `100` is tighter than live side to contain patrol paths.
4. `the froglok king` is ignored during normal rotation; burn rotation activates only on explicit trigger.
5. Pull cadence must account for `rest_mana_pct = 70` — higher than live side due to harder mobs.

These sequences are proposals derived from config values and research. They have
not been executed via TextQuest automation and must be verified in an attended
live session before being treated as confirmed pull behavior.

## Recovery Testing After Interruptions

### Current evidence state: Framework-present, Guk-specific not tested

TextQuest's zone-recovery plumbing applies to Guk sessions:

- `textquest/src/zoning/failure_codes.rs` maps `CorpseInZone = -22` as a zone-entry failure.
- `textquest/src/zoning/recovery.rs` provides `MAX_RECOVERY_RETRIES = 3` with backoff.
- `docs/zone-transition-state-map.md` specifies that live recovery tests record trigger, visible behavior, and recovery outcome.

Lower Guk-specific recovery concerns not yet validated:

| Interruption scenario | Expected behavior | Validation status |
| --- | --- | --- |
| Client death on live side (wipe to named) | Recovery attempts return to safe-coord, not camp center | Not tested |
| Client death on dead side corridor (mob train) | Narrow corridor may block recovery pathing | Not tested |
| Zone disconnect mid-pull | Pull-in-progress mob leashes; puller reconnects to safe point | Not tested |
| Operator HOME pause during active pull | Automation suspends; mob finishes combat with existing aggro | Not tested |
| Resume after extended pause (>10 min) | Camp state validated before next pull attempt | Not tested |

These scenarios must be captured in an attended session before any claim of
stable Guk automation can be made. Record each outcome in the sampling template.

## Needs live proof before this issue can close

- Record actual DPS output per group at the live-side and dead-side camps using TextQuest's
  observability hooks (combat logs, TUI kill counter).
- Sample loot-per-kill for at least 20 named kills across both camps; note whether
  randomized loot produced any FBSS, Ghoulbane, or other FBSS-tier items.
- Execute the proposed multibox pull-sequence in an attended session and note
  whether the leash and pull radius values prevent room-aggro on both sides.
- Test at least two interruption scenarios (wipe recovery and operator pause/resume)
  and record outcomes against the table above.
- Confirm live-side to dead-side camp transition travel time and whether the
  corridor between the two camp centers is safe for unescorted pathing.

## Validation Procedure

### 1. DPS sampling

- Enable TextQuest combat logging and TUI kill counter.
- Run a 30-minute attended session on each camp.
- Record kills per minute, average kill time on trash, and average kill time on
  named mobs.
- Do not report DPS numbers from a single pull — capture the sustained rotation
  rate across the full session window.

### 2. Loot-per-kill sampling

- Record every named kill with timestamp, mob name, and actual loot received.
- On Frostreaver (randomized loot), note the item tier and whether it fell in
  the expected Classic same-level pool.
- Convert totals to pp/session after selling, not before — vendor and bazaar
  prices differ.

### 3. Pull-sequence validation

- Begin with attended operator oversight and manual pull triggering.
- Confirm that the puller returns mobs cleanly without triggering roaming aggro.
- Note any pull-list mobs that linked or aggroed beyond the pull radius.
- After a clean 30-minute run, evaluate whether automation can be trusted for
  semi-attended operation.

### 4. Recovery testing

- Intentionally wipe the group on a named at least once to confirm corpse
  recovery behavior.
- Trigger an operator pause mid-pull and confirm automation suspends cleanly.
- Resume from pause and confirm camp state is consistent.
- Record each outcome in the sampling template.

## Macro Safety and Operator Risk

Lower Guk carries the same policy risk as all TextQuest-automated zones.
Daybreak Account Security Policy classifies third-party automation as cheating.
The current evidence state for unattended Guk automation is:

**BLOCKED** — No live operator proof yet that unattended Lower Guk sessions are safe.

Attended sessions with operator pause controls active are the only currently
supportable posture. See `docs/wiki/Sebilis-Farming-Validation.md` for the
full macro-safety classification framework that applies equally here.

## Checked-in Sampling Template

Use [guk-validation-template.csv](assets/guk-validation-template.csv) for live
sampling. The template is intentionally blank so the repo does not invent kill
rate, loot rate, or recovery numbers that were never observed.

Suggested `target_metric` values for the template:

- `trash_kills_per_minute`
- `named_kills_per_session`
- `loot_items_per_named_kill`
- `pull_aggro_incidents_per_hour`
- `recovery_retries_per_wipe`
- `operator_interruptions_per_hour`

## Exit Criteria

Do not call Guk a validated farming hub until the sampling template has at least:

- One live DPS sample per camp (live side and dead side) covering a 30-minute window
- One loot-per-kill log covering at least 20 named kills across both camps
- One confirmed multibox pull-sequence run with no unintended room-aggro
- One recovery test covering a wipe-and-resume or pause-and-resume scenario
- One explicit operator-risk note stating whether the session was attended or semi-attended

Current live-evidence ownership under issue `#3384`:

- `#3384` owns DPS expectations, loot-per-kill observations, multibox
  pull-sequence, and recovery testing for both Guk camps.
- This issue depends on the dead-side camp being validated before the
  `lguk_dead_side → sebilis_disco` transition can be called live-proven.
