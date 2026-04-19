# Velketor's Labyrinth Validation

This page is the canonical validation ledger for issue `#1720` and the source
of truth for any claim that the Velketor Frenzy route is live-safe for a
6-box TextQuest session.

Use this page to separate what the repository already supports from what still
needs live EverQuest proof. Do not promote the Frenzy hall route, pit branch,
or upper-dogs extension beyond the evidence state recorded here.

## Current Evidence State

| Claim                                                                                                                       | Evidence state              | Repo basis                                                                                           | Notes                                                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------- | --------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| TextQuest has a loader-safe Velketor camp baseline for the Frenzy hall.                                                     | Research-backed             | `config/camps/velketors_labyrinth_frenzy.toml`, `docs/wiki/Camp-Runbooks.md`                         | The repo now captures safe-hall med coordinates, pull defaults, named filters, and a documented waypoint lattice.              |
| TextQuest already documents the key geometry hazards that make Velketor risky for unattended routing.                       | Research-backed             | `docs/wiki/Camp-Runbooks.md`, `docs/wiki/P99-Zone-Guide.md`                                          | Slippery ramps, pit drops, see-invis mobs, and Lord Bob or Bledrek no-pull branches are now explicitly documented.             |
| TextQuest already has operator pause and camp-return controls that can support attended validation.                         | Research-backed             | `textquest/src/tui/event.rs`, `textquest/src/tui/state.rs`, `docs/wiki/Combat-and-Camp-Loop.md`      | The repo can pause or resume automation and preserve camp state, but that is not proof that the Velketor route is stable live. |
| TextQuest already has route-recovery and zoning observability that can frame leash or corpse-risk checks during a live run. | Research-backed             | `textquest/src/zoning/recovery.rs`, `docs/zone-transition-state-map.md`, `docs/dev/observability.md` | Recovery plumbing exists, but no checked-in Velketor sample confirms that Frenzy or pit recovery is safe in practice.          |
| Velketor's Labyrinth uses a fast enough spawn cadence to keep the Frenzy route busy for a full 6-box group.                 | Research-backed expectation | `docs/wiki/Camp-Runbooks.md`, `docs/wiki/Frostreaver-Farming-Guide.md`                               | The repo records the `32:50` zone timer guidance and named cluster, but still lacks live placeholder and wait-time samples.    |
| The documented Frenzy route is safe for unattended 6-box farming.                                                           | Needs live proof            | Issue `#1720`, `docs/wiki/Camp-Runbooks.md`                                                          | This is the remaining blocker: the route, leash behavior, med thresholds, and return path all still need a live session.       |

## Research-backed Inputs Already In Repo

- `config/camps/velketors_labyrinth_frenzy.toml` defines the current runtime
  baseline for camp center, pull point, radii, mana thresholds, and pull or
  ignore lists.
- `docs/wiki/Camp-Runbooks.md` records the 28-point route
  lattice, restriction zones, multibox route branches, and spawn-pattern notes.
- `textquest/src/camp/config.rs` includes a checked-in parse test for
  `velketors_labyrinth_frenzy.toml`.
- `tests/test_velketors_labyrinth_camp_docs.py` guards the companion doc,
  config linkage, and schema-gap documentation.
- `#1900` tracks the loader-schema gap for named waypoint arrays, restriction
  geometry, and ordered route segments.

## Needs Live Proof Before This Issue Can Close

- Confirm the `safe_hall_med -> frenzy_handoff -> lower spiders` loop is stable
  for a live 6-box stack and does not cause unwanted pit drops or line-of-sight
  stalls.
- Measure whether `pull_radius = 165`, `leash_radius = 110`, and
  `rest_mana_pct = 70` are actually conservative enough for rogue-spider burst
  damage and pathing resets.
- Validate that return-to-camp recovery always retraces the intended branch and
  never cuts through `lord_bob_hard_stop` or `bledrek_hard_stop`.
- Sample actual spawn cadence and dead-time on the Frenzy shelf before
  promoting the route as a repeatable farming lane.
- Record whether upper-spider, pit, or upper-dogs extensions remain attended
  only or can be promoted into the main camp notes.

## Validation Procedure

### 1. Frenzy Hall Route Validation

- Stage the group at `safe_hall_med`.
- Walk the default loop from `frenzy_handoff` through `Crystal Eyes`,
  `a Frenzied Velium Broodling`, `a Frenzied Velium Stalker`, and the upper
  spider lip.
- Record any slip events, bad corners, or unexpected add paths.

### 2. Pull And Leash Validation

- Record every pull that breaks the expected `frenzy_handoff` handoff.
- Note any pull that widens into Lord Bob, Bledrek, or the pit branch.
- Capture whether `return_no_aggro = true` actually prevents bad recoveries.

### 3. Mana And Med Validation

- Log med breaks at the safe hall after spider bursts.
- Record whether `pull_mana_pct = 45` is high enough to avoid risky low-mana
  re-engages.
- Note any healer or support casters that still dip below a practical floor.

### 4. Spawn Pattern Validation

- Sample the Frenzy shelf for at least one full zone cycle.
- Record placeholder count, named count, and average wait between viable pulls.
- If upper-spider or pit extensions are used, log them separately instead of
  mixing them into the baseline Frenzy cadence.

## Checked-in Sampling Template

Use [velketors-validation-template.csv](assets/velketors-validation-template.csv)
for live sampling. It is intentionally blank so the repository does not invent
route safety, spawn cadence, or mana-behavior numbers that were never observed.

The template includes explicit columns for:

- route branch and waypoint span
- pull count and named count
- leash breaks and return-path recoveries
- med breaks and spawn cycle minutes
- operator mode and evidence state

## Exit Criteria

Do not call the Velketor Frenzy route validated until the template has at least:

- one live 6-box sample on the default Frenzy hall loop
- one documented result for leash behavior and return-to-camp recovery
- one med-threshold sample that records whether the current mana defaults held
- one explicit operator-mode note stating whether the session remained attended,
  semi-attended, or unattended
